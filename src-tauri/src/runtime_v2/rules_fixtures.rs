//! Reference interactive applications, authored in the same JSON schema the
//! model receives. Each fixture must pass admission and its embedded self-tests.

use serde_json::{json, Value};

use super::chess;
use super::rules_engine::InteractiveAppDefinition;

pub const FIXTURE_NAMES: [&str; 10] = [
    "tic-tac-toe",
    "chess",
    "2048",
    "card-deck",
    "quiz",
    "calculator",
    "controls",
    "timer",
    "resource-sim",
    "configurator",
];

fn p(name: &str) -> Value {
    json!({ "op": "getParam", "name": name })
}
fn s(key: &str) -> Value {
    json!({ "op": "getState", "key": key })
}
fn c(value: Value) -> Value {
    json!({ "op": "const", "value": value })
}
fn eq(l: Value, r: Value) -> Value {
    json!({ "op": "eq", "left": l, "right": r })
}
fn playing(status: &str) -> Value {
    eq(s("status"), c(json!(status)))
}

fn build(v: Value) -> InteractiveAppDefinition {
    InteractiveAppDefinition::parse_and_admit(&v)
        .unwrap_or_else(|e| panic!("fixture {} invalid: {e}", v["id"]))
}

pub fn get_fixture(name: &str) -> Option<InteractiveAppDefinition> {
    Some(match name.to_ascii_lowercase().as_str() {
        "tic-tac-toe" | "tictactoe" => fixture_tic_tac_toe(),
        "chess" => fixture_chess(),
        "2048" => fixture_2048(),
        "card-deck" | "cards" => fixture_card_deck(),
        "quiz" => fixture_quiz(),
        "calculator" => fixture_calculator(),
        "controls" | "counter" => fixture_controls(),
        "timer" | "stopwatch" => fixture_timer(),
        "resource-sim" | "simulation" | "synthetic" => fixture_resource_sim(),
        "configurator" | "decision-tree" => fixture_configurator(),
        _ => return None,
    })
}

pub fn fixture_tic_tac_toe() -> InteractiveAppDefinition {
    let cell = |i: usize| json!({ "op": "getCellAt", "boardKey": "board", "index": c(json!(i)) });
    let lines = [
        [0, 1, 2],
        [3, 4, 5],
        [6, 7, 8],
        [0, 3, 6],
        [1, 4, 7],
        [2, 5, 8],
        [0, 4, 8],
        [2, 4, 6],
    ];
    let mut terminals: Vec<Value> = lines
        .iter()
        .map(|[a, b, d]| {
            json!({
                "condition": { "op": "and", "exprs": [
                    { "op": "isOccupied", "boardKey": "board", "index": c(json!(a)) },
                    eq(cell(*a), cell(*b)),
                    eq(cell(*b), cell(*d))
                ]},
                "status": "won",
                "winner": cell(*a),
                "message": "Three in a row"
            })
        })
        .collect();
    terminals.push(json!({
        "condition": { "op": "not", "expr": { "op": "contains", "array": s("board"), "item": c(json!("")) } },
        "status": "draw",
        "message": "Board full"
    }));
    build(json!({
        "id": "game-tictactoe",
        "kind": "game",
        "metadata": { "title": "Tic-Tac-Toe" },
        "stateSchema": [
            { "key": "board", "type": "array", "initialValue": ["", "", "", "", "", "", "", "", ""] },
            { "key": "currentPlayer", "type": "string", "initialValue": "X" },
            { "key": "turn", "type": "integer", "initialValue": 1 },
            { "key": "status", "type": "string", "initialValue": "playing" },
            { "key": "winner", "type": "string", "initialValue": null, "nullable": true }
        ],
        "actors": ["X", "O"],
        "turnPolicy": "turn_based",
        "actions": [
            {
                "id": "move",
                "actor": "active_player",
                "description": "Place your mark on an empty cell",
                "parameters": { "index": { "type": "integer", "min": 0, "max": 8 } },
                "guards": [playing("playing"), { "op": "isEmpty", "boardKey": "board", "index": p("index") }],
                "effects": [
                    { "effect": "setCellAt", "boardKey": "board", "index": p("index"), "value": { "op": "getPlayer" } },
                    { "effect": "advanceTurn", "players": ["X", "O"] }
                ]
            },
            { "id": "reset", "description": "Start a new game", "effects": [{ "effect": "resetGame" }] }
        ],
        "terminalConditions": terminals,
        "testCases": [
            { "name": "legal move", "actionId": "move", "params": { "index": 0 }, "expectedSuccess": true,
              "expectedStateSubset": { "currentPlayer": "O", "turn": 2 }, "expectedStatus": "playing" },
            { "name": "occupied cell rejected", "initialState": { "board": ["X", "", "", "", "", "", "", "", ""], "currentPlayer": "O", "turn": 2 },
              "actionId": "move", "params": { "index": 0 }, "expectedSuccess": false },
            { "name": "out of range rejected", "actionId": "move", "params": { "index": 9 }, "expectedSuccess": false },
            { "name": "winning move", "initialState": { "board": ["X", "X", "", "O", "O", "", "", "", ""], "turn": 5 },
              "actionId": "move", "params": { "index": 2 }, "expectedSuccess": true,
              "expectedStateSubset": { "winner": "X", "currentPlayer": "O" }, "expectedStatus": "won" },
            { "name": "no moves after win", "initialState": { "board": ["X", "X", "X", "O", "O", "", "", "", ""], "currentPlayer": "O", "status": "won", "winner": "X" },
              "actionId": "move", "params": { "index": 5 }, "expectedSuccess": false },
            { "name": "draw", "initialState": { "board": ["X", "O", "X", "X", "O", "O", "O", "X", ""], "turn": 9 },
              "actionId": "move", "params": { "index": 8 }, "expectedSuccess": true, "expectedStatus": "draw" },
            { "name": "reset restores baseline", "initialState": { "board": ["X", "O", "", "", "", "", "", "", ""], "currentPlayer": "X", "turn": 3 },
              "actionId": "reset", "expectedSuccess": true,
              "expectedStateSubset": { "board": ["", "", "", "", "", "", "", "", ""], "currentPlayer": "X", "turn": 1, "winner": null } }
        ]
    }))
}

