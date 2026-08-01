//! The single choke point for registered action execution.
//!
//! Nothing else may call a handler. Every call passes recovery, lifecycle,
//! declared access, permission, breaker, authority, and audit checks here.

use serde::Serialize;
use serde_json::Value;

use crate::db::{Database, DbResult};

use super::approvals::{self, call_hash, input_preview};
use super::audit::{self, AuditInput};
use super::breakers::{self, BreakerTrip};
use super::context::{ActionRunContext, Presence, Venue};
use super::descriptor::{find_action, ActionDescriptor};
use super::grants;
use super::handlers;
use super::policy::{decide, ActionPolicyDecision, PolicyFacts};
use crate::application_kernel::manifest::{get_manifest, ManifestRecord};
use crate::application_kernel::permissions::has_permission;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum ActionOutcome {
    Ok {
        data: Value,
    },
    Error {
        code: String,
        message: String,
    },
    PendingApproval {
        approval_id: String,
        action_name: String,
        title: String,
        risk: String,
        input_preview: String,
        reason: String,
    },
    Blocked {
        code: String,
        reason: String,
    },
}

impl ActionOutcome {
    fn blocked(code: &str, reason: impl Into<String>) -> Self {
        Self::Blocked {
            code: code.into(),
            reason: reason.into(),
        }
    }

    fn error(code: &str, message: impl Into<String>) -> Self {
        Self::Error {
            code: code.into(),
            message: message.into(),
        }
    }

    fn audit_outcome(&self) -> &'static str {
        match self {
            Self::Ok { .. } => "ok",
            Self::Error { .. } => "error",
            Self::PendingApproval { .. } => "pending_approval",
            Self::Blocked { .. } => "blocked",
        }
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok { .. })
    }
}

#[derive(Debug, Default)]
struct DecisionTrail {
    source: Option<&'static str>,
    approval_id: Option<String>,
    grant_id: Option<String>,
}

/// Whether generated application surfaces may execute at all right now.
pub fn generated_execution_allowed(db: &Database) -> bool {
    match crate::application_kernel::recovery::get_recovery_state(db) {
        Ok(state) => !state.recovery_mode && !state.disable_user_surfaces,
        // Fail closed: if recovery state cannot be read, do not run generated code.
        Err(_) => false,
    }
}

/// Execute one registered action. `ctx` must be built by trusted code.
pub fn execute_registered_action(
    db: &mut Database,
    ctx: &ActionRunContext,
    action_name: &str,
    input: &Value,
    approval_id: Option<&str>,
) -> ActionOutcome {
    let started = std::time::Instant::now();
    let mut trail = DecisionTrail::default();

    let (descriptor, outcome) = match find_action(action_name) {
        Some(d) => (Some(d), run(db, ctx, d, input, approval_id, &mut trail)),
        None => (
            None,
            ActionOutcome::error("unknown_action", "That action is not available."),
        ),
    };

    let detail = match &outcome {
        ActionOutcome::Error { message, .. } => Some(message.clone()),
        ActionOutcome::Blocked { reason, .. } => Some(reason.clone()),
        _ => None,
    };
    if let ActionOutcome::PendingApproval { approval_id, .. } = &outcome {
        trail.approval_id = Some(approval_id.clone());
        trail.source = Some("approval_pending");
    }
    let preview = input_preview(input);
    let _ = audit::append_event(
        db,
        ctx,
        AuditInput {
            kind: "action",
            action_name: descriptor.map(|d| d.name.as_str()),
            input_preview: Some(&preview),
            outcome: outcome.audit_outcome(),
            risk: descriptor.map(|d| d.risk.as_str()),
            decision_source: trail.source,
            approval_id: trail.approval_id.as_deref(),
            grant_id: trail.grant_id.as_deref(),
            detail: detail.as_deref(),
            duration_ms: Some(started.elapsed().as_millis() as i64),
        },
    );
    outcome
}

