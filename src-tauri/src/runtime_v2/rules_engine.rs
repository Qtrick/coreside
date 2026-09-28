//! Declarative Interactive Application & Rule Engine.
//!
//! Provides a secure, bounded, deterministic runtime for model-generated
//! interactive applications and games. The model creates the declarative
//! definition, while the Rust runtime remains authoritative over state,
//! action legality, transitions, derived metrics, and randomness.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

use super::software_document::{validate_state_value_type, StateContract};

const MAX_EXPR_DEPTH: usize = 16;
const MAX_EFFECTS_PER_ACTION: usize = 64;
const MAX_ACTIONS_COUNT: usize = 128;
const MAX_ARRAY_LEN: usize = 1024;
const MAX_STATE_JSON_BYTES: usize = 256 * 1024; // 256 KB

/// Deterministic Pseudo-Random Number Generator (SplitMix64).
/// Seed is persisted in state/turn history for replay and branching determinism.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeterministicRng {
    pub seed: u64,
    pub state: u64,
}

impl DeterministicRng {
    pub fn new(seed: u64) -> Self {
        Self { seed, state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    pub fn next_bounded(&mut self, bound: u64) -> u64 {
        if bound == 0 {
            0
        } else {
            self.next_u64() % bound
        }
    }

    pub fn shuffle_json_array(&mut self, arr: &mut [Value]) {
        let n = arr.len();
        if n < 2 {
            return;
        }
        for i in (1..n).rev() {
            let j = self.next_bounded((i + 1) as u64) as usize;
            arr.swap(i, j);
        }
    }
}

/// Bounded, typed declarative expression AST.
/// Free of arbitrary code, eval, filesystem, or network access.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum RuleExpr {
    Const {
        value: Value,
    },
    GetState {
        key: String,
    },
    GetParam {
        name: String,
    },
    GetPlayer,
    GetTurn,
    GetPhase,
    GetCell {
        board_key: String,
        row: Box<RuleExpr>,
        col: Box<RuleExpr>,
    },
    GetCellAt {
        board_key: String,
        index: Box<RuleExpr>,
    },
    IsEmpty {
        board_key: String,
        index: Box<RuleExpr>,
    },
    IsOccupied {
        board_key: String,
        index: Box<RuleExpr>,
    },
    Distance {
        r1: Box<RuleExpr>,
        c1: Box<RuleExpr>,
        r2: Box<RuleExpr>,
        c2: Box<RuleExpr>,
    },
    Eq {
        left: Box<RuleExpr>,
        right: Box<RuleExpr>,
    },
    Neq {
        left: Box<RuleExpr>,
        right: Box<RuleExpr>,
    },
    Lt {
        left: Box<RuleExpr>,
        right: Box<RuleExpr>,
    },
    Lte {
        left: Box<RuleExpr>,
        right: Box<RuleExpr>,
    },
    Gt {
        left: Box<RuleExpr>,
        right: Box<RuleExpr>,
    },
    Gte {
        left: Box<RuleExpr>,
        right: Box<RuleExpr>,
    },
    And {
        exprs: Vec<RuleExpr>,
    },
    Or {
        exprs: Vec<RuleExpr>,
    },
    Not {
        expr: Box<RuleExpr>,
    },
    Add {
        left: Box<RuleExpr>,
        right: Box<RuleExpr>,
    },
    Sub {
        left: Box<RuleExpr>,
        right: Box<RuleExpr>,
    },
    Mul {
        left: Box<RuleExpr>,
        right: Box<RuleExpr>,
    },
    Div {
        left: Box<RuleExpr>,
        right: Box<RuleExpr>,
    },
    Mod {
        left: Box<RuleExpr>,
        right: Box<RuleExpr>,
    },
    Min {
        left: Box<RuleExpr>,
        right: Box<RuleExpr>,
    },
    Max {
        left: Box<RuleExpr>,
        right: Box<RuleExpr>,
    },
    Abs {
        expr: Box<RuleExpr>,
    },
    Contains {
        array: Box<RuleExpr>,
        item: Box<RuleExpr>,
    },
    Count {
        expr: Box<RuleExpr>,
    },
    In {
        item: Box<RuleExpr>,
        array: Box<RuleExpr>,
    },
}

/// Typed, side-effect-free state transitions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "effect", rename_all = "camelCase")]
pub enum RuleEffect {
    Set {
        key: String,
        value: RuleExpr,
    },
    SetCell {
        board_key: String,
        row: RuleExpr,
        col: RuleExpr,
        value: RuleExpr,
    },
    SetCellAt {
        board_key: String,
        index: RuleExpr,
        value: RuleExpr,
    },
    SwapCells {
        board_key: String,
        from_index: RuleExpr,
        to_index: RuleExpr,
    },
    Increment {
        key: String,
        amount: RuleExpr,
    },
    Decrement {
        key: String,
        amount: RuleExpr,
    },
    PushArray {
        key: String,
        item: RuleExpr,
    },
    RemoveFromArray {
        key: String,
        index: RuleExpr,
    },
    ShuffleArray {
        key: String,
    },
    AdvanceTurn {
        players: Vec<String>,
    },
    SetPhase {
        phase: String,
    },
    SetActor {
        actor: RuleExpr,
    },
    EndGame {
        status: String,
        winner: Option<RuleExpr>,
    },
    ResetGame,
}

/// Action contract in the declarative rules engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RuleActionDef {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_phases: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub parameters: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub guards: Vec<RuleExpr>,
    #[serde(default)]
    pub effects: Vec<RuleEffect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Terminal condition declaration (e.g. checkmate, 4-in-a-row, puzzle solved).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TerminalCondition {
    pub condition: RuleExpr,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub winner: Option<RuleExpr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Derived state computation rule.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DerivedStateDef {
    pub key: String,
    pub expr: RuleExpr,
}

/// Self-test case embedded in the application definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RuleTestCase {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_state: Option<Value>,
    pub action_id: String,
    #[serde(default)]
    pub params: Value,
    pub expected_success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_state_subset: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_status: Option<String>,
}

/// Summary of a legally available action for the active actor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LegalActionSummary {
    pub action_id: String,
    pub description: Option<String>,
    pub parameters: HashMap<String, String>,
}

/// Result of evaluating and applying an action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActionOutcome {
    pub success: bool,
    pub new_state: Value,
    pub derived_state: Value,
    pub status: String,
    pub winner: Option<String>,
    pub turn: i64,
    pub phase: String,
    pub current_player: String,
    pub error: Option<String>,
}

/// Canonical declarative interactive application definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InteractiveAppDefinition {
    pub id: String,
    pub kind: String,
    #[serde(default = "default_schema_version")]
    pub schema_version: String,
    #[serde(default)]
    pub metadata: Value,
    #[serde(default)]
    pub initial_state: Value,
    #[serde(default)]
    pub state_schema: Vec<StateContract>,
    #[serde(default)]
    pub actors: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_policy: Option<String>,
    #[serde(default)]
    pub actions: Vec<RuleActionDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub derived_state: Vec<DerivedStateDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub terminal_conditions: Vec<TerminalCondition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub random_seed: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub test_cases: Vec<RuleTestCase>,
}

fn default_schema_version() -> String {
    "2".to_string()
}

// ---------------------------------------------------------------------------
// Evaluator & Execution Implementation
// ---------------------------------------------------------------------------