pub fn fixture_chess() -> InteractiveAppDefinition {
    let initial = chess::initial_state_object(chess::START_FEN).expect("start position");
    build(json!({
        "id": "game-chess",
        "kind": "game",
        "metadata": { "title": "Chess", "capability": "chess" },
        "stateSchema": [
            { "key": "chess", "type": "object", "initialValue": initial },
            { "key": "currentPlayer", "type": "string", "initialValue": "white" },
            { "key": "status", "type": "string", "initialValue": "playing" },
            { "key": "winner", "type": "string", "initialValue": null, "nullable": true }
        ],
        "actors": ["white", "black"],
        "turnPolicy": "turn_based",
        "actions": [
            {
                "id": "move",
                "actor": "active_player",
                "description": "Move a piece using algebraic squares, e.g. e2 to e4",
                "parameters": {
                    "from": { "type": "string", "minLength": 2, "maxLength": 2 },
                    "to": { "type": "string", "minLength": 2, "maxLength": 2 },
                    "promotion": { "type": "enum", "allowedValues": ["q", "r", "b", "n"], "required": false }
                },
                "guards": [playing("playing")],
                "effects": [{ "effect": "chessMove", "key": "chess", "from": p("from"), "to": p("to"), "promotion": p("promotion") }]
            },
            {
                "id": "claimDraw",
                "actor": "active_player",
                "description": "Claim a draw when threefold repetition or the fifty-move rule applies",
                "guards": [playing("playing")],
                "effects": [{ "effect": "chessClaimDraw", "key": "chess" }]
            },
            {
                "id": "resign",
                "actor": "active_player",
                "description": "Resign and award the win to the opponent",
                "guards": [playing("playing")],
                "effects": [{ "effect": "chessResign", "key": "chess" }]
            },
            { "id": "reset", "description": "Start a new game", "effects": [{ "effect": "resetGame" }] }
        ],
        "testCases": [
            { "name": "e4", "actionId": "move", "params": { "from": "e2", "to": "e4" }, "expectedSuccess": true,
              "expectedStateSubset": { "currentPlayer": "black" }, "expectedStatus": "playing" },
            { "name": "illegal pawn triple push", "actionId": "move", "params": { "from": "e2", "to": "e5" }, "expectedSuccess": false },
            { "name": "black cannot move first", "actor": "black", "actionId": "move", "params": { "from": "e7", "to": "e5" }, "expectedSuccess": false },
            { "name": "resign awards black", "actionId": "resign", "expectedSuccess": true, "expectedStatus": "resigned",
              "expectedStateSubset": { "winner": "black" } },
            { "name": "claim draw rejected at start", "actionId": "claimDraw", "expectedSuccess": false }
        ]
    }))
}

