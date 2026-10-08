//! Provider-neutral application generation protocol.
//!
//! The model proposes an untrusted `ApplicationPlan`. Rust validates, compiles
//! through `ChangeIntent` → `AppOperation`, then the kernel decides authority.
//! Providers (mock, recorded, live) all emit this same contract.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::Database;
use crate::runtime_v2::operations::AppOperation;
use crate::runtime_v2::packs::validate_component_type_allowed;

use super::compiler::{compile, compile_all, ChangeIntent, CompiledChange};
use super::errors::KernelError;
use super::lineage::{
    resolve_application_identity, resolve_application_surface, resolve_surface_for_application,
    LineageScope,
};
use super::testing::DeclarativeTest;
use super::COMPILER_VERSION;

pub const APPLICATION_PLAN_SCHEMA_VERSION: &str = "1";

/// Soft resource caps so a runaway model cannot expand one prompt unboundedly.
pub const MAX_PLAN_INTENTS: usize = 64;
pub const MAX_PLAN_STEPS: usize = 64;
pub const MAX_PLAN_TESTS: usize = 32;
pub const MAX_PLAN_SUMMARY_CHARS: usize = 2_000;
pub const MAX_PLAN_STEP_CHARS: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationPlanKind {
    Create,
    Evolve,
}

impl ApplicationPlanKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Evolve => "evolve",
        }
    }
}

/// Human-readable step in an inspectable change plan (not executable authority).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlanStep {
    pub id: String,
    pub description: String,
}

/// Untrusted declarative application proposal from a provider.
///
/// Authority remains in Rust: validate → compile intents → kernel apply/decide.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationPlan {
    #[serde(default = "default_plan_schema_version")]
    pub schema_version: String,
    #[serde(default = "default_plan_id")]
    pub plan_id: String,
    pub kind: ApplicationPlanKind,
    pub summary: String,
    /// Required for evolve; optional for create (derived from CreateSurface tool id).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_id: Option<String>,
    /// Optimistic concurrency base for evolve plans. Required for evolve.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_revision: Option<i64>,
    /// Inspectable human-readable steps (not trusted for execution).
    #[serde(default)]
    pub steps: Vec<PlanStep>,
    /// Executable high-level intents compiled by the Change Compiler.
    pub intents: Vec<ChangeIntent>,
    #[serde(default)]
    pub tests: Vec<DeclarativeTest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<Value>,
}

fn default_plan_schema_version() -> String {
    APPLICATION_PLAN_SCHEMA_VERSION.to_string()
}

fn default_plan_id() -> String {
    format!("plan-{}", Uuid::new_v4())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatedApplicationPlan {
    pub plan: ApplicationPlan,
    pub compiled: CompiledChange,
    pub diagnostics: Vec<String>,
}

/// Canonical ApplicationPlan JSON-schema fragment for OpenAI-family providers.
/// Nested intent details stay loosely typed; Rust validation is authoritative.
pub fn application_plan_json_schema_openai() -> Value {
    json!({
        "type": ["object", "null"],
        "description": "Provider-neutral untrusted application proposal. Rust validates and compiles; never trust as authority.",
        "additionalProperties": true,
        "properties": {
            "schemaVersion": { "type": "string" },
            "planId": { "type": "string" },
            "kind": { "type": "string", "enum": ["create", "evolve"] },
            "summary": { "type": "string" },
            "applicationId": { "type": ["string", "null"] },
            "baseRevision": { "type": ["integer", "null"] },
            "steps": {
                "type": "array",
                "items": { "type": "object" }
            },
            "intents": {
                "type": "array",
                "items": { "type": "object" }
            },
            "tests": {
                "type": ["array", "null"],
                "items": { "type": "object" }
            },
            "diagnostics": { "type": ["object", "null"] }
        },
        "required": ["kind", "summary", "intents"]
    })
}

/// Gemini-safe ApplicationPlan fragment: no nullable unions, shallow nesting.
pub fn application_plan_json_schema_gemini() -> Value {
    json!({
        "type": "object",
        "description": "Provider-neutral untrusted application proposal. Rust validates and compiles; never trust as authority.",
        "properties": {
            "schemaVersion": { "type": "string" },
            "planId": { "type": "string" },
            "kind": { "type": "string", "enum": ["create", "evolve"] },
            "summary": { "type": "string" },
            "applicationId": { "type": "string" },
            "baseRevision": { "type": "integer" },
            "steps": {
                "type": "array",
                "items": { "type": "object" }
            },
            "intents": {
                "type": "array",
                "items": { "type": "object" }
            },
            "tests": {
                "type": "array",
                "items": { "type": "object" }
            },
            "diagnostics": { "type": "object" }
        },
        "required": ["kind", "summary", "intents"]
    })
}

pub fn validate_plan(plan: &ApplicationPlan) -> Result<(), KernelError> {
    if plan.schema_version != APPLICATION_PLAN_SCHEMA_VERSION {
        return Err(KernelError::Validation(format!(
            "unsupported applicationPlan.schemaVersion '{}'",
            plan.schema_version
        )));
    }
    if plan.plan_id.trim().is_empty() {
        return Err(KernelError::Validation("planId required".into()));
    }
    if plan.summary.trim().is_empty() {
        return Err(KernelError::Validation("summary required".into()));
    }
    if plan.summary.chars().count() > MAX_PLAN_SUMMARY_CHARS {
        return Err(KernelError::Validation(format!(
            "summary exceeds {MAX_PLAN_SUMMARY_CHARS} characters"
        )));
    }
    if plan.intents.is_empty() {
        return Err(KernelError::Validation(
            "applicationPlan.intents must not be empty".into(),
        ));
    }
    if plan.intents.len() > MAX_PLAN_INTENTS {
        return Err(KernelError::Validation(format!(
            "applicationPlan.intents exceeds max of {MAX_PLAN_INTENTS}"
        )));
    }
    if plan.steps.len() > MAX_PLAN_STEPS {
        return Err(KernelError::Validation(format!(
            "applicationPlan.steps exceeds max of {MAX_PLAN_STEPS}"
        )));
    }
    if plan.tests.len() > MAX_PLAN_TESTS {
        return Err(KernelError::Validation(format!(
            "applicationPlan.tests exceeds max of {MAX_PLAN_TESTS}"
        )));
    }
    for step in &plan.steps {
        if step.id.trim().is_empty() || step.description.trim().is_empty() {
            return Err(KernelError::Validation(
                "plan steps require id and description".into(),
            ));
        }
        if step.description.chars().count() > MAX_PLAN_STEP_CHARS {
            return Err(KernelError::Validation(format!(
                "plan step description exceeds {MAX_PLAN_STEP_CHARS} characters"
            )));
        }
    }
    if plan.kind == ApplicationPlanKind::Evolve {
        match plan.base_revision {
            Some(rev) if rev >= 0 => {}
            Some(rev) => {
                return Err(KernelError::Validation(format!(
                    "evolve applicationPlan.baseRevision must be non-negative, got {rev}"
                )));
            }
            None => {
                return Err(KernelError::Validation(
                    "evolve applicationPlan requires baseRevision observed from authoritative application context"
                        .into(),
                ));
            }
        }
    }
    let mut seen_intent_keys = std::collections::HashSet::new();
    for intent in &plan.intents {
        validate_intent_refs(intent)?;
        let key = intent_dedupe_key(intent);
        if !seen_intent_keys.insert(key) {
            return Err(KernelError::Validation(
                "duplicate intents in applicationPlan".into(),
            ));
        }
    }
    // Bound application id + protected check live here (single path).
    bind_intents_to_plan_application(plan)?;
    for test in &plan.tests {
        super::testing::validate_test(test).map_err(KernelError::Validation)?;
    }
    Ok(())
}

/// Database-authoritative lineage checks for apply/proposal admission.
///
/// Structural `validate_plan` is not enough for evolve: surface IDs must resolve
/// through SQLite lineage, not string formatting.
pub fn validate_plan_against_db(
    db: &Database,
    plan: &ApplicationPlan,
    scope: &LineageScope,
) -> Result<(), KernelError> {
    validate_plan(plan)?;
    let bound = plan_bound_application_id(plan)?;
    let allow_missing = plan.kind == ApplicationPlanKind::Create;
    resolve_application_identity(db, bound, allow_missing)?;

    if plan.kind == ApplicationPlanKind::Evolve {
        let expected = plan.base_revision.expect("checked in validate_plan");
        // Authoritative application surface OCC (covers data-only evolves without component intents).
        // Prefer durable tool_canvas binding — never conversation-scoped chat_inline
        // preference from resolve_prompt_surface_for_tool. OCC must match
        // load_application_spec / diff_surface_update (both use conversation_id=None).
        // Explicit intent surface_ids are still OCC-checked below.
        let primary = resolve_application_surface(db, bound, None, None)?;
        scope.enforce(&primary)?;
        if primary.definition_revision != expected {
            return Err(KernelError::RevisionConflict(format!(
                "stale evolve baseRevision {expected}; surface '{}' is at revision {}",
                primary.surface_id, primary.definition_revision
            )));
        }
        for surface_id in intent_surface_ids(plan) {
            if surface_id == primary.surface_id {
                continue;
            }
            let lineage = resolve_surface_for_application(db, &surface_id, bound)?;
            scope.enforce(&lineage)?;
            if lineage.definition_revision != expected {
                return Err(KernelError::RevisionConflict(format!(
                    "stale evolve baseRevision {expected}; surface '{surface_id}' is at revision {}",
                    lineage.definition_revision
                )));
            }
        }
    } else {
        for surface_id in intent_surface_ids(plan) {
            match resolve_surface_for_application(db, &surface_id, bound) {
                Ok(lineage) => scope.enforce(&lineage)?,
                Err(KernelError::Validation(ref msg))
                    if msg.contains("does not exist") || msg.contains("no bound application") => {}
                Err(e) => return Err(e),
            }
        }
    }
    Ok(())
}

pub(crate) fn plan_bound_application_id(plan: &ApplicationPlan) -> Result<&str, KernelError> {
    let create_tool_id = plan.intents.iter().find_map(|i| match i {
        ChangeIntent::CreateSurface { tool, .. } => Some(tool.id.as_str()),
        _ => None,
    });
    match plan.kind {
        ApplicationPlanKind::Evolve => plan
            .application_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                KernelError::Validation("applicationId required for evolve applicationPlan".into())
            }),
        ApplicationPlanKind::Create => {
            if let Some(app) = plan
                .application_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                Ok(app)
            } else {
                create_tool_id.ok_or_else(|| {
                    KernelError::Validation(
                        "create applicationPlan requires CreateSurface or applicationId".into(),
                    )
                })
            }
        }
    }
}

