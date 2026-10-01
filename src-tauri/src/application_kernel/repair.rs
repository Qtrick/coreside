//! Bounded self-repair for generated applications after verification failure.
//!
//! GENERATE → VALIDATE → APPLY → VERIFY → FAILURE → REPAIR PLAN → APPLY → VERIFY
//! Max repair rounds are hard-capped. Repair must not escalate privileges or
//! disable tests to force a pass. On exhaustion, restore last-known-good.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::db::{Database, DbError};
use crate::runtime_v2::operations::AppOperation;
use crate::runtime_v2::software_document::SoftwareDocument;

use super::errors::KernelError;
use super::testing::{verify_after_change, VerificationResult};

pub const MAX_REPAIR_ROUNDS: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairRound {
    pub round: u32,
    pub diagnostics: Vec<String>,
    pub operations_applied: usize,
    pub verification: Option<VerificationResult>,
    pub outcome: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairLoopResult {
    pub status: String,
    pub rounds: Vec<RepairRound>,
    pub restored_last_known_good: bool,
    pub message: String,
}

/// Attempt bounded local repair of a surface definition after verification failed.
///
/// Deterministic sanitize/dedupe via SoftwareDocument — not an unbounded model re-ask.
pub fn run_bounded_repair(
    db: &mut Database,
    application_id: &str,
    surface_id: &str,
    definition: &Value,
    failed_verification: &VerificationResult,
) -> Result<RepairLoopResult, KernelError> {
    crate::security::assert_not_protected(application_id).map_err(KernelError::Protected)?;
    crate::security::assert_not_protected(surface_id).map_err(KernelError::Protected)?;

    let mut rounds = Vec::new();
    let mut current_def = definition.clone();
    let mut last_verification = failed_verification.clone();

    for round in 1..=MAX_REPAIR_ROUNDS {
        if last_verification.is_acceptable() {
            return Ok(RepairLoopResult {
                status: "repaired".into(),
                rounds,
                restored_last_known_good: false,
                message: "Verification already acceptable; no further repair".into(),
            });
        }

        let mut doc = SoftwareDocument::from_value(&current_def)
            .map_err(|e| KernelError::Validation(format!("repair cannot parse document: {e}")))?;
        let notes = doc.validate_and_repair();
        // Persist the repaired SoftwareDocument — do not round-trip through
        // ToolDefinition (drops sections / design tokens / contract shape).
        let repaired_def =
            serde_json::to_value(&doc).map_err(|e| KernelError::Validation(e.to_string()))?;
        let diagnostics: Vec<String> = notes
            .into_iter()
            .map(|n| format!("{}:{}:{}", n.kind, n.target_id, n.detail))
            .collect();

        if definition_escalates_permissions(&current_def, &repaired_def) {
            rounds.push(RepairRound {
                round,
                diagnostics: vec!["repair refused: privilege escalation detected".into()],
                operations_applied: 0,
                verification: None,
                outcome: "privilege_escalation_blocked".into(),
            });
            break;
        }

        // Verify uses the same app-id extraction path as kernel apply (tool.id / applicationId).
        let ops = vec![AppOperation {
            id: format!("op-repair-{round}"),
            op_type: "tool.full_replace".into(),
            target: crate::runtime_v2::operations::OperationTarget {
                surface_id: Some(surface_id.into()),
                tool_id: Some(application_id.into()),
                application_id: Some(application_id.into()),
                surface_type: Some("tool".into()),
                placement: Some("tool_canvas".into()),
                ..Default::default()
            },
            base_revision: None,
            transaction_group: Some(format!("repair-{round}")),
            idempotency_key: Some(format!("repair-{application_id}-{round}")),
            depends_on: None,
            payload: json!({
                "action": "update",
                "tool": repaired_def,
                "changeSummary": format!("Bounded repair round {round}"),
            }),
            requires_approval: None,
            destructive: None,
            audience: None,
        }];

        let applied = apply_repair_definition(db, surface_id, &repaired_def)?;
        current_def = repaired_def;

        let verification = verify_after_change(db, &ops).ok();
        let outcome = verification
            .as_ref()
            .map(|v| v.status.clone())
            .unwrap_or_else(|| "verify_unavailable".into());

        rounds.push(RepairRound {
            round,
            diagnostics,
            operations_applied: if applied { 1 } else { 0 },
            verification: verification.clone(),
            outcome: outcome.clone(),
        });

        if let Some(v) = verification {
            if v.is_acceptable() {
                return Ok(RepairLoopResult {
                    status: "repaired".into(),
                    rounds,
                    restored_last_known_good: false,
                    message: format!("Repaired after {round} round(s)"),
                });
            }
            last_verification = v;
        }
    }

    let restored = match super::manifest::restore_last_known_good(db, application_id) {
        Ok(_) => true,
        Err(DbError::Invalid(_)) | Err(DbError::NotFound(_)) => false,
        Err(e) => return Err(KernelError::Db(e)),
    };
    Ok(RepairLoopResult {
        status: if restored {
            "restored_last_known_good".into()
        } else {
            "failed".into()
        },
        rounds,
        restored_last_known_good: restored,
        message: format!(
            "Repair exhausted after {MAX_REPAIR_ROUNDS} round(s); restored_lkg={restored}"
        ),
    })
}

fn definition_escalates_permissions(before: &Value, after: &Value) -> bool {
    let before_perms = collect_permission_strings(before);
    let after_perms = collect_permission_strings(after);
    after_perms.iter().any(|p| !before_perms.contains(p))
}

fn collect_permission_strings(v: &Value) -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    for path in [
        v.get("permissions"),
        v.pointer("/tool/permissions"),
        v.get("applicationActionAccess"),
    ] {
        if let Some(arr) = path.and_then(|p| p.as_array()) {
            for item in arr {
                if let Some(s) = item.as_str() {
                    out.insert(s.to_string());
                }
            }
        }
    }
    out
}

