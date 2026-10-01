//! Central visibility policy for interactive definitions and state.
//!
//! One authoritative projection path for renderer, model, history, replay,
//! diagnostics, and structured-input context. Callers must not invent ad-hoc
//! field deletions for secrecy.

use serde_json::{json, Map, Value};

use super::rules_engine::{InteractiveAppDefinition, RNG_STATE_KEY};
use super::software_document::StateContract;

/// Audience that may receive a projected definition or state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audience {
    /// React renderer / Tauri IPC to the webview.
    Renderer,
    /// Model prompt / structured AI-turn context.
    Model,
    /// History / diagnostics / action logs (default-redacted).
    History,
}

/// True when a contract hides its value from public audiences.
pub fn contract_hides(c: &StateContract) -> bool {
    matches!(c.read_policy.as_str(), "restricted" | "private" | "hidden")
        || c.sensitivity.as_deref() == Some("sensitive")
}

/// True when a raw read-policy / sensitivity pair hides a value.
pub fn policy_hides(read_policy: Option<&str>, sensitivity: Option<&str>) -> bool {
    matches!(read_policy, Some("restricted" | "private" | "hidden"))
        || sensitivity == Some("sensitive")
}

/// Project engine-owned or free-form state through declared contracts.
///
/// - Always strips `__rng`.
/// - Keys with hiding contracts are omitted.
/// - Keys without contracts remain visible (fail-open for undeclared UI keys)
///   unless `strict_known_only` is true.
pub fn project_state_with_contracts(
    state: &Value,
    contracts: &[StateContract],
    strict_known_only: bool,
) -> Value {
    let mut out = Map::new();
    let Some(obj) = state.as_object() else {
        return json!({});
    };
    for (k, v) in obj {
        if k == RNG_STATE_KEY {
            continue;
        }
        match contracts.iter().find(|c| c.key == *k) {
            Some(c) if contract_hides(c) => continue,
            None if strict_known_only => continue,
            _ => {
                out.insert(k.clone(), v.clone());
            }
        }
    }
    Value::Object(out)
}

/// Parse an already-stored interactive blob for projection (no self-test re-run).
fn interactive_def_for_projection(definition: &Value) -> Option<InteractiveAppDefinition> {
    let raw = match definition.get("interactive") {
        None | Some(Value::Null) => return None,
        Some(v) => v,
    };
    serde_json::from_value::<InteractiveAppDefinition>(raw.clone()).ok()
}

/// Project a full surface state using interactive schema contracts when present,
/// otherwise top-level SoftwareDocument `stateContracts`.
///
/// Renderer keeps fail-open for undeclared UI keys. Model and History are
/// fail-closed on undeclared rest keys so prompt/log projection cannot leak
/// incidental secrets when contracts are missing or incomplete.
pub fn project_surface_state(definition: &Value, state: &Value, audience: Audience) -> Value {
    let strict_rest = matches!(audience, Audience::Model | Audience::History);
    let rest_contracts = top_level_state_contracts(definition);
    if let Some(def) = interactive_def_for_projection(definition) {
        let owned = def.owned_keys();
        let mut owned_state = Map::new();
        let mut rest = Map::new();
        if let Some(obj) = state.as_object() {
            for (k, v) in obj {
                if owned.contains(k) {
                    owned_state.insert(k.clone(), v.clone());
                } else {
                    rest.insert(k.clone(), v.clone());
                }
            }
        }
        // public_view must only see engine-owned keys — never incidental rest —
        // so top-level restricted rest keys cannot sneak in via undeclared-key pass-through.
        let public_owned = def.public_view(&Value::Object(owned_state));
        let public_rest =
            project_state_with_contracts(&Value::Object(rest), &rest_contracts, strict_rest);
        return merge_objects(&public_owned, &public_rest);
    }
    project_state_with_contracts(state, &rest_contracts, strict_rest)
}

fn top_level_state_contracts(definition: &Value) -> Vec<StateContract> {
    let Some(Value::Array(items)) = definition.get("stateContracts") else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            serde_json::from_value::<StateContract>(item.clone())
                .ok()
                .or_else(|| {
                    // Minimal fail-closed parse when full StateContract deserialize fails.
                    let key = item.get("key")?.as_str()?.to_string();
                    let read_policy = item
                        .get("readPolicy")
                        .and_then(Value::as_str)
                        .unwrap_or("public")
                        .to_string();
                    let sensitivity = item
                        .get("sensitivity")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    let mut c = StateContract::default();
                    c.key = key;
                    c.read_policy = read_policy;
                    c.sensitivity = sensitivity;
                    Some(c)
                })
        })
        .collect()
}