fn run(
    db: &mut Database,
    ctx: &ActionRunContext,
    descriptor: &ActionDescriptor,
    input: &Value,
    approval_id: Option<&str>,
    trail: &mut DecisionTrail,
) -> ActionOutcome {
    // 1. Recovery: generated surfaces *and* their away automations are inert.
    // Chat-venue calls (no application binding) remain available so the user can
    // still talk to Coreside while recovering.
    if matches!(ctx.venue, Venue::Application | Venue::Automation)
        && !generated_execution_allowed(db)
    {
        return ActionOutcome::blocked(
            "recovery_required",
            "Coreside is in Recovery Mode; generated applications are paused.",
        );
    }

    // 2. Bounded input, schema, and nesting.
    if let Some(trip) = breakers::check_input_size(input) {
        return ActionOutcome::blocked(trip.code(), trip.reason());
    }
    if let Err(message) = super::schema::validate_input(&descriptor.input_schema, input) {
        return ActionOutcome::blocked("invalid_input", message);
    }
    if let Some(trip) = breakers::check_depth(ctx.depth) {
        return ActionOutcome::blocked(trip.code(), trip.reason());
    }

    // 3. Application lifecycle and declared access.
    let manifest = match ctx.application_id.as_deref() {
        Some(app) => match get_manifest(db, app) {
            Ok(record) => Some(record),
            Err(_) => {
                return ActionOutcome::blocked(
                    "unknown_application",
                    "This application is not registered.",
                )
            }
        },
        None => None,
    };
    if let Some(record) = manifest.as_ref() {
        if record.disabled || record.lifecycle_state == "suspended" {
            return ActionOutcome::blocked("application_disabled", "This application is turned off.");
        }
    }
    let declared = match manifest.as_ref() {
        Some(record) => action_declared(record, ctx, &descriptor.name),
        // Chat-venue calls are direct user actions with no application binding.
        None => ctx.venue == Venue::Chat,
    };
    let permission_granted = match ctx.application_id.as_deref() {
        Some(app) => has_permission(db, app, &descriptor.permission_category).unwrap_or(false),
        None => ctx.venue == Venue::Chat,
    };
    // Declared access and granted permission are always required — even when
    // consuming an approval — so revoking a permission cannot be bypassed by a
    // leftover approved receipt.
    if !declared {
        return ActionOutcome::blocked(
            "policy_blocked",
            "This application did not declare this action.",
        );
    }
    if !permission_granted {
        return ActionOutcome::blocked(
            "policy_blocked",
            "This application does not have permission for this capability yet.",
        );
    }

    // 4. History-dependent breakers.
    let trips = [
        breakers::check_rate(db, ctx.application_id.as_deref()),
        breakers::check_write_budget_if_write(db, &ctx.run_id, descriptor.risk),
        breakers::check_failure_suspension(db, ctx.application_id.as_deref(), &descriptor.name),
    ];
    for trip in trips {
        match trip {
            Ok(Some(trip)) => return ActionOutcome::blocked(trip.code(), trip.reason()),
            Ok(None) => {}
            Err(_) => return ActionOutcome::error("storage_error", "Local storage is unavailable."),
        }
    }

    let hash = call_hash(ctx, descriptor, input);

    // 5. Authority: an approval the user already decided, then a remembered
    //    grant, then the default policy.
    let mut authorized = false;
    if let Some(id) = approval_id {
        match approvals::consume(db, id, &hash) {
            Ok(true) => {
                authorized = true;
                trail.source = Some("approval");
                trail.approval_id = Some(id.to_string());
            }
            Ok(false) => {
                return ActionOutcome::blocked(
                    "approval_invalid",
                    "That approval no longer applies to this action.",
                )
            }
            Err(_) => return ActionOutcome::error("storage_error", "Local storage is unavailable."),
        }
    }

    let mut used_grant = None;
    if !authorized {
        let grant = match grants::match_grant(db, ctx, descriptor, &hash) {
            Ok(g) => g,
            Err(_) => return ActionOutcome::error("storage_error", "Local storage is unavailable."),
        };
        let facts = PolicyFacts {
            declared: true,
            permission_granted: true,
            has_grant: grant.is_some(),
        };
        match decide(ctx, descriptor, facts) {
            ActionPolicyDecision::Allow { source } => {
                trail.source = Some(source);
                if source == "grant" {
                    if let Some(g) = grant {
                        trail.grant_id = Some(g.id.clone());
                        used_grant = Some(g);
                    }
                }
            }
            ActionPolicyDecision::Block { reason } => {
                return ActionOutcome::blocked("policy_blocked", reason)
            }
            ActionPolicyDecision::RequireApproval { reason } => {
                return match approvals::create_pending(db, ctx, descriptor, input, Some(reason)) {
                    Ok(approval) => ActionOutcome::PendingApproval {
                        approval_id: approval.id,
                        action_name: descriptor.name.clone(),
                        title: descriptor.title.clone(),
                        risk: descriptor.risk.as_str().into(),
                        input_preview: approval.input_preview,
                        reason: reason.into(),
                    },
                    Err(e) => ActionOutcome::error("approval_failed", e.to_string()),
                };
            }
        }
    }

    // 6. Duplicate suppression around the actual execution.
    let Some(_in_flight) = breakers::begin_in_flight(&hash) else {
        let trip = BreakerTrip::DuplicateInFlight;
        return ActionOutcome::blocked(trip.code(), trip.reason());
    };

    match handlers::dispatch(db, ctx, descriptor, input) {
        Ok(data) => {
            if let Some(trip) = breakers::check_output_size(&data) {
                return ActionOutcome::blocked(trip.code(), trip.reason());
            }
            if let Some(grant) = used_grant.as_ref() {
                let _ = grants::consume_once_grant(db, grant);
            }
            ActionOutcome::Ok { data }
        }
        Err(err) => ActionOutcome::error(&err.code, err.message),
    }
}

