//! Mock AI provider with fixture responses for tests and offline use.

use async_trait::async_trait;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::errors::AiError;
use super::provider::{
    AgentRequest, AgentResponse, AiProvider, ProviderHealth, ProviderStreamEvent, ProviderStreamTx,
    UsageMetadata,
};
use super::response_schema::SCHEMA_VERSION;
use super::structured_user_input::AgentContentPart;

pub struct MockAiProvider;

impl MockAiProvider {
    pub fn new() -> Self {
        Self
    }

    fn fixture_for(user_text: &str) -> String {
        Self::fixture_for_with_revision(user_text, None)
    }

    /// Parse the highest `definitionRevision` advertised in authoritative prompt context.
    fn observed_definition_revision(system_prompt: &str) -> Option<i64> {
        let mut best: Option<i64> = None;
        for (idx, _) in system_prompt.match_indices("\"definitionRevision\"") {
            let rest = &system_prompt[idx..];
            let Some(colon) = rest.find(':') else {
                continue;
            };
            let after = rest[colon + 1..].trim_start();
            let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = digits.parse::<i64>() {
                best = Some(best.map_or(n, |b| b.max(n)));
            }
        }
        best
    }

    fn fixture_for_with_revision(user_text: &str, observed_revision: Option<i64>) -> String {
        let lower = user_text.to_lowercase();

        if lower.contains("water") || lower.contains("hydrat") {
            return json!({
                "schemaVersion": SCHEMA_VERSION,
                "assistantMessage": "I created a simple water tracker for you. Review the preview and apply when ready.",
                "responseType": "tool_change",
                "toolChange": {
                    "action": "create",
                    "targetToolId": null,
                    "changeSummary": "Create a daily water intake tracker",
                    "tool": {
                        "id": "tool-water-tracker",
                        "name": "Water Tracker",
                        "description": "Track daily glasses of water",
                        "layout": { "type": "single-column" },
                        "components": [
                            { "id": "wt-heading", "type": "heading", "props": { "text": "Water Tracker", "level": 1 } },
                            { "id": "wt-stat", "type": "stat", "props": { "label": "Today", "valueKey": "glasses" } },
                            { "id": "wt-counter", "type": "counter", "props": { "label": "Glasses", "stateKey": "glasses", "min": 0, "max": 20 } },
                            { "id": "wt-progress", "type": "progress", "props": { "label": "Goal", "valueKey": "glasses", "max": 8 } },
                            {
                                "id": "wt-buttons",
                                "type": "buttonGroup",
                                "props": {},
                                "children": [
                                    {
                                        "id": "wt-add",
                                        "type": "button",
                                        "props": { "label": "+1" },
                                        "actions": [{ "type": "increment", "target": "glasses", "amount": 1 }]
                                    },
                                    {
                                        "id": "wt-minus",
                                        "type": "button",
                                        "props": { "label": "−1" },
                                        "actions": [{ "type": "decrement", "target": "glasses", "amount": 1 }]
                                    },
                                    {
                                        "id": "wt-reset",
                                        "type": "button",
                                        "props": { "label": "Reset" },
                                        "actions": [{ "type": "reset", "target": "glasses", "value": 0 }]
                                    }
                                ]
                            }
                        ]
                    }
                },
                "diagnostics": { "fixture": "water_tracker" }
            })
            .to_string();
        }

        if lower.contains("quiz") {
            return json!({
                "schemaVersion": SCHEMA_VERSION,
                "assistantMessage": "Here's a short quiz tool. Apply it to open and try the questions.",
                "responseType": "tool_change",
                "toolChange": {
                    "action": "create",
                    "targetToolId": null,
                    "changeSummary": "Create a sample quiz",
                    "tool": {
                        "id": "tool-sample-quiz",
                        "name": "Sample Quiz",
                        "description": "A short knowledge check",
                        "layout": { "type": "single-column" },
                        "components": [
                            { "id": "qz-heading", "type": "heading", "props": { "text": "Quick Quiz", "level": 1 } },
                            {
                                "id": "qz-quiz",
                                "type": "quiz",
                                "props": {
                                    "questions": [
                                        {
                                            "id": "q1",
                                            "prompt": "What is the capital of France?",
                                            "options": ["Berlin", "Paris", "Rome"],
                                            "answer": "Paris",
                                            "explanation": "Paris is the capital of France."
                                        },
                                        {
                                            "id": "q2",
                                            "prompt": "Which ocean is the largest?",
                                            "options": ["Atlantic", "Indian", "Pacific"],
                                            "answer": "Pacific"
                                        },
                                        {
                                            "id": "q3",
                                            "prompt": "Mount Everest is in which mountain range?",
                                            "options": ["Andes", "Alps", "Himalayas"],
                                            "answer": "Himalayas"
                                        }
                                    ]
                                }
                            }
                        ]
                    }
                },
                "diagnostics": { "fixture": "quiz" }
            })
            .to_string();
        }

        // Evolve fixtures must not steal create prompts that mention due dates
        // (e.g. "build a task tracker with due dates").
        let create_ish = lower.contains("build")
            || lower.contains("create")
            || lower.contains("make me")
            || lower.contains("generate")
            || lower.contains("new task tracker");
        if (lower.contains("due date") || lower.contains("due dates") || lower.contains("add due"))
            && !create_ish
        {
            // OCC-real: evolve fixtures must declare the revision they observed.
            // Prefer prompt context; deterministic tests without context use rev 1.
            let rev = observed_revision.unwrap_or(1);
            let plan = super::plan_fixtures::task_tracker_add_due_dates_plan(Some(rev));
            return super::plan_fixtures::plan_response_json(
                "I updated your Task Tracker to support optional due dates. Existing tasks are preserved; stable component IDs were kept so dirty form state survives.",
                &plan,
            );
        }

        if lower.contains("task manager")
            || lower.contains("task-manager")
            || lower.contains("task tracker")
            || lower.contains("task-tracker")
        {
            let plan = super::plan_fixtures::task_tracker_create_plan();
            return super::plan_fixtures::plan_response_json(
                "I generated a Task Tracker with a typed tasks data model, durable local_data CRUD, priority/status filtering, and an empty state.",
                &plan,
            );
        }

        if lower.contains("habit") {
            let plan = super::plan_fixtures::habit_tracker_create_plan();
            return super::plan_fixtures::plan_response_json(
                "I created a Habit Tracker with a habits data model, streak tracking, and durable local_data records.",
                &plan,
            );
        }

        if lower.contains("study planner") || lower.contains("study-planner") {
            let plan = super::plan_fixtures::multi_surface_planner_create_plan();
            return super::plan_fixtures::plan_response_json(
                "I created a Study Planner with dashboard and tasks sections on one surface (multi-surface routing is represented as sections for now).",
                &plan,
            );
        }

        if lower.contains("journal") || lower.contains("diary") || lower.contains("notes journal") {
            let plan = super::plan_fixtures::journal_create_plan();
            return super::plan_fixtures::plan_response_json(
                "I created a Journal with durable entries, optional mood tags, and local_data persistence.",
                &plan,
            );
        }

        if lower.contains("budget tracker")
            || lower.contains("budget-tracker")
            || (lower.contains("budget") && !lower.contains("expense"))
            || (lower.contains("spending") && !lower.contains("expense"))
        {
            let plan = super::plan_fixtures::budget_tracker_create_plan();
            return super::plan_fixtures::plan_response_json(
                "I created a Budget Tracker with an expenses data model, category enum, and durable local_data logging.",
                &plan,
            );
        }

        if lower.contains("expense") || lower.contains("spending") {
            return json!({
                "schemaVersion": SCHEMA_VERSION,
                "assistantMessage": "I created an Expense Tracker with category management, statistics, and a persistent data table.",
                "responseType": "tool_change",
                "toolChange": {
                    "action": "create",
                    "targetToolId": null,
                    "changeSummary": "Create Expense Tracker with stats and category logging",
                    "tool": {
                        "id": "tool-expense-tracker",
                        "name": "Expense Tracker",
                        "description": "Log and monitor expenses by category.",
                        "layout": { "type": "split", "splitRatio": "1:2" },
                        "components": [
                            { "id": "exp-heading", "type": "heading", "props": { "text": "Expense Tracker", "level": 1 } },
                            { "id": "exp-stat", "type": "stat", "props": { "label": "Total Logged ($)", "valueKey": "totalExpenses" } },
                            { "id": "exp-amount", "type": "textInput", "props": { "label": "Amount ($)", "placeholder": "25.00" }, "valueKey": "newAmount" },
                            { "id": "exp-category", "type": "select", "props": { "label": "Category", "options": [{ "label": "Food", "value": "food" }, { "label": "Travel", "value": "travel" }, { "label": "Supplies", "value": "supplies" }, { "label": "Services", "value": "services" }] }, "valueKey": "newCategory" },
                            { "id": "exp-note", "type": "textInput", "props": { "label": "Description", "placeholder": "Lunch, taxi, etc." }, "valueKey": "newNote" },
                            { "id": "exp-add-btn", "type": "button", "props": { "label": "Record Expense" }, "actions": [{ "type": "appendItem", "target": "expenses", "itemFromState": { "amount": "newAmount", "category": "newCategory", "note": "newNote" } }, { "type": "increment", "target": "totalExpenses", "amount": 25.0 }, { "type": "setValue", "target": "newAmount", "value": "" }, { "type": "setValue", "target": "newNote", "value": "" }] },
                            { "id": "exp-table", "type": "dataTable", "props": { "columns": [{ "id": "amount", "accessor": "amount", "header": "Amount" }, { "id": "category", "accessor": "category", "header": "Category" }, { "id": "note", "accessor": "note", "header": "Description" }], "rowsKey": "expenses", "dataKey": "expenses", "selectionKey": "selectedExpenseId" } },
                            { "id": "exp-delete-btn", "type": "button", "props": { "label": "Delete Expense" }, "actions": [{ "type": "removeItem", "target": "expenses", "idFromState": "selectedExpenseId" }] }
                        ]
                    }
                },
                "diagnostics": { "fixture": "expense_tracker" }
            })
            .to_string();
        }

        if lower.contains("todo") || lower.contains("checklist") || lower.contains("task") {
            return json!({
                "schemaVersion": SCHEMA_VERSION,
                "assistantMessage": "I drafted a checklist tool for your tasks.",
                "responseType": "tool_change",
                "toolChange": {
                    "action": "create",
                    "targetToolId": null,
                    "changeSummary": "Create a personal checklist",
                    "tool": {
                        "id": "tool-checklist",
                        "name": "Checklist",
                        "description": "Track personal tasks",
                        "layout": { "type": "single-column" },
                        "components": [
                            { "id": "cl-heading", "type": "heading", "props": { "text": "Tasks", "level": 1 } },
                            { "id": "cl-list", "type": "checklist", "props": { "stateKey": "items", "placeholder": "Add a task" } },
                            { "id": "cl-input", "type": "textInput", "props": { "label": "New task", "stateKey": "draft" } },
                            { "id": "cl-add", "type": "button", "props": { "label": "Add", "actions": [{ "type": "appendItem", "target": "items", "item": { "fromState": "draft" } }] } }
                        ]
                    }
                },
                "diagnostics": { "fixture": "checklist" }
            })
            .to_string();
        }

        if lower.contains("progress")
            || lower.contains("encourag")
            || lower.contains("goal message")
        {
            return json!({
                "schemaVersion": SCHEMA_VERSION,
                "assistantMessage": "I updated your water tracker with a progress cue and a goal celebration message.",
                "responseType": "tool_change",
                "toolChange": {
                    "action": "update",
                    "targetToolId": "tool-water-tracker",
                    "changeSummary": "Add progress encouragement",
                    "tool": {
                        "id": "tool-water-tracker",
                        "name": "Water Tracker",
                        "description": "Track daily glasses of water",
                        "layout": { "type": "single-column" },
                        "components": [
                            { "id": "wt-heading", "type": "heading", "props": { "text": "Water Tracker", "level": 1 } },
                            { "id": "wt-stat", "type": "stat", "props": { "label": "Today", "valueKey": "glasses" } },
                            { "id": "wt-counter", "type": "counter", "props": { "label": "Glasses", "stateKey": "glasses", "min": 0, "max": 20 } },
                            { "id": "wt-progress", "type": "progress", "props": { "label": "Goal", "valueKey": "glasses", "max": 8 } },
                            { "id": "wt-encourage", "type": "text", "props": { "text": "Nice work — you reached your daily goal!" } },
                            {
                                "id": "wt-buttons",
                                "type": "buttonGroup",
                                "props": {},
                                "children": [
                                    {
                                        "id": "wt-add",
                                        "type": "button",
                                        "props": { "label": "+1" },
                                        "actions": [{ "type": "increment", "target": "glasses", "amount": 1 }]
                                    },
                                    {
                                        "id": "wt-minus",
                                        "type": "button",
                                        "props": { "label": "−1" },
                                        "actions": [{ "type": "decrement", "target": "glasses", "amount": 1 }]
                                    },
                                    {
                                        "id": "wt-reset",
                                        "type": "button",
                                        "props": { "label": "Reset" },
                                        "actions": [{ "type": "reset", "target": "glasses", "value": 0 }]
                                    }
                                ]
                            }
                        ]
                    }
                },
                "diagnostics": { "fixture": "water_tracker_update" }
            })
            .to_string();
        }

        if lower.contains("noop") || lower.contains("never mind") {
            return json!({
                "schemaVersion": SCHEMA_VERSION,
                "assistantMessage": "Okay — no changes.",
                "responseType": "noop",
                "toolChange": null,
                "diagnostics": { "fixture": "noop" }
            })
            .to_string();
        }

        json!({
            "schemaVersion": SCHEMA_VERSION,
            "assistantMessage": format!(
                "I'm the Coreside mock agent. Ask me to build something like a water tracker, quiz, or checklist.\n\nYou said: {}",
                user_text.chars().take(200).collect::<String>()
            ),
            "responseType": "message",
            "toolChange": null,
            "diagnostics": { "fixture": "default_message" }
        })
        .to_string()
    }