fn merge_objects(a: &Value, b: &Value) -> Value {
    let mut out = Map::new();
    if let Some(m) = a.as_object() {
        for (k, v) in m {
            out.insert(k.clone(), v.clone());
        }
    }
    if let Some(m) = b.as_object() {
        for (k, v) in m {
            out.insert(k.clone(), v.clone());
        }
    }
    Value::Object(out)
}

/// Redact a state-schema entry's initial value for a given audience.
fn project_contract(c: &StateContract, audience: Audience) -> Value {
    let mut entry = json!({
        "key": c.key,
        "type": c.type_name,
        "nullable": c.nullable,
        "readPolicy": c.read_policy,
        "writePolicy": c.write_policy,
        "origin": c.origin,
    });
    if let Some(obj) = entry.as_object_mut() {
        if let Some(desc) = &c.description {
            obj.insert("description".into(), json!(desc));
        }
        if let Some(sens) = &c.sensitivity {
            obj.insert("sensitivity".into(), json!(sens));
        }
        if let Some(scope) = serde_json::to_value(&c.scope).ok() {
            obj.insert("scope".into(), scope);
        }
        if contract_hides(c) {
            // Model may know the key exists and its type/policy, never the value.
            // Renderer must not receive the value either.
            obj.insert("initialValue".into(), Value::Null);
            obj.insert("valueRedacted".into(), json!(true));
            if matches!(audience, Audience::Model) {
                obj.insert("modelNote".into(), json!("value intentionally unavailable"));
            }
        } else {
            obj.insert("initialValue".into(), c.initial_value.clone());
        }
    }
    entry
}

fn project_test_case(tc: &Value, contracts: &[StateContract], audience: Audience) -> Option<Value> {
    // Renderer never needs embedded self-tests.
    if matches!(audience, Audience::Renderer | Audience::History) {
        return None;
    }
    let mut out = tc.clone();
    let Some(obj) = out.as_object_mut() else {
        return Some(out);
    };
    // Strip expected hidden subsets and any initialState overrides of hidden keys.
    if let Some(Value::Object(subset)) = obj.get_mut("expectedStateSubset") {
        subset.retain(|k, _| {
            contracts
                .iter()
                .find(|c| c.key == *k)
                .map(|c| !contract_hides(c))
                .unwrap_or(true)
        });
    }
    if let Some(Value::Object(init)) = obj.get_mut("initialState") {
        init.retain(|k, _| {
            contracts
                .iter()
                .find(|c| c.key == *k)
                .map(|c| !contract_hides(c))
                .unwrap_or(true)
        });
    }
    Some(out)
}

/// Project an interactive definition for renderer or model audiences.
pub fn project_interactive_definition(def: &InteractiveAppDefinition, audience: Audience) -> Value {
    let schema: Vec<Value> = def
        .state_schema
        .iter()
        .map(|c| project_contract(c, audience))
        .collect();

    let mut out = json!({
        "id": def.id,
        "kind": def.kind,
        "schemaVersion": def.schema_version,
        "metadata": sanitize_metadata(&def.metadata, audience),
        "actors": def.actors,
        "aiActors": def.ai_actors,
        "turnPolicy": def.turn_policy,
        "stateSchema": schema,
        "actions": project_actions(def, audience),
        "derivedState": def.derived_state,
        "terminalConditions": def.terminal_conditions,
    });

    // Never expose the RNG seed to renderer/model/history.
    // Presence of RNG may be noted for the model without the seed value.
    if def.random_seed.is_some() {
        if let Some(obj) = out.as_object_mut() {
            if matches!(audience, Audience::Model) {
                obj.insert("hasDeterministicRng".into(), json!(true));
            }
        }
    }

    if matches!(audience, Audience::Model) {
        if let Some(obj) = out.as_object_mut() {
            let cases: Vec<Value> = def
                .test_cases
                .iter()
                .filter_map(|tc| {
                    let raw = serde_json::to_value(tc).ok()?;
                    project_test_case(&raw, &def.state_schema, audience)
                })
                .collect();
            if !cases.is_empty() {
                obj.insert("testCases".into(), Value::Array(cases));
            }
            obj.insert("modelSafe".into(), json!(true));
            obj.insert(
                "authorityNote".into(),
                json!("Rust owns interactive state; propose interactive.action only"),
            );
        }
    }

    out
}

