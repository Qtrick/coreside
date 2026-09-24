//! Vendo security authority invariants A through L — verified implementation tests.
//!
//! Conceptually reimplemented in Coreside's Rust application kernel.
//! Every invariant is tested against actual failure and adversarial attacks:
//! - Invariant A: Single choke point at gateway (gateway.rs is sole execution path)
//! - Invariant B: Action descriptor hashing drift / tampering rejection
//! - Invariant C: Approval exact-once CAS consumption (cannot be replayed)
//! - Invariant D: Frozen approval input immutability
//! - Invariant E: Unsuppressible confirmation for destructive / critical actions
//! - Invariant F: Grant scope enforcement (Action vs Exact vs ApplicationAction)
//! - Invariant G: Grant duration enforcement (Once vs Session vs Standing)
//! - Invariant H: Presence and venue boundary (Present vs Away; Automation standing requirements)
//! - Invariant I: Cross-application boundary isolation (App A grant cannot authorize App B)
//! - Invariant J: Breakers fail-closed on repeated failures
//! - Invariant K: Output size enforcement (output_too_large fail-closed)
//! - Invariant L: Transactional audit logging for every decision, grant, and execution

use serde_json::json;

use super::approvals::{self, decide, get_approval, RememberChoice};
use super::context::{Presence, Venue};
use super::descriptor::{find_action, ActionRisk};
use super::gateway::{execute_registered_action, ActionOutcome};
use super::grants::{mint_grant, revoke_grant, GrantDuration, GrantScope};
use super::testing::{app_ctx, insert_audit_row, seed_application, test_db};
use crate::application_kernel::manifest::{get_manifest, upsert_manifest};

const APP_A: &str = "test-app-a";
const APP_B: &str = "test-app-b";

fn outcome_code(outcome: &ActionOutcome) -> String {
    match outcome {
        ActionOutcome::Ok { .. } => "ok".into(),
        ActionOutcome::Error { code, .. } => format!("error:{code}"),
        ActionOutcome::PendingApproval { reason, .. } => format!("pending:{reason}"),
        ActionOutcome::Blocked { code, .. } => format!("blocked:{code}"),
    }
}

// INVARIANT A: Single choke point at gateway
#[test]
fn invariant_a_single_choke_point_enforces_recovery_and_permissions() {
    let (mut db, _dir) = test_db();
    seed_application(&mut db, APP_A, &["local_data.query"]);

    // Normal execution succeeds through gateway
    let ctx = app_ctx(APP_A);
    let ok_outcome = execute_registered_action(
        &mut db,
        &ctx,
        "local_data.query",
        &json!({ "modelId": "note" }),
        None,
    );
    assert_eq!(outcome_code(&ok_outcome), "ok");

    // Undeclared action blocked at choke point
    let unperm = execute_registered_action(
        &mut db,
        &ctx,
        "web_search.request",
        &json!({ "query": "test" }),
        None,
    );
    assert_eq!(outcome_code(&unperm), "blocked:policy_blocked");

    // Recovery mode blocks all execution through gateway
    crate::application_kernel::recovery::set_recovery_mode(&mut db, true)
        .unwrap();
    let blocked_recovery = execute_registered_action(
        &mut db,
        &ctx,
        "local_data.query",
        &json!({ "modelId": "note" }),
        None,
    );
    assert_eq!(outcome_code(&blocked_recovery), "blocked:recovery_required");
}

// INVARIANT B: Action descriptor hashing drift / tampering rejection
#[test]
fn invariant_b_descriptor_hashing_drift_detected_and_rejected() {
    let (mut db, _dir) = test_db();
    seed_application(&mut db, APP_A, &["local_data.query"]);

    let mut record = get_manifest(&db, APP_A).unwrap().manifest;
    record
        .action_descriptor_hashes
        .insert("local_data.query".into(), "forged-or-drifted-hash".into());
    upsert_manifest(&mut db, record).unwrap();

    let ctx = app_ctx(APP_A);
    let outcome = execute_registered_action(
        &mut db,
        &ctx,
        "local_data.query",
        &json!({ "modelId": "note" }),
        None,
    );
    assert_eq!(outcome_code(&outcome), "blocked:descriptor_tampered");
}

// INVARIANT C: Approval single-use CAS consumption
#[test]
fn invariant_c_approval_single_use_cas_consumption() {
    let (mut db, _dir) = test_db();
    seed_application(&mut db, APP_A, &["local_data.delete"]);

    let ctx = app_ctx(APP_A);
    let d = find_action("local_data.delete").unwrap();
    let pending = approvals::create_pending(
        &mut db,
        &ctx,
        d,
        &json!({ "modelId": "note", "recordId": "r-cas" }),
        None,
    )
    .unwrap();

    // User decides to approve
    decide(&mut db, &pending.id, true, None, "user").unwrap();

    // First consume via gateway succeeds
    let first = execute_registered_action(
        &mut db,
        &ctx,
        "local_data.delete",
        &json!({ "modelId": "note", "recordId": "r-cas" }),
        Some(&pending.id),
    );
    assert!(first.is_ok(), "first execution must succeed");

    // Second consume attempt with same approval ID fails closed
    let second = execute_registered_action(
        &mut db,
        &ctx,
        "local_data.delete",
        &json!({ "modelId": "note", "recordId": "r-cas" }),
        Some(&pending.id),
    );
    assert_eq!(outcome_code(&second), "blocked:approval_invalid");
}

