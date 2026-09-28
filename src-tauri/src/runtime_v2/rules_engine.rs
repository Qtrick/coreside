//! Declarative Interactive Application & Rule Engine.
//!
//! Provides a secure, bounded, deterministic runtime for model-generated
//! interactive applications and games. The model creates the declarative
//! definition, while the Rust runtime remains authoritative over state,
//! action legality, transitions, derived metrics, and randomness.
//!
//! Transition semantics:
//! 1. Pre-state and parameters are validated against declared contracts.
//! 2. Actor, phase and guards are evaluated against the pre-action state.
//! 3. Effects run on a scratch copy; any failure aborts the whole transition
//!    (no partial state, no RNG advancement).
//! 4. Derived state and terminal conditions are evaluated on the post-action
//!    state, so `getPlayer`/`getTurn`/`getPhase` observe the new values.
//! 5. The post-state is validated before it replaces the authoritative state.

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};

use super::chess;
use super::software_document::{validate_state_value_type, StateContract};

const MAX_EXPR_DEPTH: usize = 16;
const MAX_EFFECTS_PER_ACTION: usize = 64;
const MAX_NESTED_EFFECTS: usize = 256;
const MAX_GUARDS_PER_ACTION: usize = 32;
const MAX_ACTIONS_COUNT: usize = 128;
const MAX_ACTORS: usize = 16;
const MAX_STATE_CONTRACTS: usize = 128;
const MAX_DERIVED: usize = 64;
const MAX_TERMINAL: usize = 32;
const MAX_TEST_CASES: usize = 64;
const MAX_PARAMS_PER_ACTION: usize = 16;
const MAX_ARRAY_LEN: usize = 1024;
const MAX_VALUE_DEPTH: usize = 32;
const MAX_IDENT_LEN: usize = 64;
const MAX_STATE_JSON_BYTES: usize = 256 * 1024;
const MAX_DEFINITION_JSON_BYTES: usize = 512 * 1024;

const FORBIDDEN_KEYS: [&str; 3] = ["__proto__", "constructor", "prototype"];

/// Deterministic Pseudo-Random Number Generator (SplitMix64).
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

    /// Serialize for persistence inside interactive state. `u64` values are
    /// stored as decimal strings so they survive JavaScript number precision.
    pub fn to_json(&self) -> Value {
        json!({ "seed": self.seed.to_string(), "state": self.state.to_string() })
    }

    /// Restore from persisted state. Fails closed on malformed payloads.
    pub fn from_json(value: &Value) -> Result<Self, String> {
        let field = |name: &str| -> Result<u64, String> {
            value
                .get(name)
                .and_then(Value::as_str)
                .and_then(|s| s.parse::<u64>().ok())
                .ok_or_else(|| format!("RNG {name} missing or invalid"))
        };
        if value.as_object().map(|o| o.len()) != Some(2) {
            return Err("RNG state must contain exactly seed and state".into());
        }
        Ok(Self {
            seed: field("seed")?,
            state: field("state")?,
        })
    }
}

/// Key used to persist DeterministicRng inside interactive application state.
pub const RNG_STATE_KEY: &str = "__rng";

const MAX_PARAM_STRING_LEN: usize = 4096;
const MAX_PARAM_ARRAY_LEN: usize = 256;
const MAX_PARAM_OBJECT_KEYS: usize = 64;
const MAX_PARAM_NEST_DEPTH: usize = 8;

/// Bounded typed parameter contract for interactive actions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParamContract {
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(default = "default_true")]
    pub required: bool,
    #[serde(default)]
    pub nullable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_length: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_length: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_values: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<ParamContract>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, ParamContract>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_items: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

fn default_true() -> bool {
    true
}

const PARAM_TYPES: [&str; 11] = [
    "string", "integer", "int", "number", "float", "boolean", "bool", "enum", "array", "object",
    "any",
];

impl ParamContract {
    pub fn simple(type_name: &str) -> Self {
        Self {
            type_name: type_name.into(),
            required: true,
            nullable: false,
            min: None,
            max: None,
            min_length: None,
            max_length: None,
            allowed_values: None,
            items: None,
            properties: None,
            max_items: None,
            description: None,
        }
    }

    fn validate_shape(&self, name: &str, depth: usize) -> Result<(), String> {
        if depth > MAX_PARAM_NEST_DEPTH {
            return Err(format!("Parameter '{name}' contract nests too deeply"));
        }
        let t = self.type_name.to_lowercase();
        if !PARAM_TYPES.contains(&t.as_str()) || t == "any" {
            return Err(format!(
                "Parameter '{name}' has unsupported type '{}'",
                self.type_name
            ));
        }
        if t == "enum" && self.allowed_values.as_ref().map_or(true, Vec::is_empty) {
            return Err(format!("Enum parameter '{name}' requires allowedValues"));
        }
        if let (Some(min), Some(max)) = (self.min, self.max) {
            if !(min.is_finite() && max.is_finite()) || min > max {
                return Err(format!("Parameter '{name}' has invalid min/max"));
            }
        }
        if self.max_length.unwrap_or(0) > MAX_PARAM_STRING_LEN
            || self.max_items.unwrap_or(0) > MAX_PARAM_ARRAY_LEN
            || self.allowed_values.as_ref().map_or(0, Vec::len) > MAX_PARAM_ARRAY_LEN
        {
            return Err(format!("Parameter '{name}' bounds exceed runtime limits"));
        }
        if let Some(items) = &self.items {
            items.validate_shape(&format!("{name}[]"), depth + 1)?;
        }
        if let Some(props) = &self.properties {
            if props.len() > MAX_PARAM_OBJECT_KEYS {
                return Err(format!("Parameter '{name}' declares too many properties"));
            }
            for (k, c) in props {
                validate_ident(k, "parameter property")?;
                c.validate_shape(&format!("{name}.{k}"), depth + 1)?;
            }
        }
        Ok(())
    }
}

/// Parameter specification: legacy type-name string or full contract.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ParamSpec {
    TypeName(String),
    Contract(ParamContract),
}

impl From<&str> for ParamSpec {
    fn from(t: &str) -> Self {
        ParamSpec::TypeName(t.to_string())
    }
}

impl ParamSpec {
    pub fn as_contract(&self) -> ParamContract {
        match self {
            ParamSpec::TypeName(t) => ParamContract::simple(t),
            ParamSpec::Contract(c) => c.clone(),
        }
    }
}

/// Validate action params against declared contracts. Rejects unknown keys.
pub fn validate_action_params(
    schema: &HashMap<String, ParamSpec>,
    params: &Value,
) -> Result<(), String> {
    let empty = Map::new();
    let obj = match params {
        Value::Object(m) => m,
        Value::Null => &empty,
        _ => return Err("Action parameters must be an object".into()),
    };
    for key in obj.keys() {
        if !schema.contains_key(key) {
            return Err(format!("Unknown action parameter '{key}'"));
        }
    }
    for (name, spec) in schema {
        let contract = spec.as_contract();
        match obj.get(name) {
            None if contract.required => {
                return Err(format!("Missing required parameter '{name}'"))
            }
            None => {}
            Some(val) => validate_param_value(name, val, &contract, 0)?,
        }
    }
    Ok(())
}

fn validate_param_value(
    name: &str,
    value: &Value,
    contract: &ParamContract,
    depth: usize,
) -> Result<(), String> {
    if depth > MAX_PARAM_NEST_DEPTH {
        return Err(format!("Parameter '{name}' exceeds max nesting depth"));
    }
    if value.is_null() {
        if contract.nullable {
            return Ok(());
        }
        return Err(format!("Parameter '{name}' cannot be null"));
    }
    let check_range = |n: f64| -> Result<(), String> {
        if contract.min.is_some_and(|min| n < min) {
            return Err(format!("Parameter '{name}' below minimum"));
        }
        if contract.max.is_some_and(|max| n > max) {
            return Err(format!("Parameter '{name}' above maximum"));
        }
        Ok(())
    };

    match contract.type_name.to_lowercase().as_str() {
        "string" => {
            let s = value
                .as_str()
                .ok_or_else(|| format!("Parameter '{name}' must be a string"))?;
            let len = s.chars().count();
            if len > contract.max_length.unwrap_or(MAX_PARAM_STRING_LEN) {
                return Err(format!("Parameter '{name}' string too long"));
            }
            if contract.min_length.is_some_and(|min| len < min) {
                return Err(format!("Parameter '{name}' string too short"));
            }
        }
        "integer" | "int" => {
            let n = value
                .as_i64()
                .ok_or_else(|| format!("Parameter '{name}' must be an integer"))?;
            check_range(n as f64)?;
        }
        "number" | "float" => {
            let n = value
                .as_f64()
                .ok_or_else(|| format!("Parameter '{name}' must be a number"))?;
            check_range(n)?;
        }
        "boolean" | "bool" => {
            if !value.is_boolean() {
                return Err(format!("Parameter '{name}' must be a boolean"));
            }
        }
        "enum" => {}
        "array" => {
            let arr = value
                .as_array()
                .ok_or_else(|| format!("Parameter '{name}' must be an array"))?;
            if arr.len() > contract.max_items.unwrap_or(MAX_PARAM_ARRAY_LEN) {
                return Err(format!("Parameter '{name}' array too large"));
            }
            for (i, item) in arr.iter().enumerate() {
                match &contract.items {
                    Some(items) => {
                        validate_param_value(&format!("{name}[{i}]"), item, items, depth + 1)?
                    }
                    None => check_value_bounds(item, depth + 1)?,
                }
            }
        }
        "object" => {
            let obj = value
                .as_object()
                .ok_or_else(|| format!("Parameter '{name}' must be an object"))?;
            if obj.len() > MAX_PARAM_OBJECT_KEYS {
                return Err(format!("Parameter '{name}' object has too many keys"));
            }
            let props = contract
                .properties
                .as_ref()
                .ok_or_else(|| format!("Object parameter '{name}' must declare properties"))?;
            for key in obj.keys() {
                if !props.contains_key(key) {
                    return Err(format!("Unknown property '{key}' in parameter '{name}'"));
                }
            }
            for (pkey, pcontract) in props {
                match obj.get(pkey) {
                    None if pcontract.required => {
                        return Err(format!(
                            "Missing required property '{pkey}' in parameter '{name}'"
                        ));
                    }
                    None => {}
                    Some(v) => {
                        validate_param_value(&format!("{name}.{pkey}"), v, pcontract, depth + 1)?
                    }
                }
            }
        }
        other => {
            return Err(format!("Unsupported parameter type '{other}' for '{name}'"));
        }
    }

    if let Some(ref allowed) = contract.allowed_values {
        if !allowed.iter().any(|a| a == value) {
            return Err(format!("Parameter '{name}' not in allowedValues"));
        }
    }
    Ok(())
}

fn validate_ident(s: &str, what: &str) -> Result<(), String> {
    let ok = !s.is_empty()
        && s.len() <= MAX_IDENT_LEN
        && s.chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
        && !s.starts_with("__")
        && !FORBIDDEN_KEYS.contains(&s);
    if ok {
        Ok(())
    } else {
        Err(format!("Invalid {what} identifier '{s}'"))
    }
}