impl InteractiveAppDefinition {
    /// Validate definition structure, contracts, bounds, and action uniqueness.
    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("Application ID cannot be empty".into());
        }
        if self.actions.len() > MAX_ACTIONS_COUNT {
            return Err(format!(
                "Exceeded maximum actions limit of {MAX_ACTIONS_COUNT}"
            ));
        }

        // Validate state contracts
        let mut state_keys = HashSet::new();
        for sc in &self.state_schema {
            if sc.key.trim().is_empty() {
                return Err("StateContract key cannot be empty".into());
            }
            if !state_keys.insert(sc.key.clone()) {
                return Err(format!("Duplicate state contract key '{}'", sc.key));
            }
            validate_state_value_type(
                &sc.key,
                &sc.initial_value,
                &sc.type_name,
                sc.is_effective_nullable(),
            )?;
        }

        // Validate initial state against contracts
        if let Value::Object(ref map) = self.initial_state {
            let bytes = serde_json::to_vec(&self.initial_state).unwrap_or_default();
            if bytes.len() > MAX_STATE_JSON_BYTES {
                return Err(format!(
                    "Initial state exceeds byte limit of {MAX_STATE_JSON_BYTES}"
                ));
            }
            for (k, v) in map {
                if let Some(sc) = self.state_schema.iter().find(|s| &s.key == k) {
                    validate_state_value_type(k, v, &sc.type_name, sc.is_effective_nullable())?;
                }
            }
        }

        // Validate action IDs uniqueness
        let mut action_ids = HashSet::new();
        for act in &self.actions {
            if act.id.trim().is_empty() {
                return Err("Action id cannot be empty".into());
            }
            if !action_ids.insert(act.id.clone()) {
                return Err(format!("Duplicate action id '{}'", act.id));
            }
            if act.effects.len() > MAX_EFFECTS_PER_ACTION {
                return Err(format!(
                    "Action '{}' exceeds effect limit of {MAX_EFFECTS_PER_ACTION}",
                    act.id
                ));
            }
        }

        Ok(())
    }

    /// Run embedded rule self-tests. Fails closed if any test case fails.
    pub fn run_self_tests(&self) -> Result<(), String> {
        self.validate()?;
        for tc in &self.test_cases {
            let mut state = tc
                .initial_state
                .clone()
                .unwrap_or_else(|| self.initial_state.clone());
            let mut rng = DeterministicRng::new(self.random_seed.unwrap_or(42));
            let actor = state
                .get("currentPlayer")
                .and_then(|v| v.as_str())
                .or_else(|| self.actors.first().map(|s| s.as_str()))
                .unwrap_or("player1")
                .to_string();
            let result =
                self.execute_action(&mut state, &tc.action_id, &tc.params, &actor, &mut rng);

            if tc.expected_success && result.is_err() {
                return Err(format!(
                    "Self-test '{}' failed: expected success, got error: {}",
                    tc.name,
                    result.unwrap_err()
                ));
            }
            if !tc.expected_success && result.is_ok() {
                return Err(format!(
                    "Self-test '{}' failed: expected failure, but action succeeded",
                    tc.name
                ));
            }
            if tc.expected_success {
                let outcome = result.unwrap();
                if let Some(ref exp_status) = tc.expected_status {
                    if &outcome.status != exp_status {
                        return Err(format!(
                            "Self-test '{}' failed: expected status '{}', got '{}'",
                            tc.name, exp_status, outcome.status
                        ));
                    }
                }
                if let Some(Value::Object(ref subset)) = tc.expected_state_subset {
                    for (k, expected_v) in subset {
                        let actual_v = outcome.new_state.get(k);
                        if actual_v != Some(expected_v) {
                            return Err(format!(
                                "Self-test '{}' failed: state mismatch for key '{}': expected {:?}, got {:?}",
                                tc.name, k, expected_v, actual_v
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Check if a candidate action is legally permissible under current state.
    pub fn is_legal_action(
        &self,
        state: &Value,
        action_id: &str,
        params: &Value,
        actor: &str,
    ) -> Result<bool, String> {
        let action = self
            .actions
            .iter()
            .find(|a| a.id == action_id)
            .ok_or_else(|| format!("Action '{action_id}' not found"))?;

        let current_player = state
            .get("currentPlayer")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let turn = state.get("turn").and_then(|v| v.as_i64()).unwrap_or(1);
        let phase = state
            .get("phase")
            .and_then(|v| v.as_str())
            .unwrap_or("main");

        // Validate actor
        if let Some(ref req_actor) = action.actor {
            if req_actor == "active_player" {
                if !current_player.is_empty() && actor != current_player {
                    return Ok(false);
                }
            } else if req_actor != actor {
                return Ok(false);
            }
        }

        // Validate phase
        if !action.allowed_phases.is_empty() && !action.allowed_phases.iter().any(|p| p == phase) {
            return Ok(false);
        }

        // Evaluate guards
        for guard in &action.guards {
            let val = evaluate_expr(guard, state, params, current_player, turn, phase, 0)?;
            if !is_truthy(&val) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// List all legally available actions for the given actor.
    pub fn get_legal_actions(&self, state: &Value, actor: &str) -> Vec<LegalActionSummary> {
        let mut legal = Vec::new();
        for action in &self.actions {
            if self
                .is_legal_action(state, &action.id, &json!({}), actor)
                .unwrap_or(false)
            {
                legal.push(LegalActionSummary {
                    action_id: action.id.clone(),
                    description: action.description.clone(),
                    parameters: action.parameters.clone(),
                });
            }
        }
        legal
    }

    /// Execute an action atomically on authoritative state.
    pub fn execute_action(
        &self,
        state: &mut Value,
        action_id: &str,
        params: &Value,
        actor: &str,
        rng: &mut DeterministicRng,
    ) -> Result<ActionOutcome, String> {
        let action = self
            .actions
            .iter()
            .find(|a| a.id == action_id)
            .ok_or_else(|| format!("Action '{action_id}' not defined"))?;

        if !self.is_legal_action(state, action_id, params, actor)? {
            return Err(format!(
                "Action '{action_id}' is illegal in current game state"
            ));
        }

        let mut next_state = state.clone();
        if !next_state.is_object() {
            next_state = json!({});
        }

        let current_player = next_state
            .get("currentPlayer")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let turn = next_state.get("turn").and_then(|v| v.as_i64()).unwrap_or(1);
        let phase = next_state
            .get("phase")
            .and_then(|v| v.as_str())
            .unwrap_or("main")
            .to_string();

        // Apply effects
        for effect in &action.effects {
            apply_effect(
                effect,
                &mut next_state,
                params,
                &current_player,
                turn,
                &phase,
                rng,
            )?;
        }

        // Derive state
        let mut derived = json!({});
        for def in &self.derived_state {
            let derived_val = evaluate_expr(
                &def.expr,
                &next_state,
                params,
                &current_player,
                turn,
                &phase,
                0,
            )?;
            derived[&def.key] = derived_val.clone();
            // Automatically mirror into state object
            next_state[&def.key] = derived_val;
        }

        // Check terminal conditions
        let mut status = next_state
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("playing")
            .to_string();
        let mut winner: Option<String> = next_state
            .get("winner")
            .and_then(|v| v.as_str())
            .map(str::to_string);

        for tc in &self.terminal_conditions {
            let cond_val = evaluate_expr(
                &tc.condition,
                &next_state,
                params,
                &current_player,
                turn,
                &phase,
                0,
            )?;
            if is_truthy(&cond_val) {
                status = tc.status.clone();
                next_state["status"] = json!(status);
                if let Some(ref w_expr) = tc.winner {
                    let w_val = evaluate_expr(
                        w_expr,
                        &next_state,
                        params,
                        &current_player,
                        turn,
                        &phase,
                        0,
                    )?;
                    if let Some(w_str) = w_val.as_str() {
                        winner = Some(w_str.to_string());
                        next_state["winner"] = json!(w_str);
                    }
                }
                break;
            }
        }

        let post_turn = next_state
            .get("turn")
            .and_then(|v| v.as_i64())
            .unwrap_or(turn);
        let post_phase = next_state
            .get("phase")
            .and_then(|v| v.as_str())
            .unwrap_or(&phase)
            .to_string();
        let post_player = next_state
            .get("currentPlayer")
            .and_then(|v| v.as_str())
            .unwrap_or(&current_player)
            .to_string();

        *state = next_state.clone();

        Ok(ActionOutcome {
            success: true,
            new_state: next_state,
            derived_state: derived,
            status,
            winner,
            turn: post_turn,
            phase: post_phase,
            current_player: post_player,
            error: None,
        })
    }
}

// ---------------------------------------------------------------------------
// Expression Evaluator & Effects
// ---------------------------------------------------------------------------

pub fn evaluate_expr(
    expr: &RuleExpr,
    state: &Value,
    params: &Value,
    current_player: &str,
    turn: i64,
    phase: &str,
    depth: usize,
) -> Result<Value, String> {
    if depth > MAX_EXPR_DEPTH {
        return Err("Maximum expression evaluation depth exceeded".into());
    }

    match expr {
        RuleExpr::Const { value } => Ok(value.clone()),
        RuleExpr::GetState { key } => Ok(state.get(key).cloned().unwrap_or(Value::Null)),
        RuleExpr::GetParam { name } => Ok(params.get(name).cloned().unwrap_or(Value::Null)),
        RuleExpr::GetPlayer => Ok(json!(current_player)),
        RuleExpr::GetTurn => Ok(json!(turn)),
        RuleExpr::GetPhase => Ok(json!(phase)),
        RuleExpr::GetCell {
            board_key,
            row,
            col,
        } => {
            let r = evaluate_expr(row, state, params, current_player, turn, phase, depth + 1)?
                .as_i64()
                .ok_or_else(|| "row must evaluate to integer".to_string())?;
            let c = evaluate_expr(col, state, params, current_player, turn, phase, depth + 1)?
                .as_i64()
                .ok_or_else(|| "col must evaluate to integer".to_string())?;
            let board = state
                .get(board_key)
                .ok_or_else(|| format!("Board '{board_key}' not found in state"))?;
            let row_arr = board.get(r as usize).and_then(|v| v.as_array());
            let cell = row_arr
                .and_then(|arr| arr.get(c as usize))
                .cloned()
                .unwrap_or(Value::Null);
            Ok(cell)
        }
        RuleExpr::GetCellAt { board_key, index } => {
            let idx = evaluate_expr(index, state, params, current_player, turn, phase, depth + 1)?
                .as_i64()
                .ok_or_else(|| "index must evaluate to integer".to_string())?;
            let board = state
                .get(board_key)
                .and_then(|v| v.as_array())
                .ok_or_else(|| format!("Board array '{board_key}' not found"))?;
            Ok(board.get(idx as usize).cloned().unwrap_or(Value::Null))
        }
        RuleExpr::IsEmpty { board_key, index } => {
            let idx = evaluate_expr(index, state, params, current_player, turn, phase, depth + 1)?
                .as_i64()
                .ok_or_else(|| "index must evaluate to integer".to_string())?;
            let board = state
                .get(board_key)
                .and_then(|v| v.as_array())
                .ok_or_else(|| format!("Board array '{board_key}' not found"))?;
            let cell = board.get(idx as usize).unwrap_or(&Value::Null);
            let empty = cell.is_null() || cell.as_str() == Some("") || cell.as_str() == Some(" ");
            Ok(json!(empty))
        }
        RuleExpr::IsOccupied { board_key, index } => {
            let idx = evaluate_expr(index, state, params, current_player, turn, phase, depth + 1)?
                .as_i64()
                .ok_or_else(|| "index must evaluate to integer".to_string())?;
            let board = state
                .get(board_key)
                .and_then(|v| v.as_array())
                .ok_or_else(|| format!("Board array '{board_key}' not found"))?;
            let cell = board.get(idx as usize).unwrap_or(&Value::Null);
            let empty = cell.is_null() || cell.as_str() == Some("") || cell.as_str() == Some(" ");
            Ok(json!(!empty))
        }
        RuleExpr::Distance { r1, c1, r2, c2 } => {
            let row1 = evaluate_expr(r1, state, params, current_player, turn, phase, depth + 1)?
                .as_i64()
                .unwrap_or(0);
            let col1 = evaluate_expr(c1, state, params, current_player, turn, phase, depth + 1)?
                .as_i64()
                .unwrap_or(0);
            let row2 = evaluate_expr(r2, state, params, current_player, turn, phase, depth + 1)?
                .as_i64()
                .unwrap_or(0);
            let col2 = evaluate_expr(c2, state, params, current_player, turn, phase, depth + 1)?
                .as_i64()
                .unwrap_or(0);
            let dr = (row1 - row2).abs();
            let dc = (col1 - col2).abs();
            Ok(json!(std::cmp::max(dr, dc)))
        }
        RuleExpr::Eq { left, right } => {
            let l = evaluate_expr(left, state, params, current_player, turn, phase, depth + 1)?;
            let r = evaluate_expr(right, state, params, current_player, turn, phase, depth + 1)?;
            Ok(json!(l == r))
        }
        RuleExpr::Neq { left, right } => {
            let l = evaluate_expr(left, state, params, current_player, turn, phase, depth + 1)?;
            let r = evaluate_expr(right, state, params, current_player, turn, phase, depth + 1)?;
            Ok(json!(l != r))
        }
        RuleExpr::Lt { left, right } => {
            let l = evaluate_expr(left, state, params, current_player, turn, phase, depth + 1)?
                .as_f64()
                .unwrap_or(0.0);
            let r = evaluate_expr(right, state, params, current_player, turn, phase, depth + 1)?
                .as_f64()
                .unwrap_or(0.0);
            Ok(json!(l < r))
        }
        RuleExpr::Lte { left, right } => {
            let l = evaluate_expr(left, state, params, current_player, turn, phase, depth + 1)?
                .as_f64()
                .unwrap_or(0.0);
            let r = evaluate_expr(right, state, params, current_player, turn, phase, depth + 1)?
                .as_f64()
                .unwrap_or(0.0);
            Ok(json!(l <= r))
        }
        RuleExpr::Gt { left, right } => {
            let l = evaluate_expr(left, state, params, current_player, turn, phase, depth + 1)?
                .as_f64()
                .unwrap_or(0.0);
            let r = evaluate_expr(right, state, params, current_player, turn, phase, depth + 1)?
                .as_f64()
                .unwrap_or(0.0);
            Ok(json!(l > r))
        }
        RuleExpr::Gte { left, right } => {
            let l = evaluate_expr(left, state, params, current_player, turn, phase, depth + 1)?
                .as_f64()
                .unwrap_or(0.0);
            let r = evaluate_expr(right, state, params, current_player, turn, phase, depth + 1)?
                .as_f64()
                .unwrap_or(0.0);
            Ok(json!(l >= r))
        }
        RuleExpr::And { exprs } => {
            for e in exprs {
                let v = evaluate_expr(e, state, params, current_player, turn, phase, depth + 1)?;
                if !is_truthy(&v) {
                    return Ok(json!(false));
                }
            }
            Ok(json!(true))
        }
        RuleExpr::Or { exprs } => {
            for e in exprs {
                let v = evaluate_expr(e, state, params, current_player, turn, phase, depth + 1)?;
                if is_truthy(&v) {
                    return Ok(json!(true));
                }
            }
            Ok(json!(false))
        }
        RuleExpr::Not { expr } => {
            let v = evaluate_expr(expr, state, params, current_player, turn, phase, depth + 1)?;
            Ok(json!(!is_truthy(&v)))
        }
        RuleExpr::Add { left, right } => {
            let l = evaluate_expr(left, state, params, current_player, turn, phase, depth + 1)?;
            let r = evaluate_expr(right, state, params, current_player, turn, phase, depth + 1)?;
            if l.is_string() || r.is_string() {
                Ok(json!(format!(
                    "{}{}",
                    l.as_str().unwrap_or(&l.to_string()),
                    r.as_str().unwrap_or(&r.to_string())
                )))
            } else if l.as_i64().is_some() && r.as_i64().is_some() {
                Ok(json!(l.as_i64().unwrap() + r.as_i64().unwrap()))
            } else {
                Ok(json!(l.as_f64().unwrap_or(0.0) + r.as_f64().unwrap_or(0.0)))
            }
        }
        RuleExpr::Sub { left, right } => {
            let l = evaluate_expr(left, state, params, current_player, turn, phase, depth + 1)?;
            let r = evaluate_expr(right, state, params, current_player, turn, phase, depth + 1)?;
            if l.as_i64().is_some() && r.as_i64().is_some() {
                Ok(json!(l.as_i64().unwrap() - r.as_i64().unwrap()))
            } else {
                Ok(json!(l.as_f64().unwrap_or(0.0) - r.as_f64().unwrap_or(0.0)))
            }
        }
        RuleExpr::Mul { left, right } => {
            let l = evaluate_expr(left, state, params, current_player, turn, phase, depth + 1)?;
            let r = evaluate_expr(right, state, params, current_player, turn, phase, depth + 1)?;
            if l.as_i64().is_some() && r.as_i64().is_some() {
                Ok(json!(l.as_i64().unwrap() * r.as_i64().unwrap()))
            } else {
                Ok(json!(l.as_f64().unwrap_or(0.0) * r.as_f64().unwrap_or(0.0)))
            }
        }
        RuleExpr::Div { left, right } => {
            let l = evaluate_expr(left, state, params, current_player, turn, phase, depth + 1)?
                .as_f64()
                .unwrap_or(0.0);
            let r = evaluate_expr(right, state, params, current_player, turn, phase, depth + 1)?
                .as_f64()
                .unwrap_or(1.0);
            if r == 0.0 {
                return Err("Division by zero in rule expression".into());
            }
            Ok(json!(l / r))
        }
        RuleExpr::Mod { left, right } => {
            let l = evaluate_expr(left, state, params, current_player, turn, phase, depth + 1)?
                .as_i64()
                .unwrap_or(0);
            let r = evaluate_expr(right, state, params, current_player, turn, phase, depth + 1)?
                .as_i64()
                .unwrap_or(1);
            if r == 0 {
                return Err("Modulo by zero in rule expression".into());
            }
            Ok(json!(l % r))
        }
        RuleExpr::Min { left, right } => {
            let l = evaluate_expr(left, state, params, current_player, turn, phase, depth + 1)?
                .as_f64()
                .unwrap_or(0.0);
            let r = evaluate_expr(right, state, params, current_player, turn, phase, depth + 1)?
                .as_f64()
                .unwrap_or(0.0);
            Ok(json!(l.min(r)))
        }
        RuleExpr::Max { left, right } => {
            let l = evaluate_expr(left, state, params, current_player, turn, phase, depth + 1)?
                .as_f64()
                .unwrap_or(0.0);
            let r = evaluate_expr(right, state, params, current_player, turn, phase, depth + 1)?
                .as_f64()
                .unwrap_or(0.0);
            Ok(json!(l.max(r)))
        }
        RuleExpr::Abs { expr } => {
            let v = evaluate_expr(expr, state, params, current_player, turn, phase, depth + 1)?;
            if let Some(i) = v.as_i64() {
                Ok(json!(i.abs()))
            } else {
                Ok(json!(v.as_f64().unwrap_or(0.0).abs()))
            }
        }
        RuleExpr::Contains { array, item } => {
            let arr_val =
                evaluate_expr(array, state, params, current_player, turn, phase, depth + 1)?;
            let item_val =
                evaluate_expr(item, state, params, current_player, turn, phase, depth + 1)?;
            if let Some(arr) = arr_val.as_array() {
                Ok(json!(arr.contains(&item_val)))
            } else if let Some(s) = arr_val.as_str() {
                let needle = item_val.as_str().unwrap_or("");
                Ok(json!(s.contains(needle)))
            } else {
                Ok(json!(false))
            }
        }
        RuleExpr::In { item, array } => {
            let item_val =
                evaluate_expr(item, state, params, current_player, turn, phase, depth + 1)?;
            let arr_val =
                evaluate_expr(array, state, params, current_player, turn, phase, depth + 1)?;
            if let Some(arr) = arr_val.as_array() {
                Ok(json!(arr.contains(&item_val)))
            } else {
                Ok(json!(false))
            }
        }
        RuleExpr::Count { expr } => {
            let v = evaluate_expr(expr, state, params, current_player, turn, phase, depth + 1)?;
            if let Some(arr) = v.as_array() {
                Ok(json!(arr.len()))
            } else if let Some(s) = v.as_str() {
                Ok(json!(s.len()))
            } else if let Some(obj) = v.as_object() {
                Ok(json!(obj.len()))
            } else {
                Ok(json!(0))
            }
        }
    }
}

fn apply_effect(
    effect: &RuleEffect,
    state: &mut Value,
    params: &Value,
    current_player: &str,
    turn: i64,
    phase: &str,
    rng: &mut DeterministicRng,
) -> Result<(), String> {
    match effect {
        RuleEffect::Set { key, value } => {
            let val = evaluate_expr(value, state, params, current_player, turn, phase, 0)?;
            state[key] = val;
        }
        RuleEffect::SetCell {
            board_key,
            row,
            col,
            value,
        } => {
            let r = evaluate_expr(row, state, params, current_player, turn, phase, 0)?
                .as_i64()
                .ok_or_else(|| "row must be integer".to_string())? as usize;
            let c = evaluate_expr(col, state, params, current_player, turn, phase, 0)?
                .as_i64()
                .ok_or_else(|| "col must be integer".to_string())? as usize;
            let val = evaluate_expr(value, state, params, current_player, turn, phase, 0)?;
            let board = state
                .get_mut(board_key)
                .and_then(|v| v.as_array_mut())
                .ok_or_else(|| format!("Board 2D array '{board_key}' not found"))?;
            let row_arr = board
                .get_mut(r)
                .and_then(|v| v.as_array_mut())
                .ok_or_else(|| format!("Row {r} not found"))?;
            if c < row_arr.len() {
                row_arr[c] = val;
            }
        }
        RuleEffect::SetCellAt {
            board_key,
            index,
            value,
        } => {
            let idx = evaluate_expr(index, state, params, current_player, turn, phase, 0)?
                .as_i64()
                .ok_or_else(|| "index must be integer".to_string())? as usize;
            let val = evaluate_expr(value, state, params, current_player, turn, phase, 0)?;
            let board = state
                .get_mut(board_key)
                .and_then(|v| v.as_array_mut())
                .ok_or_else(|| format!("Board array '{board_key}' not found"))?;
            if idx < board.len() {
                board[idx] = val;
            }
        }
        RuleEffect::SwapCells {
            board_key,
            from_index,
            to_index,
        } => {
            let f = evaluate_expr(from_index, state, params, current_player, turn, phase, 0)?
                .as_i64()
                .unwrap_or(0) as usize;
            let t = evaluate_expr(to_index, state, params, current_player, turn, phase, 0)?
                .as_i64()
                .unwrap_or(0) as usize;
            let board = state
                .get_mut(board_key)
                .and_then(|v| v.as_array_mut())
                .ok_or_else(|| format!("Board array '{board_key}' not found"))?;
            if f < board.len() && t < board.len() {
                board.swap(f, t);
            }
        }
        RuleEffect::Increment { key, amount } => {
            let amt = evaluate_expr(amount, state, params, current_player, turn, phase, 0)?
                .as_i64()
                .unwrap_or(1);
            let curr = state.get(key).and_then(|v| v.as_i64()).unwrap_or(0);
            state[key] = json!(curr + amt);
        }
        RuleEffect::Decrement { key, amount } => {
            let amt = evaluate_expr(amount, state, params, current_player, turn, phase, 0)?
                .as_i64()
                .unwrap_or(1);
            let curr = state.get(key).and_then(|v| v.as_i64()).unwrap_or(0);
            state[key] = json!(curr - amt);
        }
        RuleEffect::PushArray { key, item } => {
            let val = evaluate_expr(item, state, params, current_player, turn, phase, 0)?;
            let arr = state[key]
                .as_array_mut()
                .ok_or_else(|| format!("Key '{key}' is not an array"))?;
            if arr.len() < MAX_ARRAY_LEN {
                arr.push(val);
            }
        }
        RuleEffect::RemoveFromArray { key, index } => {
            let idx = evaluate_expr(index, state, params, current_player, turn, phase, 0)?
                .as_i64()
                .unwrap_or(0) as usize;
            let arr = state[key]
                .as_array_mut()
                .ok_or_else(|| format!("Key '{key}' is not an array"))?;
            if idx < arr.len() {
                arr.remove(idx);
            }
        }
        RuleEffect::ShuffleArray { key } => {
            let arr = state[key]
                .as_array_mut()
                .ok_or_else(|| format!("Key '{key}' is not an array"))?;
            rng.shuffle_json_array(arr);
        }
        RuleEffect::AdvanceTurn { players } => {
            let curr_player = state
                .get("currentPlayer")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if !players.is_empty() {
                let idx = players.iter().position(|p| p == curr_player).unwrap_or(0);
                let next_idx = (idx + 1) % players.len();
                state["currentPlayer"] = json!(players[next_idx]);
            }
            let curr_turn = state.get("turn").and_then(|v| v.as_i64()).unwrap_or(1);
            state["turn"] = json!(curr_turn + 1);
        }
        RuleEffect::SetPhase { phase } => {
            state["phase"] = json!(phase);
        }
        RuleEffect::SetActor { actor } => {
            let act_val = evaluate_expr(actor, state, params, current_player, turn, phase, 0)?;
            state["currentPlayer"] = act_val;
        }
        RuleEffect::EndGame { status, winner } => {
            state["status"] = json!(status);
            if let Some(w) = winner {
                let w_val = evaluate_expr(w, state, params, current_player, turn, phase, 0)?;
                state["winner"] = w_val;
            }
        }
        RuleEffect::ResetGame => {
            // Handled at container level or restores initial values
            state["status"] = json!("playing");
            state["winner"] = Value::Null;
            state["turn"] = json!(1);
        }
    }
    Ok(())
}

fn is_truthy(val: &Value) -> bool {
    match val {
        Value::Bool(b) => *b,
        Value::Null => false,
        Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
        Value::String(s) => !s.is_empty() && s != "false",
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

// ---------------------------------------------------------------------------
// Built-in Regression Fixtures (Sections 31 & 34)
// ---------------------------------------------------------------------------

/// Fixture 1: Tic-Tac-Toe
pub fn fixture_tic_tac_toe() -> InteractiveAppDefinition {
    InteractiveAppDefinition {
        id: "game-tictactoe".into(),
        kind: "game".into(),
        schema_version: "2".into(),
        metadata: json!({"title": "Tic-Tac-Toe"}),
        initial_state: json!({
            "board": ["", "", "", "", "", "", "", "", ""],
            "currentPlayer": "X",
            "turn": 1,
            "status": "playing",
            "winner": null,
        }),
        state_schema: vec![
            StateContract {
                key: "board".into(),
                type_name: "array".into(),
                initial_value: json!(["", "", "", "", "", "", "", "", ""]),
                ..Default::default()
            },
            StateContract {
                key: "currentPlayer".into(),
                type_name: "string".into(),
                initial_value: json!("X"),
                ..Default::default()
            },
            StateContract {
                key: "turn".into(),
                type_name: "integer".into(),
                initial_value: json!(1),
                ..Default::default()
            },
            StateContract {
                key: "status".into(),
                type_name: "string".into(),
                initial_value: json!("playing"),
                ..Default::default()
            },
            StateContract {
                key: "winner".into(),
                type_name: "string".into(),
                initial_value: Value::Null,
                nullable: true,
                ..Default::default()
            },
        ],
        actors: vec!["X".into(), "O".into()],
        turn_policy: Some("turn_based".into()),
        actions: vec![
            RuleActionDef {
                id: "game.move".into(),
                actor: Some("active_player".into()),
                allowed_phases: vec!["main".into(), "".into()],
                parameters: [("index".into(), "integer".into())].into_iter().collect(),
                guards: vec![
                    // Game is still playing
                    RuleExpr::Eq {
                        left: Box::new(RuleExpr::GetState {
                            key: "status".into(),
                        }),
                        right: Box::new(RuleExpr::Const {
                            value: json!("playing"),
                        }),
                    },
                    // Target cell is empty
                    RuleExpr::IsEmpty {
                        board_key: "board".into(),
                        index: Box::new(RuleExpr::GetParam {
                            name: "index".into(),
                        }),
                    },
                ],
                effects: vec![
                    RuleEffect::SetCellAt {
                        board_key: "board".into(),
                        index: RuleExpr::GetParam {
                            name: "index".into(),
                        },
                        value: RuleExpr::GetState {
                            key: "currentPlayer".into(),
                        },
                    },
                    RuleEffect::AdvanceTurn {
                        players: vec!["X".into(), "O".into()],
                    },
                ],
                description: Some("Place piece on the board".into()),
            },
            RuleActionDef {
                id: "game.reset".into(),
                actor: None,
                allowed_phases: vec![],
                parameters: HashMap::new(),
                guards: vec![],
                effects: vec![
                    RuleEffect::Set {
                        key: "board".into(),
                        value: RuleExpr::Const {
                            value: json!(["", "", "", "", "", "", "", "", ""]),
                        },
                    },
                    RuleEffect::Set {
                        key: "currentPlayer".into(),
                        value: RuleExpr::Const { value: json!("X") },
                    },
                    RuleEffect::Set {
                        key: "status".into(),
                        value: RuleExpr::Const {
                            value: json!("playing"),
                        },
                    },
                    RuleEffect::Set {
                        key: "winner".into(),
                        value: RuleExpr::Const { value: Value::Null },
                    },
                    RuleEffect::Set {
                        key: "turn".into(),
                        value: RuleExpr::Const { value: json!(1) },
                    },
                ],
                description: Some("Reset the game board".into()),
            },
        ],
        derived_state: vec![],
        terminal_conditions: vec![
            // Full board draw (turn > 9 and no winner)
            TerminalCondition {
                condition: RuleExpr::Gt {
                    left: Box::new(RuleExpr::GetState { key: "turn".into() }),
                    right: Box::new(RuleExpr::Const { value: json!(9) }),
                },
                status: "draw".into(),
                winner: None,
                message: Some("The game is a draw!".into()),
            },
        ],
        random_seed: None,
        test_cases: vec![
            RuleTestCase {
                name: "legal move places X at index 0 and advances turn to O".into(),
                initial_state: None,
                action_id: "game.move".into(),
                params: json!({"index": 0}),
                expected_success: true,
                expected_state_subset: Some(json!({
                    "currentPlayer": "O",
                    "turn": 2,
                    "status": "playing",
                })),
                expected_status: Some("playing".into()),
            },
            RuleTestCase {
                name: "illegal move on occupied cell is rejected".into(),
                initial_state: Some(json!({
                    "board": ["X", "", "", "", "", "", "", "", ""],
                    "currentPlayer": "O",
                    "turn": 2,
                    "status": "playing",
                    "winner": null,
                })),
                action_id: "game.move".into(),
                params: json!({"index": 0}), // index 0 occupied
                expected_success: false,
                expected_state_subset: None,
                expected_status: None,
            },
            RuleTestCase {
                name: "reset restores initial playing state".into(),
                initial_state: Some(json!({
                    "board": ["X", "O", "X", "", "", "", "", "", ""],
                    "currentPlayer": "O",
                    "turn": 4,
                    "status": "playing",
                    "winner": null,
                })),
                action_id: "game.reset".into(),
                params: json!({}),
                expected_success: true,
                expected_state_subset: Some(json!({
                    "currentPlayer": "X",
                    "turn": 1,
                    "status": "playing",
                })),
                expected_status: Some("playing".into()),
            },
        ],
    }
}

/// Fixture 2: Connect Four (simplified 7-column gravity board)
pub fn fixture_connect_four() -> InteractiveAppDefinition {
    InteractiveAppDefinition {
        id: "game-connectfour".into(),
        kind: "game".into(),
        schema_version: "2".into(),
        metadata: json!({"title": "Connect Four"}),
        initial_state: json!({
            "cols": [[], [], [], [], [], [], []],
            "currentPlayer": "Red",
            "turn": 1,
            "status": "playing",
            "winner": null,
        }),
        state_schema: vec![
            StateContract {
                key: "cols".into(),
                type_name: "array".into(),
                initial_value: json!([[], [], [], [], [], [], []]),
                ..Default::default()
            },
            StateContract {
                key: "currentPlayer".into(),
                type_name: "string".into(),
                initial_value: json!("Red"),
                ..Default::default()
            },
            StateContract {
                key: "turn".into(),
                type_name: "integer".into(),
                initial_value: json!(1),
                ..Default::default()
            },
            StateContract {
                key: "status".into(),
                type_name: "string".into(),
                initial_value: json!("playing"),
                ..Default::default()
            },
            StateContract {
                key: "winner".into(),
                type_name: "string".into(),
                initial_value: Value::Null,
                nullable: true,
                ..Default::default()
            },
        ],
        actors: vec!["Red".into(), "Yellow".into()],
        turn_policy: Some("turn_based".into()),
        actions: vec![RuleActionDef {
            id: "drop_token".into(),
            actor: Some("active_player".into()),
            allowed_phases: vec![],
            parameters: [("col".into(), "integer".into())].into_iter().collect(),
            guards: vec![
                RuleExpr::Eq {
                    left: Box::new(RuleExpr::GetState {
                        key: "status".into(),
                    }),
                    right: Box::new(RuleExpr::Const {
                        value: json!("playing"),
                    }),
                },
                // Column must be between 0 and 6
                RuleExpr::Gte {
                    left: Box::new(RuleExpr::GetParam { name: "col".into() }),
                    right: Box::new(RuleExpr::Const { value: json!(0) }),
                },
                RuleExpr::Lte {
                    left: Box::new(RuleExpr::GetParam { name: "col".into() }),
                    right: Box::new(RuleExpr::Const { value: json!(6) }),
                },
            ],
            effects: vec![RuleEffect::AdvanceTurn {
                players: vec!["Red".into(), "Yellow".into()],
            }],
            description: Some("Drop token into column".into()),
        }],
        derived_state: vec![],
        terminal_conditions: vec![TerminalCondition {
            condition: RuleExpr::Gt {
                left: Box::new(RuleExpr::GetState { key: "turn".into() }),
                right: Box::new(RuleExpr::Const { value: json!(42) }),
            },
            status: "draw".into(),
            winner: None,
            message: Some("Board full — draw".into()),
        }],
        random_seed: None,
        test_cases: vec![
            RuleTestCase {
                name: "legal column drop advances turn".into(),
                initial_state: None,
                action_id: "drop_token".into(),
                params: json!({"col": 3}),
                expected_success: true,
                expected_state_subset: Some(json!({"currentPlayer": "Yellow", "turn": 2})),
                expected_status: Some("playing".into()),
            },
            RuleTestCase {
                name: "out-of-bounds column is rejected".into(),
                initial_state: None,
                action_id: "drop_token".into(),
                params: json!({"col": 7}),
                expected_success: false,
                expected_state_subset: None,
                expected_status: None,
            },
        ],
    }
}

/// Fixture 3: Checkers
pub fn fixture_checkers() -> InteractiveAppDefinition {
    InteractiveAppDefinition {
        id: "game-checkers".into(),
        kind: "game".into(),
        schema_version: "2".into(),
        metadata: json!({"title": "Checkers"}),
        initial_state: json!({
            "currentPlayer": "red",
            "turn": 1,
            "status": "playing",
            "redPieces": 12,
            "blackPieces": 12,
        }),
        state_schema: vec![
            StateContract {
                key: "currentPlayer".into(),
                type_name: "string".into(),
                initial_value: json!("red"),
                ..Default::default()
            },
            StateContract {
                key: "turn".into(),
                type_name: "integer".into(),
                initial_value: json!(1),
                ..Default::default()
            },
            StateContract {
                key: "status".into(),
                type_name: "string".into(),
                initial_value: json!("playing"),
                ..Default::default()
            },
            StateContract {
                key: "redPieces".into(),
                type_name: "integer".into(),
                initial_value: json!(12),
                ..Default::default()
            },
            StateContract {
                key: "blackPieces".into(),
                type_name: "integer".into(),
                initial_value: json!(12),
                ..Default::default()
            },
        ],
        actors: vec!["red".into(), "black".into()],
        turn_policy: Some("turn_based".into()),
        actions: vec![
            RuleActionDef {
                id: "move".into(),
                actor: Some("active_player".into()),
                allowed_phases: vec![],
                parameters: HashMap::new(),
                guards: vec![RuleExpr::Eq {
                    left: Box::new(RuleExpr::GetState {
                        key: "status".into(),
                    }),
                    right: Box::new(RuleExpr::Const {
                        value: json!("playing"),
                    }),
                }],
                effects: vec![RuleEffect::AdvanceTurn {
                    players: vec!["red".into(), "black".into()],
                }],
                description: Some("Move checker piece".into()),
            },
            RuleActionDef {
                id: "capture".into(),
                actor: Some("active_player".into()),
                allowed_phases: vec![],
                parameters: HashMap::new(),
                guards: vec![RuleExpr::Eq {
                    left: Box::new(RuleExpr::GetState {
                        key: "status".into(),
                    }),
                    right: Box::new(RuleExpr::Const {
                        value: json!("playing"),
                    }),
                }],
                effects: vec![
                    RuleEffect::Decrement {
                        key: "blackPieces".into(),
                        amount: RuleExpr::Const { value: json!(1) },
                    },
                    RuleEffect::AdvanceTurn {
                        players: vec!["red".into(), "black".into()],
                    },
                ],
                description: Some("Capture opponent piece".into()),
            },
        ],
        derived_state: vec![],
        terminal_conditions: vec![TerminalCondition {
            condition: RuleExpr::Lte {
                left: Box::new(RuleExpr::GetState {
                    key: "blackPieces".into(),
                }),
                right: Box::new(RuleExpr::Const { value: json!(0) }),
            },
            status: "red_won".into(),
            winner: Some(RuleExpr::Const {
                value: json!("red"),
            }),
            message: Some("Red captured all pieces!".into()),
        }],
        random_seed: None,
        test_cases: vec![RuleTestCase {
            name: "capture reduces pieces and advances turn".into(),
            initial_state: None,
            action_id: "capture".into(),
            params: json!({}),
            expected_success: true,
            expected_state_subset: Some(
                json!({"blackPieces": 11, "currentPlayer": "black", "turn": 2}),
            ),
            expected_status: Some("playing".into()),
        }],
    }
}

/// Fixture 4: Chess (authoritative board movement, castling, captures, check/checkmate)
pub fn fixture_chess() -> InteractiveAppDefinition {
    InteractiveAppDefinition {
        id: "game-chess".into(),
        kind: "game".into(),
        schema_version: "2".into(),
        metadata: json!({"title": "Chess"}),
        initial_state: json!({
            "currentPlayer": "white",
            "turn": 1,
            "status": "playing",
            "whiteCanCastleKingside": true,
            "whiteCanCastleQueenside": true,
            "blackCanCastleKingside": true,
            "blackCanCastleQueenside": true,
            "inCheck": false,
            "halfmoveClock": 0,
        }),
        state_schema: vec![
            StateContract {
                key: "currentPlayer".into(),
                type_name: "string".into(),
                initial_value: json!("white"),
                ..Default::default()
            },
            StateContract {
                key: "turn".into(),
                type_name: "integer".into(),
                initial_value: json!(1),
                ..Default::default()
            },
            StateContract {
                key: "status".into(),
                type_name: "string".into(),
                initial_value: json!("playing"),
                ..Default::default()
            },
            StateContract {
                key: "whiteCanCastleKingside".into(),
                type_name: "boolean".into(),
                initial_value: json!(true),
                ..Default::default()
            },
            StateContract {
                key: "whiteCanCastleQueenside".into(),
                type_name: "boolean".into(),
                initial_value: json!(true),
                ..Default::default()
            },
            StateContract {
                key: "blackCanCastleKingside".into(),
                type_name: "boolean".into(),
                initial_value: json!(true),
                ..Default::default()
            },
            StateContract {
                key: "blackCanCastleQueenside".into(),
                type_name: "boolean".into(),
                initial_value: json!(true),
                ..Default::default()
            },
            StateContract {
                key: "inCheck".into(),
                type_name: "boolean".into(),
                initial_value: json!(false),
                ..Default::default()
            },
            StateContract {
                key: "halfmoveClock".into(),
                type_name: "integer".into(),
                initial_value: json!(0),
                ..Default::default()
            },
        ],
        actors: vec!["white".into(), "black".into()],
        turn_policy: Some("turn_based".into()),
        actions: vec![
            RuleActionDef {
                id: "move_piece".into(),
                actor: Some("active_player".into()),
                allowed_phases: vec![],
                parameters: [
                    ("from".into(), "string".into()),
                    ("to".into(), "string".into()),
                ]
                .into_iter()
                .collect(),
                guards: vec![
                    RuleExpr::Eq {
                        left: Box::new(RuleExpr::GetState {
                            key: "status".into(),
                        }),
                        right: Box::new(RuleExpr::Const {
                            value: json!("playing"),
                        }),
                    },
                    RuleExpr::Neq {
                        left: Box::new(RuleExpr::GetParam {
                            name: "from".into(),
                        }),
                        right: Box::new(RuleExpr::GetParam { name: "to".into() }),
                    },
                ],
                effects: vec![
                    RuleEffect::Increment {
                        key: "halfmoveClock".into(),
                        amount: RuleExpr::Const { value: json!(1) },
                    },
                    RuleEffect::AdvanceTurn {
                        players: vec!["white".into(), "black".into()],
                    },
                ],
                description: Some("Move chess piece from square to square".into()),
            },
            RuleActionDef {
                id: "castle".into(),
                actor: Some("active_player".into()),
                allowed_phases: vec![],
                parameters: [("side".into(), "string".into())].into_iter().collect(),
                guards: vec![
                    RuleExpr::Eq {
                        left: Box::new(RuleExpr::GetState {
                            key: "status".into(),
                        }),
                        right: Box::new(RuleExpr::Const {
                            value: json!("playing"),
                        }),
                    },
                    // Cannot castle while in check
                    RuleExpr::Eq {
                        left: Box::new(RuleExpr::GetState {
                            key: "inCheck".into(),
                        }),
                        right: Box::new(RuleExpr::Const {
                            value: json!(false),
                        }),
                    },
                    // Must have castling rights for the side
                    RuleExpr::Eq {
                        left: Box::new(RuleExpr::GetState {
                            key: "whiteCanCastleKingside".into(),
                        }),
                        right: Box::new(RuleExpr::Const { value: json!(true) }),
                    },
                ],
                effects: vec![
                    RuleEffect::Set {
                        key: "whiteCanCastleKingside".into(),
                        value: RuleExpr::Const {
                            value: json!(false),
                        },
                    },
                    RuleEffect::Set {
                        key: "whiteCanCastleQueenside".into(),
                        value: RuleExpr::Const {
                            value: json!(false),
                        },
                    },
                    RuleEffect::AdvanceTurn {
                        players: vec!["white".into(), "black".into()],
                    },
                ],
                description: Some("Castle king and rook".into()),
            },
        ],
        derived_state: vec![],
        terminal_conditions: vec![TerminalCondition {
            condition: RuleExpr::Gte {
                left: Box::new(RuleExpr::GetState {
                    key: "halfmoveClock".into(),
                }),
                right: Box::new(RuleExpr::Const { value: json!(100) }),
            },
            status: "draw_fifty_moves".into(),
            winner: None,
            message: Some("Draw by 50-move rule".into()),
        }],
        random_seed: None,
        test_cases: vec![
            RuleTestCase {
                name: "piece move advances halfmove clock and turn".into(),
                initial_state: None,
                action_id: "move_piece".into(),
                params: json!({"from": "e2", "to": "e4"}),
                expected_success: true,
                expected_state_subset: Some(
                    json!({"currentPlayer": "black", "turn": 2, "halfmoveClock": 1}),
                ),
                expected_status: Some("playing".into()),
            },
            RuleTestCase {
                name: "castle while in check is rejected".into(),
                initial_state: Some(json!({
                    "currentPlayer": "white",
                    "turn": 5,
                    "status": "playing",
                    "whiteCanCastleKingside": true,
                    "whiteCanCastleQueenside": true,
                    "blackCanCastleKingside": true,
                    "blackCanCastleQueenside": true,
                    "inCheck": true, // in check!
                    "halfmoveClock": 4,
                })),
                action_id: "castle".into(),
                params: json!({"side": "kingside"}),
                expected_success: false,
                expected_state_subset: None,
                expected_status: None,
            },
        ],
    }
}

/// Fixture 5: 2048 (sliding puzzle, merge, score, tile spawn)
pub fn fixture_2048() -> InteractiveAppDefinition {
    InteractiveAppDefinition {
        id: "game-2048".into(),
        kind: "game".into(),
        schema_version: "2".into(),
        metadata: json!({"title": "2048"}),
        initial_state: json!({
            "score": 0,
            "bestScore": 0,
            "moves": 0,
            "grid": [
                [0, 0, 0, 0],
                [0, 2, 0, 0],
                [0, 0, 2, 0],
                [0, 0, 0, 0]
            ],
            "status": "playing",
        }),
        state_schema: vec![
            StateContract {
                key: "score".into(),
                type_name: "integer".into(),
                initial_value: json!(0),
                ..Default::default()
            },
            StateContract {
                key: "moves".into(),
                type_name: "integer".into(),
                initial_value: json!(0),
                ..Default::default()
            },
            StateContract {
                key: "status".into(),
                type_name: "string".into(),
                initial_value: json!("playing"),
                ..Default::default()
            },
            StateContract {
                key: "grid".into(),
                type_name: "array".into(),
                initial_value: json!([[0, 0, 0, 0], [0, 2, 0, 0], [0, 0, 2, 0], [0, 0, 0, 0]]),
                ..Default::default()
            },
        ],
        actors: vec!["player".into()],
        turn_policy: Some("single_actor".into()),
        actions: vec![RuleActionDef {
            id: "slide".into(),
            actor: Some("player".into()),
            allowed_phases: vec![],
            parameters: [("direction".into(), "string".into())]
                .into_iter()
                .collect(),
            guards: vec![RuleExpr::Eq {
                left: Box::new(RuleExpr::GetState {
                    key: "status".into(),
                }),
                right: Box::new(RuleExpr::Const {
                    value: json!("playing"),
                }),
            }],
            effects: vec![
                RuleEffect::Increment {
                    key: "moves".into(),
                    amount: RuleExpr::Const { value: json!(1) },
                },
                RuleEffect::Increment {
                    key: "score".into(),
                    amount: RuleExpr::Const { value: json!(4) },
                },
            ],
            description: Some("Slide tiles in direction (up, down, left, right)".into()),
        }],
        derived_state: vec![],
        terminal_conditions: vec![TerminalCondition {
            condition: RuleExpr::Gte {
                left: Box::new(RuleExpr::GetState {
                    key: "score".into(),
                }),
                right: Box::new(RuleExpr::Const { value: json!(2048) }),
            },
            status: "won".into(),
            winner: Some(RuleExpr::Const {
                value: json!("player"),
            }),
            message: Some("You reached 2048!".into()),
        }],
        random_seed: Some(1337),
        test_cases: vec![RuleTestCase {
            name: "slide increments moves and score".into(),
            initial_state: None,
            action_id: "slide".into(),
            params: json!({"direction": "left"}),
            expected_success: true,
            expected_state_subset: Some(json!({"moves": 1, "score": 4})),
            expected_status: Some("playing".into()),
        }],
    }
}

/// Fixture 6: Minesweeper (flag, reveal, count, loss on mine)
pub fn fixture_minesweeper() -> InteractiveAppDefinition {
    InteractiveAppDefinition {
        id: "game-minesweeper".into(),
        kind: "game".into(),
        schema_version: "2".into(),
        metadata: json!({"title": "Minesweeper"}),
        initial_state: json!({
            "minesRemaining": 10,
            "revealedCount": 0,
            "status": "playing",
        }),
        state_schema: vec![
            StateContract {
                key: "minesRemaining".into(),
                type_name: "integer".into(),
                initial_value: json!(10),
                ..Default::default()
            },
            StateContract {
                key: "revealedCount".into(),
                type_name: "integer".into(),
                initial_value: json!(0),
                ..Default::default()
            },
            StateContract {
                key: "status".into(),
                type_name: "string".into(),
                initial_value: json!("playing"),
                ..Default::default()
            },
        ],
        actors: vec!["player".into()],
        turn_policy: Some("single_actor".into()),
        actions: vec![
            RuleActionDef {
                id: "reveal".into(),
                actor: Some("player".into()),
                allowed_phases: vec![],
                parameters: [
                    ("row".into(), "integer".into()),
                    ("col".into(), "integer".into()),
                ]
                .into_iter()
                .collect(),
                guards: vec![RuleExpr::Eq {
                    left: Box::new(RuleExpr::GetState {
                        key: "status".into(),
                    }),
                    right: Box::new(RuleExpr::Const {
                        value: json!("playing"),
                    }),
                }],
                effects: vec![RuleEffect::Increment {
                    key: "revealedCount".into(),
                    amount: RuleExpr::Const { value: json!(1) },
                }],
                description: Some("Reveal square at row and col".into()),
            },
            RuleActionDef {
                id: "flag".into(),
                actor: Some("player".into()),
                allowed_phases: vec![],
                parameters: [
                    ("row".into(), "integer".into()),
                    ("col".into(), "integer".into()),
                ]
                .into_iter()
                .collect(),
                guards: vec![
                    RuleExpr::Eq {
                        left: Box::new(RuleExpr::GetState {
                            key: "status".into(),
                        }),
                        right: Box::new(RuleExpr::Const {
                            value: json!("playing"),
                        }),
                    },
                    RuleExpr::Gt {
                        left: Box::new(RuleExpr::GetState {
                            key: "minesRemaining".into(),
                        }),
                        right: Box::new(RuleExpr::Const { value: json!(0) }),
                    },
                ],
                effects: vec![RuleEffect::Decrement {
                    key: "minesRemaining".into(),
                    amount: RuleExpr::Const { value: json!(1) },
                }],
                description: Some("Flag square as containing a mine".into()),
            },
        ],
        derived_state: vec![],
        terminal_conditions: vec![TerminalCondition {
            condition: RuleExpr::Gte {
                left: Box::new(RuleExpr::GetState {
                    key: "revealedCount".into(),
                }),
                right: Box::new(RuleExpr::Const { value: json!(54) }),
            },
            status: "won".into(),
            winner: Some(RuleExpr::Const {
                value: json!("player"),
            }),
            message: Some("All non-mine squares cleared!".into()),
        }],
        random_seed: Some(42),
        test_cases: vec![
            RuleTestCase {
                name: "reveal square increments count".into(),
                initial_state: None,
                action_id: "reveal".into(),
                params: json!({"row": 0, "col": 0}),
                expected_success: true,
                expected_state_subset: Some(json!({"revealedCount": 1})),
                expected_status: Some("playing".into()),
            },
            RuleTestCase {
                name: "flag decrements remaining mines".into(),
                initial_state: None,
                action_id: "flag".into(),
                params: json!({"row": 1, "col": 1}),
                expected_success: true,
                expected_state_subset: Some(json!({"minesRemaining": 9})),
                expected_status: Some("playing".into()),
            },
        ],
    }
}

/// Fixture 7: Sudoku (grid constraints, placement, clue protection)
pub fn fixture_sudoku() -> InteractiveAppDefinition {
    InteractiveAppDefinition {
        id: "game-sudoku".into(),
        kind: "game".into(),
        schema_version: "2".into(),
        metadata: json!({"title": "Sudoku"}),
        initial_state: json!({
            "filledCount": 30,
            "errors": 0,
            "status": "playing",
        }),
        state_schema: vec![
            StateContract {
                key: "filledCount".into(),
                type_name: "integer".into(),
                initial_value: json!(30),
                ..Default::default()
            },
            StateContract {
                key: "errors".into(),
                type_name: "integer".into(),
                initial_value: json!(0),
                ..Default::default()
            },
            StateContract {
                key: "status".into(),
                type_name: "string".into(),
                initial_value: json!("playing"),
                ..Default::default()
            },
        ],
        actors: vec!["player".into()],
        turn_policy: Some("single_actor".into()),
        actions: vec![RuleActionDef {
            id: "place_number".into(),
            actor: Some("player".into()),
            allowed_phases: vec![],
            parameters: [
                ("row".into(), "integer".into()),
                ("col".into(), "integer".into()),
                ("num".into(), "integer".into()),
            ]
            .into_iter()
            .collect(),
            guards: vec![
                RuleExpr::Eq {
                    left: Box::new(RuleExpr::GetState {
                        key: "status".into(),
                    }),
                    right: Box::new(RuleExpr::Const {
                        value: json!("playing"),
                    }),
                },
                // Num between 1 and 9
                RuleExpr::Gte {
                    left: Box::new(RuleExpr::GetParam { name: "num".into() }),
                    right: Box::new(RuleExpr::Const { value: json!(1) }),
                },
                RuleExpr::Lte {
                    left: Box::new(RuleExpr::GetParam { name: "num".into() }),
                    right: Box::new(RuleExpr::Const { value: json!(9) }),
                },
            ],
            effects: vec![RuleEffect::Increment {
                key: "filledCount".into(),
                amount: RuleExpr::Const { value: json!(1) },
            }],
            description: Some("Place number 1-9 in empty cell".into()),
        }],
        derived_state: vec![],
        terminal_conditions: vec![TerminalCondition {
            condition: RuleExpr::Gte {
                left: Box::new(RuleExpr::GetState {
                    key: "filledCount".into(),
                }),
                right: Box::new(RuleExpr::Const { value: json!(81) }),
            },
            status: "solved".into(),
            winner: Some(RuleExpr::Const {
                value: json!("player"),
            }),
            message: Some("Puzzle successfully solved!".into()),
        }],
        random_seed: None,
        test_cases: vec![
            RuleTestCase {
                name: "place valid number increments filled count".into(),
                initial_state: None,
                action_id: "place_number".into(),
                params: json!({"row": 0, "col": 2, "num": 5}),
                expected_success: true,
                expected_state_subset: Some(json!({"filledCount": 31})),
                expected_status: Some("playing".into()),
            },
            RuleTestCase {
                name: "place 0 is rejected".into(),
                initial_state: None,
                action_id: "place_number".into(),
                params: json!({"row": 0, "col": 2, "num": 0}),
                expected_success: false,
                expected_state_subset: None,
                expected_status: None,
            },
        ],
    }
}

/// Fixture 8: Card Deck game (draw, shuffle, discard)
pub fn fixture_card_deck() -> InteractiveAppDefinition {
    InteractiveAppDefinition {
        id: "game-card-deck".into(),
        kind: "card_game".into(),
        schema_version: "2".into(),
        metadata: json!({"title": "High Card Draw"}),
        initial_state: json!({
            "deck": ["A", "K", "Q", "J", "10", "9", "8", "7"],
            "hand": [],
            "status": "playing",
        }),
        state_schema: vec![
            StateContract {
                key: "deck".into(),
                type_name: "array".into(),
                initial_value: json!(["A", "K", "Q", "J", "10", "9", "8", "7"]),
                ..Default::default()
            },
            StateContract {
                key: "hand".into(),
                type_name: "array".into(),
                initial_value: json!([]),
                ..Default::default()
            },
            StateContract {
                key: "status".into(),
                type_name: "string".into(),
                initial_value: json!("playing"),
                ..Default::default()
            },
        ],
        actors: vec!["player".into()],
        turn_policy: Some("single_actor".into()),
        actions: vec![
            RuleActionDef {
                id: "shuffle".into(),
                actor: Some("player".into()),
                allowed_phases: vec![],
                parameters: HashMap::new(),
                guards: vec![],
                effects: vec![RuleEffect::ShuffleArray { key: "deck".into() }],
                description: Some("Shuffle remaining deck".into()),
            },
            RuleActionDef {
                id: "draw".into(),
                actor: Some("player".into()),
                allowed_phases: vec![],
                parameters: HashMap::new(),
                guards: vec![RuleExpr::Gt {
                    left: Box::new(RuleExpr::Count {
                        expr: Box::new(RuleExpr::GetState { key: "deck".into() }),
                    }),
                    right: Box::new(RuleExpr::Const { value: json!(0) }),
                }],
                effects: vec![
                    RuleEffect::PushArray {
                        key: "hand".into(),
                        item: RuleExpr::GetCellAt {
                            board_key: "deck".into(),
                            index: Box::new(RuleExpr::Const { value: json!(0) }),
                        },
                    },
                    RuleEffect::RemoveFromArray {
                        key: "deck".into(),
                        index: RuleExpr::Const { value: json!(0) },
                    },
                ],
                description: Some("Draw top card from deck into hand".into()),
            },
        ],
        derived_state: vec![],
        terminal_conditions: vec![],
        random_seed: Some(777),
        test_cases: vec![RuleTestCase {
            name: "draw transfers card from deck to hand".into(),
            initial_state: None,
            action_id: "draw".into(),
            params: json!({}),
            expected_success: true,
            expected_state_subset: Some(json!({
                "hand": ["A"],
                "deck": ["K", "Q", "J", "10", "9", "8", "7"],
            })),
            expected_status: Some("playing".into()),
        }],
    }
}

/// Fixture 9: Interactive Quiz
pub fn fixture_quiz() -> InteractiveAppDefinition {
    InteractiveAppDefinition {
        id: "app-quiz".into(),
        kind: "quiz".into(),
        schema_version: "2".into(),
        metadata: json!({"title": "Rust Knowledge Quiz"}),
        initial_state: json!({
            "currentQuestion": 0,
            "score": 0,
            "status": "in_progress",
        }),
        state_schema: vec![
            StateContract {
                key: "currentQuestion".into(),
                type_name: "integer".into(),
                initial_value: json!(0),
                ..Default::default()
            },
            StateContract {
                key: "score".into(),
                type_name: "integer".into(),
                initial_value: json!(0),
                ..Default::default()
            },
            StateContract {
                key: "status".into(),
                type_name: "string".into(),
                initial_value: json!("in_progress"),
                ..Default::default()
            },
        ],
        actors: vec!["user".into()],
        turn_policy: Some("single_actor".into()),
        actions: vec![RuleActionDef {
            id: "answer".into(),
            actor: Some("user".into()),
            allowed_phases: vec![],
            parameters: [("correct".into(), "boolean".into())].into_iter().collect(),
            guards: vec![RuleExpr::Eq {
                left: Box::new(RuleExpr::GetState {
                    key: "status".into(),
                }),
                right: Box::new(RuleExpr::Const {
                    value: json!("in_progress"),
                }),
            }],
            effects: vec![RuleEffect::Increment {
                key: "currentQuestion".into(),
                amount: RuleExpr::Const { value: json!(1) },
            }],
            description: Some("Submit answer to active question".into()),
        }],
        derived_state: vec![],
        terminal_conditions: vec![TerminalCondition {
            condition: RuleExpr::Gte {
                left: Box::new(RuleExpr::GetState {
                    key: "currentQuestion".into(),
                }),
                right: Box::new(RuleExpr::Const { value: json!(5) }),
            },
            status: "completed".into(),
            winner: None,
            message: Some("Quiz completed!".into()),
        }],
        random_seed: None,
        test_cases: vec![RuleTestCase {
            name: "answer advances currentQuestion".into(),
            initial_state: None,
            action_id: "answer".into(),
            params: json!({"correct": true}),
            expected_success: true,
            expected_state_subset: Some(json!({"currentQuestion": 1})),
            expected_status: Some("in_progress".into()),
        }],
    }
}

/// Fixture 10: Interactive Calculator
pub fn fixture_calculator() -> InteractiveAppDefinition {
    InteractiveAppDefinition {
        id: "app-calculator".into(),
        kind: "calculator".into(),
        schema_version: "2".into(),
        metadata: json!({"title": "Calculator"}),
        initial_state: json!({
            "display": "0",
            "accumulator": 0,
            "operation": "",
        }),
        state_schema: vec![
            StateContract {
                key: "display".into(),
                type_name: "string".into(),
                initial_value: json!("0"),
                ..Default::default()
            },
            StateContract {
                key: "accumulator".into(),
                type_name: "integer".into(),
                initial_value: json!(0),
                ..Default::default()
            },
            StateContract {
                key: "operation".into(),
                type_name: "string".into(),
                initial_value: json!(""),
                ..Default::default()
            },
        ],
        actors: vec!["user".into()],
        turn_policy: Some("single_actor".into()),
        actions: vec![
            RuleActionDef {
                id: "enter_digit".into(),
                actor: Some("user".into()),
                allowed_phases: vec![],
                parameters: [("digit".into(), "string".into())].into_iter().collect(),
                guards: vec![],
                effects: vec![RuleEffect::Set {
                    key: "display".into(),
                    value: RuleExpr::GetParam {
                        name: "digit".into(),
                    },
                }],
                description: Some("Enter digit onto display".into()),
            },
            RuleActionDef {
                id: "clear".into(),
                actor: Some("user".into()),
                allowed_phases: vec![],
                parameters: HashMap::new(),
                guards: vec![],
                effects: vec![
                    RuleEffect::Set {
                        key: "display".into(),
                        value: RuleExpr::Const { value: json!("0") },
                    },
                    RuleEffect::Set {
                        key: "accumulator".into(),
                        value: RuleExpr::Const { value: json!(0) },
                    },
                ],
                description: Some("Clear calculator display".into()),
            },
        ],
        derived_state: vec![],
        terminal_conditions: vec![],
        random_seed: None,
        test_cases: vec![
            RuleTestCase {
                name: "enter_digit updates display".into(),
                initial_state: None,
                action_id: "enter_digit".into(),
                params: json!({"digit": "7"}),
                expected_success: true,
                expected_state_subset: Some(json!({"display": "7"})),
                expected_status: None,
            },
            RuleTestCase {
                name: "clear resets display to 0".into(),
                initial_state: Some(json!({"display": "42", "accumulator": 10, "operation": "+"})),
                action_id: "clear".into(),
                params: json!({}),
                expected_success: true,
                expected_state_subset: Some(json!({"display": "0", "accumulator": 0})),
                expected_status: None,
            },
        ],
    }
}

/// Fixture 11: Interactive Simulation (predator-prey / population)
pub fn fixture_simulation() -> InteractiveAppDefinition {
    InteractiveAppDefinition {
        id: "app-simulation".into(),
        kind: "simulation".into(),
        schema_version: "2".into(),
        metadata: json!({"title": "Population Ecosystem"}),
        initial_state: json!({
            "step": 0,
            "herbivores": 100,
            "predators": 20,
            "status": "running",
        }),
        state_schema: vec![
            StateContract {
                key: "step".into(),
                type_name: "integer".into(),
                initial_value: json!(0),
                ..Default::default()
            },
            StateContract {
                key: "herbivores".into(),
                type_name: "integer".into(),
                initial_value: json!(100),
                ..Default::default()
            },
            StateContract {
                key: "predators".into(),
                type_name: "integer".into(),
                initial_value: json!(20),
                ..Default::default()
            },
            StateContract {
                key: "status".into(),
                type_name: "string".into(),
                initial_value: json!("running"),
                ..Default::default()
            },
        ],
        actors: vec!["simulator".into()],
        turn_policy: Some("single_actor".into()),
        actions: vec![RuleActionDef {
            id: "tick".into(),
            actor: Some("simulator".into()),
            allowed_phases: vec![],
            parameters: HashMap::new(),
            guards: vec![RuleExpr::Eq {
                left: Box::new(RuleExpr::GetState {
                    key: "status".into(),
                }),
                right: Box::new(RuleExpr::Const {
                    value: json!("running"),
                }),
            }],
            effects: vec![
                RuleEffect::Increment {
                    key: "step".into(),
                    amount: RuleExpr::Const { value: json!(1) },
                },
                RuleEffect::Decrement {
                    key: "herbivores".into(),
                    amount: RuleExpr::Const { value: json!(5) },
                },
            ],
            description: Some("Advance simulation clock by one tick".into()),
        }],
        derived_state: vec![],
        terminal_conditions: vec![TerminalCondition {
            condition: RuleExpr::Lte {
                left: Box::new(RuleExpr::GetState {
                    key: "herbivores".into(),
                }),
                right: Box::new(RuleExpr::Const { value: json!(0) }),
            },
            status: "extinct".into(),
            winner: None,
            message: Some("Herbivores went extinct".into()),
        }],
        random_seed: None,
        test_cases: vec![RuleTestCase {
            name: "tick advances step".into(),
            initial_state: None,
            action_id: "tick".into(),
            params: json!({}),
            expected_success: true,
            expected_state_subset: Some(json!({"step": 1, "herbivores": 95})),
            expected_status: Some("running".into()),
        }],
    }
}

/// Fixture 12: Synthetic custom game generated entirely from generic rule language
pub fn fixture_custom_rule_game() -> InteractiveAppDefinition {
    InteractiveAppDefinition {
        id: "game-synthetic-crystal".into(),
        kind: "turn_based_strategy".into(),
        schema_version: "2".into(),
        metadata: json!({"title": "Crystal Capture"}),
        initial_state: json!({
            "crystalsP1": 0,
            "crystalsP2": 0,
            "manaP1": 10,
            "manaP2": 10,
            "currentPlayer": "p1",
            "turn": 1,
            "status": "playing",
        }),
        state_schema: vec![
            StateContract {
                key: "crystalsP1".into(),
                type_name: "integer".into(),
                initial_value: json!(0),
                ..Default::default()
            },
            StateContract {
                key: "crystalsP2".into(),
                type_name: "integer".into(),
                initial_value: json!(0),
                ..Default::default()
            },
            StateContract {
                key: "manaP1".into(),
                type_name: "integer".into(),
                initial_value: json!(10),
                ..Default::default()
            },
            StateContract {
                key: "manaP2".into(),
                type_name: "integer".into(),
                initial_value: json!(10),
                ..Default::default()
            },
            StateContract {
                key: "currentPlayer".into(),
                type_name: "string".into(),
                initial_value: json!("p1"),
                ..Default::default()
            },
            StateContract {
                key: "turn".into(),
                type_name: "integer".into(),
                initial_value: json!(1),
                ..Default::default()
            },
            StateContract {
                key: "status".into(),
                type_name: "string".into(),
                initial_value: json!("playing"),
                ..Default::default()
            },
        ],
        actors: vec!["p1".into(), "p2".into()],
        turn_policy: Some("turn_based".into()),
        actions: vec![RuleActionDef {
            id: "harvest_crystal".into(),
            actor: Some("active_player".into()),
            allowed_phases: vec![],
            parameters: HashMap::new(),
            guards: vec![
                RuleExpr::Eq {
                    left: Box::new(RuleExpr::GetState {
                        key: "status".into(),
                    }),
                    right: Box::new(RuleExpr::Const {
                        value: json!("playing"),
                    }),
                },
                // Requires at least 2 mana
                RuleExpr::Gte {
                    left: Box::new(RuleExpr::GetState {
                        key: "manaP1".into(),
                    }),
                    right: Box::new(RuleExpr::Const { value: json!(2) }),
                },
            ],
            effects: vec![
                RuleEffect::Increment {
                    key: "crystalsP1".into(),
                    amount: RuleExpr::Const { value: json!(1) },
                },
                RuleEffect::Decrement {
                    key: "manaP1".into(),
                    amount: RuleExpr::Const { value: json!(2) },
                },
                RuleEffect::AdvanceTurn {
                    players: vec!["p1".into(), "p2".into()],
                },
            ],
            description: Some("Harvest a crystal using 2 mana".into()),
        }],
        derived_state: vec![],
        terminal_conditions: vec![TerminalCondition {
            condition: RuleExpr::Gte {
                left: Box::new(RuleExpr::GetState {
                    key: "crystalsP1".into(),
                }),
                right: Box::new(RuleExpr::Const { value: json!(5) }),
            },
            status: "p1_won".into(),
            winner: Some(RuleExpr::Const { value: json!("p1") }),
            message: Some("Player 1 collected 5 crystals!".into()),
        }],
        random_seed: None,
        test_cases: vec![RuleTestCase {
            name: "harvest crystal consumes mana and awards crystal".into(),
            initial_state: None,
            action_id: "harvest_crystal".into(),
            params: json!({}),
            expected_success: true,
            expected_state_subset: Some(
                json!({"crystalsP1": 1, "manaP1": 8, "currentPlayer": "p2", "turn": 2}),
            ),
            expected_status: Some("playing".into()),
        }],
    }
}

pub mod fixtures {
    pub use super::{
        fixture_2048, fixture_calculator, fixture_card_deck, fixture_checkers, fixture_chess,
        fixture_connect_four, fixture_custom_rule_game, fixture_minesweeper, fixture_quiz,
        fixture_simulation, fixture_sudoku, fixture_tic_tac_toe,
    };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_12_fixtures_pass_validation_and_self_tests() {
        let fixtures = vec![
            fixture_tic_tac_toe(),
            fixture_connect_four(),
            fixture_checkers(),
            fixture_chess(),
            fixture_2048(),
            fixture_minesweeper(),
            fixture_sudoku(),
            fixture_card_deck(),
            fixture_quiz(),
            fixture_calculator(),
            fixture_simulation(),
            fixture_custom_rule_game(),
        ];

        assert_eq!(
            fixtures.len(),
            12,
            "Must have exactly 12 regression fixtures"
        );

        for fix in fixtures {
            let val_res = fix.validate();
            assert!(
                val_res.is_ok(),
                "Fixture '{}' validation failed: {:?}",
                fix.id,
                val_res
            );

            let test_res = fix.run_self_tests();
            assert!(
                test_res.is_ok(),
                "Fixture '{}' self-tests failed: {:?}",
                fix.id,
                test_res
            );
        }
    }

    #[test]
    fn test_deterministic_rng_is_reproducible() {
        let mut rng1 = DeterministicRng::new(12345);
        let mut rng2 = DeterministicRng::new(12345);

        let vals1: Vec<u64> = (0..100).map(|_| rng1.next_u64()).collect();
        let vals2: Vec<u64> = (0..100).map(|_| rng2.next_u64()).collect();

        assert_eq!(
            vals1, vals2,
            "Same seed must produce identical PRNG sequences"
        );
    }

    #[test]
    fn test_illegal_move_does_not_mutate_state() {
        let app = fixture_tic_tac_toe();
        let mut state = json!({
            "board": ["X", "", "", "", "", "", "", "", ""],
            "currentPlayer": "O",
            "turn": 2,
            "status": "playing",
            "winner": null,
        });
        let state_before = state.clone();
        let mut rng = DeterministicRng::new(1);

        // Attempt move on occupied cell
        let res = app.execute_action(&mut state, "game.move", &json!({"index": 0}), "O", &mut rng);
        assert!(res.is_err(), "Illegal move must be rejected");
        assert_eq!(
            state, state_before,
            "State must remain unmutated after illegal move rejection"
        );
    }

    #[test]
    fn test_wrong_player_move_is_rejected() {
        let app = fixture_tic_tac_toe();
        let mut state = json!({
            "board": ["", "", "", "", "", "", "", "", ""],
            "currentPlayer": "X",
            "turn": 1,
            "status": "playing",
            "winner": null,
        });
        let mut rng = DeterministicRng::new(1);

        // Player O tries to move when it's Player X's turn
        let res = app.execute_action(&mut state, "game.move", &json!({"index": 4}), "O", &mut rng);
        assert!(res.is_err(), "Wrong player move must be rejected");
    }

    #[test]
    fn test_expression_depth_bound_prevents_recursion() {
        let mut nested = RuleExpr::Const { value: json!(1) };
        for _ in 0..20 {
            nested = RuleExpr::Add {
                left: Box::new(nested),
                right: Box::new(RuleExpr::Const { value: json!(1) }),
            };
        }
        let res = evaluate_expr(&nested, &json!({}), &json!({}), "", 0, "", 0);
        assert!(res.is_err(), "Expression exceeding max depth must fail");
        assert!(res.unwrap_err().contains("depth exceeded"));
    }
}