// INVARIANT D: Frozen approval input immutability
#[test]
fn invariant_d_frozen_approval_input_immutability() {
    let (mut db, _dir) = test_db();
    seed_application(&mut db, APP_A, &["local_data.delete"]);

    let ctx = app_ctx(APP_A);
    let d = find_action("local_data.delete").unwrap();
    let original_input = json!({ "modelId": "note", "recordId": "r-target" });
    let pending = approvals::create_pending(&mut db, &ctx, d, &original_input, None).unwrap();

    // Frozen input stored in SQLite matches
    let frozen = approvals::frozen_input(&db, &pending.id).unwrap();
    assert_eq!(frozen, original_input);

    decide(&mut db, &pending.id, true, None, "user").unwrap();

    // Adversary attempts to consume approval with different input payload
    let tampered_input = json!({ "modelId": "note", "recordId": "r-ATTACK" });
    let tampered = execute_registered_action(
        &mut db,
        &ctx,
        "local_data.delete",
        &tampered_input,
        Some(&pending.id),
    );
    assert_eq!(
        outcome_code(&tampered),
        "blocked:approval_invalid",
        "Approval call_hash mismatch must block tampered input"
    );
}

// INVARIANT E: Unsuppressible confirmation for destructive / critical actions
#[test]
fn invariant_e_destructive_actions_cannot_be_remembered() {
    let (mut db, _dir) = test_db();
    seed_application(&mut db, APP_A, &["local_data.delete"]);

    let ctx = app_ctx(APP_A);
    let d = find_action("local_data.delete").unwrap();
    assert_eq!(d.risk, ActionRisk::Destructive);

    let pending = approvals::create_pending(
        &mut db,
        &ctx,
        d,
        &json!({ "modelId": "note", "recordId": "r-del" }),
        None,
    )
    .unwrap();

    // Attempting to remember destructive action must be rejected
    let err = decide(
        &mut db,
        &pending.id,
        true,
        Some(RememberChoice {
            scope: GrantScope::Action,
            duration: GrantDuration::Session,
        }),
        "user",
    );
    assert!(err.is_err(), "destructive action cannot be remembered");
    assert_eq!(get_approval(&db, &pending.id).unwrap().status, "pending");
}

// INVARIANT F: Grant scoping enforcement
#[test]
fn invariant_f_grant_scoping_exact_vs_action() {
    let (mut db, _dir) = test_db();
    seed_application(&mut db, APP_A, &["local_data.write"]);

    let ctx = app_ctx(APP_A);
    let d = find_action("local_data.write").unwrap();

    // Mint exact grant for specific input call_hash
    let input1 = json!({ "modelId": "note", "data": { "title": "exact1" } });
    let input2 = json!({ "modelId": "note", "data": { "title": "exact2" } });
    let hash1 = approvals::call_hash(&ctx, d, &input1);

    let _g = mint_grant(
        &mut db,
        &ctx,
        d,
        GrantScope::Exact,
        GrantDuration::Session,
        Some(&hash1),
        "user",
    )
    .unwrap();

    // input1 matches exact grant
    let out1 = execute_registered_action(&mut db, &ctx, "local_data.write", &input1, None);
    assert_eq!(outcome_code(&out1), "ok");

    // input2 does NOT match exact grant -> requires approval
    let out2 = execute_registered_action(&mut db, &ctx, "local_data.write", &input2, None);
    assert!(
        outcome_code(&out2).starts_with("pending:"),
        "Different input must require new approval under Exact grant scope"
    );
}

// INVARIANT G: Grant duration enforcement (Once vs Session)
#[test]
fn invariant_g_grant_duration_once_consumed_on_first_use() {
    let (mut db, _dir) = test_db();
    seed_application(&mut db, APP_A, &["local_data.write"]);

    let ctx = app_ctx(APP_A);
    let d = find_action("local_data.write").unwrap();
    let _g = mint_grant(
        &mut db,
        &ctx,
        d,
        GrantScope::ApplicationAction,
        GrantDuration::Once,
        None,
        "user",
    )
    .unwrap();

    let input = json!({ "modelId": "note", "data": { "title": "once" } });
    let out1 = execute_registered_action(&mut db, &ctx, "local_data.write", &input, None);
    assert_eq!(outcome_code(&out1), "ok");

    // Second execution must not reuse the Once grant
    let out2 = execute_registered_action(&mut db, &ctx, "local_data.write", &input, None);
    assert!(
        outcome_code(&out2).starts_with("pending:"),
        "Once grant must be consumed after first use"
    );

    // Session grant can be revoked explicitly
    let sess_grant = mint_grant(
        &mut db,
        &ctx,
        d,
        GrantScope::ApplicationAction,
        GrantDuration::Session,
        None,
        "user",
    )
    .unwrap();
    let out3 = execute_registered_action(&mut db, &ctx, "local_data.write", &input, None);
    assert_eq!(outcome_code(&out3), "ok");

    revoke_grant(&mut db, &sess_grant.id).unwrap();
    let out4 = execute_registered_action(&mut db, &ctx, "local_data.write", &input, None);
    assert!(
        outcome_code(&out4).starts_with("pending:"),
        "Revoked grant must not authorize subsequent executions"
    );
}