/// Declared access narrows from application to surface to component.
/// A level with no entry declares no extra narrowing; an entry that omits the
/// action denies it.
///
/// ponytail: empty `application_action_access` (legacy tool wraps) means every
/// bundled action covered by the manifest's declared permissions. Explicit
/// lists remain deny-by-omission.
fn action_declared(record: &ManifestRecord, ctx: &ActionRunContext, action: &str) -> bool {
    let manifest = &record.manifest;
    let app_allows = if manifest.application_action_access.is_empty() {
        super::descriptor::default_action_access_for_permissions(&manifest.permissions)
            .iter()
            .any(|a| a == action)
    } else {
        manifest.application_action_access.iter().any(|a| a == action)
    };
    if !app_allows {
        return false;
    }
    if let Some(surface_id) = ctx.surface_id.as_deref() {
        if let Some(allowed) = manifest.surface_action_access.get(surface_id) {
            if !allowed.iter().any(|a| a == action) {
                return false;
            }
        }
    }
    if let Some(component_id) = ctx.component_id.as_deref() {
        if let Some(allowed) = manifest.component_action_access.get(component_id) {
            if !allowed.iter().any(|a| a == action) {
                return false;
            }
        }
    }
    true
}

/// Trusted entry for the automation executor: presence is always `away`.
/// No IPC command can reach this path.
pub fn execute_registered_action_trusted(
    db: &mut Database,
    application_id: Option<String>,
    automation_id: &str,
    run_id: &str,
    action_name: &str,
    input: &Value,
) -> ActionOutcome {
    let ctx = ActionRunContext::for_automation(application_id, automation_id, run_id);
    debug_assert_eq!(ctx.presence, Presence::Away);
    execute_registered_action(db, &ctx, action_name, input, None)
}