fn sanitize_metadata(meta: &Value, audience: Audience) -> Value {
    let mut m = match meta {
        Value::Object(o) => o.clone(),
        _ => return json!({}),
    };
    // hostTick is unsupported unless a trusted scheduler exists — never advertise it.
    m.remove("hostTick");
    if matches!(audience, Audience::Renderer) {
        // Keep display metadata only.
        m.retain(|k, _| {
            matches!(
                k.as_str(),
                "title" | "description" | "capability" | "icon" | "theme"
            )
        });
    }
    Value::Object(m)
}

fn project_actions(def: &InteractiveAppDefinition, audience: Audience) -> Value {
    // Expose parameter contracts only. Effects/guards are Rust-owned rule bodies —
    // shipping them to the renderer or model lets clients reverse-engineer scoring
    // and hidden-key references without needing the values themselves.
    let _ = audience;
    let actions: Vec<Value> = def
        .actions
        .iter()
        .map(|a| {
            json!({
                "id": a.id,
                "actor": a.actor,
                "allowedPhases": a.allowed_phases,
                "parameters": a.parameters,
                "description": a.description,
            })
        })
        .collect();
    Value::Array(actions)
}

/// Project the `interactive` field (or whole surface definition) for an audience.
pub fn project_definition_value(definition: &Value, audience: Audience) -> Value {
    let mut out = definition.clone();
    let interactive = match out.get("interactive") {
        Some(v) if !v.is_null() => v.clone(),
        _ => return out,
    };
    match serde_json::from_value::<InteractiveAppDefinition>(interactive) {
        Ok(def) => {
            if let Some(obj) = out.as_object_mut() {
                obj.insert(
                    "interactive".into(),
                    project_interactive_definition(&def, audience),
                );
            }
        }
        Err(_) => {
            // Fail closed for untrusted audiences: drop the interactive blob.
            if let Some(obj) = out.as_object_mut() {
                obj.insert(
                    "interactive".into(),
                    json!({ "id": "invalid", "kind": "unknown", "error": "definition unavailable" }),
                );
            }
        }
    }
    // Also redact top-level stateContracts initial values that are restricted.
    if let Some(Value::Array(contracts)) = out.get_mut("stateContracts") {
        for c in contracts.iter_mut() {
            let read = c.get("readPolicy").and_then(Value::as_str);
            let sens = c.get("sensitivity").and_then(Value::as_str);
            if policy_hides(read, sens) {
                if let Some(obj) = c.as_object_mut() {
                    obj.insert("initialValue".into(), Value::Null);
                    obj.insert("valueRedacted".into(), json!(true));
                }
            }
        }
    }
    out
}

/// History params default to redacted. Only non-object / empty payloads stay as-is.
pub fn redact_history_params(params: &Value) -> Value {
    match params {
        Value::Null => Value::Null,
        Value::Object(m) if m.is_empty() => json!({}),
        Value::Object(m) => {
            let mut redacted = Map::new();
            for (k, v) in m {
                redacted.insert(k.clone(), redact_param_value(v));
            }
            Value::Object(redacted)
        }
        other => json!({
            "redacted": true,
            "type": type_name(other),
        }),
    }
}