pub fn fixture_2048() -> InteractiveAppDefinition {
    build(json!({
        "id": "game-2048",
        "kind": "puzzle",
        "metadata": { "title": "2048" },
        "stateSchema": [
            { "key": "grid", "type": "array", "initialValue": [[0, 0, 0, 0], [0, 2, 0, 0], [0, 0, 2, 0], [0, 0, 0, 0]] },
            { "key": "score", "type": "integer", "initialValue": 0 },
            { "key": "status", "type": "string", "initialValue": "playing" }
        ],
        "actors": ["player"],
        "randomSeed": 1337,
        "actions": [
            {
                "id": "slide",
                "description": "Slide all tiles",
                "parameters": { "direction": { "type": "enum", "allowedValues": ["up", "down", "left", "right"] } },
                "guards": [playing("playing")],
                "effects": [{ "effect": "grid2048Slide", "key": "grid", "direction": p("direction"), "scoreKey": "score" }]
            },
            { "id": "reset", "effects": [{ "effect": "resetGame" }] }
        ],
        "testCases": [
            { "name": "merge scores", "initialState": { "grid": [[2, 2, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]] },
              "actionId": "slide", "params": { "direction": "left" }, "expectedSuccess": true, "expectedStateSubset": { "score": 4 } },
            { "name": "no-op slide rejected", "initialState": { "grid": [[2, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]] },
              "actionId": "slide", "params": { "direction": "left" }, "expectedSuccess": false },
            { "name": "invalid direction", "actionId": "slide", "params": { "direction": "diagonal" }, "expectedSuccess": false }
        ]
    }))
}

pub fn fixture_card_deck() -> InteractiveAppDefinition {
    let count = |k: &str| json!({ "op": "count", "expr": s(k) });
    build(json!({
        "id": "cards-draw",
        "kind": "card_game",
        "metadata": { "title": "Draw & Discard" },
        "stateSchema": [
            { "key": "deck", "type": "array", "initialValue": ["A", "K", "Q", "J", "10", "9", "8", "7"] },
            { "key": "hand", "type": "array", "initialValue": [] },
            { "key": "discard", "type": "array", "initialValue": [] },
            { "key": "status", "type": "string", "initialValue": "playing" }
        ],
        "actors": ["player"],
        "randomSeed": 777,
        "actions": [
            { "id": "shuffle", "effects": [{ "effect": "shuffleArray", "key": "deck" }] },
            {
                "id": "draw",
                "guards": [
                    { "op": "gt", "left": count("deck"), "right": c(json!(0)) },
                    { "op": "lt", "left": count("hand"), "right": c(json!(5)) }
                ],
                "effects": [
                    { "effect": "pushArray", "key": "hand", "item": { "op": "getCellAt", "boardKey": "deck", "index": c(json!(0)) } },
                    { "effect": "removeFromArray", "key": "deck", "index": c(json!(0)) }
                ]
            },
            {
                "id": "discard",
                "parameters": { "index": { "type": "integer", "min": 0, "max": 4 } },
                "guards": [{ "op": "lt", "left": p("index"), "right": count("hand") }],
                "effects": [
                    { "effect": "pushArray", "key": "discard", "item": { "op": "getIndex", "expr": s("hand"), "index": p("index") } },
                    { "effect": "removeFromArray", "key": "hand", "index": p("index") }
                ]
            },
            { "id": "reset", "effects": [{ "effect": "resetGame" }] }
        ],
        "terminalConditions": [
            { "condition": { "op": "and", "exprs": [eq(count("deck"), c(json!(0))), eq(count("hand"), c(json!(0)))] },
              "status": "finished", "message": "All cards played" }
        ],
        "testCases": [
            { "name": "draw moves top card", "actionId": "draw", "expectedSuccess": true,
              "expectedStateSubset": { "hand": ["A"], "deck": ["K", "Q", "J", "10", "9", "8", "7"] } },
            { "name": "discard empty hand rejected", "actionId": "discard", "params": { "index": 0 }, "expectedSuccess": false },
            { "name": "hand limit", "initialState": { "hand": ["1", "2", "3", "4", "5"] }, "actionId": "draw", "expectedSuccess": false }
        ]
    }))
}

