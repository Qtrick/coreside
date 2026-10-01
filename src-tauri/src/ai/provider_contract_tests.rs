//! Deterministic provider-contract tests for ApplicationPlan.
//!
//! These prove: recorded/provider-shaped JSON → parse → validate → compile → ops
//! without calling a live network provider. Live provider smoke is separate.

use crate::ai::parse_agent_response;
use crate::ai::plan_fixtures::{
    plan_response_json, task_tracker_add_due_dates_plan, task_tracker_create_plan,
};
use crate::application_kernel::application_plan::{compile_plan, ApplicationPlanKind};

#[test]
fn recorded_create_plan_roundtrips_through_parser() {
    let raw = plan_response_json(
        "I generated a Task Tracker.",
        &task_tracker_create_plan(),
    );
    let parsed = parse_agent_response(&raw).expect("parse recorded create plan");
    let plan = parsed
        .payload
        .application_plan
        .as_ref()
        .expect("applicationPlan present");
    assert_eq!(plan.kind, ApplicationPlanKind::Create);
    let ops = parsed
        .payload
        .normalized_operations()
        .expect("compile via normalized_operations");
    assert!(ops.iter().any(|o| o.op_type == "data.model_upsert"));
    assert!(ops.iter().any(|o| o.op_type == "surface.create"));
    assert!(parsed.payload.validate().is_ok());
}

#[test]
fn recorded_evolve_plan_roundtrips_through_parser() {
    let raw = plan_response_json(
        "Added due dates.",
        &task_tracker_add_due_dates_plan(Some(2)),
    );
    let parsed = parse_agent_response(&raw).expect("parse recorded evolve plan");
    let plan = parsed
        .payload
        .application_plan
        .as_ref()
        .expect("applicationPlan present");
    assert_eq!(plan.kind, ApplicationPlanKind::Evolve);
    assert_eq!(plan.base_revision, Some(2));
    let validated = compile_plan(plan).expect("compile evolve plan");
    assert!(validated
        .compiled
        .operations
        .iter()
        .any(|o| o.payload.get("action") == Some(&serde_json::json!("update"))));
}

#[test]
fn malicious_plan_with_iframe_fails_closed() {
    let mut plan = task_tracker_create_plan();
    if let crate::application_kernel::compiler::ChangeIntent::CreateSurface { tool, .. } =
        &mut plan.intents[0]
    {
        tool.components[0].component_type = "iframe".into();
    }
    let raw = plan_response_json("evil", &plan);
    let parsed = parse_agent_response(&raw);
    // Either parse-time validate fails, or normalized_operations / validate fails.
    match parsed {
        Ok(p) => {
            assert!(
                p.payload.validate().is_err() || p.payload.normalized_operations().is_err(),
                "malicious iframe plan must fail closed"
            );
        }
        Err(_) => {}
    }
}

#[test]
fn oversized_plan_fails_validation() {
    use crate::application_kernel::application_plan::{validate_plan, MAX_PLAN_INTENTS};
    let mut plan = task_tracker_create_plan();
    let base = plan.intents[1].clone();
    plan.intents = (0..=MAX_PLAN_INTENTS)
        .map(|i| match &base {
            crate::application_kernel::compiler::ChangeIntent::UpsertDataModel {
                application_id,
                model,
            } => {
                let mut m = model.clone();
                m.model_id = format!("m{i}");
                crate::application_kernel::compiler::ChangeIntent::UpsertDataModel {
                    application_id: application_id.clone(),
                    model: m,
                }
            }
            other => other.clone(),
        })
        .collect();
    assert!(validate_plan(&plan).is_err());
}
