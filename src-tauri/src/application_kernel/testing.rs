//! Declarative generated-application tests + post-change verification.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::{now_rfc3339, Database, DbError, DbResult};
use crate::runtime_v2::operations::AppOperation;
use rusqlite::{params, OptionalExtension};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeclarativeTest {
    pub test_id: String,
    pub name: String,
    #[serde(default)]
    pub test_type: String,
    #[serde(default)]
    pub actions: Vec<TestAction>,
    #[serde(default)]
    pub assertions: Vec<TestAssertion>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_timeout() -> u64 {
    5_000
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TestAction {
    pub action: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub value: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TestAssertion {
    pub assertion: String,
    #[serde(default)]
    pub expected: Option<Value>,
    #[serde(default)]
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationResult {
    pub status: String,
    pub message: String,
    pub test_results: Vec<Value>,
}

impl VerificationResult {
    /// True when post-change verify is acceptable for continue / mark-LKG / stop-repair.
    ///
    /// `verify_after_change` emits `verified` / `verified_with_warnings` / `failed`.
    /// `passed` is accepted for renderer-fact and provenance-aligned callers.
    pub fn is_acceptable(&self) -> bool {
        matches!(
            self.status.as_str(),
            "verified" | "verified_with_warnings" | "passed"
        )
    }
}

pub fn upsert_test(db: &mut Database, application_id: &str, test: DeclarativeTest) -> DbResult<()> {
    validate_test(&test).map_err(DbError::Invalid)?;
    let id = format!("gtest-{}", Uuid::new_v4());
    db.conn().execute(
        "INSERT INTO generated_tests (id, application_id, test_id, definition_json, enabled, created_at)
         VALUES (?1,?2,?3,?4,1,?5)
         ON CONFLICT(application_id, test_id) DO UPDATE SET definition_json = excluded.definition_json",
        params![
            id,
            application_id,
            test.test_id,
            serde_json::to_string(&test)?,
            now_rfc3339()
        ],
    )?;
    Ok(())
}

pub fn validate_test(test: &DeclarativeTest) -> Result<(), String> {
    if test.test_id.trim().is_empty() {
        return Err("testId required".into());
    }
    for a in &test.actions {
        match a.action.as_str() {
            "render_surface" | "find_component" | "click" | "enter_value" | "submit_form"
            | "navigate_route" | "create_record" => {}
            other => return Err(format!("untrusted test action: {other}")),
        }
    }
    for a in &test.assertions {
        match a.assertion.as_str() {
            "visible_text"
            | "state_equals"
            | "record_exists"
            | "route_equals"
            | "no_overflow"
            | "has_accessible_label"
            | "no_console_error"
            | "render_ok" => {}
            other => return Err(format!("untrusted assertion: {other}")),
        }
    }
    Ok(())
}

/// Trusted bounded runner — deterministic checks only (no browser automation).
pub fn run_test(db: &mut Database, application_id: &str, test_id: &str) -> DbResult<Value> {
    let def_json: String = db.conn().query_row(
        "SELECT definition_json FROM generated_tests WHERE application_id = ?1 AND test_id = ?2",
        params![application_id, test_id],
        |row| row.get(0),
    )?;
    let test: DeclarativeTest = serde_json::from_str(&def_json)?;
    let mut passed = true;
    let mut details = Vec::new();
    let mut unverified = Vec::new();
    for a in &test.assertions {
        let mut verified = true;
        let ok = match a.assertion.as_str() {
            // UI assertions require a renderer verification channel — do not lie.
            "render_ok" | "no_console_error" | "no_overflow" | "has_accessible_label" => {
                unverified.push(a.assertion.clone());
                verified = false;
                false
            }
            "record_exists" => {
                let model = a.target.as_deref().unwrap_or("");
                let count: i64 = db
                    .conn()
                    .query_row(
                        "SELECT COUNT(*) FROM generated_data_records
                         WHERE application_id = ?1 AND model_id = ?2",
                        params![application_id, model],
                        |row| row.get(0),
                    )
                    .unwrap_or(0);
                count > 0
            }
            "route_equals" => {
                let target_route = a
                    .expected
                    .as_ref()
                    .and_then(|v| v.as_str())
                    .or(a.target.as_deref())
                    .unwrap_or("");
                if target_route.is_empty() {
                    true
                } else if let Ok(m) = super::manifest::get_manifest(db, application_id) {
                    m.manifest
                        .routes
                        .iter()
                        .any(|r| r.route_id == target_route || r.title == target_route)
                } else {
                    false
                }
            }
            "visible_text" => {
                let expected_text = a
                    .expected
                    .as_ref()
                    .and_then(|v| v.as_str())
                    .or(a.target.as_deref())
                    .unwrap_or("");
                if expected_text.is_empty() {
                    true
                } else {
                    let mut found = false;
                    if let Ok(m) = super::manifest::get_manifest(db, application_id) {
                        found = m.manifest.name.contains(expected_text)
                            || m.manifest.description.contains(expected_text)
                            || m.manifest
                                .routes
                                .iter()
                                .any(|r| r.title.contains(expected_text));
                    }
                    if !found {
                        let tool_count: i64 = db
                            .conn()
                            .query_row(
                                "SELECT COUNT(*) FROM tools WHERE id = ?1 AND definition_json LIKE ?2",
                                params![application_id, format!("%{expected_text}%")],
                                |row| row.get(0),
                            )
                            .unwrap_or(0);
                        let surface_count: i64 = db
                            .conn()
                            .query_row(
                                "SELECT COUNT(*) FROM surfaces WHERE (id = ?1 OR tool_id = ?1) AND definition_json LIKE ?2",
                                params![application_id, format!("%{expected_text}%")],
                                |row| row.get(0),
                            )
                            .unwrap_or(0);
                        found = (tool_count + surface_count) > 0;
                    }
                    found
                }
            }
            "state_equals" => {
                let key = a.target.as_deref().unwrap_or("");
                if let Some(expected_val) = &a.expected {
                    let surface_state_str: Option<String> = db
                        .conn()
                        .query_row(
                            "SELECT state_json FROM surface_state WHERE surface_id = ?1",
                            params![application_id],
                            |row| row.get(0),
                        )
                        .optional()
                        .unwrap_or(None);
                    let tool_state_str: Option<String> = if surface_state_str.is_none() {
                        db.conn()
                            .query_row(
                                "SELECT state_json FROM tool_state WHERE tool_id = ?1",
                                params![application_id],
                                |row| row.get(0),
                            )
                            .optional()
                            .unwrap_or(None)
                    } else {
                        None
                    };

                    if let Some(raw) = surface_state_str.or(tool_state_str) {
                        if let Ok(val) = serde_json::from_str::<Value>(&raw) {
                            if !key.is_empty() {
                                val.get(key) == Some(expected_val)
                            } else {
                                val == *expected_val
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            _ => false,
        };
        if !ok {
            passed = false;
        }
        details.push(json!({
            "assertion": a.assertion,
            "ok": ok,
            "verified": verified,
        }));
    }
    // Distinguish structural failure from UI asserts that Rust cannot verify yet.
    let has_failed_verified = details.iter().any(|d| {
        d.get("verified") == Some(&json!(true)) && d.get("ok") == Some(&json!(false))
    });
    let status = if passed {
        "passed"
    } else if !unverified.is_empty() && !has_failed_verified {
        "not_verified"
    } else {
        "failed"
    };
    let run_id = format!("trun-{}", Uuid::new_v4());
    let result = json!({
        "status": status,
        "details": details,
        "unverifiedAssertions": unverified,
    });
    db.conn().execute(
        "INSERT INTO generated_test_runs (id, application_id, test_id, status, result_json, created_at)
         VALUES (?1,?2,?3,?4,?5,?6)",
        params![
            run_id,
            application_id,
            test_id,
            status,
            result.to_string(),
            now_rfc3339()
        ],
    )?;
    db.conn().execute(
        "UPDATE generated_tests SET last_result = ?3, last_run_at = ?4
         WHERE application_id = ?1 AND test_id = ?2",
        params![application_id, test_id, status, now_rfc3339()],
    )?;
    Ok(result)
}

/// Maximum enabled declarative tests executed per application during post-change verify.
const MAX_VERIFY_TESTS_PER_APP: usize = 32;

pub fn verify_after_change(
    db: &mut Database,
    operations: &[AppOperation],
) -> Result<VerificationResult, DbError> {
    let mut apps = std::collections::HashSet::new();
    for op in operations {
        if let Some(a) = op.target.application_id.as_ref() {
            apps.insert(a.clone());
        }
        if let Some(a) = op.payload.get("applicationId").and_then(|v| v.as_str()) {
            apps.insert(a.to_string());
        }
        // surface.create / tool payloads often identify the app as tool.id
        if let Some(tool) = op.payload.get("tool") {
            if let Some(id) = tool.get("id").and_then(|v| v.as_str()) {
                apps.insert(id.to_string());
            }
        }
    }
    let mut results = Vec::new();
    let mut failed = false;
    let mut unverified = false;
    for app in apps {
        if let Ok(m) = super::manifest::get_manifest(db, &app) {
            if let Err(e) = super::manifest::validate_manifest(&m.manifest) {
                failed = true;
                results.push(json!({ "applicationId": app, "error": e }));
            }
        }
        let mut stmt = db.conn().prepare(
            "SELECT test_id FROM generated_tests WHERE application_id = ?1 AND enabled = 1 ORDER BY test_id LIMIT ?2",
        )?;
        let ids: Vec<String> = stmt
            .query_map(params![app, MAX_VERIFY_TESTS_PER_APP as i64], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);
        for tid in ids {
            match run_test(db, &app, &tid) {
                Ok(run) => {
                    let status = run.get("status").and_then(|v| v.as_str()).unwrap_or("");
                    if status == "failed" {
                        failed = true;
                    } else if status == "not_verified" {
                        unverified = true;
                    }
                    results.push(json!({
                        "applicationId": app,
                        "testId": tid,
                        "run": run,
                    }));
                }
                Err(e) => {
                    failed = true;
                    results.push(json!({
                        "applicationId": app,
                        "testId": tid,
                        "error": e.to_string(),
                    }));
                }
            }
        }
    }
    let status = if failed {
        "failed"
    } else if unverified {
        "verified_with_warnings"
    } else {
        "verified"
    };
    Ok(VerificationResult {
        status: status.into(),
        message: "Post-change verification complete".into(),
        test_results: results,
    })
}

/// Visual checks — prefer frontend `verifySurfaceElement` for real DOM inspection.
/// This summary only encodes width-based heuristics for offline/agent context.
pub fn visual_checks_summary(width: u32) -> Value {
    json!({
        "width": width,
        "implemented": true,
        "engine": "heuristic",
        "note": "Full overflow/label/touch checks run in the Coreside UI via verifySurfaceElement",
        "checks": [
            { "id": "no_horizontal_overflow", "status": "delegate_to_ui" },
            { "id": "touch_targets", "status": if width < 400 { "warn" } else { "pass" } },
            { "id": "labels_present", "status": "delegate_to_ui" },
            { "id": "empty_chart_state", "status": "delegate_to_ui" }
        ]
    })
}

/// Bounded renderer verification vocabulary — no arbitrary JS/selectors/eval.
pub const RENDERER_ASSERTIONS: &[&str] = &[
    "render_ok",
    "visible_text",
    "element_exists",
    "element_visible",
    "value_equals",
    "state_equals",
    "record_exists",
    "route_equals",
    "no_overflow",
    "has_accessible_label",
    "focus_is",
    "scroll_position",
    "count",
    "no_console_error",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RendererVerifyRequest {
    pub application_id: String,
    pub surface_id: String,
    pub mount_instance_id: String,
    pub assertion: String,
    #[serde(default)]
    pub expected: Option<Value>,
    #[serde(default)]
    pub target: Option<String>,
    /// Optional conversation scope — must match mount when provided.
    #[serde(default)]
    pub conversation_id: Option<String>,
}

/// Trusted facts reported by the live renderer after Rust has validated the mount.
/// The renderer may only answer with this closed vocabulary — never execute
/// generated JS or accept arbitrary selectors from the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RendererVerifyFact {
    pub assertion: String,
    pub ok: bool,
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RendererVerifyResult {
    pub status: String,
    pub verified: bool,
    pub message: String,
    #[serde(default)]
    pub fact: Option<RendererVerifyFact>,
    /// Assertion bound at authorize time — accept must match (confused-deputy guard).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorized_assertion: Option<String>,
}

/// Authorize a renderer verification request against the mount registry.
///
/// Returns `not_verified` when no fresh mount exists — never fabricates a pass.
pub fn authorize_renderer_verify(
    db: &Database,
    mounts: &crate::runtime_v2::mount_registry::MountRegistry,
    req: &RendererVerifyRequest,
) -> Result<RendererVerifyResult, String> {
    if !RENDERER_ASSERTIONS.contains(&req.assertion.as_str()) {
        return Err(format!("untrusted renderer assertion: {}", req.assertion));
    }
    crate::security::assert_not_protected(&req.application_id)?;
    crate::security::assert_not_protected(&req.surface_id)?;

    let surface = crate::runtime_v2::surfaces::get_surface(db, &req.surface_id)
        .map_err(|e| e.to_string())?;
    let app_matches = surface.tool_id.as_deref() == Some(req.application_id.as_str())
        || surface.id == req.application_id;
    if !app_matches {
        return Err("application/surface identity mismatch".into());
    }
    if let Some(wanted) = req.conversation_id.as_ref() {
        match surface.conversation_id.as_ref() {
            Some(actual) if actual == wanted => {}
            Some(_) => return Err("conversation scope mismatch".into()),
            // Fail closed: conversation-scoped verify requires a bound surface conversation.
            None => return Err("conversation scope mismatch".into()),
        }
    }

    let readiness = mounts.evaluate_renderer_readiness(&req.surface_id, None, None);
    match readiness {
        crate::runtime_v2::mount_registry::RendererReadiness::Ready { .. } => {
            if !mounts.has_fresh_instance(&req.surface_id, &req.mount_instance_id) {
                return Ok(RendererVerifyResult {
                    status: "not_verified".into(),
                    verified: false,
                    message: "mount instance not registered for surface".into(),
                    fact: None,
                    authorized_assertion: None,
                });
            }
            Ok(RendererVerifyResult {
                status: "authorized".into(),
                verified: false,
                message: "mount authorized; await renderer fact".into(),
                fact: None,
                authorized_assertion: Some(req.assertion.clone()),
            })
        }
        crate::runtime_v2::mount_registry::RendererReadiness::NotMounted => {
            Ok(RendererVerifyResult {
                status: "not_verified".into(),
                verified: false,
                message: "no live renderer mount".into(),
                fact: None,
                authorized_assertion: None,
            })
        }
        crate::runtime_v2::mount_registry::RendererReadiness::Stale { reason } => {
            Ok(RendererVerifyResult {
                status: "not_verified".into(),
                verified: false,
                message: format!("stale renderer mount: {reason}"),
                fact: None,
                authorized_assertion: None,
            })
        }
    }
}

/// Accept a renderer fact only after authorize_renderer_verify returned authorized.
pub fn accept_renderer_fact(
    authorized: &RendererVerifyResult,
    fact: RendererVerifyFact,
) -> Result<RendererVerifyResult, String> {
    if authorized.status != "authorized" {
        return Err("cannot accept renderer fact without prior authorization".into());
    }
    let Some(expected) = authorized.authorized_assertion.as_deref() else {
        return Err("authorized result missing bound assertion".into());
    };
    if fact.assertion != expected {
        return Err(format!(
            "renderer fact assertion '{}' does not match authorized '{}'",
            fact.assertion, expected
        ));
    }
    if !RENDERER_ASSERTIONS.contains(&fact.assertion.as_str()) {
        return Err(format!("untrusted renderer fact assertion: {}", fact.assertion));
    }
    Ok(RendererVerifyResult {
        status: if fact.ok { "passed".into() } else { "failed".into() },
        verified: true,
        message: fact
            .detail
            .clone()
            .unwrap_or_else(|| "renderer fact accepted".into()),
        fact: Some(fact),
        authorized_assertion: Some(expected.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_shell_action() {
        let t = DeclarativeTest {
            test_id: "t1".into(),
            name: "bad".into(),
            test_type: "interaction".into(),
            actions: vec![TestAction {
                action: "exec_shell".into(),
                target: String::new(),
                value: None,
            }],
            assertions: vec![],
            timeout_ms: 1000,
        };
        assert!(validate_test(&t).is_err());
    }

    #[test]
    fn accepts_valid_declarative_test() {
        let t = DeclarativeTest {
            test_id: "t-valid".into(),
            name: "valid-suite".into(),
            test_type: "interaction".into(),
            actions: vec![
                TestAction {
                    action: "render_surface".into(),
                    target: "surf-1".into(),
                    value: None,
                },
                TestAction {
                    action: "click".into(),
                    target: "btn-save".into(),
                    value: None,
                },
            ],
            assertions: vec![
                TestAssertion {
                    assertion: "render_ok".into(),
                    expected: None,
                    target: None,
                },
                TestAssertion {
                    assertion: "visible_text".into(),
                    expected: Some(json!("My Tool")),
                    target: None,
                },
                TestAssertion {
                    assertion: "state_equals".into(),
                    expected: Some(json!("active")),
                    target: Some("status".into()),
                },
                TestAssertion {
                    assertion: "route_equals".into(),
                    expected: Some(json!("/dashboard")),
                    target: None,
                },
            ],
            timeout_ms: 2000,
        };
        assert!(validate_test(&t).is_ok());
    }

    #[test]
    fn runs_declarative_assertions_against_db() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("test-decl.db")).unwrap();

        let app_id = "app-test-1";

        // Insert workspace, tools and surface state
        db.conn()
            .execute(
                "INSERT INTO workspaces (id, name) VALUES ('ws-1', 'Default Workspace')",
                [],
            )
            .unwrap();

        db.conn()
            .execute(
                "INSERT INTO tools (id, workspace_id, name, description, layout, definition_json, current_version, created_at, updated_at)
                 VALUES (?1, 'ws-1', 'Personal Finance Tool', '', 'dashboard', '{\"components\":[{\"id\":\"c1\",\"type\":\"text\",\"props\":{\"text\":\"Finance Overview\"}}]}', 1, datetime('now'), datetime('now'))",
                params![app_id],
            )
            .unwrap();

        db.conn()
            .execute(
                "INSERT INTO tool_state (tool_id, state_json, updated_at)
                 VALUES (?1, '{\"status\":\"active\",\"count\":42}', datetime('now'))",
                params![app_id],
            )
            .unwrap();

        let test = DeclarativeTest {
            test_id: "decl-1".into(),
            name: "finance-check".into(),
            test_type: "smoke".into(),
            actions: vec![],
            assertions: vec![
                TestAssertion {
                    assertion: "render_ok".into(),
                    expected: None,
                    target: None,
                },
                TestAssertion {
                    assertion: "visible_text".into(),
                    expected: Some(json!("Finance Overview")),
                    target: None,
                },
                TestAssertion {
                    assertion: "state_equals".into(),
                    expected: Some(json!("active")),
                    target: Some("status".into()),
                },
                TestAssertion {
                    assertion: "state_equals".into(),
                    expected: Some(json!(42)),
                    target: Some("count".into()),
                },
            ],
            timeout_ms: 1000,
        };

        upsert_test(&mut db, app_id, test).unwrap();
        let result = run_test(&mut db, app_id, "decl-1").unwrap();

        // Structural asserts pass; render_ok is not verified without a renderer channel.
        assert_eq!(result["status"], "not_verified");
        let details = result["details"].as_array().unwrap();
        assert_eq!(details.len(), 4);
        assert_eq!(details[0]["assertion"], "render_ok");
        assert_eq!(details[0]["ok"], false);
        assert_eq!(details[0]["verified"], false);
        assert_eq!(details[1]["ok"], true);
        assert_eq!(details[2]["ok"], true);
        assert_eq!(details[3]["ok"], true);
        assert!(result["unverifiedAssertions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "render_ok"));
    }

    #[test]
    fn structural_only_test_can_pass_without_ui_placeholders() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("test-struct.db")).unwrap();
        let app_id = "app-struct-1";
        db.conn()
            .execute(
                "INSERT INTO workspaces (id, name) VALUES ('ws-1', 'Default Workspace')",
                [],
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO tools (id, workspace_id, name, description, layout, definition_json, current_version, created_at, updated_at)
                 VALUES (?1, 'ws-1', 'Notes', '', 'single-column', '{\"components\":[]}', 1, datetime('now'), datetime('now'))",
                params![app_id],
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO tool_state (tool_id, state_json, updated_at)
                 VALUES (?1, '{\"ready\":true}', datetime('now'))",
                params![app_id],
            )
            .unwrap();
        upsert_test(
            &mut db,
            app_id,
            DeclarativeTest {
                test_id: "struct-1".into(),
                name: "structural".into(),
                test_type: "smoke".into(),
                actions: vec![],
                assertions: vec![TestAssertion {
                    assertion: "state_equals".into(),
                    expected: Some(json!(true)),
                    target: Some("ready".into()),
                }],
                timeout_ms: 1000,
            },
        )
        .unwrap();
        let result = run_test(&mut db, app_id, "struct-1").unwrap();
        assert_eq!(result["status"], "passed");
    }

    #[test]
    fn structural_failure_reports_failed_even_when_ui_assertions_unverified() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("test-failed.db")).unwrap();
        let app_id = "app-failed-1";
        db.conn()
            .execute(
                "INSERT INTO workspaces (id, name) VALUES ('ws-1', 'Default Workspace')",
                [],
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO tools (id, workspace_id, name, description, layout, definition_json, current_version, created_at, updated_at)
                 VALUES (?1, 'ws-1', 'Notes', '', 'single-column', '{\"components\":[]}', 1, datetime('now'), datetime('now'))",
                params![app_id],
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO tool_state (tool_id, state_json, updated_at)
                 VALUES (?1, '{\"ready\":false}', datetime('now'))",
                params![app_id],
            )
            .unwrap();
        upsert_test(
            &mut db,
            app_id,
            DeclarativeTest {
                test_id: "mixed-1".into(),
                name: "mixed".into(),
                test_type: "smoke".into(),
                actions: vec![],
                assertions: vec![
                    TestAssertion {
                        assertion: "state_equals".into(),
                        expected: Some(json!(true)),
                        target: Some("ready".into()),
                    },
                    TestAssertion {
                        assertion: "render_ok".into(),
                        expected: None,
                        target: None,
                    },
                    TestAssertion {
                        assertion: "no_overflow".into(),
                        expected: None,
                        target: None,
                    },
                ],
                timeout_ms: 1000,
            },
        )
        .unwrap();
        let result = run_test(&mut db, app_id, "mixed-1").unwrap();
        // Must not mask a failed structural check as merely not_verified.
        assert_eq!(result["status"], "failed");
        let details = result["details"].as_array().unwrap();
        assert_eq!(details[0]["assertion"], "state_equals");
        assert_eq!(details[0]["ok"], false);
        assert_eq!(details[0]["verified"], true);
        assert_eq!(details[1]["verified"], false);
        assert_eq!(details[2]["verified"], false);
        assert!(result["unverifiedAssertions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "render_ok"));
    }

    #[test]
    fn verify_after_change_runs_enabled_tests_not_just_queues() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("test-verify.db")).unwrap();
        let app_id = "app-verify-1";
        db.conn()
            .execute(
                "INSERT INTO workspaces (id, name) VALUES ('ws-1', 'Default Workspace')",
                [],
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO tools (id, workspace_id, name, description, layout, definition_json, current_version, created_at, updated_at)
                 VALUES (?1, 'ws-1', 'Verify App', '', 'single-column', '{\"components\":[]}', 1, datetime('now'), datetime('now'))",
                params![app_id],
            )
            .unwrap();
        crate::application_kernel::manifest::ensure_manifest_for_tool(
            &mut db,
            app_id,
            "Verify App",
            &format!("surf-{app_id}"),
        )
        .unwrap();
        upsert_test(
            &mut db,
            app_id,
            DeclarativeTest {
                test_id: "v1".into(),
                name: "ui-placeholder".into(),
                test_type: "smoke".into(),
                actions: vec![],
                assertions: vec![TestAssertion {
                    assertion: "render_ok".into(),
                    expected: None,
                    target: None,
                }],
                timeout_ms: 1000,
            },
        )
        .unwrap();
        let ops: Vec<crate::runtime_v2::operations::AppOperation> = serde_json::from_value(json!([{
            "id": "op-1",
            "type": "surface.create",
            "target": { "applicationId": app_id },
            "payload": { "applicationId": app_id }
        }]))
        .unwrap();
        let v = verify_after_change(&mut db, &ops).unwrap();
        assert_eq!(
            v.status, "verified_with_warnings",
            "UI-only assertions must not mint verified / LKG"
        );
        assert!(
            v.test_results.iter().any(|r| r.get("run").is_some()),
            "must execute tests, not only queue: {:?}",
            v.test_results
        );
    }

    #[test]
    fn authorize_renderer_verify_without_mount_is_not_verified() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("rend-auth.db")).unwrap();
        let conv = crate::db::create_conversation(
            &mut db,
            crate::db::DEFAULT_WORKSPACE_ID,
            "Rend Conv",
            None,
        )
        .unwrap();
        let surf = crate::runtime_v2::surfaces::create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Rend Surface",
            &json!({
                "id": "doc-rend",
                "name": "Rend Surface",
                "layout": "stack",
                "components": [{"id": "t", "type": "text", "props": {"text": "hi"}}]
            }),
            &[],
        )
        .unwrap();
        let mounts = crate::runtime_v2::mount_registry::MountRegistry::new();
        let req = RendererVerifyRequest {
            application_id: surf.id.clone(),
            surface_id: surf.id.clone(),
            mount_instance_id: "missing-instance".into(),
            assertion: "render_ok".into(),
            expected: None,
            target: None,
            conversation_id: None,
        };
        let result = authorize_renderer_verify(&db, &mounts, &req).unwrap();
        assert_eq!(result.status, "not_verified");
        assert!(!result.verified);
        assert!(result.message.contains("no live renderer mount"));
    }

    #[test]
    fn authorize_renderer_verify_rejects_unknown_assertion() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("rend-unknown.db")).unwrap();
        let conv = crate::db::create_conversation(
            &mut db,
            crate::db::DEFAULT_WORKSPACE_ID,
            "Rend Conv",
            None,
        )
        .unwrap();
        let surf = crate::runtime_v2::surfaces::create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Rend Surface",
            &json!({
                "id": "doc-rend",
                "name": "Rend Surface",
                "layout": "stack",
                "components": [{"id": "t", "type": "text", "props": {"text": "hi"}}]
            }),
            &[],
        )
        .unwrap();
        let mounts = crate::runtime_v2::mount_registry::MountRegistry::new();
        let req = RendererVerifyRequest {
            application_id: surf.id.clone(),
            surface_id: surf.id.clone(),
            mount_instance_id: "any".into(),
            assertion: "eval_javascript".into(),
            expected: None,
            target: None,
            conversation_id: None,
        };
        let err = authorize_renderer_verify(&db, &mounts, &req).unwrap_err();
        assert!(
            err.contains("untrusted renderer assertion"),
            "unexpected: {err}"
        );
    }

    #[test]
    fn accept_renderer_fact_without_authorize_fails() {
        let unauthorized = RendererVerifyResult {
            status: "not_verified".into(),
            verified: false,
            message: "no live renderer mount".into(),
            fact: None,
            authorized_assertion: None,
        };
        let err = accept_renderer_fact(
            &unauthorized,
            RendererVerifyFact {
                assertion: "render_ok".into(),
                ok: true,
                detail: None,
            },
        )
        .unwrap_err();
        assert!(err.contains("without prior authorization"));
    }

    #[test]
    fn accept_renderer_fact_rejects_unknown_assertion() {
        let authorized = RendererVerifyResult {
            status: "authorized".into(),
            verified: false,
            message: "mount authorized; await renderer fact".into(),
            fact: None,
            authorized_assertion: Some("run_arbitrary_script".into()),
        };
        let err = accept_renderer_fact(
            &authorized,
            RendererVerifyFact {
                assertion: "run_arbitrary_script".into(),
                ok: true,
                detail: None,
            },
        )
        .unwrap_err();
        assert!(err.contains("untrusted renderer fact assertion"));
    }

    #[test]
    fn accept_renderer_fact_rejects_assertion_mismatch() {
        let authorized = RendererVerifyResult {
            status: "authorized".into(),
            verified: false,
            message: "mount authorized; await renderer fact".into(),
            fact: None,
            authorized_assertion: Some("render_ok".into()),
        };
        let err = accept_renderer_fact(
            &authorized,
            RendererVerifyFact {
                assertion: "visible_text".into(),
                ok: true,
                detail: None,
            },
        )
        .unwrap_err();
        assert!(
            err.contains("does not match authorized"),
            "unexpected: {err}"
        );
    }

    #[test]
    fn accept_renderer_fact_after_authorize_passes() {
        let authorized = RendererVerifyResult {
            status: "authorized".into(),
            verified: false,
            message: "mount authorized; await renderer fact".into(),
            fact: None,
            authorized_assertion: Some("render_ok".into()),
        };
        let accepted = accept_renderer_fact(
            &authorized,
            RendererVerifyFact {
                assertion: "render_ok".into(),
                ok: true,
                detail: Some("ok".into()),
            },
        )
        .unwrap();
        assert_eq!(accepted.status, "passed");
        assert!(accepted.verified);
    }
}
