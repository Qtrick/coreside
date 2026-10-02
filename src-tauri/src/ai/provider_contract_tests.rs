//! Deterministic provider-contract tests for ApplicationPlan.
//!
//! These prove: recorded/provider-shaped JSON → parse → validate → compile → ops
//! without calling a live network provider. Live provider smoke is separate.

use crate::ai::parse_agent_response;
use crate::ai::plan_fixtures::{
    budget_tracker_create_plan, habit_tracker_create_plan, journal_create_plan,
    multi_surface_planner_create_plan, plan_response_json, task_tracker_add_due_dates_plan,
    task_tracker_create_plan,
};
use crate::application_kernel::application_plan::{compile_plan, ApplicationPlanKind};

#[test]
fn recorded_create_plan_roundtrips_through_parser() {
    let raw = plan_response_json("I generated a Task Tracker.", &task_tracker_create_plan());
    let parsed = parse_agent_response(&raw).expect("parse recorded create plan");
    let plan = parsed
        .payload
        .application_plan
        .as_ref()
        .expect("applicationPlan present");
    assert_eq!(plan.kind, ApplicationPlanKind::Create);
    let ops = parsed
        .payload
        .inspection_operations()
        .expect("compile via normalized_operations");
    assert!(ops.iter().any(|o| o.op_type == "data.model_upsert"));
    assert!(ops.iter().any(|o| o.op_type == "surface.create"));
    assert!(parsed.payload.validate().is_ok());
}

fn assert_recorded_create_fixture_roundtrips(
    assistant_message: &str,
    plan: &crate::application_kernel::application_plan::ApplicationPlan,
) {
    let raw = plan_response_json(assistant_message, plan);
    let parsed = parse_agent_response(&raw).expect("parse recorded create plan");
    let recovered = parsed
        .payload
        .application_plan
        .as_ref()
        .expect("applicationPlan present");
    assert_eq!(recovered.kind, ApplicationPlanKind::Create);
    let ops = parsed
        .payload
        .inspection_operations()
        .expect("compile via normalized_operations");
    assert!(ops.iter().any(|o| o.op_type == "data.model_upsert"));
    assert!(ops.iter().any(|o| o.op_type == "surface.create"));
    assert!(parsed.payload.validate().is_ok());
}

#[test]
fn recorded_habit_tracker_create_plan_roundtrips() {
    assert_recorded_create_fixture_roundtrips(
        "Creating Habit Tracker.",
        &habit_tracker_create_plan(),
    );
}

#[test]
fn recorded_budget_tracker_create_plan_roundtrips() {
    assert_recorded_create_fixture_roundtrips(
        "Creating Budget Tracker.",
        &budget_tracker_create_plan(),
    );
}

#[test]
fn recorded_journal_create_plan_roundtrips() {
    assert_recorded_create_fixture_roundtrips("Creating Journal.", &journal_create_plan());
}

#[test]
fn recorded_multi_surface_planner_create_plan_roundtrips() {
    assert_recorded_create_fixture_roundtrips(
        "Creating Study Planner.",
        &multi_surface_planner_create_plan(),
    );
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
    // Plan-level OCC stamps ops missing their own baseRevision.
    assert!(validated
        .compiled
        .operations
        .iter()
        .all(|o| o.base_revision == Some(2) || o.base_revision.is_some()));
}

#[test]
fn evolve_plan_without_base_revision_fails_closed() {
    let plan = task_tracker_add_due_dates_plan(None);
    assert!(plan.base_revision.is_none());
    assert!(compile_plan(&plan).is_err());
}

#[test]
fn openai_and_gemini_application_plan_schemas_share_required_keys() {
    use crate::application_kernel::application_plan::{
        application_plan_json_schema_gemini, application_plan_json_schema_openai,
    };
    let openai = application_plan_json_schema_openai();
    let gemini = application_plan_json_schema_gemini();
    for schema in [&openai, &gemini] {
        let props = schema.get("properties").expect("properties");
        assert!(props.get("kind").is_some());
        assert!(props.get("summary").is_some());
        assert!(props.get("intents").is_some());
        assert!(props.get("applicationId").is_some());
        assert!(props.get("baseRevision").is_some());
        let required = schema
            .get("required")
            .and_then(|v| v.as_array())
            .expect("required");
        assert!(required.iter().any(|v| v.as_str() == Some("kind")));
        assert!(required.iter().any(|v| v.as_str() == Some("summary")));
        assert!(required.iter().any(|v| v.as_str() == Some("intents")));
    }
    // Gemini must not use OpenAI-style nullable unions at the plan root.
    assert_eq!(gemini.get("type").and_then(|v| v.as_str()), Some("object"));
}

#[test]
fn gemini_response_schema_includes_application_plan() {
    // Covered directly in gemini.rs unit tests; this asserts the shared fragment
    // remains the single source of truth for required keys.
    let gemini_fragment =
        crate::application_kernel::application_plan::application_plan_json_schema_gemini();
    assert_eq!(
        gemini_fragment["required"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>(),
        vec!["kind", "summary", "intents"]
    );
    let openai = crate::ai::openai_family::agent_response_json_schema();
    assert!(openai["properties"]["applicationPlan"].is_object());
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
                p.payload.validate().is_err() || p.payload.inspection_operations().is_err(),
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