fn redact_param_value(v: &Value) -> Value {
    match v {
        Value::Null => Value::Null,
        Value::Bool(_) | Value::Number(_) => v.clone(),
        // Strings in action params may encode hidden cards/answers — always redact.
        Value::String(_) => json!({ "redacted": true, "type": "string" }),
        Value::Array(a) => json!({
            "redacted": true,
            "type": "array",
            "length": a.len(),
        }),
        Value::Object(m) => json!({
            "redacted": true,
            "type": "object",
            "keys": m.keys().cloned().collect::<Vec<_>>(),
        }),
    }
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Build a machine-readable interactive capability catalog for the model.
pub fn model_capability_catalog(def: &InteractiveAppDefinition, public_state: &Value) -> Value {
    let visible_keys: Vec<String> = def
        .state_schema
        .iter()
        .filter(|c| !contract_hides(c))
        .map(|c| c.key.clone())
        .collect();
    let actions: Vec<Value> = def
        .actions
        .iter()
        .map(|a| {
            json!({
                "actionId": a.id,
                "description": a.description,
                "parameters": a.parameters,
                "actor": a.actor,
            })
        })
        .collect();
    json!({
        "applicationId": def.id,
        "kind": def.kind,
        "actors": def.actors,
        "aiActors": def.ai_actors,
        "turnPolicy": def.turn_policy,
        "visibleStateKeys": visible_keys,
        "publicState": public_state,
        "status": public_state.get("status"),
        "currentPlayer": public_state.get("currentPlayer"),
        "phase": public_state.get("phase"),
        "actions": actions,
        "capability": def.metadata.get("capability"),
        "authority": "rust",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_v2::rules_fixtures;

    #[test]
    fn quiz_answers_redacted_in_model_and_renderer_definitions() {
        let def = rules_fixtures::fixture_quiz();
        for audience in [Audience::Renderer, Audience::Model] {
            let projected = project_interactive_definition(&def, audience);
            let schema = projected["stateSchema"].as_array().unwrap();
            let answers = schema.iter().find(|c| c["key"] == "answers").unwrap();
            assert!(answers["initialValue"].is_null(), "{audience:?}");
            assert_eq!(answers["valueRedacted"], true);
            assert_ne!(answers["readPolicy"], "public");
        }
        let renderer = project_interactive_definition(&def, Audience::Renderer);
        assert!(renderer.get("testCases").is_none());
        assert!(renderer.get("randomSeed").is_none());
    }

    #[test]
    fn public_state_omits_restricted_keys() {
        let def = rules_fixtures::fixture_quiz();
        let state = def.initial_instance_state().unwrap();
        assert!(state.get("answers").is_some());
        let public = def.public_view(&state);
        assert!(public.get("answers").is_none());
        assert!(public.get("questions").is_some());
    }

    #[test]
    fn history_params_are_redacted_by_default() {
        let raw = json!({ "card": "QH", "index": 3, "note": "secret-long-value" });
        let redacted = redact_history_params(&raw);
        assert_eq!(redacted["index"], 3);
        assert_eq!(redacted["card"]["redacted"], true);
        assert_eq!(redacted["note"]["redacted"], true);
    }

    #[test]
    fn host_tick_stripped_from_metadata() {
        let def = rules_fixtures::fixture_timer();
        let projected = project_interactive_definition(&def, Audience::Renderer);
        assert!(projected["metadata"].get("hostTick").is_none());
    }

    #[test]
    fn projected_actions_omit_effects_and_guards() {
        let def = rules_fixtures::fixture_quiz();
        for audience in [Audience::Renderer, Audience::Model] {
            let projected = project_interactive_definition(&def, audience);
            let actions = projected["actions"].as_array().unwrap();
            let answer = actions.iter().find(|a| a["id"] == "answer").unwrap();
            assert!(answer.get("effects").is_none(), "{audience:?}");
            assert!(answer.get("guards").is_none(), "{audience:?}");
            assert!(answer.get("parameters").is_some(), "{audience:?}");
            let encoded = answer.to_string();
            assert!(
                !encoded.contains("getIndex") && !encoded.contains("\"answers\""),
                "action projection must not reference hidden scoring keys: {encoded}"
            );
        }
        let catalog = model_capability_catalog(
            &def,
            &def.public_view(&def.initial_instance_state().unwrap()),
        );
        let catalog_actions = catalog["actions"].as_array().unwrap();
        assert!(catalog_actions.iter().all(|a| a.get("effects").is_none()));
    }

    #[test]
    fn project_surface_state_strips_restricted_from_owned_and_rest() {
        let quiz = rules_fixtures::fixture_quiz();
        let owned = quiz.initial_instance_state().unwrap();
        // Incidental non-owned rest that must also be filtered by top-level contracts.
        let definition = json!({
            "stateContracts": [
                { "key": "secretHint", "type": "string", "initialValue": "x", "readPolicy": "restricted" },
                { "key": "theme", "type": "string", "initialValue": "light", "writePolicy": "user" }
            ],
            "interactive": serde_json::to_value(&quiz).unwrap(),
        });
        let state = json!({
            "answers": owned["answers"],
            "questions": owned["questions"],
            "currentQuestion": owned["currentQuestion"],
            "score": owned["score"],
            "lastResult": owned["lastResult"],
            "status": owned["status"],
            "secretHint": "never-leak",
            "theme": "dark",
            "__rng": { "counter": 1 },
        });
        for audience in [Audience::Renderer, Audience::Model, Audience::History] {
            let public = project_surface_state(&definition, &state, audience);
            assert!(
                public.get("answers").is_none(),
                "owned restricted answers leaked for {audience:?}: {public}"
            );
            assert!(
                public.get("secretHint").is_none(),
                "rest restricted key leaked for {audience:?}: {public}"
            );
            assert!(public.get("__rng").is_none(), "rng leaked for {audience:?}");
            assert_eq!(public["theme"], "dark");
            assert!(public.get("questions").is_some());
        }
    }

    #[test]
    fn model_and_history_omit_undeclared_rest_keys() {
        let definition = json!({
            "stateContracts": [
                { "key": "theme", "type": "string", "initialValue": "light", "writePolicy": "user" }
            ]
        });
        let state = json!({
            "theme": "dark",
            "incidentalSecret": "sk-live-should-not-reach-model",
        });
        let renderer = project_surface_state(&definition, &state, Audience::Renderer);
        assert_eq!(renderer["theme"], "dark");
        assert_eq!(
            renderer["incidentalSecret"],
            "sk-live-should-not-reach-model"
        );
        for audience in [Audience::Model, Audience::History] {
            let public = project_surface_state(&definition, &state, audience);
            assert_eq!(public["theme"], "dark");
            assert!(
                public.get("incidentalSecret").is_none(),
                "undeclared rest leaked for {audience:?}: {public}"
            );
        }
    }

    #[test]
    fn history_redacts_drawn_card_string_params() {
        let raw = json!({ "card": "QH", "pile": "draw", "index": 0 });
        let redacted = redact_history_params(&raw);
        assert_eq!(redacted["index"], 0);
        assert_eq!(
            redacted["card"],
            json!({ "redacted": true, "type": "string" })
        );
        assert_eq!(
            redacted["pile"],
            json!({ "redacted": true, "type": "string" })
        );
        let encoded = redacted.to_string();
        assert!(!encoded.contains("QH"));
        assert!(!encoded.contains("draw"));
    }

    #[test]
    fn model_capability_catalog_omits_restricted_keys() {
        let def = rules_fixtures::fixture_quiz();
        let state = def.initial_instance_state().unwrap();
        let public = def.public_view(&state);
        let catalog = model_capability_catalog(&def, &public);
        let visible = catalog["visibleStateKeys"].as_array().unwrap();
        assert!(!visible.iter().any(|k| k == "answers"));
        assert!(visible.iter().any(|k| k == "questions"));
        assert!(catalog["publicState"].get("answers").is_none());
        assert_eq!(catalog["authority"], "rust");
    }

    #[test]
    fn model_test_cases_strip_hidden_initial_and_expected_keys() {
        let mut def = rules_fixtures::fixture_quiz();
        def.test_cases.push(
            serde_json::from_value(json!({
                "name": "leak attempt",
                "actionId": "answer",
                "params": { "choice": 1 },
                "initialState": { "answers": [2, 2, 2], "score": 0 },
                "expectedStateSubset": { "answers": [2, 2, 2], "score": 1 },
                "expectedSuccess": true
            }))
            .unwrap(),
        );
        let model = project_interactive_definition(&def, Audience::Model);
        let cases = model["testCases"]
            .as_array()
            .expect("model keeps test cases");
        let leak = cases.iter().find(|c| c["name"] == "leak attempt").unwrap();
        assert!(leak["initialState"].get("answers").is_none());
        assert!(leak["expectedStateSubset"].get("answers").is_none());
        assert_eq!(leak["expectedStateSubset"]["score"], 1);
        let renderer = project_interactive_definition(&def, Audience::Renderer);
        assert!(renderer.get("testCases").is_none());
    }
}