fn intent_surface_ids(plan: &ApplicationPlan) -> Vec<String> {
    let mut out = Vec::new();
    for intent in &plan.intents {
        match intent {
            ChangeIntent::InsertComponent { surface_id, .. }
            | ChangeIntent::UpdateComponent { surface_id, .. }
            | ChangeIntent::RemoveComponent { surface_id, .. } => {
                if !surface_id.trim().is_empty() {
                    out.push(surface_id.clone());
                }
            }
            _ => {}
        }
    }
    out.sort();
    out.dedup();
    out
}

/// One Apply must not silently mutate multiple applications.
fn bind_intents_to_plan_application(plan: &ApplicationPlan) -> Result<(), KernelError> {
    let bound = plan_bound_application_id(plan)?;
    crate::security::assert_not_protected(bound).map_err(KernelError::Protected)?;

    for intent in &plan.intents {
        if let Some(other) = intent_application_id(intent) {
            if other != bound {
                return Err(KernelError::Validation(format!(
                    "intent application '{other}' does not match plan application '{bound}'"
                )));
            }
        }
    }
    Ok(())
}

/// Explicit application identity from an intent. Component intents do **not**
/// derive identity from `surf-*` formatting — that requires DB lineage.
fn intent_application_id(intent: &ChangeIntent) -> Option<&str> {
    match intent {
        ChangeIntent::CreateSurface { tool, .. } => Some(tool.id.as_str()),
        ChangeIntent::UpdateSurface { tool_id, .. } => Some(tool_id.as_str()),
        ChangeIntent::UpsertDataModel { application_id, .. }
        | ChangeIntent::MigrateDataModel { application_id, .. } => Some(application_id.as_str()),
        ChangeIntent::InsertComponent { .. }
        | ChangeIntent::UpdateComponent { .. }
        | ChangeIntent::RemoveComponent { .. }
        | ChangeIntent::AddSetting { .. }
        | ChangeIntent::UpsertManifest { .. }
        | ChangeIntent::NavigateRoute { .. } => None,
    }
}

fn validate_intent_refs(intent: &ChangeIntent) -> Result<(), KernelError> {
    match intent {
        ChangeIntent::CreateSurface { tool, .. } => {
            crate::security::assert_not_protected(&tool.id).map_err(KernelError::Protected)?;
            if tool.id.trim().is_empty() || tool.name.trim().is_empty() {
                return Err(KernelError::Validation(
                    "CreateSurface requires tool.id and tool.name".into(),
                ));
            }
            if tool.components.is_empty() {
                return Err(KernelError::Validation(
                    "CreateSurface requires at least one component".into(),
                ));
            }
            let mut ids = std::collections::HashSet::new();
            for c in &tool.components {
                if c.id.trim().is_empty() {
                    return Err(KernelError::Validation(
                        "component id must be non-empty".into(),
                    ));
                }
                if !ids.insert(c.id.as_str()) {
                    return Err(KernelError::Validation(format!(
                        "duplicate component id '{}'",
                        c.id
                    )));
                }
                validate_component_type_allowed(&c.component_type)
                    .map_err(KernelError::CapabilityUnavailable)?;
            }
        }
        ChangeIntent::UpdateSurface { tool_id, tool, .. } => {
            crate::security::assert_not_protected(tool_id).map_err(KernelError::Protected)?;
            crate::security::assert_not_protected(&tool.id).map_err(KernelError::Protected)?;
            if tool_id.trim().is_empty() {
                return Err(KernelError::Validation(
                    "UpdateSurface requires toolId".into(),
                ));
            }
            if !tool.id.is_empty() && tool.id != *tool_id {
                return Err(KernelError::Validation(
                    "UpdateSurface tool.id must match toolId".into(),
                ));
            }
            for c in &tool.components {
                validate_component_type_allowed(&c.component_type)
                    .map_err(KernelError::CapabilityUnavailable)?;
            }
        }
        ChangeIntent::UpsertDataModel {
            application_id,
            model,
        }
        | ChangeIntent::MigrateDataModel {
            application_id,
            model,
            ..
        } => {
            crate::security::assert_not_protected(application_id)
                .map_err(KernelError::Protected)?;
            super::data::validate_model(model).map_err(KernelError::Validation)?;
        }
        ChangeIntent::InsertComponent {
            surface_id,
            component_type,
            component_id,
            ..
        } => {
            crate::security::assert_not_protected(surface_id).map_err(KernelError::Protected)?;
            if surface_id.trim().is_empty() {
                return Err(KernelError::Validation(
                    "InsertComponent requires surfaceId".into(),
                ));
            }
            if let Some(id) = component_id {
                if id.trim().is_empty() {
                    return Err(KernelError::Validation(
                        "InsertComponent.componentId must be non-empty when provided".into(),
                    ));
                }
            }
            validate_component_type_allowed(component_type)
                .map_err(KernelError::CapabilityUnavailable)?;
        }
        ChangeIntent::UpdateComponent {
            surface_id,
            component_type,
            ..
        } => {
            crate::security::assert_not_protected(surface_id).map_err(KernelError::Protected)?;
            if surface_id.trim().is_empty() {
                return Err(KernelError::Validation(
                    "UpdateComponent requires surfaceId".into(),
                ));
            }
            if let Some(component_type) = component_type {
                validate_component_type_allowed(component_type)
                    .map_err(KernelError::CapabilityUnavailable)?;
            }
        }
        ChangeIntent::RemoveComponent { surface_id, .. } => {
            crate::security::assert_not_protected(surface_id).map_err(KernelError::Protected)?;
            if surface_id.trim().is_empty() {
                return Err(KernelError::Validation(
                    "RemoveComponent requires surfaceId".into(),
                ));
            }
        }
        ChangeIntent::AddSetting { .. }
        | ChangeIntent::UpsertManifest { .. }
        | ChangeIntent::NavigateRoute { .. } => {
            return Err(KernelError::Validation(
                "applicationPlan does not allow UpsertManifest, AddSetting, or NavigateRoute intents"
                    .into(),
            ));
        }
    }
    Ok(())
}

fn intent_dedupe_key(intent: &ChangeIntent) -> String {
    match intent {
        ChangeIntent::CreateSurface { tool, .. } => format!("create_surface:{}", tool.id),
        ChangeIntent::UpdateSurface { tool_id, .. } => format!("update_surface:{tool_id}"),
        ChangeIntent::UpsertDataModel {
            application_id,
            model,
        } => format!("upsert_model:{application_id}:{}", model.model_id),
        ChangeIntent::MigrateDataModel {
            application_id,
            model,
            ..
        } => format!(
            "migrate_model:{application_id}:{}:{}",
            model.model_id, model.schema_version
        ),
        ChangeIntent::InsertComponent {
            surface_id,
            parent_id,
            component_type,
            component_id,
            props,
            ..
        } => {
            // Distinct inserts of the same type must not false-dupe; bind stable
            // componentId when present, otherwise fingerprint parent+props.
            match component_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                Some(id) => format!("insert:{surface_id}:{id}"),
                None => {
                    use std::collections::hash_map::DefaultHasher;
                    use std::hash::{Hash, Hasher};
                    let mut hasher = DefaultHasher::new();
                    props.to_string().hash(&mut hasher);
                    format!(
                        "insert:{surface_id}:{}:{component_type}:{}",
                        parent_id.as_deref().unwrap_or("-"),
                        hasher.finish()
                    )
                }
            }
        }
        ChangeIntent::UpdateComponent {
            surface_id,
            component_id,
            ..
        } => format!("update_component:{surface_id}:{component_id}"),
        ChangeIntent::RemoveComponent {
            surface_id,
            component_id,
            ..
        } => format!("remove_component:{surface_id}:{component_id}"),
        ChangeIntent::AddSetting {
            application_id,
            setting_id,
            ..
        } => format!("setting:{application_id}:{setting_id}"),
        ChangeIntent::UpsertManifest { manifest } => {
            format!("manifest:{}", manifest.application_id)
        }
        ChangeIntent::NavigateRoute {
            application_id,
            route_id,
        } => format!("route:{application_id}:{route_id}"),
    }
}