pub fn fixture_quiz() -> InteractiveAppDefinition {
    build(json!({
        "id": "app-quiz",
        "kind": "quiz",
        "metadata": { "title": "Planets Quiz" },
        "stateSchema": [
            { "key": "questions", "type": "array", "initialValue": [
                { "prompt": "Largest planet?", "choices": ["Mars", "Jupiter", "Venus"] },
                { "prompt": "Closest to the Sun?", "choices": ["Mercury", "Earth", "Neptune"] },
                { "prompt": "Known for its rings?", "choices": ["Saturn", "Mars", "Earth"] }
            ] },
            { "key": "answers", "type": "array", "initialValue": [1, 0, 0], "readPolicy": "restricted" },
            { "key": "currentQuestion", "type": "integer", "initialValue": 0 },
            { "key": "score", "type": "integer", "initialValue": 0 },
            { "key": "lastResult", "type": "string", "initialValue": null, "nullable": true },
            { "key": "status", "type": "string", "initialValue": "in_progress" }
        ],
        "actors": ["user"],
        "actions": [
            {
                "id": "answer",
                "parameters": { "choice": { "type": "integer", "min": 0, "max": 2 } },
                "guards": [playing("in_progress")],
                "effects": [
                    { "effect": "if",
                      "condition": eq(p("choice"), json!({ "op": "getIndex", "expr": s("answers"), "index": s("currentQuestion") })),
                      "then": [
                          { "effect": "increment", "key": "score", "amount": c(json!(1)) },
                          { "effect": "set", "key": "lastResult", "value": c(json!("correct")) }
                      ],
                      "else": [{ "effect": "set", "key": "lastResult", "value": c(json!("incorrect")) }] },
                    { "effect": "increment", "key": "currentQuestion", "amount": c(json!(1)) }
                ]
            },
            { "id": "restart", "effects": [{ "effect": "resetGame" }] }
        ],
        "derivedState": [{ "key": "total", "expr": { "op": "count", "expr": s("questions") } }],
        "terminalConditions": [
            { "condition": { "op": "gte", "left": s("currentQuestion"), "right": { "op": "count", "expr": s("questions") } },
              "status": "completed", "message": "Quiz complete" }
        ],
        "testCases": [
            { "name": "correct answer scores", "actionId": "answer", "params": { "choice": 1 }, "expectedSuccess": true,
              "expectedStateSubset": { "score": 1, "currentQuestion": 1, "lastResult": "correct" } },
            { "name": "wrong answer does not score", "actionId": "answer", "params": { "choice": 0 }, "expectedSuccess": true,
              "expectedStateSubset": { "score": 0, "lastResult": "incorrect" } },
            { "name": "last answer completes", "initialState": { "currentQuestion": 2, "score": 2 }, "actionId": "answer",
              "params": { "choice": 0 }, "expectedSuccess": true, "expectedStatus": "completed", "expectedStateSubset": { "score": 3 } },
            { "name": "no answers after completion", "initialState": { "currentQuestion": 3, "status": "completed" },
              "actionId": "answer", "params": { "choice": 0 }, "expectedSuccess": false }
        ]
    }))
}