/// Reject prototype-pollution keys, excessive depth and oversized arrays.
fn check_value_bounds(value: &Value, depth: usize) -> Result<(), String> {
    if depth > MAX_VALUE_DEPTH {
        return Err("Value nesting exceeds runtime limit".into());
    }
    match value {
        Value::Array(a) => {
            if a.len() > MAX_ARRAY_LEN {
                return Err("Array exceeds runtime length limit".into());
            }
            a.iter().try_for_each(|v| check_value_bounds(v, depth + 1))
        }
        Value::Object(o) => {
            if let Some(k) = o.keys().find(|k| FORBIDDEN_KEYS.contains(&k.as_str())) {
                return Err(format!("Forbidden object key '{k}'"));
            }
            o.values()
                .try_for_each(|v| check_value_bounds(v, depth + 1))
        }
        _ => Ok(()),
    }
}

/// Bounded, typed declarative expression AST.
/// Free of arbitrary code, eval, filesystem, or network access.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(
    tag = "op",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
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
    /// The actor invoking the action.
    GetActor,
    /// `state.currentPlayer` of the state being evaluated.
    GetPlayer,
    GetTurn,
    GetPhase,
    GetField {
        expr: Box<RuleExpr>,
        field: String,
    },
    GetIndex {
        expr: Box<RuleExpr>,
        index: Box<RuleExpr>,
    },
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
    If {
        condition: Box<RuleExpr>,
        then: Box<RuleExpr>,
        #[serde(rename = "else")]
        otherwise: Box<RuleExpr>,
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
    /// Parse a numeric string (or pass through a number). Fails on malformed input.
    ToNumber {
        expr: Box<RuleExpr>,
    },
    Concat {
        exprs: Vec<RuleExpr>,
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

/// Typed, bounded state transitions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(
    tag = "effect",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
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
    /// Uniform integer in `[min, max]` from the persisted deterministic RNG.
    SetRandomInt {
        key: String,
        min: RuleExpr,
        max: RuleExpr,
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
    /// Restore the declared application baseline (state, actor, turn, RNG, derived).
    ResetGame,
    If {
        condition: RuleExpr,
        then: Vec<RuleEffect>,
        #[serde(rename = "else", default)]
        otherwise: Vec<RuleEffect>,
    },
    /// Trusted chess capability: apply a legal move to the chess object at `key`
    /// and mirror `status`, `winner` and `currentPlayer`.
    ChessMove {
        key: String,
        from: RuleExpr,
        to: RuleExpr,
        #[serde(default)]
        promotion: Option<RuleExpr>,
    },
    /// Claim a FIDE optional draw (threefold / fifty-move) when claimable.
    ChessClaimDraw {
        key: String,
    },
    /// Resign for the side to move.
    ChessResign {
        key: String,
    },
    /// Trusted 2048 capability: slide/merge the square grid at `key`, spawn a
    /// tile from the persisted RNG, and update `scoreKey` and `status`.
    Grid2048Slide {
        key: String,
        direction: RuleExpr,
        #[serde(default)]
        score_key: Option<String>,
    },
}

/// Action contract in the declarative rules engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuleActionDef {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_phases: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub parameters: HashMap<String, ParamSpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub guards: Vec<RuleExpr>,
    #[serde(default)]
    pub effects: Vec<RuleEffect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Terminal condition declaration (e.g. checkmate, 4-in-a-row, puzzle solved).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalCondition {
    pub condition: RuleExpr,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub winner: Option<RuleExpr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Derived state computation rule, evaluated on every committed state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DerivedStateDef {
    pub key: String,
    pub expr: RuleExpr,
}

/// Self-test case embedded in the application definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuleTestCase {
    pub name: String,
    /// Partial state merged over the instance baseline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_state: Option<Value>,
    pub action_id: String,
    #[serde(default)]
    pub params: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    pub expected_success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_state_subset: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_status: Option<String>,
}

/// Summary of an action currently available to an actor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LegalActionSummary {
    pub action_id: String,
    pub description: Option<String>,
    pub parameters: HashMap<String, ParamContract>,
    /// True when guards depend on parameters, so legality is decided per invocation.
    pub requires_parameters: bool,
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
    pub turn: Option<i64>,
    pub phase: Option<String>,
    pub current_player: Option<String>,
    pub message: Option<String>,
}

/// Canonical declarative interactive application definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
    /// Actors controlled by the model. Their turns are proposed by the model and
    /// validated here; the user cannot act on their behalf.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ai_actors: Vec<String>,
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

/// Engine-managed keys that may appear without an explicit contract.
fn reserved_key_contract(key: &str) -> Option<(&'static str, bool)> {
    match key {
        "status" | "phase" | "currentPlayer" => Some(("string", false)),
        "winner" => Some(("string", true)),
        "turn" => Some(("integer", false)),
        _ => None,
    }
}

struct Ctx<'a> {
    params: &'a Value,
    actor: &'a str,
}

struct EffectEnv<'a> {
    rng: Option<DeterministicRng>,
    baseline: &'a Value,
    budget: usize,
}

// ---------------------------------------------------------------------------
// Definition validation & execution
// ---------------------------------------------------------------------------

impl InteractiveAppDefinition {
    /// Parse and fully validate a definition (structure, contracts, self-tests).
    pub fn parse_and_admit(value: &Value) -> Result<Self, String> {
        let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?.len();
        if bytes > MAX_DEFINITION_JSON_BYTES {
            return Err("Interactive definition exceeds size limit".into());
        }
        let def: Self = serde_json::from_value(value.clone())
            .map_err(|e| format!("Invalid interactive definition: {e}"))?;
        def.run_self_tests()?;
        Ok(def)
    }

    fn contract(&self, key: &str) -> Option<&StateContract> {
        self.state_schema.iter().find(|s| s.key == key)
    }

    /// State keys whose values are owned by this rules engine.
    pub fn owned_keys(&self) -> HashSet<String> {
        let mut keys: HashSet<String> = self.state_schema.iter().map(|s| s.key.clone()).collect();
        keys.extend(self.derived_state.iter().map(|d| d.key.clone()));
        if let Some(obj) = self.initial_state.as_object() {
            keys.extend(obj.keys().cloned());
        }
        for k in [
            "status",
            "winner",
            "turn",
            "phase",
            "currentPlayer",
            RNG_STATE_KEY,
        ] {
            keys.insert(k.to_string());
        }
        keys
    }

    /// Validate definition structure, contracts, bounds, and action uniqueness.
    pub fn validate(&self) -> Result<(), String> {
        validate_ident(&self.id, "application")?;
        if self.kind.trim().is_empty() || self.kind.len() > MAX_IDENT_LEN {
            return Err("Application kind must be 1-64 characters".into());
        }
        if self.schema_version != "2" {
            return Err(format!(
                "Unsupported interactive schemaVersion '{}'",
                self.schema_version
            ));
        }
        if !(self.metadata.is_object() || self.metadata.is_null()) {
            return Err("metadata must be an object".into());
        }
        check_value_bounds(&self.metadata, 0)?;
        let size = serde_json::to_vec(self).map_err(|e| e.to_string())?.len();
        if size > MAX_DEFINITION_JSON_BYTES {
            return Err("Interactive definition exceeds size limit".into());
        }
        if self.actions.len() > MAX_ACTIONS_COUNT {
            return Err(format!(
                "Exceeded maximum actions limit of {MAX_ACTIONS_COUNT}"
            ));
        }
        if self.state_schema.len() > MAX_STATE_CONTRACTS
            || self.derived_state.len() > MAX_DERIVED
            || self.terminal_conditions.len() > MAX_TERMINAL
            || self.test_cases.len() > MAX_TEST_CASES
            || self.actors.len() > MAX_ACTORS
        {
            return Err("Interactive definition exceeds collection limits".into());
        }

        let mut state_keys = HashSet::new();
        for sc in &self.state_schema {
            validate_ident(&sc.key, "state key")?;
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
        if !self.initial_state.is_object() && !self.initial_state.is_null() {
            return Err("initialState must be an object".into());
        }
        let mut derived_keys = HashSet::new();
        for d in &self.derived_state {
            validate_ident(&d.key, "derived key")?;
            if state_keys.contains(&d.key) || !derived_keys.insert(d.key.clone()) {
                return Err(format!("Derived key '{}' collides with another key", d.key));
            }
            check_expr_shape(&d.expr, 0)?;
        }

        let mut actors = HashSet::new();
        for a in &self.actors {
            validate_ident(a, "actor")?;
            if !actors.insert(a.as_str()) {
                return Err(format!("Duplicate actor '{a}'"));
            }
        }
        for a in &self.ai_actors {
            if !actors.contains(a.as_str()) {
                return Err(format!("AI actor '{a}' is not a declared actor"));
            }
        }

        let mut action_ids = HashSet::new();
        let mut uses_rng = false;
        for act in &self.actions {
            validate_ident(&act.id, "action")?;
            if !action_ids.insert(act.id.clone()) {
                return Err(format!("Duplicate action id '{}'", act.id));
            }
            match act.actor.as_deref() {
                None | Some("active_player") => {}
                Some(a) if actors.contains(a) => {}
                Some(a) => {
                    return Err(format!(
                        "Action '{}' references undeclared actor '{a}'",
                        act.id
                    ))
                }
            }
            if act.parameters.len() > MAX_PARAMS_PER_ACTION {
                return Err(format!("Action '{}' declares too many parameters", act.id));
            }
            for (name, spec) in &act.parameters {
                validate_ident(name, "parameter")?;
                spec.as_contract().validate_shape(name, 0)?;
            }
            if act.guards.len() > MAX_GUARDS_PER_ACTION {
                return Err(format!("Action '{}' declares too many guards", act.id));
            }
            for g in &act.guards {
                check_expr_shape(g, 0)?;
            }
            if act.effects.len() > MAX_EFFECTS_PER_ACTION {
                return Err(format!(
                    "Action '{}' exceeds effect limit of {MAX_EFFECTS_PER_ACTION}",
                    act.id
                ));
            }
            let mut count = 0;
            for e in &act.effects {
                uses_rng |= check_effect_shape(e, 0, &mut count)?;
            }
        }
        if uses_rng && self.random_seed.is_none() {
            return Err("Actions using randomness require randomSeed".into());
        }
        for t in &self.terminal_conditions {
            check_expr_shape(&t.condition, 0)?;
            if let Some(w) = &t.winner {
                check_expr_shape(w, 0)?;
            }
            if t.status.trim().is_empty() || t.status.len() > MAX_IDENT_LEN {
                return Err("Terminal status must be 1-64 characters".into());
            }
        }

        let initial = self.initial_instance_state()?;
        self.validate_state(&initial)
            .map_err(|e| format!("Initial state invalid: {e}"))?;
        Ok(())
    }

    /// Strict state validation: declared keys present with correct types; unknown
    /// keys rejected; RNG payload well-formed; bounded size and nesting.
    pub fn validate_state(&self, state: &Value) -> Result<(), String> {
        let obj = state
            .as_object()
            .ok_or("Interactive state must be an object")?;
        let size = serde_json::to_vec(state).map_err(|e| e.to_string())?.len();
        if size > MAX_STATE_JSON_BYTES {
            return Err(format!(
                "State exceeds byte limit of {MAX_STATE_JSON_BYTES}"
            ));
        }
        for (k, v) in obj {
            if k == RNG_STATE_KEY {
                DeterministicRng::from_json(v)?;
                continue;
            }
            check_value_bounds(v, 1)?;
            if let Some(sc) = self.contract(k) {
                validate_state_value_type(k, v, &sc.type_name, sc.is_effective_nullable())?;
            } else if self.derived_state.iter().any(|d| &d.key == k) {
            } else if let Some((t, nullable)) = reserved_key_contract(k) {
                validate_state_value_type(k, v, t, nullable)?;
            } else {
                return Err(format!("Undeclared state key '{k}'"));
            }
        }
        for sc in &self.state_schema {
            if !obj.contains_key(&sc.key) {
                return Err(format!("Missing declared state key '{}'", sc.key));
            }
        }
        Ok(())
    }

    /// Authoritative baseline for a new instance and for `resetGame`.
    pub fn initial_instance_state(&self) -> Result<Value, String> {
        let mut state = match &self.initial_state {
            Value::Object(m) => Value::Object(m.clone()),
            _ => json!({}),
        };
        for sc in &self.state_schema {
            if state.get(&sc.key).is_none() {
                state[&sc.key] = sc.initial_value.clone();
            }
        }
        if let Some(seed) = self.random_seed {
            state[RNG_STATE_KEY] = DeterministicRng::new(seed).to_json();
        }
        self.apply_derived(&mut state, &json!({}), "")?;
        Ok(state)
    }

    fn apply_derived(
        &self,
        state: &mut Value,
        params: &Value,
        actor: &str,
    ) -> Result<Value, String> {
        let ctx = Ctx { params, actor };
        let mut derived = json!({});
        for def in &self.derived_state {
            let v = evaluate(&def.expr, state, &ctx, 0)?;
            derived[&def.key] = v.clone();
            state[&def.key] = v;
        }
        Ok(derived)
    }

    /// Resolve which actor a local user gesture acts as.
    pub fn resolve_user_actor(&self, state: &Value) -> Result<String, String> {
        if let Some(cp) = state.get("currentPlayer").and_then(Value::as_str) {
            if self.ai_actors.iter().any(|a| a == cp) {
                return Err(format!("Waiting for {cp} to move"));
            }
            return Ok(cp.to_string());
        }
        Ok(self
            .actors
            .iter()
            .find(|a| !self.ai_actors.contains(a))
            .cloned()
            .unwrap_or_else(|| "user".into()))
    }

    /// State view safe to send to the renderer or the model: strips RNG internals
    /// and keys whose contract restricts reads.
    pub fn public_view(&self, state: &Value) -> Value {
        let mut out = Map::new();
        if let Some(obj) = state.as_object() {
            for (k, v) in obj {
                if k == RNG_STATE_KEY {
                    continue;
                }
                let hidden = self.contract(k).is_some_and(|c| {
                    matches!(c.read_policy.as_str(), "restricted" | "private" | "hidden")
                        || c.sensitivity.as_deref() == Some("sensitive")
                });
                if !hidden {
                    out.insert(k.clone(), v.clone());
                }
            }
        }
        Value::Object(out)
    }
}

fn contract_hides_from_public(c: &StateContract) -> bool {
    super::visibility::contract_hides(c)
}

impl InteractiveAppDefinition {
    /// Renderer-safe projection of this definition (no seeds, tests, or hidden initials).
    pub fn renderer_view(&self) -> Value {
        super::visibility::project_interactive_definition(
            self,
            super::visibility::Audience::Renderer,
        )
    }