/// Stamp plan-level baseRevision onto compiled ops that lack their own.
fn apply_plan_base_revision(plan: &ApplicationPlan, compiled: &mut CompiledChange) {
    let Some(plan_rev) = plan.base_revision else {
        return;
    };
    for op in &mut compiled.operations {
        if op.base_revision.is_none() {
            op.base_revision = Some(plan_rev);
        }
    }
}

/// Structural / inspection compile only.
///
/// Does **not** consult SQLite lineage or OCC. Must never be treated as the
/// admission path for durable Apply. Prefer [`compile_plan_against_db`] for
/// proposal/apply, or [`compile_for_inspection`] when the call site needs an
/// explicitly non-authoritative result type.
pub fn compile_plan(plan: &ApplicationPlan) -> Result<ValidatedApplicationPlan, KernelError> {
    validate_plan(plan)?;
    let mut compiled = compile_all(&plan.intents)?;
    apply_plan_base_revision(plan, &mut compiled);
    Ok(ValidatedApplicationPlan {
        plan: plan.clone(),
        compiled,
        diagnostics: vec!["inspection-only: no SQLite lineage/OCC".into()],
    })
}

/// Explicitly non-authoritative compile for schema tests, UI preview diagnostics,
/// and provider-contract checks. The result must not be passed to Apply as the
/// sole authority for ApplicationPlan mutations.
#[derive(Debug, Clone)]
pub struct InspectionCompiledPlan {
    pub plan: ApplicationPlan,
    pub compiled: CompiledChange,
    pub diagnostics: Vec<String>,
}

impl InspectionCompiledPlan {
    /// Inspection ops are intentionally not a durable admission ticket.
    pub fn operations_for_inspection(&self) -> &[AppOperation] {
        &self.compiled.operations
    }
}

pub fn compile_for_inspection(
    plan: &ApplicationPlan,
) -> Result<InspectionCompiledPlan, KernelError> {
    let validated = compile_plan(plan)?;
    Ok(InspectionCompiledPlan {
        plan: validated.plan,
        compiled: validated.compiled,
        diagnostics: validated.diagnostics,
    })
}

/// Build ownership scope for ApplicationPlan admission.
///
/// Workspace-hosted tool surfaces are globals (no conversation/project binding).
/// Callers must omit those dimensions or [`LineageScope::enforce`] fails closed.
/// Conversation-bound / project-bound targets require a matching ownership claim
/// in the request — omitting the claim is a reject, not a silent pass.
/// Conversation id is still used as a lookup hint when resolving evolve targets
/// that do carry a conversation binding.
pub fn lineage_scope_for_plan(
    db: &Database,
    plan: &ApplicationPlan,
    conversation_id: Option<&str>,
    project_id: Option<&str>,
) -> Result<LineageScope, KernelError> {
    let request = LineageScope {
        conversation_id: conversation_id
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        project_id: project_id
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
    };
    if plan.kind != ApplicationPlanKind::Evolve {
        // Create produces workspace tool canvases without conversation ownership.
        return Ok(LineageScope::default());
    }
    let Ok(bound) = plan_bound_application_id(plan) else {
        // Structural validate will reject; keep request claims fail-closed.
        return Ok(request);
    };
    // ApplicationPlan evolve ownership follows the durable tool_canvas (or sole
    // bound surface), not the chat_inline preferred by prompt resolve when a
    // conversation_id claim is present. Otherwise OCC/scope can bind to inline
    // while UpdateSurface diffs against tool_canvas.
    match resolve_application_surface(db, bound, None, None) {
        Ok(lineage) => {
            let conversation_id = match (lineage.conversation_id.is_some(), request.conversation_id)
            {
                (false, _) => None,
                (true, Some(cid)) => Some(cid),
                (true, None) => {
                    return Err(KernelError::Validation(format!(
                        "surface '{}' is conversation-bound; ownership proof required",
                        lineage.surface_id
                    )));
                }
            };
            let project_id = match (lineage.project_id.is_some(), request.project_id) {
                (false, _) => None,
                (true, Some(pid)) => Some(pid),
                (true, None) => {
                    return Err(KernelError::Validation(format!(
                        "surface '{}' is project-bound; ownership proof required",
                        lineage.surface_id
                    )));
                }
            };
            Ok(LineageScope {
                conversation_id,
                project_id,
            })
        }
        // Surface missing / unresolvable: preserve claims so validate fails closed.
        Err(_) => Ok(request),
    }
}

/// Validate (including DB lineage + OCC) + compile with ApplicationSpec diffing.
/// This is the only ApplicationPlan path that may produce durable Apply ops.
pub fn compile_plan_against_db(
    db: &Database,
    plan: &ApplicationPlan,
    scope: &LineageScope,
) -> Result<ValidatedApplicationPlan, KernelError> {
    validate_plan_against_db(db, plan, scope)?;
    let mut compiled = compile_all_against_db(db, &plan.intents)?;
    apply_plan_base_revision(plan, &mut compiled);
    Ok(ValidatedApplicationPlan {
        plan: plan.clone(),
        compiled,
        diagnostics: Vec::new(),
    })
}

/// Compile intents with DB-backed surface diff for UpdateSurface.
fn compile_all_against_db(
    db: &Database,
    intents: &[ChangeIntent],
) -> Result<CompiledChange, KernelError> {
    if intents.is_empty() {
        return Err(KernelError::Validation(
            "compile_all requires at least one intent".into(),
        ));
    }
    let mut operations = Vec::new();
    let mut summaries = Vec::new();
    let mut rollback_hints = Vec::new();
    for intent in intents {
        let compiled = match intent {
            ChangeIntent::UpdateSurface {
                tool_id,
                tool,
                change_summary,
                base_revision,
            } => {
                // Prefer granular diff against trusted ApplicationSpec.
                // After lineage/OCC validation the surface must load; never fall
                // back to DB-less structural compile (that would reintroduce
                // synthetic surf-* targeting as authority).
                let spec = super::surface_diff::load_application_spec(db, tool_id)?;
                super::surface_diff::diff_surface_update(
                    &spec,
                    tool,
                    tool_id,
                    change_summary.as_deref(),
                    *base_revision,
                )?
            }
            _ => compile(intent.clone())?,
        };
        operations.extend(compiled.operations);
        summaries.push(compiled.summary);
        rollback_hints.push(compiled.rollback_hint);
    }
    Ok(CompiledChange {
        compiler_version: COMPILER_VERSION.into(),
        operations,
        summary: summaries.join("; "),
        rollback_hint: rollback_hints.join("; "),
    })
}

/// Derive a legacy toolChange-compatible payload from the first Create/Update surface intent.
/// Used so preview UI keeps working while ApplicationPlan is authoritative.
pub fn derive_tool_change(
    plan: &ApplicationPlan,
) -> Option<crate::ai::response_schema::ToolChangePayload> {
    use crate::ai::response_schema::{ToolAction, ToolChangePayload};
    for intent in &plan.intents {
        match intent {
            ChangeIntent::CreateSurface {
                tool,
                change_summary,
            } => {
                return Some(ToolChangePayload {
                    action: ToolAction::Create,
                    target_tool_id: None,
                    tool: Some(tool.clone()),
                    change_summary: change_summary
                        .clone()
                        .unwrap_or_else(|| plan.summary.clone()),
                });
            }
            ChangeIntent::UpdateSurface {
                tool_id,
                tool,
                change_summary,
                ..
            } => {
                return Some(ToolChangePayload {
                    action: ToolAction::Update,
                    target_tool_id: Some(tool_id.clone()),
                    tool: Some(tool.clone()),
                    change_summary: change_summary
                        .clone()
                        .unwrap_or_else(|| plan.summary.clone()),
                });
            }
            _ => {}
        }
    }
    None
}