pub fn fixture_calculator() -> InteractiveAppDefinition {
    let entry = json!({ "op": "toNumber", "expr": s("display") });
    let compute = json!({ "op": "if", "condition": eq(s("operator"), c(json!("+"))),
        "then": { "op": "add", "left": s("accumulator"), "right": entry },
        "else": { "op": "if", "condition": eq(s("operator"), c(json!("-"))),
            "then": { "op": "sub", "left": s("accumulator"), "right": entry },
            "else": { "op": "if", "condition": eq(s("operator"), c(json!("*"))),
                "then": { "op": "mul", "left": s("accumulator"), "right": entry },
                "else": { "op": "div", "left": s("accumulator"), "right": entry } } } });
    let pending = json!({ "op": "and", "exprs": [
        { "op": "neq", "left": s("operator"), "right": c(json!("")) }, s("entering") ] });
    build(json!({
        "id": "app-calculator",
        "kind": "calculator",
        "metadata": { "title": "Calculator" },
        "stateSchema": [
            { "key": "display", "type": "string", "initialValue": "0" },
            { "key": "accumulator", "type": "number", "initialValue": 0 },
            { "key": "operator", "type": "string", "initialValue": "" },
            { "key": "entering", "type": "boolean", "initialValue": false },
            { "key": "history", "type": "array", "initialValue": [] }
        ],
        "actors": ["user"],
        "actions": [
            {
                "id": "digit",
                "parameters": { "d": { "type": "enum", "allowedValues": ["0","1","2","3","4","5","6","7","8","9","."] } },
                "guards": [
                    { "op": "lt", "left": { "op": "count", "expr": s("display") }, "right": c(json!(16)) },
                    { "op": "not", "expr": { "op": "and", "exprs": [ eq(p("d"), c(json!("."))), s("entering"),
                        { "op": "contains", "array": s("display"), "item": c(json!(".")) } ] } }
                ],
                "effects": [
                    { "effect": "if", "condition": { "op": "or", "exprs": [ { "op": "not", "expr": s("entering") }, eq(s("display"), c(json!("0"))) ] },
                      "then": [ { "effect": "set", "key": "display", "value": { "op": "if", "condition": eq(p("d"), c(json!("."))), "then": c(json!("0.")), "else": p("d") } },
                                { "effect": "set", "key": "entering", "value": c(json!(true)) } ],
                      "else": [ { "effect": "set", "key": "display", "value": { "op": "concat", "exprs": [s("display"), p("d")] } } ] }
                ]
            },
            {
                "id": "operator",
                "parameters": { "op": { "type": "enum", "allowedValues": ["+", "-", "*", "/"] } },
                "effects": [
                    { "effect": "set", "key": "accumulator", "value": { "op": "if", "condition": pending, "then": compute, "else": entry } },
                    { "effect": "set", "key": "display", "value": { "op": "concat", "exprs": [s("accumulator")] } },
                    { "effect": "set", "key": "operator", "value": p("op") },
                    { "effect": "set", "key": "entering", "value": c(json!(false)) }
                ]
            },
            {
                "id": "equals",
                "guards": [{ "op": "neq", "left": s("operator"), "right": c(json!("")) }],
                "effects": [
                    { "effect": "set", "key": "accumulator", "value": compute },
                    { "effect": "pushArray", "key": "history", "item": { "op": "concat", "exprs": [s("accumulator")] } },
                    { "effect": "set", "key": "display", "value": { "op": "concat", "exprs": [s("accumulator")] } },
                    { "effect": "set", "key": "operator", "value": c(json!("")) },
                    { "effect": "set", "key": "entering", "value": c(json!(false)) }
                ]
            },
            { "id": "clear", "effects": [{ "effect": "resetGame" }] }
        ],
        "testCases": [
            { "name": "digit replaces zero", "actionId": "digit", "params": { "d": "7" }, "expectedSuccess": true, "expectedStateSubset": { "display": "7" } },
            { "name": "digits append", "initialState": { "display": "12", "entering": true }, "actionId": "digit", "params": { "d": "3" },
              "expectedSuccess": true, "expectedStateSubset": { "display": "123" } },
            { "name": "second decimal rejected", "initialState": { "display": "1.5", "entering": true }, "actionId": "digit", "params": { "d": "." }, "expectedSuccess": false },
            { "name": "add", "initialState": { "display": "5", "accumulator": 7, "operator": "+", "entering": true }, "actionId": "equals",
              "expectedSuccess": true, "expectedStateSubset": { "display": "12", "accumulator": 12, "history": ["12"] } },
            { "name": "decimal division", "initialState": { "display": "4", "accumulator": 10, "operator": "/", "entering": true }, "actionId": "equals",
              "expectedSuccess": true, "expectedStateSubset": { "display": "2.5" } },
            { "name": "division by zero rejected", "initialState": { "display": "0", "accumulator": 3, "operator": "/", "entering": true },
              "actionId": "equals", "expectedSuccess": false },
            { "name": "equals without operator rejected", "actionId": "equals", "expectedSuccess": false },
            { "name": "clear", "initialState": { "display": "99", "accumulator": 4, "history": ["4"] }, "actionId": "clear",
              "expectedSuccess": true, "expectedStateSubset": { "display": "0", "accumulator": 0, "history": [] } }
        ]
    }))
}