// INVARIANT H: Presence and venue boundary
#[test]
fn invariant_h_away_automation_boundary() {
    let (mut db, _dir) = test_db();
    seed_application(&mut db, APP_A, &["local_data.write"]);

    // Away presence without standing grant must block
    let mut away_ctx = app_ctx(APP_A);
    away_ctx.presence = Presence::Away;
    away_ctx.venue = Venue::Automation;

    let input = json!({ "modelId": "note", "data": { "title": "auto" } });
    let out = execute_registered_action(&mut db, &away_ctx, "local_data.write", &input, None);
    assert!(
        outcome_code(&out).starts_with("pending:"),
        "Away automation execution without standing grant must be queued as pending approval"
    );

    // With standing grant, away automation succeeds
    let d = find_action("local_data.write").unwrap();
    let _g = mint_grant(
        &mut db,
        &away_ctx,
        d,
        GrantScope::ApplicationAction,
        GrantDuration::Standing,
        None,
        "user",
    )
    .unwrap();

    let out_with_grant =
        execute_registered_action(&mut db, &away_ctx, "local_data.write", &input, None);
    assert_eq!(outcome_code(&out_with_grant), "ok");
}

// INVARIANT I: Cross-application boundary isolation
#[test]
fn invariant_i_cross_application_isolation() {
    let (mut db, _dir) = test_db();
    seed_application(&mut db, APP_A, &["local_data.write"]);
    seed_application(&mut db, APP_B, &["local_data.write"]);

    let ctx_a = app_ctx(APP_A);
    let ctx_b = app_ctx(APP_B);
    let d = find_action("local_data.write").unwrap();

    // Mint grant for APP_A
    let _g = mint_grant(
        &mut db,
        &ctx_a,
        d,
        GrantScope::ApplicationAction,
        GrantDuration::Session,
        None,
        "user",
    )
    .unwrap();

    let input = json!({ "modelId": "note", "data": { "title": "app-test" } });

    // APP_A executes with grant
    let out_a = execute_registered_action(&mut db, &ctx_a, "local_data.write", &input, None);
    assert_eq!(outcome_code(&out_a), "ok");

    // APP_B cannot use APP_A's grant
    let out_b = execute_registered_action(&mut db, &ctx_b, "local_data.write", &input, None);
    assert!(
        outcome_code(&out_b).starts_with("pending:"),
        "Grant for App A must never authorize App B"
    );
}

// INVARIANT J: Breakers fail-closed on repeated failures
#[test]
fn invariant_j_breakers_trip_after_consecutive_failures() {
    let (mut db, _dir) = test_db();
    seed_application(&mut db, APP_A, &["local_data.query"]);

    let ctx = app_ctx(APP_A);
    // Insert 5 consecutive audit error rows to trip breaker
    for _ in 0..5 {
        insert_audit_row(
            &mut db,
            APP_A,
            "local_data.query",
            "error",
            "read",
            &ctx.run_id,
        );
    }

    let out = execute_registered_action(
        &mut db,
        &ctx,
        "local_data.query",
        &json!({ "modelId": "note" }),
        None,
    );
    assert_eq!(
        outcome_code(&out),
        "blocked:action_suspended",
        "Repeated failures must trip breaker and fail closed"
    );
}

// INVARIANT K: Output size bounding
#[test]
fn invariant_k_output_size_enforcement() {
    let (mut db, _dir) = test_db();
    seed_application(&mut db, APP_A, &["local_data.query", "local_data.write"]);

    // Populate large dataset in app data
    let filler = "x".repeat(2048);
    for _ in 0..64 {
        crate::application_kernel::data::create_record(
            &mut db,
            APP_A,
            "note",
            json!({ "title": filler }),
        )
        .unwrap();
    }

    let ctx = app_ctx(APP_A);
    let outcome = execute_registered_action(
        &mut db,
        &ctx,
        "local_data.query",
        &json!({ "modelId": "note", "limit": 500 }),
        None,
    );
    assert_eq!(
        outcome_code(&outcome),
        "blocked:output_too_large",
        "Oversized output must fail closed"
    );
}

// INVARIANT L: Transactional audit logging
#[test]
fn invariant_l_transactional_audit_logging() {
    let (mut db, _dir) = test_db();
    seed_application(&mut db, APP_A, &["local_data.query"]);

    let ctx = app_ctx(APP_A);
    execute_registered_action(
        &mut db,
        &ctx,
        "local_data.query",
        &json!({ "modelId": "note" }),
        None,
    );

    let audit_count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM runtime_audit_events WHERE application_id = ?1 AND action_name = ?2",
            rusqlite::params![APP_A, "local_data.query"],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        audit_count, 1,
        "Every registered action execution must write an audit record"
    );
}