/// Single-intent convenience used by unit tests and IPC.
pub fn compile_intent(intent: ChangeIntent) -> Result<CompiledChange, KernelError> {
    compile(intent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ponytail: reuse shared fixture — do not duplicate Task Tracker plan JSON here.
    fn sample_create_plan() -> ApplicationPlan {
        crate::ai::plan_fixtures::task_tracker_create_plan()
    }

    #[test]
    fn valid_create_plan_compiles() {
        let plan = sample_create_plan();
        let validated = compile_plan(&plan).unwrap();
        assert!(!validated.compiled.operations.is_empty());
        assert!(validated
            .compiled
            .operations
            .iter()
            .any(|op| op.op_type == "surface.create"));
        assert!(validated
            .compiled
            .operations
            .iter()
            .any(|op| op.op_type == "data.model_upsert"));
    }

    #[test]
    fn rejects_empty_intents() {
        let mut plan = sample_create_plan();
        plan.intents.clear();
        assert!(matches!(
            validate_plan(&plan),
            Err(KernelError::Validation(_))
        ));
    }

    /// Adversarial empty-plan / channel-boundary safety.
    /// Distinguishes valid no-op responses from plan smuggling and empty mutate plans.
    mod empty_plan_safety {
        use super::*;
        use crate::ai::response_schema::{AgentResponsePayload, ResponseType};

        #[test]
        fn valid_noop_response_without_plan_or_ops_is_accepted() {
            let payload = AgentResponsePayload {
                assistant_message: String::new(),
                response_type: ResponseType::Noop,
                application_plan: None,
                operations: None,
                tool_change: None,
                ..Default::default()
            };
            payload
                .validate()
                .expect("pure noop must remain a valid non-mutating channel");
        }

        #[test]
        fn empty_intents_application_plan_is_not_a_valid_noop() {
            let mut plan = sample_create_plan();
            plan.intents.clear();
            let err = validate_plan(&plan).expect_err("empty intents must not admit as no-op");
            assert!(
                matches!(err, KernelError::Validation(ref m) if m.contains("must not be empty")),
                "got {err:?}"
            );
        }

        #[test]
        fn companion_operations_cannot_smuggle_mutations_beside_plan() {
            let mut payload = AgentResponsePayload {
                application_plan: Some(sample_create_plan()),
                assistant_message: "create".into(),
                response_type: ResponseType::ToolChange,
                ..Default::default()
            };
            payload.operations = Some(vec![json!({
                "type": "component.remove",
                "target": { "surfaceId": "surf-x", "componentId": "c1" },
                "payload": {}
            })]);
            let err = payload
                .validate()
                .expect_err("companion operations must fail closed");
            assert!(err.contains("cannot be combined"), "got {err}");
        }

        #[test]
        fn invalid_lineage_scope_rejects_evolve_with_unbound_surface() {
            use crate::application_kernel::lineage::LineageScope;
            use crate::db::{create_conversation, Database, DEFAULT_WORKSPACE_ID};
            use crate::runtime_v2::surfaces::upsert_surface_from_tool;
            use tempfile::tempdir;

            let dir = tempdir().unwrap();
            let mut db = Database::open_path(&dir.path().join("empty-plan-lineage.db")).unwrap();
            let conv =
                create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Lineage adversarial", None)
                    .unwrap();

            // Workspace tool surface: no conversation/project binding on the row.
            let tool = crate::ai::ToolDefinition {
                id: "tool-task-tracker".into(),
                name: "Task Tracker".into(),
                description: "d".into(),
                layout: json!("stack"),
                components: vec![crate::ai::ToolComponent {
                    id: "t".into(),
                    component_type: "text".into(),
                    props: Some(json!({"text": "hi"})),
                    ..Default::default()
                }],
                ..Default::default()
            };
            let applied = crate::db::apply_tool_change(
                &mut db,
                DEFAULT_WORKSPACE_ID,
                &tool,
                "create",
                None,
                "test",
            )
            .unwrap();
            let surface =
                upsert_surface_from_tool(&mut db, &applied.definition, DEFAULT_WORKSPACE_ID, 1)
                    .unwrap();
            assert!(
                surface.conversation_id.is_none(),
                "adversarial fixture must lack conversation binding"
            );

            let plan = crate::ai::plan_fixtures::task_tracker_add_due_dates_plan(Some(1));
            let scope = LineageScope {
                conversation_id: Some(conv.id.clone()),
                project_id: None,
            };
            let err = validate_plan_against_db(&db, &plan, &scope)
                .expect_err("expected conversation with unbound surface must reject");
            assert!(
                matches!(
                    err,
                    KernelError::Validation(ref m) if m.contains("no conversation binding")
                ),
                "got {err:?}"
            );

            // Product callers omit ownership claims for workspace globals via helper.
            let adapted = lineage_scope_for_plan(&db, &plan, Some(&conv.id), None)
                .expect("workspace global must adapt without error");
            assert!(
                adapted.conversation_id.is_none(),
                "helper must omit conversation scope for unbound globals"
            );
            validate_plan_against_db(&db, &plan, &adapted)
                .expect("omitted scope must admit workspace global evolve");
        }
    }

    /// Unit coverage for [`lineage_scope_for_plan`] itself (not only via validate).
    mod lineage_scope_for_plan_tests {
        use super::*;
        use crate::db::{create_conversation, Database, DEFAULT_WORKSPACE_ID};
        use crate::runtime_v2::surfaces::{create_inline_surface, upsert_surface_from_tool};
        use tempfile::tempdir;

        fn open_db(name: &str) -> (tempfile::TempDir, Database) {
            let dir = tempdir().unwrap();
            let db = Database::open_path(&dir.path().join(name)).unwrap();
            (dir, db)
        }

        fn insert_tool(db: &Database, tool_id: &str) {
            db.conn()
                .execute(
                    "INSERT INTO tools (id, workspace_id, name, description, layout, definition_json, current_version, created_at, updated_at)
                     VALUES (?1, ?2, ?1, 'd', 'stack', '{}', 1, datetime('now'), datetime('now'))",
                    rusqlite::params![tool_id, DEFAULT_WORKSPACE_ID],
                )
                .unwrap();
        }

        fn bind_surface_tool_id(db: &mut Database, surface_id: &str, tool_id: &str) {
            db.conn()
                .execute(
                    "UPDATE surfaces SET tool_id = ?1 WHERE id = ?2",
                    rusqlite::params![tool_id, surface_id],
                )
                .unwrap();
        }

        fn workspace_tool_surface(
            db: &mut Database,
            tool_id: &str,
        ) -> crate::runtime_v2::surfaces::SurfaceRecord {
            let tool = crate::ai::ToolDefinition {
                id: tool_id.into(),
                name: "Scope Tool".into(),
                description: "d".into(),
                layout: json!("stack"),
                components: vec![crate::ai::ToolComponent {
                    id: "t".into(),
                    component_type: "text".into(),
                    props: Some(json!({"text": "hi"})),
                    ..Default::default()
                }],
                ..Default::default()
            };
            let applied = crate::db::apply_tool_change(
                db,
                DEFAULT_WORKSPACE_ID,
                &tool,
                "create",
                None,
                "test",
            )
            .unwrap();
            upsert_surface_from_tool(db, &applied.definition, DEFAULT_WORKSPACE_ID, 1).unwrap()
        }

        #[test]
        fn create_plan_omits_ownership_claims() {
            let (_dir, mut db) = open_db("scope-create.db");
            let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "C", None).unwrap();
            let plan = sample_create_plan();
            let scope = lineage_scope_for_plan(&db, &plan, Some(&conv.id), Some("proj-a"))
                .expect("create must succeed");
            assert!(scope.conversation_id.is_none());
            assert!(scope.project_id.is_none());
        }

        #[test]
        fn evolve_workspace_global_omits_conversation_and_project() {
            let (_dir, mut db) = open_db("scope-global.db");
            let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "C", None).unwrap();
            let surface = workspace_tool_surface(&mut db, "tool-task-tracker");
            assert!(surface.conversation_id.is_none());
            assert!(surface.project_id.is_none());
            let plan = crate::ai::plan_fixtures::task_tracker_add_due_dates_plan(Some(1));
            let scope = lineage_scope_for_plan(&db, &plan, Some(&conv.id), Some("proj-a"))
                .expect("global evolve must adapt");
            assert!(scope.conversation_id.is_none());
            assert!(scope.project_id.is_none());
        }

        #[test]
        fn evolve_prefers_tool_canvas_over_conversation_inline_for_occ_and_scope() {
            // Regression: with both tool_canvas and chat_inline for the same tool_id,
            // ApplicationPlan must OCC/scope against canvas (matching load_application_spec),
            // not the inline surface preferred by resolve_prompt_surface_for_tool.
            let (_dir, mut db) = open_db("scope-canvas-vs-inline.db");
            let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "C", None).unwrap();
            let canvas = workspace_tool_surface(&mut db, "tool-dual-surface");
            assert_eq!(canvas.placement, "tool_canvas");
            assert_eq!(canvas.current_revision, 1);

            let inline = create_inline_surface(
                &mut db,
                &conv.id,
                None,
                None,
                "Inline dual",
                &json!({
                    "id": "doc-inline-dual",
                    "name": "Inline dual",
                    "layout": "stack",
                    "components": [{"id": "t", "type": "text", "props": {"text": "inline"}}]
                }),
                &[],
            )
            .unwrap();
            bind_surface_tool_id(&mut db, &inline.id, "tool-dual-surface");
            // Advance inline revision so a mistaken inline OCC would conflict.
            db.conn()
                .execute(
                    "UPDATE surfaces SET current_revision = 99 WHERE id = ?1",
                    rusqlite::params![inline.id],
                )
                .unwrap();

            let plan = {
                let mut p = sample_create_plan();
                p.kind = ApplicationPlanKind::Evolve;
                p.application_id = Some("tool-dual-surface".into());
                p.base_revision = Some(1);
                p.intents = vec![ChangeIntent::UpdateSurface {
                    tool_id: "tool-dual-surface".into(),
                    tool: crate::ai::ToolDefinition {
                        id: "tool-dual-surface".into(),
                        name: "Dual".into(),
                        description: "d".into(),
                        layout: json!("stack"),
                        components: vec![crate::ai::ToolComponent {
                            id: "t".into(),
                            component_type: "text".into(),
                            props: Some(json!({"text": "evolved"})),
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                    change_summary: Some("evolve canvas".into()),
                    base_revision: Some(1),
                }];
                p
            };

            let scope = lineage_scope_for_plan(&db, &plan, Some(&conv.id), None)
                .expect("canvas primary must omit conversation ownership");
            assert!(
                scope.conversation_id.is_none(),
                "must not bind evolve scope to chat_inline when tool_canvas exists"
            );

            validate_plan_against_db(&db, &plan, &scope)
                .expect("OCC must use canvas revision 1, not inline 99");

            let validated = compile_plan_against_db(&db, &plan, &scope).expect("compile");
            let targets: Vec<_> = validated
                .compiled
                .operations
                .iter()
                .filter_map(|o| o.target.surface_id.as_deref())
                .collect();
            assert!(
                targets.iter().any(|sid| *sid == canvas.id.as_str()),
                "diff must target tool_canvas {}, got {targets:?}",
                canvas.id
            );
            assert!(
                !targets.iter().any(|sid| *sid == inline.id.as_str()),
                "diff must not target chat_inline {}",
                inline.id
            );
        }

        #[test]
        fn evolve_conversation_bound_keeps_claim_for_enforce() {
            let (_dir, mut db) = open_db("scope-bound.db");
            let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "C", None).unwrap();
            insert_tool(&db, "app-bound");
            let surface = create_inline_surface(
                &mut db,
                &conv.id,
                None,
                None,
                "Bound",
                &json!({
                    "id": "doc-bound",
                    "name": "Bound",
                    "layout": "stack",
                    "components": [{"id": "t", "type": "text", "props": {"text": "hi"}}]
                }),
                &[],
            )
            .unwrap();
            bind_surface_tool_id(&mut db, &surface.id, "app-bound");

            let mut plan = sample_create_plan();
            plan.kind = ApplicationPlanKind::Evolve;
            plan.application_id = Some("app-bound".into());
            plan.base_revision = Some(1);
            plan.intents = vec![ChangeIntent::UpdateSurface {
                tool_id: "app-bound".into(),
                tool: crate::ai::ToolDefinition {
                    id: "app-bound".into(),
                    name: "Bound".into(),
                    description: "d".into(),
                    layout: json!("stack"),
                    components: vec![crate::ai::ToolComponent {
                        id: "t".into(),
                        component_type: "text".into(),
                        props: Some(json!({"text": "hi"})),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                change_summary: Some("touch".into()),
                base_revision: Some(1),
            }];

            let scope = lineage_scope_for_plan(&db, &plan, Some(&conv.id), None)
                .expect("bound evolve with claim must adapt");
            assert_eq!(scope.conversation_id.as_deref(), Some(conv.id.as_str()));
        }

        #[test]
        fn evolve_conversation_bound_without_claim_rejects() {
            let (_dir, mut db) = open_db("scope-bound-missing.db");
            let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "C", None).unwrap();
            insert_tool(&db, "app-bound-miss");
            let surface = create_inline_surface(
                &mut db,
                &conv.id,
                None,
                None,
                "Bound",
                &json!({
                    "id": "doc-bound-miss",
                    "name": "Bound",
                    "layout": "stack",
                    "components": [{"id": "t", "type": "text", "props": {"text": "hi"}}]
                }),
                &[],
            )
            .unwrap();
            bind_surface_tool_id(&mut db, &surface.id, "app-bound-miss");

            let mut plan = sample_create_plan();
            plan.kind = ApplicationPlanKind::Evolve;
            plan.application_id = Some("app-bound-miss".into());
            plan.base_revision = Some(1);
            plan.intents = vec![ChangeIntent::UpdateSurface {
                tool_id: "app-bound-miss".into(),
                tool: crate::ai::ToolDefinition {
                    id: "app-bound-miss".into(),
                    name: "Bound".into(),
                    description: "d".into(),
                    layout: json!("stack"),
                    components: vec![crate::ai::ToolComponent {
                        id: "t".into(),
                        component_type: "text".into(),
                        props: Some(json!({"text": "hi"})),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                change_summary: Some("touch".into()),
                base_revision: Some(1),
            }];

            let err = lineage_scope_for_plan(&db, &plan, None, None)
                .expect_err("missing conversation claim must reject");
            assert!(
                matches!(
                    err,
                    KernelError::Validation(ref m) if m.contains("ownership proof required")
                ),
                "got {err:?}"
            );
        }

        #[test]
        fn evolve_project_bound_without_claim_rejects() {
            let (_dir, mut db) = open_db("scope-proj-miss.db");
            let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "C", None).unwrap();
            insert_tool(&db, "app-proj");
            let surface = create_inline_surface(
                &mut db,
                &conv.id,
                None,
                Some("proj-a"),
                "Proj",
                &json!({
                    "id": "doc-proj",
                    "name": "Proj",
                    "layout": "stack",
                    "components": [{"id": "t", "type": "text", "props": {"text": "hi"}}]
                }),
                &[],
            )
            .unwrap();
            bind_surface_tool_id(&mut db, &surface.id, "app-proj");

            let mut plan = sample_create_plan();
            plan.kind = ApplicationPlanKind::Evolve;
            plan.application_id = Some("app-proj".into());
            plan.base_revision = Some(1);
            plan.intents = vec![ChangeIntent::UpdateSurface {
                tool_id: "app-proj".into(),
                tool: crate::ai::ToolDefinition {
                    id: "app-proj".into(),
                    name: "Proj".into(),
                    description: "d".into(),
                    layout: json!("stack"),
                    components: vec![crate::ai::ToolComponent {
                        id: "t".into(),
                        component_type: "text".into(),
                        props: Some(json!({"text": "hi"})),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                change_summary: Some("touch".into()),
                base_revision: Some(1),
            }];

            let err = lineage_scope_for_plan(&db, &plan, Some(&conv.id), None)
                .expect_err("missing project claim must reject");
            assert!(
                matches!(
                    err,
                    KernelError::Validation(ref m) if m.contains("project-bound")
                ),
                "got {err:?}"
            );
        }

        #[test]
        fn evolve_unresolvable_preserves_request_claims() {
            let (_dir, mut db) = open_db("scope-miss-surface.db");
            let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "C", None).unwrap();
            let mut plan = sample_create_plan();
            plan.kind = ApplicationPlanKind::Evolve;
            plan.application_id = Some("app-does-not-exist".into());
            plan.base_revision = Some(1);
            plan.intents = vec![ChangeIntent::UpdateSurface {
                tool_id: "app-does-not-exist".into(),
                tool: crate::ai::ToolDefinition {
                    id: "app-does-not-exist".into(),
                    name: "Missing".into(),
                    description: "d".into(),
                    layout: json!("stack"),
                    components: vec![crate::ai::ToolComponent {
                        id: "t".into(),
                        component_type: "text".into(),
                        props: Some(json!({"text": "hi"})),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                change_summary: Some("touch".into()),
                base_revision: Some(1),
            }];

            let scope = lineage_scope_for_plan(&db, &plan, Some(&conv.id), Some("proj-a"))
                .expect("unresolvable must keep claims for later validate");
            assert_eq!(scope.conversation_id.as_deref(), Some(conv.id.as_str()));
            assert_eq!(scope.project_id.as_deref(), Some("proj-a"));
        }
    }

    #[test]
    fn rejects_evolve_without_application_id() {
        let mut plan = sample_create_plan();
        plan.kind = ApplicationPlanKind::Evolve;
        plan.application_id = None;
        plan.base_revision = Some(1);
        assert!(matches!(
            validate_plan(&plan),
            Err(KernelError::Validation(_))
        ));
    }

    #[test]
    fn rejects_evolve_without_base_revision() {
        let mut plan = sample_create_plan();
        plan.kind = ApplicationPlanKind::Evolve;
        plan.application_id = Some("tool-task-tracker".into());
        plan.base_revision = None;
        let err = validate_plan(&plan).expect_err("missing baseRevision must fail");
        assert!(
            matches!(err, KernelError::Validation(ref m) if m.contains("baseRevision")),
            "got {err:?}"
        );
    }

    #[test]
    fn rejects_evolve_with_negative_base_revision() {
        let mut plan = sample_create_plan();
        plan.kind = ApplicationPlanKind::Evolve;
        plan.application_id = Some("tool-task-tracker".into());
        plan.base_revision = Some(-1);
        assert!(matches!(
            validate_plan(&plan),
            Err(KernelError::Validation(_))
        ));
    }

    #[test]
    fn rejects_duplicate_component_ids() {
        let mut plan = sample_create_plan();
        if let ChangeIntent::CreateSurface { tool, .. } = &mut plan.intents[0] {
            tool.components.push(crate::ai::ToolComponent {
                id: "tm-heading".into(),
                component_type: "text".into(),
                ..Default::default()
            });
        }
        assert!(matches!(
            validate_plan(&plan),
            Err(KernelError::Validation(_))
        ));
    }

    #[test]
    fn rejects_disallowed_component() {
        let mut plan = sample_create_plan();
        if let ChangeIntent::CreateSurface { tool, .. } = &mut plan.intents[0] {
            tool.components[0].component_type = "iframe".into();
        }
        assert!(matches!(
            validate_plan(&plan),
            Err(KernelError::CapabilityUnavailable(_))
        ));
    }

    #[test]
    fn rejects_excessive_intents() {
        let mut plan = sample_create_plan();
        let base = plan.intents[1].clone();
        plan.intents = (0..=MAX_PLAN_INTENTS)
            .map(|i| match &base {
                ChangeIntent::UpsertDataModel {
                    application_id,
                    model,
                } => {
                    let mut m = model.clone();
                    m.model_id = format!("model-{i}");
                    ChangeIntent::UpsertDataModel {
                        application_id: application_id.clone(),
                        model: m,
                    }
                }
                other => other.clone(),
            })
            .collect();
        assert!(matches!(
            validate_plan(&plan),
            Err(KernelError::Validation(_))
        ));
    }

    #[test]
    fn derive_tool_change_from_create() {
        let plan = sample_create_plan();
        let tc = derive_tool_change(&plan).unwrap();
        assert_eq!(tc.tool.as_ref().unwrap().id, "tool-task-tracker");
    }

    #[test]
    fn rejects_unsupported_schema_version() {
        let mut plan = sample_create_plan();
        plan.schema_version = "99".into();
        assert!(matches!(
            validate_plan(&plan),
            Err(KernelError::Validation(_))
        ));
    }

    #[test]
    fn rejects_empty_plan_id_and_summary() {
        let mut plan = sample_create_plan();
        plan.plan_id = "  ".into();
        assert!(matches!(
            validate_plan(&plan),
            Err(KernelError::Validation(_))
        ));
        plan.plan_id = "plan-ok".into();
        plan.summary = "".into();
        assert!(matches!(
            validate_plan(&plan),
            Err(KernelError::Validation(_))
        ));
    }

    #[test]
    fn rejects_excessive_steps_and_tests() {
        let mut plan = sample_create_plan();
        plan.steps = (0..=MAX_PLAN_STEPS)
            .map(|i| PlanStep {
                id: format!("s{i}"),
                description: format!("step {i}"),
            })
            .collect();
        assert!(matches!(
            validate_plan(&plan),
            Err(KernelError::Validation(_))
        ));
        plan.steps = vec![PlanStep {
            id: "s1".into(),
            description: "ok".into(),
        }];
        plan.tests = (0..=MAX_PLAN_TESTS)
            .map(|i| crate::application_kernel::testing::DeclarativeTest {
                test_id: format!("t{i}"),
                name: format!("test {i}"),
                test_type: "smoke".into(),
                actions: vec![],
                assertions: vec![],
                timeout_ms: 1000,
            })
            .collect();
        assert!(matches!(
            validate_plan(&plan),
            Err(KernelError::Validation(_))
        ));
    }

    #[test]
    fn rejects_duplicate_intents() {
        let mut plan = sample_create_plan();
        let dup = plan.intents[0].clone();
        plan.intents.push(dup);
        assert!(matches!(
            validate_plan(&plan),
            Err(KernelError::Validation(_))
        ));
    }

    #[test]
    fn rejects_empty_component_id() {
        let mut plan = sample_create_plan();
        if let ChangeIntent::CreateSurface { tool, .. } = &mut plan.intents[0] {
            tool.components[0].id = "  ".into();
        }
        assert!(matches!(
            validate_plan(&plan),
            Err(KernelError::Validation(_))
        ));
    }

    #[test]
    fn rejects_malformed_plan_json() {
        let raw = r#"{"schemaVersion":"1","kind":"create","summary":"x","intents":"not-an-array"}"#;
        assert!(serde_json::from_str::<ApplicationPlan>(raw).is_err());
    }

    #[test]
    fn allows_multiple_inserts_of_same_component_type() {
        let mut plan = sample_create_plan();
        plan.intents.push(ChangeIntent::InsertComponent {
            surface_id: "surf-tool-task-tracker".into(),
            parent_id: None,
            component_type: "text".into(),
            props: json!({"text": "A"}),
            base_revision: None,
            component_id: None,
        });
        plan.intents.push(ChangeIntent::InsertComponent {
            surface_id: "surf-tool-task-tracker".into(),
            parent_id: None,
            component_type: "text".into(),
            props: json!({"text": "B"}),
            base_revision: None,
            component_id: None,
        });
        assert!(validate_plan(&plan).is_ok());
    }

    #[test]
    fn rejects_oversized_summary() {
        let mut plan = sample_create_plan();
        plan.summary = "x".repeat(MAX_PLAN_SUMMARY_CHARS + 1);
        assert!(matches!(
            validate_plan(&plan),
            Err(KernelError::Validation(_))
        ));
    }

    #[test]
    fn rejects_protected_application_id_on_create_and_evolve() {
        let mut plan = sample_create_plan();
        plan.application_id = Some("core.settings".into());
        assert!(matches!(
            validate_plan(&plan),
            Err(KernelError::Protected(_))
        ));

        let mut evolve = sample_create_plan();
        evolve.kind = ApplicationPlanKind::Evolve;
        evolve.application_id = Some("core.settings".into());
        evolve.base_revision = Some(1);
        assert!(matches!(
            validate_plan(&evolve),
            Err(KernelError::Protected(_))
        ));
    }

    #[test]
    fn rejects_malicious_create_surface_targeting_protected_tool() {
        let mut plan = sample_create_plan();
        if let ChangeIntent::CreateSurface { tool, .. } = &mut plan.intents[0] {
            tool.id = "core.settings".into();
        }
        assert!(matches!(
            validate_plan(&plan),
            Err(KernelError::Protected(_))
        ));
    }

    #[test]
    fn create_plan_intents_are_surface_then_data_model() {
        let plan = sample_create_plan();
        assert!(
            matches!(plan.intents[0], ChangeIntent::CreateSurface { .. }),
            "create plans must lead with CreateSurface so surface registers before model upsert"
        );
        assert!(
            matches!(plan.intents[1], ChangeIntent::UpsertDataModel { .. }),
            "create plans must follow with UpsertDataModel"
        );
        let ops = compile_for_inspection(&plan)
            .unwrap()
            .operations_for_inspection()
            .to_vec();
        let create_idx = ops
            .iter()
            .position(|o| o.op_type == "surface.create")
            .unwrap();
        let model_idx = ops
            .iter()
            .position(|o| o.op_type == "data.model_upsert")
            .unwrap();
        assert!(create_idx < model_idx);
    }

    #[test]
    fn rejects_cross_application_upsert_in_plan() {
        let mut plan = sample_create_plan();
        if let ChangeIntent::UpsertDataModel { application_id, .. } = &mut plan.intents[1] {
            *application_id = "tool-other-app".into();
        }
        let err = validate_plan(&plan).expect_err("cross-app upsert must fail");
        assert!(
            matches!(err, KernelError::Validation(ref m) if m.contains("does not match")),
            "got {err:?}"
        );
    }

    #[test]
    fn rejects_create_surface_id_mismatch_with_plan_application() {
        let mut plan = sample_create_plan();
        if let ChangeIntent::CreateSurface { tool, .. } = &mut plan.intents[0] {
            tool.id = "tool-other-app".into();
        }
        let err = validate_plan(&plan).expect_err("mismatched create surface must fail");
        assert!(matches!(err, KernelError::Validation(_)), "got {err:?}");
    }

    #[test]
    fn rejects_disallowed_intents_in_application_plan() {
        let mut plan = sample_create_plan();
        plan.intents.push(ChangeIntent::AddSetting {
            application_id: "tool-task-tracker".into(),
            setting_id: "theme".into(),
            label: "Theme".into(),
            value_type: "string".into(),
            default: json!("light"),
        });
        let err = validate_plan(&plan).expect_err("settings intent must fail");
        assert!(
            matches!(err, KernelError::Validation(ref m) if m.contains("does not allow")),
            "got {err:?}"
        );

        let mut plan = sample_create_plan();
        plan.intents.push(ChangeIntent::NavigateRoute {
            application_id: "tool-task-tracker".into(),
            route_id: "home".into(),
        });
        let err = validate_plan(&plan).expect_err("navigate intent must fail");
        assert!(
            matches!(err, KernelError::Validation(ref m) if m.contains("does not allow")),
            "got {err:?}"
        );

        let mut plan = sample_create_plan();
        plan.intents.push(ChangeIntent::UpsertManifest {
            manifest: crate::application_kernel::manifest::ApplicationManifest {
                schema_version: "1".into(),
                application_id: "tool-task-tracker".into(),
                instance_id: "inst".into(),
                name: "Task Tracker".into(),
                description: String::new(),
                version: 1,
                surfaces: vec![],
                routes: vec![],
                data_models: vec![],
                settings: vec![],
                capabilities: vec![],
                permissions: vec![],
                events: vec![],
                tests: vec![],
                search_keywords: vec![],
                tags: vec![],
                agent_description: String::new(),
                project_id: None,
                conversation_id: None,
                organization_id: None,
                ownership: None,
                application_action_access: vec![],
                surface_action_access: Default::default(),
                component_action_access: Default::default(),
                action_descriptor_hashes: Default::default(),
            },
        });
        let err = validate_plan(&plan).expect_err("manifest intent must fail");
        assert!(
            matches!(err, KernelError::Validation(ref m) if m.contains("does not allow")),
            "got {err:?}"
        );
    }

    mod db_lineage {
        use super::*;
        use crate::application_kernel::lineage::LineageScope;
        use crate::db::{create_conversation, Database, DEFAULT_WORKSPACE_ID};
        use crate::runtime_v2::surfaces::{bind_surface_tool_id, get_surface};
        use serde_json::json;
        use tempfile::tempdir;

        const APP_ID: &str = "tool-task-tracker";

        fn insert_tool(db: &Database, id: &str) {
            db.conn()
                .execute(
                    "INSERT INTO tools (id, workspace_id, name, description, layout, definition_json, current_version, created_at, updated_at)
                     VALUES (?1, ?2, ?1, '', 'stack', '{}', 1, datetime('now'), datetime('now'))",
                    rusqlite::params![id, DEFAULT_WORKSPACE_ID],
                )
                .unwrap();
        }

        /// Real inline surface bound to Task Tracker tool (revision 1).
        fn setup_bound_task_tracker_surface() -> (Database, String, String) {
            let dir = tempdir().unwrap();
            let mut db = Database::open_path(&dir.path().join("plan-lineage.db")).unwrap();
            let conv =
                create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Plan lineage", None).unwrap();
            insert_tool(&db, APP_ID);
            let surf = crate::runtime_v2::surfaces::create_inline_surface(
                &mut db,
                &conv.id,
                None,
                None,
                "Task Tracker",
                &json!({
                    "id": "doc-tt",
                    "name": "Task Tracker",
                    "layout": "stack",
                    "components": [{"id": "t", "type": "text", "props": {"text": "hi"}}]
                }),
                &[],
            )
            .unwrap();
            bind_surface_tool_id(&mut db, &surf.id, APP_ID).unwrap();
            assert_eq!(get_surface(&db, &surf.id).unwrap().current_revision, 1);
            (db, conv.id, surf.id)
        }

        fn scope_for(conv_id: &str) -> LineageScope {
            LineageScope {
                conversation_id: Some(conv_id.to_string()),
                project_id: None,
            }
        }

        #[test]
        fn matching_revision_evolve_compiles_against_db() {
            let (db, conv_id, _) = setup_bound_task_tracker_surface();
            let plan = crate::ai::plan_fixtures::task_tracker_add_due_dates_plan(Some(1));
            let validated =
                compile_plan_against_db(&db, &plan, &scope_for(&conv_id)).expect("must compile");
            assert!(!validated.compiled.operations.is_empty());
            assert!(
                validated
                    .compiled
                    .operations
                    .iter()
                    .any(|op| op.op_type == "data.model_upsert"),
                "evolve fixture must emit data.model_upsert"
            );
            assert!(
                validated
                    .compiled
                    .operations
                    .iter()
                    .any(|op| op.op_type == "tool.full_replace"),
                "evolve fixture must emit tool.full_replace"
            );
            assert!(
                validated
                    .compiled
                    .operations
                    .iter()
                    .all(|op| op.base_revision == Some(1)),
                "plan baseRevision must stamp compiled ops"
            );
        }

        #[test]
        fn stale_surface_revision_returns_revision_conflict() {
            let (db, conv_id, surface_id) = setup_bound_task_tracker_surface();
            db.conn()
                .execute(
                    "UPDATE surfaces SET current_revision = 2 WHERE id = ?1",
                    [&surface_id],
                )
                .unwrap();
            let plan = crate::ai::plan_fixtures::task_tracker_add_due_dates_plan(Some(1));
            let err = compile_plan_against_db(&db, &plan, &scope_for(&conv_id))
                .expect_err("stale baseRevision must fail");
            assert!(
                matches!(err, KernelError::RevisionConflict(ref m) if m.contains("stale evolve baseRevision")),
                "got {err:?}"
            );
        }

        #[test]
        fn omitted_base_revision_fails_validation_before_db() {
            let (db, conv_id, _) = setup_bound_task_tracker_surface();
            let plan = crate::ai::plan_fixtures::task_tracker_add_due_dates_plan(None);
            let err = validate_plan_against_db(&db, &plan, &scope_for(&conv_id))
                .expect_err("missing baseRevision");
            assert!(
                matches!(err, KernelError::Validation(ref m) if m.contains("baseRevision")),
                "got {err:?}"
            );
        }

        #[test]
        fn base_revision_ahead_of_surface_returns_revision_conflict() {
            let (db, conv_id, _) = setup_bound_task_tracker_surface();
            let plan = crate::ai::plan_fixtures::task_tracker_add_due_dates_plan(Some(99));
            let err = compile_plan_against_db(&db, &plan, &scope_for(&conv_id))
                .expect_err("future baseRevision must fail");
            assert!(
                matches!(err, KernelError::RevisionConflict(ref m) if m.contains("stale evolve baseRevision")),
                "got {err:?}"
            );
        }

        #[test]
        fn cross_application_surface_intent_fails_closed() {
            let (mut db, conv_id, surface_id) = setup_bound_task_tracker_surface();
            insert_tool(&db, "tool-other-app");
            bind_surface_tool_id(&mut db, &surface_id, "tool-other-app").unwrap();

            let mut plan = crate::ai::plan_fixtures::task_tracker_add_due_dates_plan(Some(1));
            plan.intents.push(ChangeIntent::InsertComponent {
                surface_id: surface_id.clone(),
                parent_id: None,
                component_type: "text".into(),
                props: json!({"text": "orphan"}),
                base_revision: None,
                component_id: Some("tm-cross-test".into()),
            });
            let err = validate_plan_against_db(&db, &plan, &scope_for(&conv_id))
                .expect_err("surface owned by other app");
            assert!(
                matches!(
                    err,
                    KernelError::Validation(ref m)
                        if m.contains("belongs to") || m.contains("does not exist")
                ),
                "got {err:?}"
            );
        }

        #[test]
        fn missing_surface_intent_fails_closed() {
            let (db, conv_id, _) = setup_bound_task_tracker_surface();
            let mut plan = crate::ai::plan_fixtures::task_tracker_add_due_dates_plan(Some(1));
            plan.intents.push(ChangeIntent::InsertComponent {
                surface_id: "surf-does-not-exist".into(),
                parent_id: None,
                component_type: "text".into(),
                props: json!({"text": "ghost"}),
                base_revision: None,
                component_id: Some("tm-missing".into()),
            });
            let err = validate_plan_against_db(&db, &plan, &scope_for(&conv_id))
                .expect_err("missing surface");
            assert!(
                matches!(err, KernelError::Validation(ref m) if m.contains("does not exist")),
                "got {err:?}"
            );
        }

        #[test]
        fn evolve_update_surface_other_application_tool_id_rejected() {
            let (mut db, conv_id, _) = setup_bound_task_tracker_surface();
            insert_tool(&db, "tool-other-app");
            let mut plan = crate::ai::plan_fixtures::task_tracker_add_due_dates_plan(Some(1));
            for intent in &mut plan.intents {
                if let ChangeIntent::UpdateSurface { tool_id, tool, .. } = intent {
                    *tool_id = "tool-other-app".into();
                    tool.id = "tool-other-app".into();
                }
            }
            let err = validate_plan_against_db(&db, &plan, &scope_for(&conv_id))
                .expect_err("UpdateSurface toolId must match plan application");
            assert!(
                matches!(err, KernelError::Validation(ref m) if m.contains("does not match")),
                "got {err:?}"
            );
        }

        #[test]
        fn foreign_bound_surface_uuid_cannot_redirect_evolve() {
            let (mut db, conv_id, _) = setup_bound_task_tracker_surface();
            insert_tool(&db, "tool-other-app");
            let other = crate::runtime_v2::surfaces::create_inline_surface(
                &mut db,
                &conv_id,
                None,
                None,
                "Other app",
                &json!({
                    "id": "doc-other",
                    "name": "Other",
                    "layout": "stack",
                    "components": [{"id": "x", "type": "text", "props": {"text": "other"}}]
                }),
                &[],
            )
            .unwrap();
            bind_surface_tool_id(&mut db, &other.id, "tool-other-app").unwrap();

            let mut plan = crate::ai::plan_fixtures::task_tracker_add_due_dates_plan(Some(1));
            plan.intents.push(ChangeIntent::InsertComponent {
                surface_id: other.id.clone(),
                parent_id: None,
                component_type: "text".into(),
                props: json!({"text": "hostile redirect"}),
                base_revision: None,
                component_id: Some("tm-hostile".into()),
            });
            let err = validate_plan_against_db(&db, &plan, &scope_for(&conv_id))
                .expect_err("other app's surface must not accept intents");
            assert!(
                matches!(err, KernelError::Validation(ref m) if m.contains("belongs to")),
                "got {err:?}"
            );
        }

        #[test]
        fn evolve_compile_fails_closed_when_application_spec_unloadable() {
            use crate::application_kernel::compiler::compile;

            let (db, conv_id, surface_id) = setup_bound_task_tracker_surface();
            db.conn()
                .execute(
                    "UPDATE surfaces SET definition_json = ?1 WHERE id = ?2",
                    rusqlite::params!["{}", surface_id],
                )
                .unwrap();

            let plan = crate::ai::plan_fixtures::task_tracker_add_due_dates_plan(Some(1));
            let err = compile_plan_against_db(&db, &plan, &scope_for(&conv_id))
                .expect_err("invalid surface definition must fail closed");
            assert!(
                matches!(err, KernelError::Validation(ref m) if m.contains("ApplicationSpec")),
                "got {err:?}"
            );

            let update_only: Vec<ChangeIntent> = plan
                .intents
                .into_iter()
                .filter(|i| matches!(i, ChangeIntent::UpdateSurface { .. }))
                .collect();
            let compile_err = compile_all_against_db(&db, &update_only)
                .expect_err("compile_all_against_db must not fall back");
            assert!(
                matches!(compile_err, KernelError::Validation(ref m) if m.contains("ApplicationSpec")),
                "got {compile_err:?}"
            );

            let legacy = compile(update_only[0].clone()).expect("DB-less inspection compile");
            assert!(
                legacy
                    .operations
                    .iter()
                    .any(|o| o.op_type == "tool.full_replace"),
                "DB-less path still full_replaces; DB path must not reach this"
            );
        }

        #[test]
        fn evolve_surface_ops_target_lineage_surface_not_synthetic_prefix() {
            let (db, conv_id, real_surface_id) = setup_bound_task_tracker_surface();
            let synthetic = format!("surf-{APP_ID}");
            assert_ne!(real_surface_id, synthetic);
            assert!(get_surface(&db, &synthetic).is_err());

            let plan = crate::ai::plan_fixtures::task_tracker_add_due_dates_plan(Some(1));
            let validated =
                compile_plan_against_db(&db, &plan, &scope_for(&conv_id)).expect("compile");
            let surface_ops: Vec<_> = validated
                .compiled
                .operations
                .iter()
                .filter(|o| o.target.surface_id.is_some())
                .collect();
            assert!(
                !surface_ops.is_empty(),
                "expected surface-targeted ops, got {:?}",
                validated
                    .compiled
                    .operations
                    .iter()
                    .map(|o| o.op_type.as_str())
                    .collect::<Vec<_>>()
            );
            for op in &surface_ops {
                assert_eq!(
                    op.target.surface_id.as_deref(),
                    Some(real_surface_id.as_str()),
                    "op {} must target lineage surface, not synthetic id",
                    op.op_type
                );
                assert_ne!(
                    op.target.surface_id.as_deref(),
                    Some(synthetic.as_str()),
                    "must not redirect via surf-{{applicationId}} prefix"
                );
            }
        }

        #[test]
        fn application_plan_with_companion_operations_rejected_at_validate() {
            use crate::ai::response_schema::{AgentResponsePayload, ResponseType};

            let mut payload = AgentResponsePayload {
                application_plan: Some(crate::ai::plan_fixtures::task_tracker_create_plan()),
                assistant_message: "create".into(),
                response_type: ResponseType::ToolChange,
                ..Default::default()
            };
            payload.operations = Some(vec![json!({
                "type": "component.remove",
                "target": { "surfaceId": "surf-x", "componentId": "c1" },
                "payload": {}
            })]);
            let err = payload
                .validate()
                .expect_err("companion operations must fail closed");
            assert!(err.contains("cannot be combined"), "got {err}");
        }

        #[test]
        fn spoofed_surf_prefix_without_db_row_fails() {
            let (db, conv_id, real_surface_id) = setup_bound_task_tracker_surface();
            let spoof = format!("surf-{APP_ID}");
            assert_ne!(
                spoof, real_surface_id,
                "canonical id must not match inline uuid"
            );

            let mut plan = crate::ai::plan_fixtures::task_tracker_add_due_dates_plan(Some(1));
            plan.intents.push(ChangeIntent::InsertComponent {
                surface_id: spoof,
                parent_id: None,
                component_type: "text".into(),
                props: json!({"text": "spoof"}),
                base_revision: None,
                component_id: Some("tm-spoof".into()),
            });
            let err = validate_plan_against_db(&db, &plan, &scope_for(&conv_id))
                .expect_err("spoofed surf-* must fail");
            assert!(
                matches!(err, KernelError::Validation(ref m) if m.contains("does not exist")),
                "got {err:?}"
            );
        }

        #[test]
        fn db_less_normalized_operations_cannot_admit_application_plan() {
            use crate::ai::plan_fixtures::task_tracker_create_plan;
            use crate::ai::response_schema::AgentResponsePayload;

            let plan = task_tracker_create_plan();
            let payload = AgentResponsePayload {
                application_plan: Some(plan.clone()),
                assistant_message: "create".into(),
                response_type: crate::ai::response_schema::ResponseType::ToolChange,
                ..Default::default()
            };
            let err = payload
                .normalized_operations()
                .expect_err("ApplicationPlan must not compile via DB-less path");
            assert!(
                err.contains("compile_plan_against_db"),
                "error must name authoritative API, got {err}"
            );

            // Inspection remains available for structural checks.
            let inspected = payload
                .inspection_operations()
                .expect("inspection compile must work");
            assert!(!inspected.is_empty());

            let inspection =
                crate::application_kernel::application_plan::compile_for_inspection(&plan)
                    .expect("inspection");
            assert!(inspection
                .diagnostics
                .iter()
                .any(|d| d.contains("inspection-only")));
        }

        #[test]
        fn study_planner_evolve_preserves_dashboard_when_tasks_gain_priority() {
            use crate::ai::plan_fixtures::{
                multi_surface_planner_add_priority_plan, multi_surface_planner_create_plan,
            };
            use crate::application_kernel::application_plan::{
                compile_plan_against_db, lineage_scope_for_plan,
            };
            use crate::application_kernel::{apply_change, decide_proposal, ChangeRequest};

            const STUDY_APP: &str = "tool-study-planner";

            let dir = tempdir().unwrap();
            let mut db = Database::open_path(&dir.path().join("study-evolve.db")).unwrap();
            let conv =
                create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Study planner evolve", None)
                    .unwrap();
            let create = multi_surface_planner_create_plan();
            let create_ops = crate::application_kernel::application_plan::compile_plan(&create)
                .unwrap()
                .compiled
                .operations;
            let prop = apply_change(
                &mut db,
                None,
                ChangeRequest {
                    conversation_id: Some(conv.id.clone()),
                    summary: "Create Study Planner".into(),
                    operations: create_ops,
                    source_type: "agent".into(),
                    ..Default::default()
                },
            )
            .unwrap()
            .proposal_id
            .unwrap();
            assert!(decide_proposal(&mut db, None, &prop, true)
                .unwrap()
                .is_committed());

            let surface = get_surface(&db, &format!("surf-{STUDY_APP}")).unwrap();
            let evolve = multi_surface_planner_add_priority_plan(Some(surface.current_revision));
            // Workspace tool canvas has no conversation binding — omit ownership scope.
            let validated = compile_plan_against_db(
                &db,
                &evolve,
                &lineage_scope_for_plan(&db, &evolve, Some(&conv.id), None)
                    .expect("workspace study planner must omit ownership claims"),
            )
            .expect("study planner evolve");
            let ops = &validated.compiled.operations;
            assert!(
                !ops.iter().any(|o| o.op_type == "tool.full_replace"),
                "tasks-only evolve must stay granular, got {:?}",
                ops.iter().map(|o| o.op_type.as_str()).collect::<Vec<_>>()
            );
            assert!(
                ops.iter().any(|o| o.op_type == "component.update_props"
                    && o.target.component_id.as_deref() == Some("sp-tasks-table")),
                "expected tasks table props update"
            );
            assert!(
                !ops.iter().any(|o| {
                    (o.op_type == "component.remove" || o.op_type == "component.insert")
                        && matches!(
                            o.target.component_id.as_deref(),
                            Some("sp-dash-heading" | "sp-dash-summary")
                        )
                }),
                "dashboard section must not be remounted"
            );
        }
    }
}