    pub fn fixture_for_request(request: &AgentRequest) -> String {
        let user_msg = request
            .messages
            .iter()
            .rev()
            .find(|m| m.role == crate::ai::AgentRole::User);
        let user_text = user_msg.map(|m| m.content.as_str()).unwrap_or("");
        let lower = user_text.to_lowercase();

        // 1. Structured input turns (interactive games / app actions)
        let structured = user_msg.and_then(|m| {
            m.parts.iter().find_map(|p| match p {
                AgentContentPart::StructuredUserInput(sui) => Some(sui),
                _ => None,
            })
        });

        let event_name = structured
            .and_then(|s| s.event_name.as_deref())
            .or_else(|| {
                if lower.contains("game.reset") || lower.contains("eventname=game.reset") {
                    Some("game.reset")
                } else if lower.contains("game.move") || lower.contains("eventname=game.move") {
                    Some("game.move")
                } else {
                    None
                }
            });

        if let Some("game.reset") = event_name {
            let surface_id = structured
                .and_then(|s| s.surface_id.as_deref())
                .unwrap_or("tool-tictactoe");
            return json!({
                "schemaVersion": "2",
                "assistantMessage": "New game started! Your turn (X). Click any square to play.",
                "responseType": "message",
                "silent": true,
                "operations": [
                    {
                        "id": "op-tictactoe-reset",
                        "type": "state.patch",
                        "target": { "surfaceId": surface_id },
                        "payload": {
                            "c0": "", "c1": "", "c2": "",
                            "c3": "", "c4": "", "c5": "",
                            "c6": "", "c7": "", "c8": "",
                            "turn": "X",
                            "status": "New game started! Your turn (X).",
                            "winner": ""
                        }
                    }
                ],
                "diagnostics": { "fixture": "tictactoe_reset" }
            })
            .to_string();
        }

        if let Some("game.move") = event_name {
            let surface_id = structured
                .and_then(|s| s.surface_id.as_deref())
                .unwrap_or("tool-tictactoe");

            let mut board: Vec<String> = (0..9)
                .map(|i| {
                    structured
                        .and_then(|s| s.fields.get(&format!("c{i}")))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string()
                })
                .collect();

            let last_move = structured
                .and_then(|s| s.fields.get("lastMove").or_else(|| s.fields.get("cell")))
                .and_then(|v| {
                    v.as_u64()
                        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                })
                .map(|v| v as usize);

            if let Some(cell) = last_move {
                if cell < 9 && board[cell].is_empty() {
                    board[cell] = "X".to_string();
                }
            } else if let Some(empty_pos) = board.iter().position(|c| c.is_empty()) {
                board[empty_pos] = "X".to_string();
            }

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
            let check_win = |b: &[String], mark: &str| -> bool {
                lines
                    .iter()
                    .any(|&[x, y, z]| b[x] == mark && b[y] == mark && b[z] == mark)
            };

            let (status, winner, next_turn) = if check_win(&board, "X") {
                (
                    "You win! Congratulations!".to_string(),
                    "X".to_string(),
                    "gameover".to_string(),
                )
            } else if board.iter().all(|c| !c.is_empty()) {
                (
                    "It's a draw! Click New Game to play again.".to_string(),
                    "draw".to_string(),
                    "gameover".to_string(),
                )
            } else {
                let ai_cell = if board[4].is_empty() {
                    4
                } else {
                    let candidates = [0, 2, 6, 8, 1, 3, 5, 7];
                    candidates
                        .into_iter()
                        .find(|&idx| board[idx].is_empty())
                        .unwrap_or(0)
                };
                board[ai_cell] = "O".to_string();

                if check_win(&board, "O") {
                    (
                        "AI wins! Better luck next time.".to_string(),
                        "O".to_string(),
                        "gameover".to_string(),
                    )
                } else if board.iter().all(|c| !c.is_empty()) {
                    (
                        "It's a draw! Click New Game to play again.".to_string(),
                        "draw".to_string(),
                        "gameover".to_string(),
                    )
                } else {
                    (
                        format!("AI played square {}. Your turn (X)!", ai_cell + 1),
                        "".to_string(),
                        "X".to_string(),
                    )
                }
            };

            return json!({
                "schemaVersion": "2",
                "assistantMessage": status,
                "responseType": "message",
                "silent": true,
                "operations": [
                    {
                        "id": "op-tictactoe-ai-move",
                        "type": "state.patch",
                        "target": { "surfaceId": surface_id },
                        "payload": {
                            "c0": board[0], "c1": board[1], "c2": board[2],
                            "c3": board[3], "c4": board[4], "c5": board[5],
                            "c6": board[6], "c7": board[7], "c8": board[8],
                            "turn": next_turn,
                            "status": status,
                            "winner": winner
                        }
                    }
                ],
                "diagnostics": { "fixture": "tictactoe_move" }
            })
            .to_string();
        }

        // 2. Tic-Tac-Toe generation prompt
        if lower.contains("tic-tac-toe") || lower.contains("tictactoe") {
            return json!({
                "schemaVersion": "2",
                "assistantMessage": "I built a Tic-Tac-Toe game where you play against me! Click any square to make your move.",
                "responseType": "message",
                "operations": [
                    {
                        "id": "op-create-tictactoe",
                        "type": "surface.create",
                        "target": {},
                        "payload": {
                            "id": "tool-tictactoe",
                            "name": "Tic-Tac-Toe vs AI",
                            "description": "Play Tic-Tac-Toe directly inside your conversation against the AI",
                            "layout": { "type": "single-column" },
                            "components": [
                                {
                                    "id": "ttt-heading",
                                    "type": "heading",
                                    "props": { "text": "Tic-Tac-Toe vs AI", "level": 1 }
                                },
                                {
                                    "id": "ttt-status",
                                    "type": "badge",
                                    "props": { "text": "Your turn (X). Click any square to play!", "valueKey": "status" }
                                },
                                {
                                    "id": "ttt-row-0",
                                    "type": "row",
                                    "props": {},
                                    "children": [
                                        {
                                            "id": "ttt-c0",
                                            "type": "button",
                                            "props": { "label": "·", "valueKey": "c0" },
                                            "actions": [
                                                { "type": "setValue", "target": "c0", "value": "X" },
                                                { "type": "setValue", "target": "lastMove", "value": 0 },
                                                { "type": "submitToAgent", "eventName": "game.move", "includeFields": ["c0", "c1", "c2", "c3", "c4", "c5", "c6", "c7", "c8", "lastMove", "turn"] }
                                            ]
                                        },
                                        {
                                            "id": "ttt-c1",
                                            "type": "button",
                                            "props": { "label": "·", "valueKey": "c1" },
                                            "actions": [
                                                { "type": "setValue", "target": "c1", "value": "X" },
                                                { "type": "setValue", "target": "lastMove", "value": 1 },
                                                { "type": "submitToAgent", "eventName": "game.move", "includeFields": ["c0", "c1", "c2", "c3", "c4", "c5", "c6", "c7", "c8", "lastMove", "turn"] }
                                            ]
                                        },
                                        {
                                            "id": "ttt-c2",
                                            "type": "button",
                                            "props": { "label": "·", "valueKey": "c2" },
                                            "actions": [
                                                { "type": "setValue", "target": "c2", "value": "X" },
                                                { "type": "setValue", "target": "lastMove", "value": 2 },
                                                { "type": "submitToAgent", "eventName": "game.move", "includeFields": ["c0", "c1", "c2", "c3", "c4", "c5", "c6", "c7", "c8", "lastMove", "turn"] }
                                            ]
                                        }
                                    ]
                                },
                                {
                                    "id": "ttt-row-1",
                                    "type": "row",
                                    "props": {},
                                    "children": [
                                        {
                                            "id": "ttt-c3",
                                            "type": "button",
                                            "props": { "label": "·", "valueKey": "c3" },
                                            "actions": [
                                                { "type": "setValue", "target": "c3", "value": "X" },
                                                { "type": "setValue", "target": "lastMove", "value": 3 },
                                                { "type": "submitToAgent", "eventName": "game.move", "includeFields": ["c0", "c1", "c2", "c3", "c4", "c5", "c6", "c7", "c8", "lastMove", "turn"] }
                                            ]
                                        },
                                        {
                                            "id": "ttt-c4",
                                            "type": "button",
                                            "props": { "label": "·", "valueKey": "c4" },
                                            "actions": [
                                                { "type": "setValue", "target": "c4", "value": "X" },
                                                { "type": "setValue", "target": "lastMove", "value": 4 },
                                                { "type": "submitToAgent", "eventName": "game.move", "includeFields": ["c0", "c1", "c2", "c3", "c4", "c5", "c6", "c7", "c8", "lastMove", "turn"] }
                                            ]
                                        },
                                        {
                                            "id": "ttt-c5",
                                            "type": "button",
                                            "props": { "label": "·", "valueKey": "c5" },
                                            "actions": [
                                                { "type": "setValue", "target": "c5", "value": "X" },
                                                { "type": "setValue", "target": "lastMove", "value": 5 },
                                                { "type": "submitToAgent", "eventName": "game.move", "includeFields": ["c0", "c1", "c2", "c3", "c4", "c5", "c6", "c7", "c8", "lastMove", "turn"] }
                                            ]
                                        }
                                    ]
                                },
                                {
                                    "id": "ttt-row-2",
                                    "type": "row",
                                    "props": {},
                                    "children": [
                                        {
                                            "id": "ttt-c6",
                                            "type": "button",
                                            "props": { "label": "·", "valueKey": "c6" },
                                            "actions": [
                                                { "type": "setValue", "target": "c6", "value": "X" },
                                                { "type": "setValue", "target": "lastMove", "value": 6 },
                                                { "type": "submitToAgent", "eventName": "game.move", "includeFields": ["c0", "c1", "c2", "c3", "c4", "c5", "c6", "c7", "c8", "lastMove", "turn"] }
                                            ]
                                        },
                                        {
                                            "id": "ttt-c7",
                                            "type": "button",
                                            "props": { "label": "·", "valueKey": "c7" },
                                            "actions": [
                                                { "type": "setValue", "target": "c7", "value": "X" },
                                                { "type": "setValue", "target": "lastMove", "value": 7 },
                                                { "type": "submitToAgent", "eventName": "game.move", "includeFields": ["c0", "c1", "c2", "c3", "c4", "c5", "c6", "c7", "c8", "lastMove", "turn"] }
                                            ]
                                        },
                                        {
                                            "id": "ttt-c8",
                                            "type": "button",
                                            "props": { "label": "·", "valueKey": "c8" },
                                            "actions": [
                                                { "type": "setValue", "target": "c8", "value": "X" },
                                                { "type": "setValue", "target": "lastMove", "value": 8 },
                                                { "type": "submitToAgent", "eventName": "game.move", "includeFields": ["c0", "c1", "c2", "c3", "c4", "c5", "c6", "c7", "c8", "lastMove", "turn"] }
                                            ]
                                        }
                                    ]
                                },
                                {
                                    "id": "ttt-reset",
                                    "type": "button",
                                    "props": { "label": "New Game", "variant": "secondary" },
                                    "actions": [
                                        { "type": "submitToAgent", "eventName": "game.reset", "includeFields": [] }
                                    ]
                                }
                            ]
                        }
                    }
                ],
                "diagnostics": { "fixture": "tictactoe_create" }
            }).to_string();
        }

        if lower.contains("chess") {
            return json!({
                "schemaVersion": "2",
                "assistantMessage": "I built an interactive Chess board for you! Standard rules apply with legal moves, turn tracking, and check detection.",
                "responseType": "message",
                "operations": [
                    {
                        "id": "op-create-chess",
                        "type": "surface.create",
                        "target": {},
                        "payload": {
                            "id": "tool-chess",
                            "name": "Chess vs AI",
                            "description": "Play Chess with authoritative rule validation",
                            "layout": { "type": "dashboard", "columns": 2 },
                            "stateContracts": [
                                { "key": "currentPlayer", "type": "string", "initialValue": "white", "writePolicy": "model" },
                                { "key": "turn", "type": "integer", "initialValue": 1, "writePolicy": "model" },
                                { "key": "status", "type": "string", "initialValue": "Your turn (white)", "writePolicy": "model" },
                                { "key": "inCheck", "type": "boolean", "initialValue": false, "writePolicy": "model" }
                            ],
                            "components": [
                                { "id": "chess-heading", "type": "heading", "props": { "text": "Chess vs AI", "level": 1 } },
                                { "id": "chess-status", "type": "badge", "props": { "text": "Your turn (White)", "valueKey": "status" } },
                                { "id": "chess-board", "type": "container", "props": { "padding": "md" } },
                                {
                                    "id": "chess-reset",
                                    "type": "button",
                                    "props": { "label": "New Game", "variant": "secondary" },
                                    "actions": [
                                        { "type": "submitToAgent", "eventName": "game.reset", "includeFields": [] }
                                    ]
                                }
                            ]
                        }
                    }
                ],
                "diagnostics": { "fixture": "chess_create" }
            }).to_string();
        }

        if lower.contains("connect-four")
            || lower.contains("connect four")
            || lower.contains("connectfour")
        {
            return json!({
                "schemaVersion": "2",
                "assistantMessage": "I created a Connect Four game! Choose a column to drop your chip.",
                "responseType": "message",
                "operations": [
                    {
                        "id": "op-create-connect-four",
                        "type": "surface.create",
                        "target": {},
                        "payload": {
                            "id": "tool-connect-four",
                            "name": "Connect Four",
                            "description": "Classic 4-in-a-row chip dropping game",
                            "layout": { "type": "single-column" },
                            "stateContracts": [
                                { "key": "currentPlayer", "type": "string", "initialValue": "red", "writePolicy": "model" },
                                { "key": "turn", "type": "integer", "initialValue": 1, "writePolicy": "model" },
                                { "key": "status", "type": "string", "initialValue": "playing", "writePolicy": "model" }
                            ],
                            "components": [
                                { "id": "c4-heading", "type": "heading", "props": { "text": "Connect Four", "level": 1 } },
                                { "id": "c4-status", "type": "badge", "props": { "text": "Red's turn", "valueKey": "status" } },
                                {
                                    "id": "c4-reset",
                                    "type": "button",
                                    "props": { "label": "Restart", "variant": "secondary" },
                                    "actions": [
                                        { "type": "submitToAgent", "eventName": "game.reset", "includeFields": [] }
                                    ]
                                }
                            ]
                        }
                    }
                ],
                "diagnostics": { "fixture": "connect_four_create" }
            }).to_string();
        }

        if lower.contains("2048") {
            return json!({
                "schemaVersion": "2",
                "assistantMessage": "Here is 2048! Use directional controls or arrows to slide and merge matching numbers.",
                "responseType": "message",
                "operations": [
                    {
                        "id": "op-create-2048",
                        "type": "surface.create",
                        "target": {},
                        "payload": {
                            "id": "tool-2048",
                            "name": "2048 Puzzle",
                            "description": "Slide matching tiles to reach 2048",
                            "layout": { "type": "single-column" },
                            "stateContracts": [
                                { "key": "score", "type": "integer", "initialValue": 0, "writePolicy": "model" },
                                { "key": "moves", "type": "integer", "initialValue": 0, "writePolicy": "model" },
                                { "key": "status", "type": "string", "initialValue": "playing", "writePolicy": "model" }
                            ],
                            "components": [
                                { "id": "g2048-heading", "type": "heading", "props": { "text": "2048", "level": 1 } },
                                { "id": "g2048-score", "type": "stat", "props": { "label": "Score", "valueKey": "score" } },
                                {
                                    "id": "g2048-reset",
                                    "type": "button",
                                    "props": { "label": "New Game", "variant": "secondary" },
                                    "actions": [
                                        { "type": "submitToAgent", "eventName": "game.reset", "includeFields": [] }
                                    ]
                                }
                            ]
                        }
                    }
                ],
                "diagnostics": { "fixture": "2048_create" }
            }).to_string();
        }

        if lower.contains("minesweeper") {
            return json!({
                "schemaVersion": "2",
                "assistantMessage": "I built Minesweeper! Click squares to reveal them and flag suspect mine locations.",
                "responseType": "message",
                "operations": [
                    {
                        "id": "op-create-minesweeper",
                        "type": "surface.create",
                        "target": {},
                        "payload": {
                            "id": "tool-minesweeper",
                            "name": "Minesweeper",
                            "description": "Clear the minefield without detonating any mines",
                            "layout": { "type": "single-column" },
                            "stateContracts": [
                                { "key": "minesRemaining", "type": "integer", "initialValue": 10, "writePolicy": "model" },
                                { "key": "status", "type": "string", "initialValue": "playing", "writePolicy": "model" }
                            ],
                            "components": [
                                { "id": "ms-heading", "type": "heading", "props": { "text": "Minesweeper", "level": 1 } },
                                { "id": "ms-stat", "type": "stat", "props": { "label": "Mines Left", "valueKey": "minesRemaining" } },
                                {
                                    "id": "ms-reset",
                                    "type": "button",
                                    "props": { "label": "Restart", "variant": "secondary" },
                                    "actions": [
                                        { "type": "submitToAgent", "eventName": "game.reset", "includeFields": [] }
                                    ]
                                }
                            ]
                        }
                    }
                ],
                "diagnostics": { "fixture": "minesweeper_create" }
            }).to_string();
        }

        if lower.contains("calculator") {
            return json!({
                "schemaVersion": "2",
                "assistantMessage": "Here is an interactive calculator with numeric keypad and arithmetic operations.",
                "responseType": "message",
                "operations": [
                    {
                        "id": "op-create-calculator",
                        "type": "surface.create",
                        "target": {},
                        "payload": {
                            "id": "tool-calculator",
                            "name": "Calculator",
                            "description": "Interactive arithmetic calculator",
                            "layout": { "type": "single-column" },
                            "stateContracts": [
                                { "key": "display", "type": "string", "initialValue": "0", "writePolicy": "model" },
                                { "key": "acc", "type": "number", "initialValue": 0.0, "writePolicy": "model" }
                            ],
                            "components": [
                                { "id": "calc-heading", "type": "heading", "props": { "text": "Calculator", "level": 1 } },
                                { "id": "calc-display", "type": "stat", "props": { "label": "Result", "valueKey": "display" } },
                                {
                                    "id": "calc-clear",
                                    "type": "button",
                                    "props": { "label": "Clear", "variant": "secondary" },
                                    "actions": [
                                        { "type": "setValue", "target": "display", "value": "0" },
                                        { "type": "submitToAgent", "eventName": "calc.clear", "includeFields": [] }
                                    ]
                                }
                            ]
                        }
                    }
                ],
                "diagnostics": { "fixture": "calculator_create" }
            }).to_string();
        }

        if lower.contains("sudoku") {
            return json!({
                "schemaVersion": "2",
                "assistantMessage": "I generated a Sudoku puzzle! Fill in the 9x9 grid with numbers 1 through 9.",
                "responseType": "message",
                "operations": [
                    {
                        "id": "op-create-sudoku",
                        "type": "surface.create",
                        "target": {},
                        "payload": {
                            "id": "tool-sudoku",
                            "name": "Sudoku",
                            "description": "Standard 9x9 Sudoku logic puzzle",
                            "layout": { "type": "single-column" },
                            "stateContracts": [
                                { "key": "remainingEmpty", "type": "integer", "initialValue": 45, "writePolicy": "model" },
                                { "key": "status", "type": "string", "initialValue": "playing", "writePolicy": "model" }
                            ],
                            "components": [
                                { "id": "sdk-heading", "type": "heading", "props": { "text": "Sudoku", "level": 1 } },
                                { "id": "sdk-stat", "type": "stat", "props": { "label": "Empty Cells", "valueKey": "remainingEmpty" } },
                                {
                                    "id": "sdk-reset",
                                    "type": "button",
                                    "props": { "label": "Reset Clues", "variant": "secondary" },
                                    "actions": [
                                        { "type": "submitToAgent", "eventName": "game.reset", "includeFields": [] }
                                    ]
                                }
                            ]
                        }
                    }
                ],
                "diagnostics": { "fixture": "sudoku_create" }
            }).to_string();
        }

        Self::fixture_for_with_revision(
            user_text,
            Self::observed_definition_revision(&request.system_prompt),
        )
    }
}