    /// Model-safe projection (hidden values redacted; action contracts retained).
    pub fn model_view(&self) -> Value {
        super::visibility::project_interactive_definition(self, super::visibility::Audience::Model)
    }
}

/// Reject interactive definition updates that would expose previously hidden keys.
/// Mirrors software-document Rule B for top-level `stateContracts`.
pub fn assert_interactive_read_policies_not_broadened(
    previous: &InteractiveAppDefinition,
    next: &InteractiveAppDefinition,
) -> Result<(), String> {
    for old in &previous.state_schema {
        if !contract_hides_from_public(old) {
            continue;
        }
        match next.contract(&old.key) {
            None => {
                return Err(format!(
                    "interactive replacement cannot omit protected state key '{}'",
                    old.key
                ));
            }
            Some(new_c) if !contract_hides_from_public(new_c) => {
                return Err(format!(
                    "cannot broaden interactive read policy of protected state key '{}'",
                    old.key
                ));
            }
            Some(new_c)
                if old.sensitivity.as_deref() == Some("sensitive")
                    && new_c.sensitivity.as_deref() != Some("sensitive") =>
            {
                return Err(format!(
                    "cannot remove sensitivity classification from interactive state key '{}'",
                    old.key
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

impl InteractiveAppDefinition {
    /// Run embedded rule self-tests. Fails closed if any test case fails.
    pub fn run_self_tests(&self) -> Result<(), String> {
        self.validate()?;
        let baseline = self.initial_instance_state()?;
        for tc in &self.test_cases {
            let mut state = baseline.clone();
            if let Some(Value::Object(over)) = &tc.initial_state {
                for (k, v) in over {
                    state[k] = v.clone();
                }
                self.apply_derived(&mut state, &json!({}), "")?;
            }
            let actor = match &tc.actor {
                Some(a) => a.clone(),
                None => self.resolve_user_actor(&state).or_else(|_| {
                    state
                        .get("currentPlayer")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .ok_or_else(|| "no actor".to_string())
                })?,
            };
            let result = self.execute_action(&mut state, &tc.action_id, &tc.params, &actor);
            match (tc.expected_success, result) {
                (true, Err(e)) => {
                    return Err(format!(
                        "Self-test '{}' failed: expected success, got error: {e}",
                        tc.name
                    ))
                }
                (false, Ok(_)) => {
                    return Err(format!(
                        "Self-test '{}' failed: expected failure, but action succeeded",
                        tc.name
                    ))
                }
                (false, Err(_)) => {}
                (true, Ok(outcome)) => {
                    if let Some(exp) = &tc.expected_status {
                        if &outcome.status != exp {
                            return Err(format!(
                                "Self-test '{}' failed: expected status '{exp}', got '{}'",
                                tc.name, outcome.status
                            ));
                        }
                    }
                    if let Some(Value::Object(subset)) = &tc.expected_state_subset {
                        for (k, expected) in subset {
                            if outcome.new_state.get(k) != Some(expected) {
                                return Err(format!(
                                    "Self-test '{}' failed: state mismatch for key '{k}': expected {expected}, got {:?}",
                                    tc.name,
                                    outcome.new_state.get(k)
                                ));
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn find_action(&self, action_id: &str) -> Result<&RuleActionDef, String> {
        self.actions
            .iter()
            .find(|a| a.id == action_id)
            .ok_or_else(|| format!("Action '{action_id}' not defined"))
    }

    fn actor_and_phase_allowed(&self, action: &RuleActionDef, state: &Value, actor: &str) -> bool {
        let actor_ok = match action.actor.as_deref() {
            None => true,
            Some("active_player") => {
                state.get("currentPlayer").and_then(Value::as_str) == Some(actor)
            }
            Some(required) => required == actor,
        };
        let phase = state.get("phase").and_then(Value::as_str).unwrap_or("main");
        actor_ok
            && (action.allowed_phases.is_empty()
                || action.allowed_phases.iter().any(|p| p == phase))
    }

    /// Check if a candidate action is legally permissible under current state.
    /// Returns `Err` for unknown actions or parameter contract violations.
    pub fn is_legal_action(
        &self,
        state: &Value,
        action_id: &str,
        params: &Value,
        actor: &str,
    ) -> Result<bool, String> {
        let action = self.find_action(action_id)?;
        validate_action_params(&action.parameters, params)?;
        if !self.actor_and_phase_allowed(action, state, actor) {
            return Ok(false);
        }
        let ctx = Ctx { params, actor };
        for guard in &action.guards {
            if !is_truthy(&evaluate(guard, state, &ctx, 0)?) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Actions currently available to `actor`. Actions with required parameters
    /// are listed when actor/phase allow them; their guards run per invocation.
    pub fn get_legal_actions(&self, state: &Value, actor: &str) -> Vec<LegalActionSummary> {
        self.actions
            .iter()
            .filter_map(|action| {
                let requires = action.parameters.values().any(|s| s.as_contract().required);
                let legal = if requires {
                    self.actor_and_phase_allowed(action, state, actor)
                } else {
                    self.is_legal_action(state, &action.id, &json!({}), actor)
                        .unwrap_or(false)
                };
                legal.then(|| LegalActionSummary {
                    action_id: action.id.clone(),
                    description: action.description.clone(),
                    parameters: action
                        .parameters
                        .iter()
                        .map(|(k, v)| (k.clone(), v.as_contract()))
                        .collect(),
                    requires_parameters: requires,
                })
            })
            .collect()
    }

    /// Execute an action atomically on authoritative state. On error `state` is
    /// left untouched (including the persisted RNG).
    pub fn execute_action(
        &self,
        state: &mut Value,
        action_id: &str,
        params: &Value,
        actor: &str,
    ) -> Result<ActionOutcome, String> {
        self.validate_state(state)
            .map_err(|e| format!("Corrupted interactive state: {e}"))?;
        let action = self.find_action(action_id)?;
        let params = if params.is_null() {
            json!({})
        } else {
            params.clone()
        };
        if !self.is_legal_action(state, action_id, &params, actor)? {
            return Err(format!(
                "Action '{action_id}' is not allowed in the current state"
            ));
        }

        let baseline = self.initial_instance_state()?;
        let mut env = EffectEnv {
            rng: match state.get(RNG_STATE_KEY) {
                Some(v) => Some(DeterministicRng::from_json(v)?),
                None => None,
            },
            baseline: &baseline,
            budget: MAX_NESTED_EFFECTS,
        };
        let ctx = Ctx {
            params: &params,
            actor,
        };
        let mut next = state.clone();
        for effect in &action.effects {
            apply_effect(effect, &mut next, &ctx, &mut env)?;
        }
        if let Some(rng) = &env.rng {
            next[RNG_STATE_KEY] = rng.to_json();
        }

        let derived = self.apply_derived(&mut next, &params, actor)?;
        let mut message = None;
        for tc in &self.terminal_conditions {
            if is_truthy(&evaluate(&tc.condition, &next, &ctx, 0)?) {
                next["status"] = json!(tc.status);
                if let Some(w) = &tc.winner {
                    let w_val = evaluate(w, &next, &ctx, 0)?;
                    if !(w_val.is_string() || w_val.is_null()) {
                        return Err("Terminal winner must be a string".into());
                    }
                    next["winner"] = w_val;
                }
                message = tc.message.clone();
                break;
            }
        }

        self.validate_state(&next)
            .map_err(|e| format!("Transition produced invalid state: {e}"))?;

        let outcome = ActionOutcome {
            success: true,
            status: next
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("active")
                .to_string(),
            winner: next
                .get("winner")
                .and_then(Value::as_str)
                .map(str::to_string),
            turn: next.get("turn").and_then(Value::as_i64),
            phase: next
                .get("phase")
                .and_then(Value::as_str)
                .map(str::to_string),
            current_player: next
                .get("currentPlayer")
                .and_then(Value::as_str)
                .map(str::to_string),
            message,
            derived_state: derived,
            new_state: next.clone(),
        };
        *state = next;
        Ok(outcome)
    }
}

// ---------------------------------------------------------------------------
// Static shape checks
// ---------------------------------------------------------------------------

fn check_expr_shape(expr: &RuleExpr, depth: usize) -> Result<(), String> {
    if depth > MAX_EXPR_DEPTH {
        return Err("Maximum expression depth exceeded".into());
    }
    let d = depth + 1;
    match expr {
        RuleExpr::Const { value } => check_value_bounds(value, d),
        RuleExpr::GetState { key } => validate_state_ref(key),
        RuleExpr::GetParam { name } => validate_ident(name, "parameter"),
        RuleExpr::GetActor | RuleExpr::GetPlayer | RuleExpr::GetTurn | RuleExpr::GetPhase => Ok(()),
        RuleExpr::GetField { expr, field } => {
            if FORBIDDEN_KEYS.contains(&field.as_str()) {
                return Err(format!("Forbidden field '{field}'"));
            }
            check_expr_shape(expr, d)
        }
        RuleExpr::GetCellAt { board_key, index }
        | RuleExpr::IsEmpty { board_key, index }
        | RuleExpr::IsOccupied { board_key, index } => {
            validate_state_ref(board_key)?;
            check_expr_shape(index, d)
        }
        RuleExpr::GetCell {
            board_key,
            row,
            col,
        } => {
            validate_state_ref(board_key)?;
            check_expr_shape(row, d)?;
            check_expr_shape(col, d)
        }
        RuleExpr::Distance { r1, c1, r2, c2 } => [r1, c1, r2, c2]
            .iter()
            .try_for_each(|e| check_expr_shape(e, d)),
        RuleExpr::And { exprs } | RuleExpr::Or { exprs } | RuleExpr::Concat { exprs } => {
            if exprs.len() > 32 {
                return Err("Too many operands".into());
            }
            exprs.iter().try_for_each(|e| check_expr_shape(e, d))
        }
        RuleExpr::Not { expr }
        | RuleExpr::Abs { expr }
        | RuleExpr::Count { expr }
        | RuleExpr::ToNumber { expr } => check_expr_shape(expr, d),
        RuleExpr::If {
            condition,
            then,
            otherwise,
        } => {
            check_expr_shape(condition, d)?;
            check_expr_shape(then, d)?;
            check_expr_shape(otherwise, d)
        }
        RuleExpr::GetIndex { expr: l, index: r }
        | RuleExpr::Eq { left: l, right: r }
        | RuleExpr::Neq { left: l, right: r }
        | RuleExpr::Lt { left: l, right: r }
        | RuleExpr::Lte { left: l, right: r }
        | RuleExpr::Gt { left: l, right: r }
        | RuleExpr::Gte { left: l, right: r }
        | RuleExpr::Add { left: l, right: r }
        | RuleExpr::Sub { left: l, right: r }
        | RuleExpr::Mul { left: l, right: r }
        | RuleExpr::Div { left: l, right: r }
        | RuleExpr::Mod { left: l, right: r }
        | RuleExpr::Min { left: l, right: r }
        | RuleExpr::Max { left: l, right: r }
        | RuleExpr::Contains { array: l, item: r }
        | RuleExpr::In { item: l, array: r } => {
            check_expr_shape(l, d)?;
            check_expr_shape(r, d)
        }
    }
}

fn validate_state_ref(key: &str) -> Result<(), String> {
    if reserved_key_contract(key).is_some() {
        return Ok(());
    }
    validate_ident(key, "state key")
}

/// Returns whether the effect (or a nested effect) consumes randomness.
fn check_effect_shape(
    effect: &RuleEffect,
    depth: usize,
    count: &mut usize,
) -> Result<bool, String> {
    *count += 1;
    if *count > MAX_NESTED_EFFECTS || depth > 4 {
        return Err("Effect nesting exceeds runtime limits".into());
    }
    let e = |x: &RuleExpr| check_expr_shape(x, 0);
    Ok(match effect {
        RuleEffect::Set { key, value } => {
            validate_state_ref(key)?;
            e(value)?;
            false
        }
        RuleEffect::SetCell {
            board_key,
            row,
            col,
            value,
        } => {
            validate_state_ref(board_key)?;
            e(row)?;
            e(col)?;
            e(value)?;
            false
        }
        RuleEffect::SetCellAt {
            board_key,
            index,
            value,
        } => {
            validate_state_ref(board_key)?;
            e(index)?;
            e(value)?;
            false
        }
        RuleEffect::SwapCells {
            board_key,
            from_index,
            to_index,
        } => {
            validate_state_ref(board_key)?;
            e(from_index)?;
            e(to_index)?;
            false
        }
        RuleEffect::Increment { key, amount } | RuleEffect::Decrement { key, amount } => {
            validate_state_ref(key)?;
            e(amount)?;
            false
        }
        RuleEffect::PushArray { key, item } => {
            validate_state_ref(key)?;
            e(item)?;
            false
        }
        RuleEffect::RemoveFromArray { key, index } => {
            validate_state_ref(key)?;
            e(index)?;
            false
        }
        RuleEffect::ShuffleArray { key } => {
            validate_state_ref(key)?;
            true
        }
        RuleEffect::SetRandomInt { key, min, max } => {
            validate_state_ref(key)?;
            e(min)?;
            e(max)?;
            true
        }
        RuleEffect::AdvanceTurn { players } => {
            if players.is_empty() || players.len() > MAX_ACTORS {
                return Err("advanceTurn requires 1-16 players".into());
            }
            false
        }
        RuleEffect::SetPhase { phase } => {
            validate_ident(phase, "phase")?;
            false
        }
        RuleEffect::SetActor { actor } => {
            e(actor)?;
            false
        }
        RuleEffect::EndGame { status, winner } => {
            if status.trim().is_empty() || status.len() > MAX_IDENT_LEN {
                return Err("endGame status must be 1-64 characters".into());
            }
            if let Some(w) = winner {
                e(w)?;
            }
            false
        }
        RuleEffect::ResetGame => false,
        RuleEffect::If {
            condition,
            then,
            otherwise,
        } => {
            e(condition)?;
            let mut rng = false;
            for x in then.iter().chain(otherwise.iter()) {
                rng |= check_effect_shape(x, depth + 1, count)?;
            }
            rng
        }
        RuleEffect::ChessMove {
            key,
            from,
            to,
            promotion,
        } => {
            validate_state_ref(key)?;
            e(from)?;
            e(to)?;
            if let Some(p) = promotion {
                e(p)?;
            }
            false
        }
        RuleEffect::ChessClaimDraw { key } | RuleEffect::ChessResign { key } => {
            validate_state_ref(key)?;
            false
        }
        RuleEffect::Grid2048Slide {
            key,
            direction,
            score_key,
        } => {
            validate_state_ref(key)?;
            e(direction)?;
            if let Some(s) = score_key {
                validate_state_ref(s)?;
            }
            true
        }
    })
}

// ---------------------------------------------------------------------------
// Expression Evaluator & Effects
// ---------------------------------------------------------------------------

fn num(v: &Value, what: &str) -> Result<f64, String> {
    v.as_f64()
        .ok_or_else(|| format!("{what} must be a number, got {v}"))
}

fn int(v: &Value, what: &str) -> Result<i64, String> {
    if let Some(i) = v.as_i64() {
        return Ok(i);
    }
    match v.as_f64() {
        Some(f) if f.is_finite() && f.fract() == 0.0 && f.abs() < 9.0e15 => Ok(f as i64),
        _ => Err(format!("{what} must be an integer, got {v}")),
    }
}

fn index_in(v: &Value, len: usize, what: &str) -> Result<usize, String> {
    let i = int(v, what)?;
    if i < 0 || i as usize >= len {
        return Err(format!("{what} {i} out of range (len {len})"));
    }
    Ok(i as usize)
}

fn number_value(f: f64) -> Result<Value, String> {
    if !f.is_finite() {
        return Err("Arithmetic produced a non-finite number".into());
    }
    if f.fract() == 0.0 && f.abs() < 9.0e15 {
        Ok(json!(f as i64))
    } else {
        Ok(json!(f))
    }
}

fn arith(
    l: &Value,
    r: &Value,
    op: &str,
    int_op: fn(i64, i64) -> Option<i64>,
    f_op: fn(f64, f64) -> f64,
) -> Result<Value, String> {
    if let (Some(a), Some(b)) = (l.as_i64(), r.as_i64()) {
        return int_op(a, b)
            .map(|v| json!(v))
            .ok_or_else(|| format!("Integer overflow in {op}"));
    }
    number_value(f_op(num(l, op)?, num(r, op)?))
}

fn board<'a>(state: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    state
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("Board array '{key}' not found"))
}

fn cell_is_empty(v: &Value) -> bool {
    v.is_null() || matches!(v.as_str(), Some("") | Some(" ")) || v.as_i64() == Some(0)
}

/// Public entry for evaluating a standalone expression (used by tests/tools).
pub fn evaluate_expr(
    expr: &RuleExpr,
    state: &Value,
    params: &Value,
    actor: &str,
) -> Result<Value, String> {
    evaluate(expr, state, &Ctx { params, actor }, 0)
}

fn evaluate(expr: &RuleExpr, state: &Value, ctx: &Ctx, depth: usize) -> Result<Value, String> {
    if depth > MAX_EXPR_DEPTH {
        return Err("Maximum expression evaluation depth exceeded".into());
    }
    let ev = |e: &RuleExpr| evaluate(e, state, ctx, depth + 1);
    let pair =
        |l: &RuleExpr, r: &RuleExpr| -> Result<(Value, Value), String> { Ok((ev(l)?, ev(r)?)) };

    Ok(match expr {
        RuleExpr::Const { value } => value.clone(),
        RuleExpr::GetState { key } => state.get(key).cloned().unwrap_or(Value::Null),
        RuleExpr::GetParam { name } => ctx.params.get(name).cloned().unwrap_or(Value::Null),
        RuleExpr::GetActor => json!(ctx.actor),
        RuleExpr::GetPlayer => state.get("currentPlayer").cloned().unwrap_or(Value::Null),
        RuleExpr::GetTurn => state.get("turn").cloned().unwrap_or(Value::Null),
        RuleExpr::GetPhase => state.get("phase").cloned().unwrap_or_else(|| json!("main")),
        RuleExpr::GetField { expr, field } => ev(expr)?
            .as_object()
            .ok_or("getField requires an object")?
            .get(field)
            .cloned()
            .unwrap_or(Value::Null),
        RuleExpr::GetIndex { expr, index } => {
            let arr = ev(expr)?;
            let arr = arr.as_array().ok_or("getIndex requires an array")?;
            let i = index_in(&ev(index)?, arr.len(), "index")?;
            arr[i].clone()
        }
        RuleExpr::GetCell {
            board_key,
            row,
            col,
        } => {
            let b = board(state, board_key)?;
            let r = index_in(&ev(row)?, b.len(), "row")?;
            let row_arr = b[r].as_array().ok_or("board row must be an array")?;
            let c = index_in(&ev(col)?, row_arr.len(), "col")?;
            row_arr[c].clone()
        }
        RuleExpr::GetCellAt { board_key, index } => {
            let b = board(state, board_key)?;
            b[index_in(&ev(index)?, b.len(), "index")?].clone()
        }
        RuleExpr::IsEmpty { board_key, index } => {
            let b = board(state, board_key)?;
            json!(cell_is_empty(&b[index_in(&ev(index)?, b.len(), "index")?]))
        }
        RuleExpr::IsOccupied { board_key, index } => {
            let b = board(state, board_key)?;
            json!(!cell_is_empty(&b[index_in(&ev(index)?, b.len(), "index")?]))
        }
        RuleExpr::Distance { r1, c1, r2, c2 } => {
            let v = [ev(r1)?, ev(c1)?, ev(r2)?, ev(c2)?];
            let n: Vec<i64> = v
                .iter()
                .map(|x| int(x, "distance operand"))
                .collect::<Result<_, _>>()?;
            json!((n[0] - n[2]).abs().max((n[1] - n[3]).abs()))
        }
        RuleExpr::Eq { left, right } => {
            let (l, r) = pair(left, right)?;
            json!(values_equal(&l, &r))
        }
        RuleExpr::Neq { left, right } => {
            let (l, r) = pair(left, right)?;
            json!(!values_equal(&l, &r))
        }
        RuleExpr::Lt { left, right } => {
            let (l, r) = pair(left, right)?;
            json!(num(&l, "lt")? < num(&r, "lt")?)
        }
        RuleExpr::Lte { left, right } => {
            let (l, r) = pair(left, right)?;
            json!(num(&l, "lte")? <= num(&r, "lte")?)
        }
        RuleExpr::Gt { left, right } => {
            let (l, r) = pair(left, right)?;
            json!(num(&l, "gt")? > num(&r, "gt")?)
        }
        RuleExpr::Gte { left, right } => {
            let (l, r) = pair(left, right)?;
            json!(num(&l, "gte")? >= num(&r, "gte")?)
        }
        RuleExpr::And { exprs } => {
            for e in exprs {
                if !is_truthy(&ev(e)?) {
                    return Ok(json!(false));
                }
            }
            json!(true)
        }
        RuleExpr::Or { exprs } => {
            for e in exprs {
                if is_truthy(&ev(e)?) {
                    return Ok(json!(true));
                }
            }
            json!(false)
        }
        RuleExpr::Not { expr } => json!(!is_truthy(&ev(expr)?)),
        RuleExpr::If {
            condition,
            then,
            otherwise,
        } => {
            if is_truthy(&ev(condition)?) {
                ev(then)?
            } else {
                ev(otherwise)?
            }
        }
        RuleExpr::Add { left, right } => {
            let (l, r) = pair(left, right)?;
            if l.is_string() || r.is_string() {
                json!(format!("{}{}", scalar_str(&l)?, scalar_str(&r)?))
            } else {
                arith(&l, &r, "add", i64::checked_add, |a, b| a + b)?
            }
        }
        RuleExpr::Sub { left, right } => {
            let (l, r) = pair(left, right)?;
            arith(&l, &r, "sub", i64::checked_sub, |a, b| a - b)?
        }
        RuleExpr::Mul { left, right } => {
            let (l, r) = pair(left, right)?;
            arith(&l, &r, "mul", i64::checked_mul, |a, b| a * b)?
        }
        RuleExpr::Div { left, right } => {
            let (l, r) = pair(left, right)?;
            let d = num(&r, "div")?;
            if d == 0.0 {
                return Err("Division by zero in rule expression".into());
            }
            number_value(num(&l, "div")? / d)?
        }
        RuleExpr::Mod { left, right } => {
            let (l, r) = pair(left, right)?;
            let (a, b) = (int(&l, "mod")?, int(&r, "mod")?);
            if b == 0 {
                return Err("Modulo by zero in rule expression".into());
            }
            json!(a.rem_euclid(b))
        }
        RuleExpr::Min { left, right } => {
            let (l, r) = pair(left, right)?;
            number_value(num(&l, "min")?.min(num(&r, "min")?))?
        }
        RuleExpr::Max { left, right } => {
            let (l, r) = pair(left, right)?;
            number_value(num(&l, "max")?.max(num(&r, "max")?))?
        }
        RuleExpr::Abs { expr } => number_value(num(&ev(expr)?, "abs")?.abs())?,
        RuleExpr::ToNumber { expr } => {
            let v = ev(expr)?;
            match &v {
                Value::Number(_) => v,
                Value::String(s) => {
                    let t = s.trim();
                    if t.is_empty() || t.len() > 64 {
                        return Err(format!("Cannot convert '{s}' to a number"));
                    }
                    number_value(
                        t.parse::<f64>()
                            .map_err(|_| format!("Cannot convert '{s}' to a number"))?,
                    )?
                }
                _ => return Err(format!("Cannot convert {v} to a number")),
            }
        }
        RuleExpr::Concat { exprs } => {
            let mut s = String::new();
            for e in exprs {
                s.push_str(&scalar_str(&ev(e)?)?);
            }
            if s.len() > MAX_PARAM_STRING_LEN {
                return Err("Concatenated string exceeds limit".into());
            }
            json!(s)
        }
        RuleExpr::Contains { array, item } => {
            let (a, i) = pair(array, item)?;
            match &a {
                Value::Array(arr) => json!(arr.iter().any(|x| values_equal(x, &i))),
                Value::String(s) => {
                    json!(s.contains(i.as_str().ok_or("contains needle must be a string")?))
                }
                _ => return Err("contains requires an array or string".into()),
            }
        }
        RuleExpr::In { item, array } => {
            let (i, a) = pair(item, array)?;
            json!(a
                .as_array()
                .ok_or("in requires an array")?
                .iter()
                .any(|x| values_equal(x, &i)))
        }
        RuleExpr::Count { expr } => match ev(expr)? {
            Value::Array(a) => json!(a.len()),
            Value::String(s) => json!(s.chars().count()),
            Value::Object(o) => json!(o.len()),
            other => return Err(format!("count requires a collection, got {other}")),
        },
    })
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

fn scalar_str(v: &Value) -> Result<String, String> {
    match v {
        Value::String(s) => Ok(s.clone()),
        Value::Number(n) => Ok(n.to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        _ => Err(format!("Cannot use {v} as text")),
    }
}

fn existing_array<'a>(state: &'a mut Value, key: &str) -> Result<&'a mut Vec<Value>, String> {
    state
        .get_mut(key)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| format!("State key '{key}' is not an array"))
}

fn apply_effect(
    effect: &RuleEffect,
    state: &mut Value,
    ctx: &Ctx,
    env: &mut EffectEnv,
) -> Result<(), String> {
    if env.budget == 0 {
        return Err("Effect execution budget exhausted".into());
    }
    env.budget -= 1;
    let ev = |e: &RuleExpr, s: &Value| evaluate(e, s, ctx, 0);
    match effect {
        RuleEffect::Set { key, value } => {
            let v = ev(value, state)?;
            state[key] = v;
        }
        RuleEffect::SetCell {
            board_key,
            row,
            col,
            value,
        } => {
            let (r, c, v) = (ev(row, state)?, ev(col, state)?, ev(value, state)?);
            let b = existing_array(state, board_key)?;
            let ri = index_in(&r, b.len(), "row")?;
            let row_arr = b[ri].as_array_mut().ok_or("board row must be an array")?;
            let ci = index_in(&c, row_arr.len(), "col")?;
            row_arr[ci] = v;
        }
        RuleEffect::SetCellAt {
            board_key,
            index,
            value,
        } => {
            let (i, v) = (ev(index, state)?, ev(value, state)?);
            let b = existing_array(state, board_key)?;
            let idx = index_in(&i, b.len(), "index")?;
            b[idx] = v;
        }
        RuleEffect::SwapCells {
            board_key,
            from_index,
            to_index,
        } => {
            let (f, t) = (ev(from_index, state)?, ev(to_index, state)?);
            let b = existing_array(state, board_key)?;
            let (fi, ti) = (
                index_in(&f, b.len(), "fromIndex")?,
                index_in(&t, b.len(), "toIndex")?,
            );
            b.swap(fi, ti);
        }
        RuleEffect::Increment { key, amount } | RuleEffect::Decrement { key, amount } => {
            let amt = ev(amount, state)?;
            let cur = state
                .get(key)
                .ok_or_else(|| format!("State key '{key}' does not exist"))?;
            let next = if matches!(effect, RuleEffect::Increment { .. }) {
                arith(cur, &amt, "increment", i64::checked_add, |a, b| a + b)?
            } else {
                arith(cur, &amt, "decrement", i64::checked_sub, |a, b| a - b)?
            };
            state[key] = next;
        }
        RuleEffect::PushArray { key, item } => {
            let v = ev(item, state)?;
            let arr = existing_array(state, key)?;
            if arr.len() >= MAX_ARRAY_LEN {
                return Err(format!("Array '{key}' is full"));
            }
            arr.push(v);
        }
        RuleEffect::RemoveFromArray { key, index } => {
            let i = ev(index, state)?;
            let arr = existing_array(state, key)?;
            let idx = index_in(&i, arr.len(), "index")?;
            arr.remove(idx);
        }
        RuleEffect::ShuffleArray { key } => {
            let rng = env.rng.as_mut().ok_or("Randomness requires randomSeed")?;
            rng.shuffle_json_array(existing_array(state, key)?);
        }
        RuleEffect::SetRandomInt { key, min, max } => {
            let (lo, hi) = (int(&ev(min, state)?, "min")?, int(&ev(max, state)?, "max")?);
            if lo > hi || hi - lo > 1_000_000_000 {
                return Err("setRandomInt requires min <= max within bounds".into());
            }
            let rng = env.rng.as_mut().ok_or("Randomness requires randomSeed")?;
            state[key] = json!(lo + rng.next_bounded((hi - lo + 1) as u64) as i64);
        }
        RuleEffect::AdvanceTurn { players } => {
            let cur = state
                .get("currentPlayer")
                .and_then(Value::as_str)
                .ok_or("advanceTurn requires currentPlayer")?;
            let idx = players
                .iter()
                .position(|p| p == cur)
                .ok_or_else(|| format!("currentPlayer '{cur}' is not in the turn order"))?;
            state["currentPlayer"] = json!(players[(idx + 1) % players.len()]);
            let turn = match state.get("turn") {
                Some(t) => int(t, "turn")?,
                None => 1,
            };
            state["turn"] = json!(turn.checked_add(1).ok_or("turn overflow")?);
        }
        RuleEffect::SetPhase { phase } => state["phase"] = json!(phase),
        RuleEffect::SetActor { actor } => {
            let v = ev(actor, state)?;
            if !v.is_string() {
                return Err("setActor requires a string actor".into());
            }
            state["currentPlayer"] = v;
        }
        RuleEffect::EndGame { status, winner } => {
            state["status"] = json!(status);
            if let Some(w) = winner {
                let v = ev(w, state)?;
                if !(v.is_string() || v.is_null()) {
                    return Err("endGame winner must be a string".into());
                }
                state["winner"] = v;
            }
        }
        RuleEffect::ResetGame => {
            *state = env.baseline.clone();
            env.rng = match state.get(RNG_STATE_KEY) {
                Some(v) => Some(DeterministicRng::from_json(v)?),
                None => None,
            };
        }
        RuleEffect::If {
            condition,
            then,
            otherwise,
        } => {
            let branch = if is_truthy(&ev(condition, state)?) {
                then
            } else {
                otherwise
            };
            for e in branch {
                apply_effect(e, state, ctx, env)?;
            }
        }
        RuleEffect::ChessMove {
            key,
            from,
            to,
            promotion,
        } => {
            let f = ev(from, state)?;
            let t = ev(to, state)?;
            let p = match promotion {
                Some(p) => ev(p, state)?,
                None => Value::Null,
            };
            let cur = state
                .get(key)
                .ok_or_else(|| format!("Chess state '{key}' missing"))?;
            let next = chess::apply_move(
                cur,
                f.as_str().ok_or("from must be a square")?,
                t.as_str().ok_or("to must be a square")?,
                &p,
            )?;
            state["status"] = next["status"].clone();
            state["winner"] = next["winner"].clone();
            state["currentPlayer"] = next["sideToMove"].clone();
            state[key] = next;
        }
        RuleEffect::ChessClaimDraw { key } => {
            let cur = state
                .get(key)
                .ok_or_else(|| format!("Chess state '{key}' missing"))?;
            let next = chess::claim_draw(cur)?;
            state["status"] = next["status"].clone();
            state["winner"] = next["winner"].clone();
            state[key] = next;
        }
        RuleEffect::ChessResign { key } => {
            let cur = state
                .get(key)
                .ok_or_else(|| format!("Chess state '{key}' missing"))?;
            let next = chess::resign(cur)?;
            state["status"] = next["status"].clone();
            state["winner"] = next["winner"].clone();
            state[key] = next;
        }
        RuleEffect::Grid2048Slide {
            key,
            direction,
            score_key,
        } => {
            let dir = ev(direction, state)?;
            let dir = dir
                .as_str()
                .ok_or("direction must be a string")?
                .to_string();
            let grid = parse_grid(state.get(key).ok_or("grid missing")?)?;
            let (mut next, gained) = slide_grid(&grid, &dir)?;
            if next == grid {
                return Err("Move does not change the board".into());
            }
            let rng = env.rng.as_mut().ok_or("Randomness requires randomSeed")?;
            let empties: Vec<(usize, usize)> = (0..next.len())
                .flat_map(|r| (0..next.len()).map(move |c| (r, c)))
                .filter(|&(r, c)| next[r][c] == 0)
                .collect();
            if !empties.is_empty() {
                let (r, c) = empties[rng.next_bounded(empties.len() as u64) as usize];
                next[r][c] = if rng.next_bounded(10) == 0 { 4 } else { 2 };
            }
            if let Some(sk) = score_key {
                let cur = state
                    .get(sk)
                    .ok_or_else(|| format!("State key '{sk}' does not exist"))?;
                state[sk] = arith(cur, &json!(gained), "score", i64::checked_add, |a, b| a + b)?;
            }
            let won = next.iter().flatten().any(|&v| v >= 2048);
            let can_move = ["up", "down", "left", "right"].iter().any(|d| {
                slide_grid(&next, d)
                    .map(|(g, _)| g != next)
                    .unwrap_or(false)
            });
            if won {
                state["status"] = json!("won");
            } else if !can_move {
                state["status"] = json!("lost");
            }
            state[key] = json!(next);
        }
    }
    Ok(())
}

fn parse_grid(v: &Value) -> Result<Vec<Vec<i64>>, String> {
    let rows = v.as_array().ok_or("grid must be an array")?;
    let n = rows.len();
    if !(2..=8).contains(&n) {
        return Err("grid must be 2x2 to 8x8".into());
    }
    rows.iter()
        .map(|r| {
            let r = r
                .as_array()
                .filter(|r| r.len() == n)
                .ok_or("grid must be square")?;
            r.iter()
                .map(|c| match c.as_i64() {
                    Some(x) if x == 0 || (x >= 2 && x <= 1 << 20 && x & (x - 1) == 0) => Ok(x),
                    _ => Err("grid tiles must be 0 or powers of two".to_string()),
                })
                .collect()
        })
        .collect()
}

fn slide_grid(grid: &[Vec<i64>], dir: &str) -> Result<(Vec<Vec<i64>>, i64), String> {
    let n = grid.len();
    let mut out = vec![vec![0; n]; n];
    let mut gained = 0;
    for line in 0..n {
        let coords: Vec<(usize, usize)> = (0..n)
            .map(|i| match dir {
                "left" => Ok((line, i)),
                "right" => Ok((line, n - 1 - i)),
                "up" => Ok((i, line)),
                "down" => Ok((n - 1 - i, line)),
                _ => Err(format!("Invalid direction '{dir}'")),
            })
            .collect::<Result<_, _>>()?;
        let tiles: Vec<i64> = coords
            .iter()
            .map(|&(r, c)| grid[r][c])
            .filter(|&v| v != 0)
            .collect();
        let mut merged = Vec::with_capacity(n);
        let mut i = 0;
        while i < tiles.len() {
            if i + 1 < tiles.len() && tiles[i] == tiles[i + 1] {
                merged.push(tiles[i] * 2);
                gained += tiles[i] * 2;
                i += 2;
            } else {
                merged.push(tiles[i]);
                i += 1;
            }
        }
        for (k, &(r, c)) in coords.iter().enumerate() {
            out[r][c] = merged.get(k).copied().unwrap_or(0);
        }
    }
    Ok((out, gained))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn counter_def() -> InteractiveAppDefinition {
        serde_json::from_value(json!({
            "id": "counter",
            "kind": "control",
            "stateSchema": [
                { "key": "count", "type": "integer", "initialValue": 0 },
                { "key": "items", "type": "array", "initialValue": [1, 2] }
            ],
            "actors": ["user"],
            "actions": [
                { "id": "add", "parameters": { "by": { "type": "integer", "min": 1, "max": 10 } },
                  "effects": [{ "effect": "increment", "key": "count", "amount": { "op": "getParam", "name": "by" } }] },
                { "id": "badThenGood", "effects": [
                    { "effect": "increment", "key": "count", "amount": { "op": "const", "value": 1 } },
                    { "effect": "removeFromArray", "key": "items", "index": { "op": "const", "value": 9 } }
                ] },
                { "id": "corrupt", "effects": [
                    { "effect": "set", "key": "count", "value": { "op": "const", "value": "x" } }
                ] },
                { "id": "reset", "effects": [{ "effect": "resetGame" }] }
            ]
        }))
        .unwrap()
    }

    #[test]
    fn params_are_validated_strictly() {
        let d = counter_def();
        let mut s = d.initial_instance_state().unwrap();
        for bad in [
            json!({}),
            json!({"by": 0}),
            json!({"by": 11}),
            json!({"by": 1.5}),
            json!({"by": "2"}),
            json!({"by": 1, "extra": true}),
            json!([1]),
        ] {
            assert!(
                d.execute_action(&mut s, "add", &bad, "user").is_err(),
                "{bad}"
            );
        }
        assert_eq!(s["count"], 0);
        d.execute_action(&mut s, "add", &json!({"by": 3}), "user")
            .unwrap();
        assert_eq!(s["count"], 3);
    }

    #[test]
    fn failing_effect_aborts_whole_transition() {
        let d = counter_def();
        let mut s = d.initial_instance_state().unwrap();
        let before = s.clone();
        assert!(d
            .execute_action(&mut s, "badThenGood", &json!({}), "user")
            .is_err());
        assert_eq!(s, before, "partial increment must not leak");
        assert!(d
            .execute_action(&mut s, "corrupt", &json!({}), "user")
            .is_err());
        assert_eq!(s, before, "post-state type violation must be rejected");
    }

    #[test]
    fn corrupted_or_unknown_state_is_rejected() {
        let d = counter_def();
        let mut extra = d.initial_instance_state().unwrap();
        extra["injected"] = json!(1);
        assert!(d
            .execute_action(&mut extra, "add", &json!({"by": 1}), "user")
            .is_err());
        let mut missing = json!({"items": []});
        assert!(d
            .execute_action(&mut missing, "add", &json!({"by": 1}), "user")
            .is_err());
        let mut polluted = d.initial_instance_state().unwrap();
        polluted["items"] = json!([{"__proto__": {"x": 1}}]);
        assert!(d
            .execute_action(&mut polluted, "add", &json!({"by": 1}), "user")
            .is_err());
    }

    #[test]
    fn interactive_read_policy_cannot_be_broadened_or_omitted() {
        let quiz = crate::runtime_v2::rules_fixtures::get_fixture("quiz").unwrap();
        let mut leaked = quiz.clone();
        for sc in &mut leaked.state_schema {
            if sc.key == "answers" {
                sc.read_policy = "public".into();
            }
        }
        let err = assert_interactive_read_policies_not_broadened(&quiz, &leaked).unwrap_err();
        assert!(err.contains("broaden"), "{err}");

        let mut omitted = quiz.clone();
        omitted.state_schema.retain(|sc| sc.key != "answers");
        let err = assert_interactive_read_policies_not_broadened(&quiz, &omitted).unwrap_err();
        assert!(err.contains("omit"), "{err}");

        assert!(assert_interactive_read_policies_not_broadened(&quiz, &quiz).is_ok());
    }

    #[test]
    fn deterministic_rng_is_reproducible_and_string_encoded() {
        let mut a = DeterministicRng::new(u64::MAX - 3);
        let mut b = DeterministicRng::from_json(&a.to_json()).unwrap();
        for _ in 0..50 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        assert!(a.to_json()["state"].is_string());
        assert!(DeterministicRng::from_json(&json!({"seed": 1, "state": 2})).is_err());
        assert!(DeterministicRng::from_json(&json!({"seed": "1"})).is_err());
    }

    #[test]
    fn expression_depth_bound_prevents_recursion() {
        let mut nested = RuleExpr::Const { value: json!(1) };
        for _ in 0..20 {
            nested = RuleExpr::Add {
                left: Box::new(nested),
                right: Box::new(RuleExpr::Const { value: json!(1) }),
            };
        }
        let res = evaluate_expr(&nested, &json!({}), &json!({}), "");
        assert!(res.unwrap_err().contains("depth exceeded"));
        assert!(check_expr_shape(&nested, 0).is_err());
    }

    #[test]
    fn unknown_fields_and_ops_are_rejected_at_parse() {
        let base = json!({"id": "x", "kind": "k", "actions": []});
        let mut with_extra = base.clone();
        with_extra["script"] = json!("alert(1)");
        assert!(InteractiveAppDefinition::parse_and_admit(&with_extra).is_err());
        let mut bad_op = base.clone();
        bad_op["derivedState"] = json!([{ "key": "d", "expr": { "op": "eval", "code": "1" } }]);
        assert!(InteractiveAppDefinition::parse_and_admit(&bad_op).is_err());
        let mut bad_effect = base;
        bad_effect["actions"] =
            json!([{ "id": "a", "effects": [{ "effect": "fetch", "url": "http://x" }] }]);
        assert!(InteractiveAppDefinition::parse_and_admit(&bad_effect).is_err());
    }

    fn c(v: Value) -> Value {
        json!({ "op": "const", "value": v })
    }

    fn st(k: &str) -> Value {
        json!({ "op": "getState", "key": k })
    }

    /// Definition with a fixed adversarial state and one action `a` running `effects`.
    fn probe_def(effects: Value) -> InteractiveAppDefinition {
        let full: Vec<i64> = (0..MAX_ARRAY_LEN as i64).collect();
        InteractiveAppDefinition::parse_and_admit(&json!({
            "id": "probe",
            "kind": "test",
            "stateSchema": [
                { "key": "cells", "type": "array", "initialValue": ["", "", ""] },
                { "key": "grid", "type": "array", "initialValue": [["", ""], ["", ""]] },
                { "key": "n", "type": "integer", "initialValue": 1 },
                { "key": "big", "type": "integer", "initialValue": i64::MAX },
                { "key": "label", "type": "string", "initialValue": "abc" },
                { "key": "maybe", "type": "integer", "initialValue": null, "nullable": true },
                { "key": "full", "type": "array", "initialValue": full },
                { "key": "currentPlayer", "type": "string", "initialValue": "Z" }
            ],
            "actors": ["user"],
            "randomSeed": 5,
            "actions": [{ "id": "a", "effects": effects }]
        }))
        .unwrap()
    }

    #[test]
    fn terminal_conditions_observe_the_post_action_player_turn_and_phase() {
        let d = InteractiveAppDefinition::parse_and_admit(&json!({
            "id": "turns",
            "kind": "game",
            "stateSchema": [
                { "key": "currentPlayer", "type": "string", "initialValue": "A" },
                { "key": "turn", "type": "integer", "initialValue": 1 },
                { "key": "phase", "type": "string", "initialValue": "main" },
                { "key": "status", "type": "string", "initialValue": "playing" }
            ],
            "actors": ["A", "B"],
            "actions": [
                { "id": "pass", "actor": "active_player", "effects": [{ "effect": "advanceTurn", "players": ["A", "B"] }] },
                { "id": "finish", "effects": [{ "effect": "setPhase", "phase": "end" }] }
            ],
            "derivedState": [{ "key": "mover", "expr": { "op": "getPlayer" } }],
            "terminalConditions": [
                { "condition": { "op": "eq", "left": { "op": "getPhase" }, "right": c(json!("end")) }, "status": "ended" },
                { "condition": { "op": "gte", "left": { "op": "getTurn" }, "right": c(json!(3)) },
                  "status": "turn_limit", "winner": { "op": "getPlayer" } },
                { "condition": { "op": "eq", "left": { "op": "getPlayer" }, "right": c(json!("B")) }, "status": "b_to_move" }
            ]
        }))
        .unwrap();
        let mut s = d.initial_instance_state().unwrap();
        assert_eq!(s["mover"], "A");

        let out = d.execute_action(&mut s, "pass", &json!({}), "A").unwrap();
        assert_eq!(
            out.status, "b_to_move",
            "terminal must see the new currentPlayer"
        );
        assert_eq!(
            s["mover"], "B",
            "derived state must see the new currentPlayer"
        );
        assert_eq!(out.current_player.as_deref(), Some("B"));

        let out = d.execute_action(&mut s, "pass", &json!({}), "B").unwrap();
        assert_eq!(
            out.status, "turn_limit",
            "terminal must see the new turn (3), not the old (2)"
        );
        assert_eq!(
            out.winner.as_deref(),
            Some("A"),
            "winner evaluated on post-state"
        );

        let mut s = d.initial_instance_state().unwrap();
        let out = d.execute_action(&mut s, "finish", &json!({}), "A").unwrap();
        assert_eq!(out.status, "ended", "terminal must see the new phase");
    }

    #[test]
    fn reset_restores_the_full_baseline_including_rng_and_derived_keys() {
        use super::super::rules_fixtures::get_fixture;
        type Step = (&'static str, Value);
        let cases: Vec<(&str, &str, Vec<Step>)> = vec![
            (
                "tic-tac-toe",
                "reset",
                vec![
                    ("move", json!({"index": 0})),
                    ("move", json!({"index": 4})),
                    ("move", json!({"index": 8})),
                ],
            ),
            (
                "chess",
                "reset",
                vec![
                    ("move", json!({"from": "e2", "to": "e4"})),
                    ("move", json!({"from": "e7", "to": "e5"})),
                    ("move", json!({"from": "g1", "to": "f3"})),
                ],
            ),
            (
                "2048",
                "reset",
                vec![
                    ("slide", json!({"direction": "left"})),
                    ("slide", json!({"direction": "up"})),
                    ("slide", json!({"direction": "right"})),
                ],
            ),
            (
                "card-deck",
                "reset",
                vec![
                    ("shuffle", json!({})),
                    ("draw", json!({})),
                    ("shuffle", json!({})),
                    ("discard", json!({"index": 0})),
                ],
            ),
            (
                "controls",
                "reset",
                vec![
                    ("increment", json!({})),
                    ("toggle", json!({})),
                    ("setVolume", json!({"value": 7})),
                    ("setMode", json!({"mode": "dark"})),
                    ("setNote", json!({"text": "hi"})),
                ],
            ),
            (
                "quiz",
                "restart",
                vec![
                    ("answer", json!({"choice": 1})),
                    ("answer", json!({"choice": 2})),
                ],
            ),
        ];
        for (name, reset, steps) in cases {
            let d = get_fixture(name).unwrap();
            let baseline = d.initial_instance_state().unwrap();
            let mut s = baseline.clone();
            for (action, params) in &steps {
                let actor = d.resolve_user_actor(&s).unwrap();
                d.execute_action(&mut s, action, params, &actor)
                    .unwrap_or_else(|e| panic!("{name} {action}: {e}"));
            }
            assert_ne!(s, baseline, "{name}: steps must change state");
            let actor = d.resolve_user_actor(&s).unwrap();
            d.execute_action(&mut s, reset, &json!({}), &actor).unwrap();
            assert_eq!(
                s, baseline,
                "{name}: reset must restore every declared/derived/RNG key"
            );
            if d.random_seed.is_some() {
                // Replaying the same steps after reset reproduces the same random outcomes.
                let mut again = s.clone();
                let mut fresh = baseline.clone();
                for (action, params) in &steps {
                    d.execute_action(&mut again, action, params, "player")
                        .unwrap();
                    d.execute_action(&mut fresh, action, params, "player")
                        .unwrap();
                }
                assert_eq!(again, fresh);
            }
        }
    }

    #[test]
    fn rng_is_rolled_back_when_a_later_effect_fails() {
        for effects in [
            json!([{ "effect": "shuffleArray", "key": "cells" },
                   { "effect": "removeFromArray", "key": "cells", "index": c(json!(99)) }]),
            json!([{ "effect": "setRandomInt", "key": "n", "min": c(json!(0)), "max": c(json!(100)) },
                   { "effect": "increment", "key": "label", "amount": c(json!(1)) }]),
            json!([{ "effect": "setRandomInt", "key": "n", "min": c(json!(0)), "max": c(json!(100)) },
                   { "effect": "set", "key": "n", "value": c(json!("not a number")) }]),
        ] {
            let d = probe_def(effects.clone());
            let mut s = d.initial_instance_state().unwrap();
            s["cells"] = json!(["a", "b", "c"]);
            let before = s.clone();
            assert!(
                d.execute_action(&mut s, "a", &json!({}), "user").is_err(),
                "{effects}"
            );
            assert_eq!(s, before, "state and __rng must be untouched: {effects}");
        }
    }

    #[test]
    fn out_of_range_and_ill_typed_effects_fail_closed() {
        let idx = |i: i64| c(json!(i));
        let cases = [
            json!([{ "effect": "setCellAt", "boardKey": "cells", "index": idx(3), "value": c(json!("X")) }]),
            json!([{ "effect": "setCellAt", "boardKey": "cells", "index": idx(-1), "value": c(json!("X")) }]),
            json!([{ "effect": "setCellAt", "boardKey": "cells", "index": c(json!(1.5)), "value": c(json!("X")) }]),
            json!([{ "effect": "setCell", "boardKey": "grid", "row": idx(2), "col": idx(0), "value": c(json!("X")) }]),
            json!([{ "effect": "setCell", "boardKey": "grid", "row": idx(0), "col": idx(-1), "value": c(json!("X")) }]),
            json!([{ "effect": "setCell", "boardKey": "cells", "row": idx(0), "col": idx(0), "value": c(json!("X")) }]),
            json!([{ "effect": "swapCells", "boardKey": "cells", "fromIndex": idx(0), "toIndex": idx(5) }]),
            json!([{ "effect": "swapCells", "boardKey": "cells", "fromIndex": idx(-1), "toIndex": idx(0) }]),
            json!([{ "effect": "removeFromArray", "key": "cells", "index": idx(-1) }]),
            json!([{ "effect": "removeFromArray", "key": "cells", "index": idx(3) }]),
            json!([{ "effect": "removeFromArray", "key": "label", "index": idx(0) }]),
            json!([{ "effect": "increment", "key": "label", "amount": idx(1) }]),
            json!([{ "effect": "increment", "key": "missing", "amount": idx(1) }]),
            json!([{ "effect": "increment", "key": "n", "amount": c(json!(0.5)) }]),
            json!([{ "effect": "decrement", "key": "maybe", "amount": idx(1) }]),
            json!([{ "effect": "pushArray", "key": "full", "item": idx(1) }]),
            json!([{ "effect": "pushArray", "key": "missing", "item": idx(1) }]),
            json!([{ "effect": "increment", "key": "big", "amount": idx(1) }]),
            json!([{ "effect": "set", "key": "big", "value": { "op": "mul", "left": st("big"), "right": idx(2) } }]),
            json!([{ "effect": "set", "key": "n", "value": { "op": "add", "left": st("maybe"), "right": idx(1) } }]),
            json!([{ "effect": "set", "key": "n", "value": { "op": "sub", "left": st("maybe"), "right": idx(1) } }]),
            json!([{ "effect": "set", "key": "n", "value": { "op": "toNumber", "expr": c(json!("abc")) } }]),
            json!([{ "effect": "set", "key": "n", "value": { "op": "toNumber", "expr": c(json!("")) } }]),
            json!([{ "effect": "set", "key": "n", "value": { "op": "toNumber", "expr": c(json!(null)) } }]),
            json!([{ "effect": "set", "key": "n", "value": { "op": "div", "left": idx(1), "right": idx(0) } }]),
            json!([{ "effect": "set", "key": "n", "value": { "op": "mod", "left": idx(1), "right": idx(0) } }]),
            json!([{ "effect": "set", "key": "undeclared", "value": idx(1) }]),
            json!([{ "effect": "advanceTurn", "players": ["A", "B"] }]),
            json!([{ "effect": "setActor", "actor": idx(1) }]),
            json!([{ "effect": "endGame", "status": "won", "winner": idx(1) }]),
            json!([{ "effect": "setRandomInt", "key": "n", "min": idx(5), "max": idx(1) }]),
            json!([{ "effect": "chessMove", "key": "cells", "from": c(json!("e2")), "to": c(json!("e4")) }]),
            json!([{ "effect": "grid2048Slide", "key": "cells", "direction": c(json!("left")) }]),
        ];
        for effects in cases {
            let d = probe_def(effects.clone());
            let mut s = d.initial_instance_state().unwrap();
            let before = s.clone();
            let res = d.execute_action(&mut s, "a", &json!({}), "user");
            assert!(res.is_err(), "must fail closed: {effects} -> {res:?}");
            assert_eq!(s, before, "no partial state: {effects}");
        }
    }

    #[test]
    fn prototype_pollution_keys_are_rejected_everywhere() {
        let base = || {
            json!({
                "id": "pp", "kind": "test",
                "stateSchema": [{ "key": "n", "type": "integer", "initialValue": 0 }],
                "actors": ["user"],
                "actions": [{ "id": "a", "effects": [] }]
            })
        };
        let mut bad: Vec<Value> = Vec::new();
        for k in FORBIDDEN_KEYS {
            let mut d = base();
            d["stateSchema"] = json!([{ "key": k, "type": "integer", "initialValue": 0 }]);
            bad.push(d);
            let mut d = base();
            d["actions"][0]["parameters"] = json!({ k: { "type": "integer" } });
            bad.push(d);
            let mut d = base();
            d["actions"][0]["parameters"] =
                json!({ "p": { "type": "object", "properties": { k: { "type": "integer" } } } });
            bad.push(d);
            let mut d = base();
            d["actions"][0]["guards"] = json!([c(json!({ k: { "polluted": true } }))]);
            bad.push(d);
            let mut d = base();
            d["actions"][0]["effects"] = json!([{ "effect": "set", "key": "n", "value": { "op": "getField", "expr": c(json!({})), "field": k } }]);
            bad.push(d);
            let mut d = base();
            d["actions"][0]["effects"] =
                json!([{ "effect": "set", "key": k, "value": c(json!(1)) }]);
            bad.push(d);
            let mut d = base();
            d["actions"][0]["id"] = json!(k);
            bad.push(d);
            let mut d = base();
            d["stateSchema"] =
                json!([{ "key": "blob", "type": "array", "initialValue": [{ k: 1 }] }]);
            bad.push(d);
            let mut d = base();
            d["metadata"] = json!({ "nested": { k: 1 } });
            bad.push(d);
        }
        // The RNG slot is engine-internal: not declarable, readable or writable by rules.
        let mut d = base();
        d["stateSchema"] = json!([{ "key": RNG_STATE_KEY, "type": "object", "initialValue": {} }]);
        bad.push(d);
        let mut d = base();
        d["actions"][0]["guards"] = json!([st(RNG_STATE_KEY)]);
        bad.push(d);
        let mut d = base();
        d["actions"][0]["effects"] =
            json!([{ "effect": "set", "key": RNG_STATE_KEY, "value": c(json!({})) }]);
        bad.push(d);

        for d in bad {
            assert!(
                InteractiveAppDefinition::parse_and_admit(&d).is_err(),
                "must reject: {d}"
            );
        }

        // Runtime: forbidden keys inside free-form param values and in state.
        let d = InteractiveAppDefinition::parse_and_admit(&json!({
            "id": "pp2", "kind": "test",
            "stateSchema": [{ "key": "n", "type": "integer", "initialValue": 0 }],
            "actors": ["user"],
            "actions": [{ "id": "a", "parameters": { "list": { "type": "array" } }, "effects": [] }]
        }))
        .unwrap();
        let mut s = d.initial_instance_state().unwrap();
        assert!(d
            .execute_action(&mut s, "a", &json!({"list": [{"constructor": 1}]}), "user")
            .is_err());
        assert!(d
            .execute_action(&mut s, "a", &json!({"list": [1]}), "user")
            .is_ok());
        let mut polluted = s.clone();
        polluted["prototype"] = json!(1);
        assert!(d
            .execute_action(&mut polluted, "a", &json!({"list": []}), "user")
            .is_err());
    }

    #[test]
    fn oversized_or_deeply_nested_inputs_are_rejected() {
        let big = "x".repeat(MAX_DEFINITION_JSON_BYTES + 1);
        let def = json!({ "id": "big", "kind": "k", "metadata": { "blob": big }, "actions": [] });
        assert!(InteractiveAppDefinition::parse_and_admit(&def)
            .unwrap_err()
            .contains("size limit"));

        let state_blob = "y".repeat(MAX_STATE_JSON_BYTES + 1);
        let def = json!({ "id": "bigstate", "kind": "k", "actors": ["user"], "actions": [],
            "stateSchema": [{ "key": "blob", "type": "string", "initialValue": state_blob }] });
        assert!(InteractiveAppDefinition::parse_and_admit(&def).is_err());

        let mut nested = json!(1);
        for _ in 0..(MAX_VALUE_DEPTH + 2) {
            nested = json!([nested]);
        }
        let def = json!({ "id": "deep", "kind": "k", "actors": ["user"],
            "actions": [{ "id": "a", "guards": [c(nested.clone())] }] });
        assert!(InteractiveAppDefinition::parse_and_admit(&def).is_err());
        let def = json!({ "id": "deep2", "kind": "k", "actors": ["user"], "actions": [],
            "stateSchema": [{ "key": "tree", "type": "array", "initialValue": nested }] });
        assert!(InteractiveAppDefinition::parse_and_admit(&def).is_err());

        // State growth past the byte limit is rejected post-transition, atomically.
        let chunk = "z".repeat(4000);
        let pushes: Vec<Value> = (0..MAX_EFFECTS_PER_ACTION)
            .map(|_| json!({ "effect": "pushArray", "key": "log", "item": st("chunk") }))
            .collect();
        let d = InteractiveAppDefinition::parse_and_admit(&json!({
            "id": "grow", "kind": "k", "actors": ["user"],
            "stateSchema": [
                { "key": "chunk", "type": "string", "initialValue": chunk },
                { "key": "log", "type": "array", "initialValue": [] }
            ],
            "actions": [{ "id": "spam", "effects": pushes }]
        }))
        .unwrap();
        let mut s = d.initial_instance_state().unwrap();
        d.execute_action(&mut s, "spam", &json!({}), "user")
            .unwrap();
        let before = s.clone();
        let err = d
            .execute_action(&mut s, "spam", &json!({}), "user")
            .unwrap_err();
        assert!(err.contains("byte limit"), "{err}");
        assert_eq!(s, before);
    }

    #[test]
    fn param_contracts_enforce_nesting_bounds_and_nullability() {
        let d = InteractiveAppDefinition::parse_and_admit(&json!({
            "id": "params", "kind": "k", "actors": ["user"],
            "actions": [{ "id": "a", "parameters": {
                "cfg": { "type": "object", "required": false, "properties": {
                    "size": { "type": "integer", "min": 1, "max": 3 },
                    "tag": { "type": "string", "required": false }
                } },
                "list": { "type": "array", "required": false, "maxItems": 2, "items": { "type": "integer" } },
                "opt": { "type": "string", "required": false },
                "nul": { "type": "string", "required": false, "nullable": true },
                "must": { "type": "string", "nullable": true }
            } }]
        }))
        .unwrap();
        let mut s = d.initial_instance_state().unwrap();
        let ok = |s: &mut Value, p: Value| d.execute_action(s, "a", &p, "user").map(|_| ());

        assert!(ok(&mut s, json!({"must": "x"})).is_ok());
        assert!(
            ok(&mut s, json!({"must": null})).is_ok(),
            "nullable required accepts null"
        );
        assert!(ok(&mut s, json!({})).is_err(), "required param missing");
        assert!(
            ok(&mut s, json!({"must": "x", "opt": null})).is_err(),
            "optional but not nullable"
        );
        assert!(ok(&mut s, json!({"must": "x", "nul": null})).is_ok());

        assert!(ok(&mut s, json!({"must": "x", "cfg": {"size": 2}})).is_ok());
        assert!(ok(&mut s, json!({"must": "x", "cfg": {"size": 2, "tag": "t"}})).is_ok());
        assert!(
            ok(&mut s, json!({"must": "x", "cfg": {"size": 2, "evil": 1}})).is_err(),
            "unknown nested property"
        );
        assert!(
            ok(&mut s, json!({"must": "x", "cfg": {}})).is_err(),
            "missing nested required property"
        );
        assert!(
            ok(&mut s, json!({"must": "x", "cfg": {"size": 9}})).is_err(),
            "nested bound"
        );
        assert!(ok(&mut s, json!({"must": "x", "cfg": [1]})).is_err());

        assert!(ok(&mut s, json!({"must": "x", "list": [1, 2]})).is_ok());
        assert!(
            ok(&mut s, json!({"must": "x", "list": [1, 2, 3]})).is_err(),
            "maxItems"
        );
        assert!(
            ok(&mut s, json!({"must": "x", "list": ["1"]})).is_err(),
            "item type"
        );

        let shape = |params: Value| {
            InteractiveAppDefinition::parse_and_admit(&json!({
                "id": "shape", "kind": "k", "actors": ["user"],
                "actions": [{ "id": "a", "parameters": params }]
            }))
        };
        for bad in [
            json!({ "e": { "type": "enum" } }),
            json!({ "e": { "type": "enum", "allowedValues": [] } }),
            json!({ "e": "any" }),
            json!({ "e": { "type": "any" } }),
            json!({ "e": { "type": "integer", "min": 5, "max": 1 } }),
            json!({ "e": { "type": "array", "maxItems": 100000 } }),
            json!({ "e": { "type": "string", "maxLength": 1000000 } }),
            json!({ "e": { "type": "integer", "unknownKnob": true } }),
            json!({ "e": { "type": "array", "items": { "type": "enum" } } }),
        ] {
            assert!(shape(bad.clone()).is_err(), "must reject contract {bad}");
        }
        assert!(shape(json!({ "e": { "type": "enum", "allowedValues": ["a"] } })).is_ok());
    }
}