pub fn fixture_controls() -> InteractiveAppDefinition {
    build(json!({
        "id": "app-controls",
        "kind": "control_panel",
        "metadata": { "title": "Focus Controls" },
        "stateSchema": [
            { "key": "count", "type": "integer", "initialValue": 0 },
            { "key": "enabled", "type": "boolean", "initialValue": true },
            { "key": "volume", "type": "integer", "initialValue": 50 },
            { "key": "mode", "type": "string", "initialValue": "auto" },
            { "key": "note", "type": "string", "initialValue": "" }
        ],
        "actors": ["user"],
        "actions": [
            { "id": "increment", "guards": [{ "op": "lt", "left": s("count"), "right": c(json!(99)) }],
              "effects": [{ "effect": "increment", "key": "count", "amount": c(json!(1)) }] },
            { "id": "decrement", "guards": [{ "op": "gt", "left": s("count"), "right": c(json!(0)) }],
              "effects": [{ "effect": "decrement", "key": "count", "amount": c(json!(1)) }] },
            { "id": "toggle", "effects": [{ "effect": "set", "key": "enabled", "value": { "op": "not", "expr": s("enabled") } }] },
            { "id": "setVolume", "parameters": { "value": { "type": "integer", "min": 0, "max": 100 } },
              "effects": [{ "effect": "set", "key": "volume", "value": p("value") }] },
            { "id": "setMode", "parameters": { "mode": { "type": "enum", "allowedValues": ["light", "dark", "auto"] } },
              "effects": [{ "effect": "set", "key": "mode", "value": p("mode") }] },
            { "id": "setNote", "parameters": { "text": { "type": "string", "maxLength": 280 } },
              "effects": [{ "effect": "set", "key": "note", "value": p("text") }] },
            { "id": "reset", "effects": [{ "effect": "resetGame" }] }
        ],
        "testCases": [
            { "name": "increment", "actionId": "increment", "expectedSuccess": true, "expectedStateSubset": { "count": 1 } },
            { "name": "decrement floor", "actionId": "decrement", "expectedSuccess": false },
            { "name": "toggle", "actionId": "toggle", "expectedSuccess": true, "expectedStateSubset": { "enabled": false } },
            { "name": "volume bounds", "actionId": "setVolume", "params": { "value": 101 }, "expectedSuccess": false },
            { "name": "mode enum", "actionId": "setMode", "params": { "mode": "neon" }, "expectedSuccess": false },
            { "name": "reset", "initialState": { "count": 9, "volume": 3 }, "actionId": "reset", "expectedSuccess": true,
              "expectedStateSubset": { "count": 0, "volume": 50 } }
        ]
    }))
}

pub fn fixture_timer() -> InteractiveAppDefinition {
    let running = s("running");
    build(json!({
        "id": "app-timer",
        "kind": "timer",
        "metadata": { "title": "Stopwatch" },
        "stateSchema": [
            { "key": "running", "type": "boolean", "initialValue": false },
            { "key": "elapsed", "type": "integer", "initialValue": 0 },
            { "key": "laps", "type": "array", "initialValue": [] }
        ],
        "actors": ["user"],
        "actions": [
            { "id": "start", "guards": [{ "op": "not", "expr": running }],
              "effects": [{ "effect": "set", "key": "running", "value": c(json!(true)) }] },
            { "id": "pause", "guards": [running],
              "effects": [{ "effect": "set", "key": "running", "value": c(json!(false)) }] },
            { "id": "tick", "description": "Host clock tick; ignored while paused",
              "parameters": { "seconds": { "type": "integer", "min": 1, "max": 60 } },
              "guards": [running, { "op": "lt", "left": s("elapsed"), "right": c(json!(86400)) }],
              "effects": [{ "effect": "set", "key": "elapsed", "value": { "op": "min",
                  "left": { "op": "add", "left": s("elapsed"), "right": p("seconds") }, "right": c(json!(86400)) } }] },
            { "id": "lap", "guards": [running, { "op": "lt", "left": { "op": "count", "expr": s("laps") }, "right": c(json!(50)) }],
              "effects": [{ "effect": "pushArray", "key": "laps", "item": s("elapsed") }] },
            { "id": "reset", "effects": [{ "effect": "resetGame" }] }
        ],
        "testCases": [
            { "name": "tick ignored while paused", "actionId": "tick", "params": { "seconds": 1 }, "expectedSuccess": false },
            { "name": "tick while running", "initialState": { "running": true }, "actionId": "tick", "params": { "seconds": 5 },
              "expectedSuccess": true, "expectedStateSubset": { "elapsed": 5 } },
            { "name": "tick bounded", "initialState": { "running": true, "elapsed": 86399 }, "actionId": "tick", "params": { "seconds": 60 },
              "expectedSuccess": true, "expectedStateSubset": { "elapsed": 86400 } },
            { "name": "oversized tick rejected", "initialState": { "running": true }, "actionId": "tick", "params": { "seconds": 3600 }, "expectedSuccess": false },
            { "name": "pause", "initialState": { "running": true }, "actionId": "pause", "expectedSuccess": true, "expectedStateSubset": { "running": false } }
        ]
    }))
}