impl Default for MockAiProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AiProvider for MockAiProvider {
    fn provider_id(&self) -> &str {
        "mock"
    }

    fn display_name(&self) -> &str {
        "Mock AI"
    }

    async fn health_check(&self, cancel: CancellationToken) -> Result<ProviderHealth, AiError> {
        if cancel.is_cancelled() {
            return Err(AiError::Cancelled);
        }
        Ok(ProviderHealth {
            ok: true,
            message: "Mock provider ready".into(),
            models: vec!["mock-fixture".into()],
        })
    }

    async fn chat(&self, request: AgentRequest) -> Result<AgentResponse, AiError> {
        if request.cancel.is_cancelled() {
            return Err(AiError::Cancelled);
        }

        // Tiny yield so cancellation can race in tests.
        tokio::task::yield_now().await;
        if request.cancel.is_cancelled() {
            return Err(AiError::Cancelled);
        }

        Ok(AgentResponse {
            raw_text: Self::fixture_for_request(&request),
            usage: UsageMetadata {
                prompt_tokens: Some(10),
                completion_tokens: Some(50),
                total_tokens: Some(60),
            },
            model: "mock-fixture".into(),
            provider_id: self.provider_id().to_string(),
        })
    }

    /// Live stream fixtures:
    /// - `live stream probe` — text-only progressive TextDelta (TS-1)
    /// - `progressive op preview` — NDJSON StreamEvent op frame before completion (RC3.3)
    /// - `progressive surface preview` — paint-capable `state.set` before completion (P0.3)
    async fn chat_stream(
        &self,
        request: AgentRequest,
        tx: ProviderStreamTx,
    ) -> Result<AgentResponse, AiError> {
        let user_text = request
            .messages
            .iter()
            .rev()
            .find(|m| m.role == crate::ai::AgentRole::User)
            .map(|m| m.content.as_str())
            .unwrap_or("");
        let lower = user_text.to_lowercase();
        let progressive_surface = lower.contains("progressive surface preview");
        let progressive_ops = lower.contains("progressive op preview");
        let live = progressive_surface || progressive_ops || lower.contains("live stream probe");
        if request.cancel.is_cancelled() {
            let _ = tx.send(ProviderStreamEvent::ResponseCancelled).await;
            return Err(AiError::Cancelled);
        }

        if !live {
            // Honest buffered fallback — no fake TextDelta (same contract as trait default).
            let _ = tx
                .send(ProviderStreamEvent::ResponseStarted {
                    provider_id: self.provider_id().to_string(),
                    model: String::new(),
                    live: false,
                })
                .await;
            let response = self.chat(request).await.map_err(|err| {
                // Best-effort terminal event; ignore send failures on closed channel.
                let ev = if matches!(err, AiError::Cancelled) {
                    ProviderStreamEvent::ResponseCancelled
                } else {
                    ProviderStreamEvent::ResponseFailed {
                        code: err.code().to_string(),
                        message: err.to_string(),
                    }
                };
                let _ = tx.try_send(ev);
                err
            })?;
            let _ = tx
                .send(ProviderStreamEvent::TextCompleted {
                    text: response.raw_text.clone(),
                })
                .await;
            let _ = tx
                .send(ProviderStreamEvent::ResponseCompleted {
                    response: response.clone(),
                    buffered: true,
                })
                .await;
            return Ok(response);
        }

        if progressive_surface {
            return Self::stream_progressive_surface_preview(request, tx).await;
        }
        if progressive_ops {
            return Self::stream_progressive_op_preview(request, tx).await;
        }

        let raw = json!({
            "schemaVersion": SCHEMA_VERSION,
            "assistantMessage": "Live stream probe complete",
            "responseType": "message",
            "diagnostics": { "fixture": "live_stream_probe" }
        })
        .to_string();

        let _ = tx
            .send(ProviderStreamEvent::ResponseStarted {
                provider_id: self.provider_id().to_string(),
                model: "mock-fixture".into(),
                live: true,
            })
            .await;
        let _ = tx
            .send(ProviderStreamEvent::TextDelta {
                text: r#"{"assistantMessage":"Live stream"#.into(),
            })
            .await;

        tokio::select! {
            _ = request.cancel.cancelled() => {
                let _ = tx.send(ProviderStreamEvent::ResponseCancelled).await;
                return Err(AiError::Cancelled);
            }
            // Keep the live DOM mounted long enough for Journey 12 WebDriver.
            _ = tokio::time::sleep(std::time::Duration::from_millis(600)) => {}
        }

        let _ = tx
            .send(ProviderStreamEvent::TextDelta {
                text: r#" probe complete","responseType":"message"}"#.into(),
            })
            .await;

        let response = AgentResponse {
            raw_text: raw.clone(),
            usage: UsageMetadata {
                prompt_tokens: Some(10),
                completion_tokens: Some(20),
                total_tokens: Some(30),
            },
            model: "mock-fixture".into(),
            provider_id: self.provider_id().to_string(),
        };
        let _ = tx
            .send(ProviderStreamEvent::TextCompleted { text: raw })
            .await;
        let _ = tx
            .send(ProviderStreamEvent::ResponseCompleted {
                response: response.clone(),
                buffered: false,
            })
            .await;
        Ok(response)
    }
}