pub fn pending_approval_count(db: &Database) -> DbResult<i64> {
    db.conn()
        .query_row(
            "SELECT COUNT(*) FROM runtime_approvals WHERE status = 'pending'",
            [],
            |r| r.get(0),
        )
        .map_err(crate::db::DbError::Sqlite)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application_kernel::data::{upsert_model, DataField, DataModelDefinition};
    use crate::application_kernel::registered_actions::approvals::RememberChoice;
    use crate::application_kernel::registered_actions::grants::{GrantDuration, GrantScope};
    use crate::application_kernel::registered_actions::testing::{app_ctx, seed_application, test_db};
    use crate::db::Database;
    use serde_json::json;

    const APP: &str = "app-registered-actions";

    fn seed(db: &mut Database) {
        seed_application(
            db,
            APP,
            &[
                "local_data.query",
                "local_data.write",
                "local_data.delete",
                "media.read",
                "export.prepare",
            ],
        );
        upsert_model(
            db,
            APP,
            DataModelDefinition {
                model_id: "note".into(),
                display_name: "Note".into(),
                schema_version: 1,
                fields: vec![DataField {
                    field_id: "title".into(),
                    field_type: "text".into(),
                    required: true,
                    default: None,
                    enum_values: None,
                }],
            },
        )
        .unwrap();
    }

    fn outcome_code(outcome: &ActionOutcome) -> String {
        match outcome {
            ActionOutcome::Ok { .. } => "ok".into(),
            ActionOutcome::Error { code, .. } => format!("error:{code}"),
            ActionOutcome::Blocked { code, .. } => format!("blocked:{code}"),
            ActionOutcome::PendingApproval { .. } => "pending".into(),
        }
    }

    #[test]
    fn read_runs_without_asking() {
        let (mut db, _dir) = test_db();
        seed(&mut db);
        let ctx = app_ctx(APP);
        let outcome = execute_registered_action(
            &mut db,
            &ctx,
            "local_data.query",
            &json!({ "modelId": "note" }),
            None,
        );
        assert!(outcome.is_ok(), "{}", outcome_code(&outcome));
    }

    #[test]
    fn write_asks_then_runs_once_approved() {
        let (mut db, _dir) = test_db();
        seed(&mut db);
        let ctx = app_ctx(APP);
        let input = json!({ "modelId": "note", "data": { "title": "Hello" } });

        let first = execute_registered_action(&mut db, &ctx, "local_data.write", &input, None);
        let approval_id = match &first {
            ActionOutcome::PendingApproval { approval_id, .. } => approval_id.clone(),
            other => panic!("expected approval, got {}", outcome_code(other)),
        };

        approvals::decide(&mut db, &approval_id, true, None, "user").unwrap();
        let second =
            execute_registered_action(&mut db, &ctx, "local_data.write", &input, Some(&approval_id));
        assert!(second.is_ok(), "{}", outcome_code(&second));

        // The same approval cannot authorize a second write.
        let third =
            execute_registered_action(&mut db, &ctx, "local_data.write", &input, Some(&approval_id));
        assert_eq!(outcome_code(&third), "blocked:approval_invalid");
    }

    #[test]
    fn remembered_grant_skips_the_next_prompt() {
        let (mut db, _dir) = test_db();
        seed(&mut db);
        let ctx = app_ctx(APP);
        let input = json!({ "modelId": "note", "data": { "title": "Hello" } });

        let first = execute_registered_action(&mut db, &ctx, "local_data.write", &input, None);
        let approval_id = match &first {
            ActionOutcome::PendingApproval { approval_id, .. } => approval_id.clone(),
            other => panic!("expected approval, got {}", outcome_code(other)),
        };
        approvals::decide(
            &mut db,
            &approval_id,
            true,
            Some(RememberChoice {
                scope: GrantScope::Action,
                duration: GrantDuration::Session,
            }),
            "user",
        )
        .unwrap();

        let next = execute_registered_action(
            &mut db,
            &ctx,
            "local_data.write",
            &json!({ "modelId": "note", "data": { "title": "Another" } }),
            None,
        );
        assert!(next.is_ok(), "{}", outcome_code(&next));
    }

    #[test]
    fn destructive_always_asks_and_cannot_be_remembered() {
        let (mut db, _dir) = test_db();
        seed(&mut db);
        let ctx = app_ctx(APP);
        assert!(
            grants::mint_grant(
                &mut db,
                &ctx,
                find_action("local_data.delete").unwrap(),
                GrantScope::Action,
                GrantDuration::Session,
                None,
                "user",
            )
            .is_err()
        );
        let outcome = execute_registered_action(
            &mut db,
            &ctx,
            "local_data.delete",
            &json!({ "recordId": "rec-1" }),
            None,
        );
        assert_eq!(outcome_code(&outcome), "pending");
    }

    #[test]
    fn undeclared_action_is_blocked() {
        let (mut db, _dir) = test_db();
        // Declares only reads, so a write is not declared.
        seed_application(&mut db, "app-narrow", &["local_data.query"]);
        let ctx = app_ctx("app-narrow");
        let outcome = execute_registered_action(
            &mut db,
            &ctx,
            "local_data.write",
            &json!({ "modelId": "note", "data": {} }),
            None,
        );
        assert_eq!(outcome_code(&outcome), "blocked:policy_blocked");
    }

    #[test]
    fn surface_and_component_narrowing_is_enforced() {
        let (mut db, _dir) = test_db();
        seed(&mut db);
        let mut record = get_manifest(&db, APP).unwrap().manifest;
        record
            .surface_action_access
            .insert("surface-locked".into(), vec!["local_data.query".into()]);
        record
            .component_action_access
            .insert("component-locked".into(), vec![]);
        crate::application_kernel::manifest::upsert_manifest(&mut db, record).unwrap();

        let mut ctx = app_ctx(APP);
        ctx.surface_id = Some("surface-locked".into());
        let allowed = execute_registered_action(
            &mut db,
            &ctx,
            "local_data.query",
            &json!({ "modelId": "note" }),
            None,
        );
        assert!(allowed.is_ok(), "{}", outcome_code(&allowed));

        ctx.component_id = Some("component-locked".into());
        let denied = execute_registered_action(
            &mut db,
            &ctx,
            "local_data.query",
            &json!({ "modelId": "note" }),
            None,
        );
        assert_eq!(outcome_code(&denied), "blocked:policy_blocked");
    }

    #[test]
    fn ungranted_permission_is_blocked() {
        let (mut db, _dir) = test_db();
        seed(&mut db);
        crate::application_kernel::permissions::revoke_permission(&mut db, APP, "local_data.read")
            .unwrap();
        let outcome = execute_registered_action(
            &mut db,
            &app_ctx(APP),
            "local_data.query",
            &json!({ "modelId": "note" }),
            None,
        );
        assert_eq!(outcome_code(&outcome), "blocked:policy_blocked");
    }

    #[test]
    fn revoked_permission_blocks_even_with_approved_receipt() {
        let (mut db, _dir) = test_db();
        seed(&mut db);
        let ctx = app_ctx(APP);
        let input = json!({ "modelId": "note", "data": { "title": "Hello" } });
        let first = execute_registered_action(&mut db, &ctx, "local_data.write", &input, None);
        let approval_id = match &first {
            ActionOutcome::PendingApproval { approval_id, .. } => approval_id.clone(),
            other => panic!("expected approval, got {}", outcome_code(other)),
        };
        approvals::decide(&mut db, &approval_id, true, None, "user").unwrap();
        crate::application_kernel::permissions::revoke_permission(&mut db, APP, "local_data.write")
            .unwrap();
        let second =
            execute_registered_action(&mut db, &ctx, "local_data.write", &input, Some(&approval_id));
        assert_eq!(outcome_code(&second), "blocked:policy_blocked");
    }

    #[test]
    fn empty_action_access_falls_back_to_declared_permissions() {
        let (mut db, _dir) = test_db();
        seed_application(&mut db, "app-legacy", &[]);
        // Replace with a legacy-shaped manifest: permissions only, empty access list.
        let mut record = get_manifest(&db, "app-legacy").unwrap().manifest;
        record.permissions = vec!["local_data.read".into()];
        record.application_action_access.clear();
        crate::application_kernel::manifest::upsert_manifest(&mut db, record).unwrap();
        crate::application_kernel::permissions::grant_permission(
            &mut db,
            "app-legacy",
            "local_data.read",
            json!({ "scope": "application" }),
            "user",
        )
        .unwrap();
        upsert_model(
            &mut db,
            "app-legacy",
            DataModelDefinition {
                model_id: "note".into(),
                display_name: "Note".into(),
                schema_version: 1,
                fields: vec![DataField {
                    field_id: "title".into(),
                    field_type: "text".into(),
                    required: true,
                    default: None,
                    enum_values: None,
                }],
            },
        )
        .unwrap();
        let outcome = execute_registered_action(
            &mut db,
            &app_ctx("app-legacy"),
            "local_data.query",
            &json!({ "modelId": "note" }),
            None,
        );
        assert!(outcome.is_ok(), "{}", outcome_code(&outcome));
    }

    #[test]
    fn recovery_mode_stops_generated_applications() {
        let (mut db, _dir) = test_db();
        seed(&mut db);
        crate::application_kernel::recovery::set_recovery_mode(&mut db, true).unwrap();
        assert!(!generated_execution_allowed(&db));
        let outcome = execute_registered_action(
            &mut db,
            &app_ctx(APP),
            "local_data.query",
            &json!({ "modelId": "note" }),
            None,
        );
        assert_eq!(outcome_code(&outcome), "blocked:recovery_required");
    }

    #[test]
    fn recovery_mode_stops_application_bound_automations() {
        let (mut db, _dir) = test_db();
        seed(&mut db);
        // Standing grant would authorize an away write when present — recovery
        // must still refuse so automations cannot bypass the pause.
        grants::mint_grant(
            &mut db,
            &app_ctx(APP),
            find_action("local_data.write").unwrap(),
            GrantScope::ApplicationAction,
            GrantDuration::Standing,
            None,
            "user",
        )
        .unwrap();
        crate::application_kernel::recovery::set_recovery_mode(&mut db, true).unwrap();
        let outcome = execute_registered_action_trusted(
            &mut db,
            Some(APP.into()),
            "auto-1",
            "run-recovery",
            "local_data.write",
            &json!({ "modelId": "note", "data": { "title": "away" } }),
        );
        assert_eq!(outcome_code(&outcome), "blocked:recovery_required");
    }

    #[test]
    fn disabled_application_cannot_act() {
        let (mut db, _dir) = test_db();
        seed(&mut db);
        crate::application_kernel::lifecycle::set_application_enabled(&mut db, APP, false).unwrap();
        let outcome = execute_registered_action(
            &mut db,
            &app_ctx(APP),
            "local_data.query",
            &json!({ "modelId": "note" }),
            None,
        );
        assert_eq!(outcome_code(&outcome), "blocked:application_disabled");
    }

    #[test]
    fn disabling_revokes_remembered_grants() {
        let (mut db, _dir) = test_db();
        seed(&mut db);
        let ctx = app_ctx(APP);
        grants::mint_grant(
            &mut db,
            &ctx,
            find_action("local_data.write").unwrap(),
            GrantScope::Action,
            GrantDuration::Session,
            None,
            "user",
        )
        .unwrap();
        crate::application_kernel::lifecycle::set_application_enabled(&mut db, APP, false).unwrap();
        crate::application_kernel::lifecycle::set_application_enabled(&mut db, APP, true).unwrap();
        let outcome = execute_registered_action(
            &mut db,
            &ctx,
            "local_data.write",
            &json!({ "modelId": "note", "data": { "title": "x" } }),
            None,
        );
        assert_eq!(outcome_code(&outcome), "pending");
    }

    #[test]
    fn unknown_application_and_action_are_refused() {
        let (mut db, _dir) = test_db();
        seed(&mut db);
        let unknown_app = execute_registered_action(
            &mut db,
            &app_ctx("app-missing"),
            "local_data.query",
            &json!({ "modelId": "note" }),
            None,
        );
        assert_eq!(outcome_code(&unknown_app), "blocked:unknown_application");

        let unknown_action = execute_registered_action(
            &mut db,
            &app_ctx(APP),
            "shell.exec",
            &json!({}),
            None,
        );
        assert_eq!(outcome_code(&unknown_action), "error:unknown_action");
    }

    #[test]
    fn away_write_parks_and_away_destructive_is_blocked() {
        let (mut db, _dir) = test_db();
        seed(&mut db);
        let mut ctx = app_ctx(APP);
        ctx.presence = Presence::Away;
        ctx.venue = Venue::Automation;

        let parked = execute_registered_action(
            &mut db,
            &ctx,
            "local_data.write",
            &json!({ "modelId": "note", "data": { "title": "away" } }),
            None,
        );
        assert_eq!(outcome_code(&parked), "pending");

        let blocked = execute_registered_action(
            &mut db,
            &ctx,
            "local_data.delete",
            &json!({ "recordId": "rec-1" }),
            None,
        );
        assert_eq!(outcome_code(&blocked), "blocked:policy_blocked");
    }

    #[test]
    fn away_write_runs_on_standing_application_bound_grant() {
        let (mut db, _dir) = test_db();
        seed(&mut db);
        let present = app_ctx(APP);
        grants::mint_grant(
            &mut db,
            &present,
            find_action("local_data.write").unwrap(),
            GrantScope::ApplicationAction,
            GrantDuration::Standing,
            None,
            "automation",
        )
        .unwrap();

        let mut away = app_ctx(APP);
        away.presence = Presence::Away;
        away.venue = Venue::Automation;
        let outcome = execute_registered_action(
            &mut db,
            &away,
            "local_data.write",
            &json!({ "modelId": "note", "data": { "title": "away" } }),
            None,
        );
        assert!(outcome.is_ok(), "{}", outcome_code(&outcome));
    }

    #[test]
    fn oversized_input_is_blocked() {
        let (mut db, _dir) = test_db();
        seed(&mut db);
        let big = json!({ "modelId": "note", "blob": "x".repeat(breakers::MAX_INPUT_BYTES + 10) });
        let outcome =
            execute_registered_action(&mut db, &app_ctx(APP), "local_data.query", &big, None);
        assert_eq!(outcome_code(&outcome), "blocked:input_too_large");
    }

    #[test]
    fn every_call_is_audited() {
        let (mut db, _dir) = test_db();
        seed(&mut db);
        execute_registered_action(
            &mut db,
            &app_ctx(APP),
            "local_data.query",
            &json!({ "modelId": "note" }),
            None,
        );
        let events = audit::list_events(&db, Some(APP), 10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action_name.as_deref(), Some("local_data.query"));
        assert_eq!(events[0].outcome, "ok");
        assert_eq!(events[0].risk.as_deref(), Some("read"));
    }

    #[test]
    fn cross_application_data_access_is_refused() {
        let (mut db, _dir) = test_db();
        seed(&mut db);
        seed_application(&mut db, "app-other", &["local_data.query"]);
        // `app-other` has no `note` model of its own, so reading it must fail
        // rather than fall through to another application's data.
        let outcome = execute_registered_action(
            &mut db,
            &app_ctx("app-other"),
            "local_data.query",
            &json!({ "modelId": "note" }),
            None,
        );
        assert!(matches!(outcome, ActionOutcome::Ok { ref data }
            if data.get("count").and_then(|v| v.as_u64()) == Some(0)));
    }
}