/// Synthetic application not modelled on any built-in: a colony resource planner.
pub fn fixture_resource_sim() -> InteractiveAppDefinition {
    let alloc = |area: &str| {
        json!({ "effect": "if", "condition": eq(p("area"), c(json!(area))),
                "then": [{ "effect": "increment", "key": area, "amount": p("amount") }] })
    };
    build(json!({
        "id": "sim-colony",
        "kind": "simulation",
        "metadata": { "title": "Colony Planner" },
        "stateSchema": [
            { "key": "budget", "type": "integer", "initialValue": 100 },
            { "key": "farms", "type": "integer", "initialValue": 0 },
            { "key": "labs", "type": "integer", "initialValue": 0 },
            { "key": "defense", "type": "integer", "initialValue": 0 },
            { "key": "population", "type": "integer", "initialValue": 50 },
            { "key": "round", "type": "integer", "initialValue": 1 },
            { "key": "lastEvent", "type": "integer", "initialValue": -1 },
            { "key": "status", "type": "string", "initialValue": "planning" }
        ],
        "actors": ["planner"],
        "randomSeed": 99,
        "actions": [
            {
                "id": "allocate",
                "parameters": {
                    "area": { "type": "enum", "allowedValues": ["farms", "labs", "defense"] },
                    "amount": { "type": "integer", "min": 1, "max": 50 }
                },
                "guards": [playing("planning"), { "op": "lte", "left": p("amount"), "right": s("budget") }],
                "effects": [alloc("farms"), alloc("labs"), alloc("defense"),
                    { "effect": "decrement", "key": "budget", "amount": p("amount") }]
            },
            {
                "id": "endRound",
                "guards": [playing("planning")],
                "effects": [
                    { "effect": "setRandomInt", "key": "lastEvent", "min": c(json!(0)), "max": c(json!(9)) },
                    { "effect": "increment", "key": "population", "amount": { "op": "sub",
                        "left": { "op": "mul", "left": s("farms"), "right": c(json!(2)) },
                        "right": { "op": "if", "condition": { "op": "and", "exprs": [
                                { "op": "lt", "left": s("lastEvent"), "right": c(json!(3)) },
                                { "op": "lt", "left": s("defense"), "right": c(json!(10)) } ] },
                            "then": c(json!(15)), "else": c(json!(0)) } } },
                    { "effect": "increment", "key": "budget", "amount": { "op": "add", "left": c(json!(20)), "right": s("labs") } },
                    { "effect": "increment", "key": "round", "amount": c(json!(1)) }
                ]
            },
            { "id": "restart", "effects": [{ "effect": "resetGame" }] }
        ],
        "terminalConditions": [
            { "condition": { "op": "lte", "left": s("population"), "right": c(json!(0)) }, "status": "collapsed", "message": "The colony collapsed" },
            { "condition": { "op": "gt", "left": s("round"), "right": c(json!(10)) }, "status": "completed", "message": "Ten rounds survived" }
        ],
        "testCases": [
            { "name": "allocate spends budget", "actionId": "allocate", "params": { "area": "labs", "amount": 30 }, "expectedSuccess": true,
              "expectedStateSubset": { "labs": 30, "budget": 70, "farms": 0 } },
            { "name": "overspend rejected", "actionId": "allocate", "params": { "area": "farms", "amount": 50 }, "initialState": { "budget": 10 }, "expectedSuccess": false },
            { "name": "unknown area rejected", "actionId": "allocate", "params": { "area": "casino", "amount": 5 }, "expectedSuccess": false },
            { "name": "round advances", "actionId": "endRound", "expectedSuccess": true, "expectedStateSubset": { "round": 2, "budget": 120 } },
            { "name": "final round completes", "initialState": { "round": 10, "farms": 5 }, "actionId": "endRound", "expectedSuccess": true, "expectedStatus": "completed" }
        ]
    }))
}