impl MockAiProvider {
    /// Paint-capable progressive preview for seeded E2E Notes (`surf-tool-e2e-notes`).
    async fn stream_progressive_surface_preview(
        request: AgentRequest,
        tx: ProviderStreamTx,
    ) -> Result<AgentResponse, AiError> {
        let paint_op = json!({
            "id": "op-progressive-surface-preview",
            "type": "state.set",
            "target": {
                "surfaceId": "surf-tool-e2e-notes",
                "toolId": "tool-e2e-notes"
            },
            "payload": {
                "state": { "note": "progressive preview note" }
            }
        });
        let start = json!({
            "v": "coreside.ops.v1",
            "type": "start",
            "groupId": "g-progressive-surface-preview",
            "schemaVersion": "2",
            "capabilityVersion": "1"
        })
        .to_string()
            + "\n";
        let op = json!({
            "v": "coreside.ops.v1",
            "type": "op",
            "frameId": 1,
            "groupId": "g-progressive-surface-preview",
            "op": paint_op.clone()
        })
        .to_string()
            + "\n";
        let complete = json!({
            "v": "coreside.ops.v1",
            "type": "complete",
            "frameId": 2,
            "groupId": "g-progressive-surface-preview"
        })
        .to_string()
            + "\n";

        // Final durable ops must match the progressive set (empty [] would diverge).
        let raw = json!({
            "schemaVersion": "2",
            "assistantMessage": "Progressive surface preview complete",
            "responseType": "message",
            "operations": [paint_op],
            "diagnostics": { "fixture": "progressive_surface_preview" }
        })
        .to_string();

        let _ = tx
            .send(ProviderStreamEvent::ResponseStarted {
                provider_id: "mock".into(),
                model: "mock-fixture".into(),
                live: true,
            })
            .await;
        // Send a newline-terminated start+op before the hold. A byte-midpoint
        // split leaves `state.set` incomplete, so Preview never paints during sleep.
        let _ = tx
            .send(ProviderStreamEvent::TextDelta {
                text: format!("{start}{op}"),
            })
            .await;

        // Hold after the paint-capable op so desktop E2E can observe Preview
        // and (Journey 20) click Cancel before durable complete.
        let hold_ms = if std::env::var("CORESIDE_E2E").is_ok() {
            4500
        } else {
            200
        };
        tokio::select! {
            _ = request.cancel.cancelled() => {
                let _ = tx.send(ProviderStreamEvent::ResponseCancelled).await;
                return Err(AiError::Cancelled);
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(hold_ms)) => {}
        }

        let _ = tx
            .send(ProviderStreamEvent::TextDelta { text: complete })
            .await;

        let response = AgentResponse {
            raw_text: raw.clone(),
            usage: UsageMetadata {
                prompt_tokens: Some(10),
                completion_tokens: Some(28),
                total_tokens: Some(38),
            },
            model: "mock-fixture".into(),
            provider_id: "mock".into(),
        };
        let _ = tx
            .send(ProviderStreamEvent::TextCompleted { text: raw.clone() })
            .await;
        let _ = tx
            .send(ProviderStreamEvent::ResponseCompleted {
                response: response.clone(),
                buffered: false,
            })
            .await;
        Ok(response)
    }

