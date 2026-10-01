//! Provider-neutral application generation protocol.
//!
//! The model proposes an untrusted `ApplicationPlan`. Rust validates, compiles
//! through `ChangeIntent` → `AppOperation`, then the kernel decides authority.
//! Providers (mock, recorded, live) all emit this same contract.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::runtime_v2::operations::AppOperation;
use crate::runtime_v2::packs::validate_component_type_allowed;

use super::compiler::{compile, compile_all, ChangeIntent, CompiledChange};
use super::errors::KernelError;
use super::testing::DeclarativeTest;

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
    /// Optimistic concurrency base for evolve plans.
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

/// One Apply must not silently mutate multiple applications.
fn bind_intents_to_plan_application(plan: &ApplicationPlan) -> Result<(), KernelError> {
    let create_tool_id = plan.intents.iter().find_map(|i| match i {
        ChangeIntent::CreateSurface { tool, .. } => Some(tool.id.as_str()),
        _ => None,
    });
    let bound = match plan.kind {
        ApplicationPlanKind::Evolve => plan
            .application_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                KernelError::Validation("applicationId required for evolve applicationPlan".into())
            })?,
        ApplicationPlanKind::Create => {
            if let Some(app) = plan
                .application_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                app
            } else {
                create_tool_id.ok_or_else(|| {
                    KernelError::Validation(
                        "create applicationPlan requires CreateSurface or applicationId".into(),
                    )
                })?
            }
        }
    };
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

fn intent_application_id(intent: &ChangeIntent) -> Option<&str> {
    match intent {
        ChangeIntent::CreateSurface { tool, .. } => Some(tool.id.as_str()),
        ChangeIntent::UpdateSurface { tool_id, .. } => Some(tool_id.as_str()),
        ChangeIntent::UpsertDataModel { application_id, .. }
        | ChangeIntent::MigrateDataModel { application_id, .. } => Some(application_id.as_str()),
        ChangeIntent::InsertComponent { surface_id, .. }
        | ChangeIntent::UpdateComponent { surface_id, .. }
        | ChangeIntent::RemoveComponent { surface_id, .. } => {
            surface_id.strip_prefix("surf-").or(Some(surface_id.as_str()))
        }
        // Exhaustiveness: these intents are rejected in validate_intent_refs.
        ChangeIntent::AddSetting { .. }
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
            if let Some(component_type) = component_type {
                validate_component_type_allowed(component_type)
                    .map_err(KernelError::CapabilityUnavailable)?;
            }
        }
        ChangeIntent::RemoveComponent { surface_id, .. } => {
            crate::security::assert_not_protected(surface_id).map_err(KernelError::Protected)?;
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

/// Validate + compile an ApplicationPlan into kernel-ready operations.
pub fn compile_plan(plan: &ApplicationPlan) -> Result<ValidatedApplicationPlan, KernelError> {
    validate_plan(plan)?;
    let compiled = compile_all(&plan.intents)?;
    let mut diagnostics = Vec::new();
    if plan.kind == ApplicationPlanKind::Evolve && plan.base_revision.is_none() {
        diagnostics.push(
            "evolve plan has no baseRevision; OCC will rely on per-operation baseRevision"
                .into(),
        );
    }
    Ok(ValidatedApplicationPlan {
        plan: plan.clone(),
        compiled,
        diagnostics,
    })
}

/// Expand a validated plan into AppOperations (same path as ChangeIntent compile).
pub fn plan_to_operations(plan: &ApplicationPlan) -> Result<Vec<AppOperation>, KernelError> {
    Ok(compile_plan(plan)?.compiled.operations)
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

    #[test]
    fn rejects_evolve_without_application_id() {
        let mut plan = sample_create_plan();
        plan.kind = ApplicationPlanKind::Evolve;
        plan.application_id = None;
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
        let ops = plan_to_operations(&plan).unwrap();
        let create_idx = ops.iter().position(|o| o.op_type == "surface.create").unwrap();
        let model_idx = ops
            .iter()
            .position(|o| o.op_type == "data.model_upsert")
            .unwrap();
        assert!(create_idx < model_idx);
    }

    #[test]
    fn rejects_cross_application_upsert_in_plan() {
        let mut plan = sample_create_plan();
        if let ChangeIntent::UpsertDataModel {
            application_id, ..
        } = &mut plan.intents[1]
        {
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
}