pub fn fixture_configurator() -> InteractiveAppDefinition {
    build(json!({
        "id": "app-configurator",
        "kind": "workflow",
        "metadata": { "title": "Desk Configurator" },
        "stateSchema": [
            { "key": "phase", "type": "string", "initialValue": "size" },
            { "key": "size", "type": "string", "initialValue": null, "nullable": true },
            { "key": "finish", "type": "string", "initialValue": null, "nullable": true },
            { "key": "price", "type": "integer", "initialValue": 0 },
            { "key": "status", "type": "string", "initialValue": "editing" }
        ],
        "actors": ["user"],
        "actions": [
            { "id": "chooseSize", "allowedPhases": ["size"],
              "parameters": { "size": { "type": "enum", "allowedValues": ["compact", "standard", "wide"] } },
              "effects": [
                { "effect": "set", "key": "size", "value": p("size") },
                { "effect": "set", "key": "price", "value": { "op": "if", "condition": eq(p("size"), c(json!("wide"))), "then": c(json!(450)),
                    "else": { "op": "if", "condition": eq(p("size"), c(json!("standard"))), "then": c(json!(320)), "else": c(json!(240)) } } },
                { "effect": "setPhase", "phase": "finish" } ] },
            { "id": "chooseFinish", "allowedPhases": ["finish"],
              "parameters": { "finish": { "type": "enum", "allowedValues": ["oak", "walnut", "white"] } },
              "effects": [
                { "effect": "set", "key": "finish", "value": p("finish") },
                { "effect": "if", "condition": eq(p("finish"), c(json!("walnut"))),
                  "then": [{ "effect": "increment", "key": "price", "amount": c(json!(80)) }] },
                { "effect": "setPhase", "phase": "review" } ] },
            { "id": "back", "allowedPhases": ["finish", "review"],
              "effects": [{ "effect": "if", "condition": eq(json!({ "op": "getPhase" }), c(json!("review"))),
                  "then": [{ "effect": "setPhase", "phase": "finish" }], "else": [{ "effect": "setPhase", "phase": "size" }] }] },
            { "id": "confirm", "allowedPhases": ["review"],
              "effects": [{ "effect": "setPhase", "phase": "done" }, { "effect": "endGame", "status": "confirmed" }] },
            { "id": "startOver", "effects": [{ "effect": "resetGame" }] }
        ],
        "testCases": [
            { "name": "size then finish", "actionId": "chooseSize", "params": { "size": "wide" }, "expectedSuccess": true,
              "expectedStateSubset": { "phase": "finish", "price": 450 } },
            { "name": "finish out of order rejected", "actionId": "chooseFinish", "params": { "finish": "oak" }, "expectedSuccess": false },
            { "name": "walnut surcharge", "initialState": { "phase": "finish", "size": "compact", "price": 240 }, "actionId": "chooseFinish",
              "params": { "finish": "walnut" }, "expectedSuccess": true, "expectedStateSubset": { "price": 320, "phase": "review" } },
            { "name": "confirm", "initialState": { "phase": "review", "size": "compact", "finish": "oak", "price": 240 }, "actionId": "confirm",
              "expectedSuccess": true, "expectedStatus": "confirmed" },
            { "name": "back from review", "initialState": { "phase": "review" }, "actionId": "back", "expectedSuccess": true, "expectedStateSubset": { "phase": "finish" } }
        ]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_fixtures_admit_and_pass_self_tests() {
        for name in FIXTURE_NAMES {
            let def = get_fixture(name).expect(name);
            def.run_self_tests()
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            let reparsed =
                InteractiveAppDefinition::parse_and_admit(&serde_json::to_value(&def).unwrap());
            assert!(
                reparsed.is_ok(),
                "{name} must round-trip through the model schema"
            );
        }
        assert!(get_fixture("checkers").is_none());
    }

    #[test]
    fn quiz_hides_answers_from_public_view() {
        let def = fixture_quiz();
        let st = def.initial_instance_state().unwrap();
        let view = def.public_view(&st);
        assert!(view.get("answers").is_none());
        assert!(view.get("questions").is_some());
        assert_eq!(view["total"], 3);
    }
}