    /// NDJSON `coreside.ops.v1` frames arrive as TextDelta before ResponseCompleted.
    async fn stream_progressive_op_preview(
        request: AgentRequest,
        tx: ProviderStreamTx,
    ) -> Result<AgentResponse, AiError> {
        let start = json!({
            "v": "coreside.ops.v1",
            "type": "start",
            "groupId": "g-progressive-preview",
            "schemaVersion": "2",
            "capabilityVersion": "1"
        })
        .to_string()
            + "\n";
        let op = json!({
            "v": "coreside.ops.v1",
            "type": "op",
            "frameId": 1,
            "groupId": "g-progressive-preview",
            "op": {
                "id": "op-progressive-preview",
                "type": "chat.status",
                "target": {},
                "payload": { "message": "progressive preview" }
            }
        })
        .to_string()
            + "\n";
        let complete = json!({
            "v": "coreside.ops.v1",
            "type": "complete",
            "frameId": 2,
            "groupId": "g-progressive-preview"
        })
        .to_string()
            + "\n";
        let stream_body = format!("{start}{op}{complete}");

        let raw = json!({
            // Runtime V2 apply path requires schemaVersion "2" (distinct from SCHEMA_VERSION="1").
            "schemaVersion": "2",
            "assistantMessage": "Progressive op preview complete",
            "responseType": "message",
            "operations": [],
            "diagnostics": { "fixture": "progressive_op_preview" }
        })
        .to_string();

        let _ = tx
            .send(ProviderStreamEvent::ResponseStarted {
                provider_id: "mock".into(),
                model: "mock-fixture".into(),
                live: true,
            })
            .await;
        // Split the stream across two deltas so the parser buffers until newline.
        let mid = stream_body.len() / 2;
        let _ = tx
            .send(ProviderStreamEvent::TextDelta {
                text: stream_body[..mid].to_string(),
            })
            .await;

        tokio::select! {
            _ = request.cancel.cancelled() => {
                let _ = tx.send(ProviderStreamEvent::ResponseCancelled).await;
                return Err(AiError::Cancelled);
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(80)) => {}
        }

        let _ = tx
            .send(ProviderStreamEvent::TextDelta {
                text: stream_body[mid..].to_string(),
            })
            .await;

        let response = AgentResponse {
            raw_text: raw.clone(),
            usage: UsageMetadata {
                prompt_tokens: Some(10),
                completion_tokens: Some(24),
                total_tokens: Some(34),
            },
            model: "mock-fixture".into(),
            provider_id: "mock".into(),
        };
        let _ = tx
            .send(ProviderStreamEvent::TextCompleted { text: raw.clone() })
            .await;
        let _ = tx
            .send(ProviderStreamEvent::ResponseCompleted {
                response: response.clone(),
                buffered: false,
            })
            .await;
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::response_schema::{ActionDefinition, ResponseType};
    use crate::ai::{parse_agent_response, AgentMessage};

    #[tokio::test]
    async fn water_fixture_parses_as_tool_change() {
        let provider = MockAiProvider::new();
        let response = provider
            .chat(AgentRequest {
                system_prompt: "test".into(),
                messages: vec![AgentMessage::text(
                    crate::ai::AgentRole::User,
                    "Create a simple water tracker",
                )],
                cancel: CancellationToken::new(),
                idempotency_key: None,
            })
            .await
            .unwrap();
        let parsed = parse_agent_response(&response.raw_text).unwrap();
        assert_eq!(parsed.payload.response_type, ResponseType::ToolChange);
        let tool = parsed
            .payload
            .tool_change
            .as_ref()
            .unwrap()
            .tool
            .as_ref()
            .unwrap();
        assert_eq!(tool.id, "tool-water-tracker");
        assert!(tool
            .components
            .iter()
            .any(|c| c.component_type == "counter"));
    }

    #[tokio::test]
    async fn live_stream_probe_emits_delta_before_completion() {
        let provider = MockAiProvider::new();
        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        let cancel = CancellationToken::new();
        let request = AgentRequest {
            system_prompt: "test".into(),
            messages: vec![AgentMessage::text(
                crate::ai::AgentRole::User,
                "please run live stream probe now",
            )],
            cancel: cancel.clone(),
            idempotency_key: Some("probe".into()),
        };
        let started = std::time::Instant::now();
        let join = tokio::spawn(async move { provider.chat_stream(request, tx).await });
        let mut first_delta_at = None;
        while let Some(ev) = rx.recv().await {
            if matches!(ev, ProviderStreamEvent::TextDelta { .. }) && first_delta_at.is_none() {
                first_delta_at = Some(started.elapsed());
            }
            if matches!(ev, ProviderStreamEvent::ResponseCompleted { .. }) {
                break;
            }
        }
        let _ = join.await.unwrap().unwrap();
        let first = first_delta_at.expect("expected live TextDelta");
        assert!(
            first < std::time::Duration::from_millis(80),
            "probe delta should arrive before delayed completion, got {first:?}"
        );
    }

    #[tokio::test]
    async fn progressive_op_preview_completes_ndjson_frame_before_response_completed() {
        use crate::runtime_v2::{
            finish_progressive_ingest, ingest_progressive_chunk_with_seed, PreviewTransaction,
            ProgressiveOpsExpect, ProgressiveOpsParser,
        };

        let provider = MockAiProvider::new();
        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        let request = AgentRequest {
            system_prompt: "test".into(),
            messages: vec![AgentMessage::text(
                crate::ai::AgentRole::User,
                "please run progressive op preview now",
            )],
            cancel: CancellationToken::new(),
            idempotency_key: Some("progressive".into()),
        };
        let join = tokio::spawn(async move { provider.chat_stream(request, tx).await });

        let mut parser = ProgressiveOpsParser::new(ProgressiveOpsExpect {
            capability_version: Some("1".into()),
            ..Default::default()
        });
        let mut preview = PreviewTransaction::new("test-turn", None);
        let mut saw_preview = false;
        let mut preview_before_complete = false;

        while let Some(ev) = rx.recv().await {
            match ev {
                ProviderStreamEvent::TextDelta { text } => {
                    let events = ingest_progressive_chunk_with_seed(
                        &mut parser,
                        &mut preview,
                        &text,
                        |_| None,
                    );
                    if events.iter().any(|e| e.status == "preview") {
                        saw_preview = true;
                    }
                }
                ProviderStreamEvent::ResponseCompleted { .. } => {
                    preview_before_complete = saw_preview;
                    break;
                }
                _ => {}
            }
        }
        let response = join.await.unwrap().unwrap();
        let _ = finish_progressive_ingest(&mut parser, &mut preview);
        assert!(
            preview_before_complete,
            "Operation preview must fire before ResponseCompleted"
        );
        assert_eq!(
            parser.durable_operations().map(|o| o.len()),
            Some(1),
            "complete terminal must authorize durable ops"
        );
        assert_eq!(
            parser.durable_operations().unwrap()[0].id,
            "op-progressive-preview"
        );
        assert!(response
            .raw_text
            .contains("Progressive op preview complete"));
        let parsed = parse_agent_response(&response.raw_text).unwrap();
        assert_eq!(parsed.payload.schema_version, "2");
        assert!(parsed
            .payload
            .assistant_message
            .contains("Progressive op preview complete"));
    }

    #[tokio::test]
    async fn progressive_surface_preview_paints_before_response_completed() {
        use crate::runtime_v2::{
            finish_progressive_ingest, ingest_progressive_chunk_with_seed, PreviewSurfaceModel,
            PreviewTransaction, ProgressiveOpsExpect, ProgressiveOpsParser,
        };

        let provider = MockAiProvider::new();
        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        let request = AgentRequest {
            system_prompt: "test".into(),
            messages: vec![AgentMessage::text(
                crate::ai::AgentRole::User,
                "please run progressive surface preview now",
            )],
            cancel: CancellationToken::new(),
            idempotency_key: Some("progressive-surface".into()),
        };
        let join = tokio::spawn(async move { provider.chat_stream(request, tx).await });

        let mut parser = ProgressiveOpsParser::new(ProgressiveOpsExpect {
            capability_version: Some("1".into()),
            ..Default::default()
        });
        let mut preview = PreviewTransaction::new("test-turn-surface", None);
        let mut saw_paint = false;
        let mut paint_before_complete = false;

        while let Some(ev) = rx.recv().await {
            match ev {
                ProviderStreamEvent::TextDelta { text } => {
                    let events = ingest_progressive_chunk_with_seed(
                        &mut parser,
                        &mut preview,
                        &text,
                        |sid| {
                            if sid == "surf-tool-e2e-notes" {
                                Some(PreviewSurfaceModel {
                                    surface_id: sid.to_string(),
                                    tool_id: Some("tool-e2e-notes".into()),
                                    application_id: Some("tool-e2e-notes".into()),
                                    capability_packs: vec!["coreside.core".into()],
                                    // Match seeded E2E Notes: opaque keys with prior state
                                    // require a declared model-writable contract to paint.
                                    definition: json!({
                                        "id": "tool-e2e-notes",
                                        "name": "E2E Notes",
                                        "stateContracts": [{
                                            "key": "note",
                                            "type": "string",
                                            "initialValue": "",
                                            "writePolicy": "model"
                                        }],
                                        "components": [{
                                            "id": "e2e-note-input",
                                            "type": "textInput",
                                            "valueKey": "note",
                                            "props": {
                                                "label": "Note",
                                                "placeholder": "Type a note"
                                            }
                                        }]
                                    }),
                                    state: json!({ "note": "" }),
                                    base_revision: 1,
                                    preview_revision: 1,
                                })
                            } else {
                                None
                            }
                        },
                    );
                    if events.iter().any(|e| e.paint.is_some()) {
                        saw_paint = true;
                    } else if events.iter().any(|e| e.status == "rejected" || e.status == "fatal")
                    {
                        let reasons: Vec<_> = events
                            .iter()
                            .filter_map(|e| e.reason.as_deref())
                            .collect();
                        panic!(
                            "progressive surface preview rejected before paint: {}",
                            reasons.join("; ")
                        );
                    }
                }
                ProviderStreamEvent::ResponseCompleted { .. } => {
                    paint_before_complete = saw_paint;
                    break;
                }
                _ => {}
            }
        }
        let response = join.await.unwrap().unwrap();
        let _ = finish_progressive_ingest(&mut parser, &mut preview);
        assert!(
            paint_before_complete,
            "surface paint must arrive before ResponseCompleted"
        );
        assert_eq!(
            parser.durable_operations().map(|o| o.len()),
            Some(1),
            "complete terminal must authorize durable paint op"
        );
        assert_eq!(
            parser.durable_operations().unwrap()[0].id,
            "op-progressive-surface-preview"
        );
        assert!(response
            .raw_text
            .contains("Progressive surface preview complete"));
    }

    #[tokio::test]
    async fn progressive_surface_preview_cancel_during_hold_returns_cancelled() {
        let provider = MockAiProvider::new();
        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        let cancel = CancellationToken::new();
        let request = AgentRequest {
            system_prompt: "test".into(),
            messages: vec![AgentMessage::text(
                crate::ai::AgentRole::User,
                "please run progressive surface preview now",
            )],
            cancel: cancel.clone(),
            idempotency_key: Some("progressive-surface-cancel".into()),
        };
        let join = tokio::spawn(async move { provider.chat_stream(request, tx).await });

        let mut saw_paint_delta = false;
        let mut saw_cancelled = false;
        let mut saw_completed = false;
        while let Some(ev) = rx.recv().await {
            match ev {
                ProviderStreamEvent::TextDelta { text } => {
                    if text.contains("op-progressive-surface-preview") {
                        saw_paint_delta = true;
                        cancel.cancel();
                    }
                }
                ProviderStreamEvent::ResponseCancelled => saw_cancelled = true,
                ProviderStreamEvent::ResponseCompleted { .. } => saw_completed = true,
                _ => {}
            }
        }
        let result = join.await.unwrap();
        assert!(saw_paint_delta, "paint op must arrive before cancel");
        assert!(
            matches!(result, Err(AiError::Cancelled)),
            "cancel during hold must return Cancelled"
        );
        assert!(saw_cancelled, "must emit ResponseCancelled");
        assert!(
            !saw_completed,
            "must not emit ResponseCompleted after cancel"
        );
    }

    #[tokio::test]
    async fn buffered_chat_stream_has_no_fake_deltas() {
        let provider = MockAiProvider::new();
        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        let request = AgentRequest {
            system_prompt: "test".into(),
            messages: vec![AgentMessage::text(crate::ai::AgentRole::User, "hello")],
            cancel: CancellationToken::new(),
            idempotency_key: None,
        };
        let join = tokio::spawn(async move { provider.chat_stream(request, tx).await });
        let mut saw_delta = false;
        let mut buffered_complete = false;
        while let Some(ev) = rx.recv().await {
            match ev {
                ProviderStreamEvent::TextDelta { .. } => saw_delta = true,
                ProviderStreamEvent::ResponseCompleted { buffered, .. } => {
                    buffered_complete = buffered;
                    break;
                }
                _ => {}
            }
        }
        let _ = join.await.unwrap().unwrap();
        assert!(!saw_delta, "buffered path must not fabricate TextDelta");
        assert!(buffered_complete);
    }

    #[tokio::test]
    async fn quiz_fixture_has_three_questions() {
        let provider = MockAiProvider::new();
        let response = provider
            .chat(AgentRequest {
                system_prompt: "test".into(),
                messages: vec![AgentMessage::text(
                    crate::ai::AgentRole::User,
                    "Create a geography quiz",
                )],
                cancel: CancellationToken::new(),
                idempotency_key: None,
            })
            .await
            .unwrap();
        let parsed = parse_agent_response(&response.raw_text).unwrap();
        let tool = parsed
            .payload
            .tool_change
            .as_ref()
            .unwrap()
            .tool
            .as_ref()
            .unwrap();
        let quiz = tool
            .components
            .iter()
            .find(|c| c.component_type == "quiz")
            .unwrap();
        let questions = quiz.props.as_ref().unwrap()["questions"]
            .as_array()
            .unwrap();
        assert_eq!(questions.len(), 3);
    }

    #[tokio::test]
    async fn task_tracker_fixture_generates_local_data_crud_tool() {
        let provider = MockAiProvider::new();
        let response = provider
            .chat(AgentRequest {
                system_prompt: "test".into(),
                messages: vec![AgentMessage::text(
                    crate::ai::AgentRole::User,
                    "Build me a simple task tracker",
                )],
                cancel: CancellationToken::new(),
                idempotency_key: None,
            })
            .await
            .unwrap();
        let parsed = parse_agent_response(&response.raw_text).unwrap();
        assert!(
            parsed.payload.application_plan.is_some(),
            "task tracker fixture must emit ApplicationPlan"
        );
        let plan = parsed.payload.application_plan.as_ref().unwrap();
        assert_eq!(
            plan.kind,
            crate::application_kernel::application_plan::ApplicationPlanKind::Create
        );
        let ops = parsed.payload.normalized_operations().unwrap();
        assert!(
            ops.iter().any(|o| o.op_type == "surface.create"),
            "plan must compile to surface.create"
        );
        assert!(
            ops.iter().any(|o| o.op_type == "data.model_upsert"),
            "plan must compile to data.model_upsert"
        );
        let tc = parsed.payload.tool_change.as_ref().unwrap();
        let tool = tc.tool.as_ref().unwrap();
        assert_eq!(tool.id, "tool-task-tracker");
        assert_eq!(tool.name, "Task Tracker");
        let table = tool
            .components
            .iter()
            .find(|c| c.component_type == "dataTable")
            .expect("dataTable component");
        assert_eq!(table.props.as_ref().unwrap()["rowsKey"], "tasksResult");
        assert_eq!(
            table.props.as_ref().unwrap()["dataSource"]["actionName"],
            "local_data.query"
        );
        let add_btn = tool
            .components
            .iter()
            .find(|c| c.id == "tm-add-btn")
            .expect("tm-add-btn");
        let actions = add_btn.actions.as_ref().unwrap();
        assert!(actions.iter().any(|a| {
            matches!(
                a,
                ActionDefinition::InvokeRegisteredAction { action_name, .. }
                    if action_name == "local_data.write"
            )
        }));
        assert!(actions.iter().any(|a| {
            matches!(
                a,
                ActionDefinition::InvokeRegisteredAction { action_name, .. }
                    if action_name == "local_data.query"
            )
        }));
    }

    #[tokio::test]
    async fn due_dates_evolution_fixture_emits_evolve_plan() {
        let provider = MockAiProvider::new();
        let response = provider
            .chat(AgentRequest {
                system_prompt: "test".into(),
                messages: vec![AgentMessage::text(
                    crate::ai::AgentRole::User,
                    "Add due dates to my task tracker",
                )],
                cancel: CancellationToken::new(),
                idempotency_key: None,
            })
            .await
            .unwrap();
        let parsed = parse_agent_response(&response.raw_text).unwrap();
        let plan = parsed
            .payload
            .application_plan
            .as_ref()
            .expect("evolve fixture must emit ApplicationPlan");
        assert_eq!(
            plan.kind,
            crate::application_kernel::application_plan::ApplicationPlanKind::Evolve
        );
        let ops = parsed.payload.normalized_operations().unwrap();
        assert!(ops.iter().any(|o| o.op_type == "data.model_upsert"));
        assert!(ops
            .iter()
            .any(|o| o.payload.get("action") == Some(&serde_json::json!("update"))));
    }

    #[tokio::test]
    async fn create_prompt_with_due_dates_still_emits_create_plan() {
        let provider = MockAiProvider::new();
        let response = provider
            .chat(AgentRequest {
                system_prompt: "test".into(),
                messages: vec![AgentMessage::text(
                    crate::ai::AgentRole::User,
                    "Build me a task tracker with due dates",
                )],
                cancel: CancellationToken::new(),
                idempotency_key: None,
            })
            .await
            .unwrap();
        let parsed = parse_agent_response(&response.raw_text).unwrap();
        let plan = parsed
            .payload
            .application_plan
            .as_ref()
            .expect("create+due-dates prompt must still emit ApplicationPlan");
        assert_eq!(
            plan.kind,
            crate::application_kernel::application_plan::ApplicationPlanKind::Create,
            "create-ish prompts must not be stolen by the evolve due-dates fixture"
        );
    }

    #[tokio::test]
    async fn expense_tracker_fixture_generates_valid_tool() {
        let provider = MockAiProvider::new();
        let response = provider
            .chat(AgentRequest {
                system_prompt: "test".into(),
                messages: vec![AgentMessage::text(
                    crate::ai::AgentRole::User,
                    "Create an expense tracker for daily budget",
                )],
                cancel: CancellationToken::new(),
                idempotency_key: None,
            })
            .await
            .unwrap();
        let parsed = parse_agent_response(&response.raw_text).unwrap();
        let tc = parsed.payload.tool_change.as_ref().unwrap();
        let tool = tc.tool.as_ref().unwrap();
        assert_eq!(tool.id, "tool-expense-tracker");
        assert_eq!(tool.name, "Expense Tracker");
        let mut tool = tool.clone();
        tool.normalize_for_frontend();
        let stat = tool
            .components
            .iter()
            .find(|c| c.component_type == "stat")
            .expect("stat component");
        assert_eq!(stat.value_key.as_deref(), Some("totalExpenses"));
    }

    #[tokio::test]
    async fn tictactoe_fixture_generates_valid_surface() {
        use crate::ai::ToolDefinition;
        let provider = MockAiProvider::new();
        let response = provider
            .chat(AgentRequest {
                system_prompt: "test".into(),
                messages: vec![AgentMessage::text(
                    crate::ai::AgentRole::User,
                    "Build me a Tic-Tac-Toe game where I play against you",
                )],
                cancel: CancellationToken::new(),
                idempotency_key: None,
            })
            .await
            .unwrap();
        let parsed = parse_agent_response(&response.raw_text).unwrap();
        assert_eq!(parsed.payload.schema_version, "2");
        let ops = parsed.payload.operations.as_ref().expect("operations");
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0]["type"], "surface.create");
        let tool: ToolDefinition =
            serde_json::from_value(ops[0]["payload"].clone()).expect("tool payload");
        assert_eq!(tool.id, "tool-tictactoe");
        assert_eq!(tool.name, "Tic-Tac-Toe vs AI");
    }

    #[tokio::test]
    async fn tictactoe_move_and_reset_turn_returns_targeted_state_patch() {
        use crate::ai::StructuredUserInput;
        use std::collections::HashMap;
        let provider = MockAiProvider::new();

        // 1. Move turn with structured user input
        let mut fields_map = serde_json::Map::new();
        fields_map.insert("cell".to_string(), json!(4));
        fields_map.insert("lastMove".to_string(), json!(4));
        for i in 0..9 {
            fields_map.insert(format!("c{i}"), json!(""));
        }
        let sui = StructuredUserInput {
            submission_id: "sui-1".into(),
            form_id: "ttt-c4".into(),
            event_name: Some("game.move".into()),
            application_id: Some("tool-tictactoe".into()),
            surface_id: Some("tool-tictactoe".into()),
            surface_revision: None,
            state_revision: None,
            component_id: None,
            idempotency_key: None,
            conversation_id: "conv-1".into(),
            fields: fields_map,
            content_hash: "hash".into(),
            trust_class: crate::ai::structured_user_input::TrustClass::LocalUserGesture,
            instruction_eligibility:
                crate::ai::structured_user_input::InstructionEligibility::LocalUserContent,
        };

        let response = provider
            .chat(AgentRequest {
                system_prompt: "test".into(),
                messages: vec![AgentMessage::with_parts(
                    crate::ai::AgentRole::User,
                    "App interaction (game.move)",
                    vec![AgentContentPart::StructuredUserInput(sui)],
                )],
                cancel: CancellationToken::new(),
                idempotency_key: None,
            })
            .await
            .unwrap();

        let parsed = parse_agent_response(&response.raw_text).unwrap();
        assert_eq!(parsed.payload.schema_version, "2");
        assert_eq!(parsed.payload.silent, Some(true));
        let ops = parsed.payload.operations.as_ref().expect("operations");
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0]["type"], "state.patch");
        assert_eq!(ops[0]["target"]["surfaceId"], "tool-tictactoe");
        let state = &ops[0]["payload"];
        assert_eq!(state["c4"], "X"); // User move
        assert_eq!(state["c0"], "O"); // AI counter move
        assert_eq!(state["turn"], "X"); // Next turn

        // 2. Reset turn
        let reset_sui = StructuredUserInput {
            submission_id: "sui-2".into(),
            form_id: "ttt-reset".into(),
            event_name: Some("game.reset".into()),
            application_id: Some("tool-tictactoe".into()),
            surface_id: Some("tool-tictactoe".into()),
            surface_revision: None,
            state_revision: None,
            component_id: None,
            idempotency_key: None,
            conversation_id: "conv-1".into(),
            fields: serde_json::Map::new(),
            content_hash: "hash2".into(),
            trust_class: crate::ai::structured_user_input::TrustClass::LocalUserGesture,
            instruction_eligibility:
                crate::ai::structured_user_input::InstructionEligibility::LocalUserContent,
        };

        let reset_response = provider
            .chat(AgentRequest {
                system_prompt: "test".into(),
                messages: vec![AgentMessage::with_parts(
                    crate::ai::AgentRole::User,
                    "App interaction (game.reset)",
                    vec![AgentContentPart::StructuredUserInput(reset_sui)],
                )],
                cancel: CancellationToken::new(),
                idempotency_key: None,
            })
            .await
            .unwrap();

        let reset_parsed = parse_agent_response(&reset_response.raw_text).unwrap();
        let reset_ops = reset_parsed
            .payload
            .operations
            .as_ref()
            .expect("operations");
        assert_eq!(reset_ops.len(), 1);
        assert_eq!(reset_ops[0]["type"], "state.patch");
        assert_eq!(reset_ops[0]["payload"]["c4"], "");
        assert_eq!(reset_ops[0]["payload"]["turn"], "X");
    }

    #[tokio::test]
    async fn interactive_app_fixtures_generate_valid_surfaces() {
        let provider = MockAiProvider::new();
        let fixtures = [
            ("Build me a chess game", "tool-chess"),
            ("Build a Connect Four game", "tool-connect-four"),
            ("Create 2048", "tool-2048"),
            ("Make minesweeper", "tool-minesweeper"),
            ("Build me a calculator", "tool-calculator"),
            ("Build a sudoku puzzle", "tool-sudoku"),
        ];

        for (prompt, expected_id) in fixtures {
            let resp = provider
                .chat(AgentRequest {
                    system_prompt: "test".into(),
                    messages: vec![AgentMessage::text(crate::ai::AgentRole::User, prompt)],
                    cancel: CancellationToken::new(),
                    idempotency_key: None,
                })
                .await
                .unwrap();

            let parsed = parse_agent_response(&resp.raw_text).unwrap();
            let ops = parsed.payload.operations.as_ref().expect("operations");
            assert_eq!(
                ops.len(),
                1,
                "Must generate exactly 1 surface create op for {prompt}"
            );
            assert_eq!(ops[0]["type"], "surface.create");
            assert_eq!(ops[0]["payload"]["id"], expected_id);
            assert!(ops[0]["payload"]["components"].as_array().unwrap().len() > 0);
        }
    }
}