fn apply_repair_definition(
    db: &mut Database,
    surface_id: &str,
    definition: &Value,
) -> Result<bool, KernelError> {
    match crate::runtime_v2::surfaces::get_surface(db, surface_id) {
        Ok(_) => {
            crate::runtime_v2::surfaces::update_surface_definition(
                db,
                surface_id,
                definition,
                "bounded self-repair",
                None,
            )
            .map_err(KernelError::Db)?;
            Ok(true)
        }
        Err(DbError::NotFound(_)) => Ok(false),
        Err(e) => Err(KernelError::Db(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_permission_escalation() {
        let before = json!({ "permissions": ["local_data.read"] });
        let after = json!({ "permissions": ["local_data.read", "local_data.write"] });
        assert!(definition_escalates_permissions(&before, &after));
        assert!(!definition_escalates_permissions(&before, &before));
    }

    #[test]
    fn refuses_application_action_access_and_nested_tool_escalation() {
        let before = json!({ "applicationActionAccess": ["local_data.read"] });
        let after = json!({
            "applicationActionAccess": ["local_data.read", "local_data.write"]
        });
        assert!(
            definition_escalates_permissions(&before, &after),
            "new applicationActionAccess must be blocked"
        );
        let nested_before = json!({ "tool": { "permissions": ["local_data.read"] } });
        let nested_after = json!({
            "tool": { "permissions": ["local_data.read", "network.fetch"] }
        });
        assert!(
            definition_escalates_permissions(&nested_before, &nested_after),
            "nested tool.permissions escalation must be blocked"
        );
        // Shrinking the set is not escalation (repair may strip).
        assert!(!definition_escalates_permissions(&after, &before));
    }

    #[test]
    fn max_rounds_is_bounded() {
        assert_eq!(MAX_REPAIR_ROUNDS, 3);
    }

    #[test]
    fn acceptable_status_matches_verify_after_change() {
        // Regression: repair looked for "passed" while verify_after_change emits
        // "verified" — that forced LKG restore after a successful sanitize.
        assert!(VerificationResult {
            status: "verified".into(),
            message: "ok".into(),
            test_results: vec![],
        }
        .is_acceptable());
        assert!(VerificationResult {
            status: "verified_with_warnings".into(),
            message: "ok".into(),
            test_results: vec![],
        }
        .is_acceptable());
        assert!(!VerificationResult {
            status: "failed".into(),
            message: "no".into(),
            test_results: vec![],
        }
        .is_acceptable());
    }

    #[test]
    fn run_bounded_repair_rejects_protected_application() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = crate::db::Database::open_path(&dir.path().join("repair-prot.db")).unwrap();
        let failed = VerificationResult {
            status: "failed".into(),
            message: "boom".into(),
            test_results: vec![],
        };
        let err = run_bounded_repair(
            &mut db,
            "core.settings",
            "surf-core.settings",
            &json!({ "id": "core.settings", "name": "Settings", "components": [] }),
            &failed,
        )
        .unwrap_err();
        assert!(matches!(err, KernelError::Protected(_)));
    }

    #[test]
    fn run_bounded_repair_stops_within_max_rounds_when_verify_keeps_failing() {
        use crate::ai::ToolDefinition;
        use crate::application_kernel::manifest::ensure_manifest_for_tool;
        use crate::application_kernel::testing::{upsert_test, DeclarativeTest, TestAssertion};
        use crate::db::{apply_tool_change, Database, DEFAULT_WORKSPACE_ID};
        use crate::runtime_v2::surfaces::{surface_id_for_tool, upsert_surface_from_tool};

        let dir = tempfile::tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("repair-rounds.db")).unwrap();
        let app_id = "tool-repair-bound";
        let surface_id = surface_id_for_tool(app_id);
        let tool = ToolDefinition {
            id: app_id.into(),
            name: "Repair Bound".into(),
            description: "d".into(),
            layout: json!({"type": "stack"}),
            components: vec![crate::ai::ToolComponent {
                id: "h".into(),
                component_type: "heading".into(),
                props: Some(json!({"text": "Repair Bound"})),
                ..Default::default()
            }],
            ..Default::default()
        };
        let applied =
            apply_tool_change(&mut db, DEFAULT_WORKSPACE_ID, &tool, "create", None, "seed")
                .unwrap();
        let def = serde_json::to_value(&applied.definition).unwrap();
        upsert_surface_from_tool(&mut db, &applied.definition, DEFAULT_WORKSPACE_ID, 1).unwrap();
        ensure_manifest_for_tool(&mut db, app_id, "Repair Bound", &surface_id).unwrap();
        // Persistent failing structural assertion — repair sanitize cannot satisfy it.
        upsert_test(
            &mut db,
            app_id,
            DeclarativeTest {
                test_id: "must-have-records".into(),
                name: "requires records".into(),
                test_type: "smoke".into(),
                actions: vec![],
                assertions: vec![TestAssertion {
                    assertion: "record_exists".into(),
                    expected: None,
                    target: Some("missing_model".into()),
                }],
                timeout_ms: 1000,
            },
        )
        .unwrap();

        let failed = VerificationResult {
            status: "failed".into(),
            message: "records missing".into(),
            test_results: vec![],
        };
        let result = run_bounded_repair(&mut db, app_id, &surface_id, &def, &failed).unwrap();
        assert!(
            result.rounds.len() as u32 <= MAX_REPAIR_ROUNDS,
            "repair must hard-cap rounds; got {} rounds (max {MAX_REPAIR_ROUNDS})",
            result.rounds.len()
        );
        assert!(
            !result.rounds.is_empty(),
            "failing verify must enter at least one repair round"
        );
        assert_ne!(
            result.status, "repaired",
            "failing record_exists must not be force-passed by repair; status={}",
            result.status
        );
        assert!(
            result.rounds.len() as u32 == MAX_REPAIR_ROUNDS
                || result
                    .rounds
                    .iter()
                    .any(|r| r.outcome == "privilege_escalation_blocked"),
            "exhausted path should consume max rounds unless privilege block; status={} rounds={}",
            result.status,
            result.rounds.len()
        );
    }

    #[test]
    fn privilege_escalation_outcome_is_blocked_not_applied() {
        // Guardrail contract: escalation detection must refuse apply (0 ops) with
        // privilege_escalation_blocked — never silently grant new permissions.
        assert!(definition_escalates_permissions(
            &json!({ "permissions": ["local_data.read"] }),
            &json!({ "permissions": ["local_data.read", "unrestricted.shell"] }),
        ));
        assert!(definition_escalates_permissions(
            &json!({ "applicationActionAccess": [] }),
            &json!({ "applicationActionAccess": ["local_data.write"] }),
        ));
    }
}
