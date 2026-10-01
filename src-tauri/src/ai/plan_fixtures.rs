//! Deterministic ApplicationPlan fixtures shared by mock provider and contract tests.
//!
//! These exercise the same production ApplicationPlan → validate → compile path
//! that live providers must emit. They are not production intelligence.

use serde_json::json;

use crate::ai::response_schema::{ActionDefinition, ToolComponent, ToolDefinition};
use crate::application_kernel::application_plan::{
    ApplicationPlan, ApplicationPlanKind, PlanStep, APPLICATION_PLAN_SCHEMA_VERSION,
};
use crate::application_kernel::compiler::ChangeIntent;
use crate::application_kernel::data::{DataField, DataModelDefinition};

fn tasks_model_v1() -> DataModelDefinition {
    DataModelDefinition {
        model_id: "tasks".into(),
        display_name: "Tasks".into(),
        schema_version: 1,
        fields: vec![
            DataField {
                field_id: "title".into(),
                field_type: "text".into(),
                required: true,
                default: None,
                enum_values: None,
            },
            DataField {
                field_id: "priority".into(),
                field_type: "enum".into(),
                required: false,
                default: Some(json!("medium")),
                enum_values: Some(vec!["high".into(), "medium".into(), "low".into()]),
            },
            DataField {
                field_id: "status".into(),
                field_type: "enum".into(),
                required: false,
                default: Some(json!("todo")),
                enum_values: Some(vec!["todo".into(), "in_progress".into(), "done".into()]),
            },
            DataField {
                field_id: "completed".into(),
                field_type: "boolean".into(),
                required: false,
                default: Some(json!(false)),
                enum_values: None,
            },
        ],
    }
}

fn tasks_model_v2_with_due_date() -> DataModelDefinition {
    let mut model = tasks_model_v1();
    model.schema_version = 2;
    model.fields.push(DataField {
        field_id: "dueDate".into(),
        field_type: "date".into(),
        required: false,
        default: None,
        enum_values: None,
    });
    model
}

fn task_tracker_tool_v1() -> ToolDefinition {
    ToolDefinition {
        id: "tool-task-tracker".into(),
        name: "Task Tracker".into(),
        description: "Track tasks with title, priority, status, and durable local records.".into(),
        layout: json!({ "type": "dashboard", "columns": 2, "density": "normal" }),
        components: vec![
            ToolComponent {
                id: "tm-heading".into(),
                component_type: "heading".into(),
                props: Some(json!({ "text": "Task Tracker", "level": 1 })),
                ..Default::default()
            },
            ToolComponent {
                id: "tm-empty".into(),
                component_type: "text".into(),
                props: Some(json!({ "text": "No tasks yet. Add one below.", "tone": "muted" })),
                ..Default::default()
            },
            ToolComponent {
                id: "tm-search".into(),
                component_type: "textInput".into(),
                value_key: Some("searchQuery".into()),
                props: Some(json!({ "label": "Search Tasks", "placeholder": "Search..." })),
                ..Default::default()
            },
            ToolComponent {
                id: "tm-priority-filter".into(),
                component_type: "select".into(),
                value_key: Some("priorityFilter".into()),
                props: Some(json!({
                    "label": "Priority Filter",
                    "options": [
                        { "label": "All", "value": "" },
                        { "label": "High", "value": "high" },
                        { "label": "Medium", "value": "medium" },
                        { "label": "Low", "value": "low" }
                    ]
                })),
                ..Default::default()
            },
            ToolComponent {
                id: "tm-status-filter".into(),
                component_type: "select".into(),
                value_key: Some("statusFilter".into()),
                props: Some(json!({
                    "label": "Status Filter",
                    "options": [
                        { "label": "All", "value": "" },
                        { "label": "To Do", "value": "todo" },
                        { "label": "In Progress", "value": "in_progress" },
                        { "label": "Done", "value": "done" }
                    ]
                })),
                ..Default::default()
            },
            ToolComponent {
                id: "tm-list".into(),
                component_type: "dataTable".into(),
                props: Some(json!({
                    "columns": [
                        { "id": "title", "accessor": "title", "header": "Task" },
                        { "id": "priority", "accessor": "priority", "header": "Priority" },
                        { "id": "status", "accessor": "status", "header": "Status" }
                    ],
                    "rowsKey": "tasksResult",
                    "selectionKey": "selectedTaskId",
                    "searchKey": "searchQuery",
                    "filtersFromState": { "priority": "priorityFilter", "status": "statusFilter" },
                    "dataSource": {
                        "actionName": "local_data.query",
                        "input": { "modelId": "tasks", "limit": 100 },
                        "resultKey": "tasksResult"
                    }
                })),
                ..Default::default()
            },
            ToolComponent {
                id: "tm-new-title".into(),
                component_type: "textInput".into(),
                value_key: Some("newTaskTitle".into()),
                props: Some(json!({
                    "label": "New Task Title",
                    "placeholder": "Finish biology homework"
                })),
                ..Default::default()
            },
            ToolComponent {
                id: "tm-new-priority".into(),
                component_type: "select".into(),
                value_key: Some("newTaskPriority".into()),
                props: Some(json!({
                    "label": "Task Priority",
                    "options": [
                        { "label": "High", "value": "high" },
                        { "label": "Medium", "value": "medium" },
                        { "label": "Low", "value": "low" }
                    ]
                })),
                ..Default::default()
            },
            ToolComponent {
                id: "tm-new-status".into(),
                component_type: "select".into(),
                value_key: Some("newTaskStatus".into()),
                props: Some(json!({
                    "label": "Task Status",
                    "options": [
                        { "label": "To Do", "value": "todo" },
                        { "label": "In Progress", "value": "in_progress" },
                        { "label": "Done", "value": "done" }
                    ]
                })),
                ..Default::default()
            },
            ToolComponent {
                id: "tm-add-btn".into(),
                component_type: "button".into(),
                props: Some(json!({ "label": "Add Task" })),
                actions: Some(vec![
                    ActionDefinition::InvokeRegisteredAction {
                        action_name: "local_data.write".into(),
                        input: Some(json!({ "modelId": "tasks", "data": {} })),
                        input_from_state: Some(
                            [
                                ("data.title".into(), "newTaskTitle".into()),
                                ("data.priority".into(), "newTaskPriority".into()),
                                ("data.status".into(), "newTaskStatus".into()),
                            ]
                            .into_iter()
                            .collect(),
                        ),
                        component_id: None,
                        result_key: Some("lastTask".into()),
                    },
                    ActionDefinition::InvokeRegisteredAction {
                        action_name: "local_data.query".into(),
                        input: Some(json!({ "modelId": "tasks", "limit": 100 })),
                        input_from_state: None,
                        component_id: None,
                        result_key: Some("tasksResult".into()),
                    },
                    ActionDefinition::SetValue {
                        target: "newTaskTitle".into(),
                        value: json!(""),
                    },
                ]),
                ..Default::default()
            },
            ToolComponent {
                id: "tm-delete-btn".into(),
                component_type: "button".into(),
                props: Some(json!({ "label": "Delete Task" })),
                actions: Some(vec![
                    ActionDefinition::InvokeRegisteredAction {
                        action_name: "local_data.delete".into(),
                        input: Some(json!({})),
                        input_from_state: Some(
                            [("recordId".into(), "selectedTaskId".into())]
                                .into_iter()
                                .collect(),
                        ),
                        component_id: None,
                        result_key: None,
                    },
                    ActionDefinition::InvokeRegisteredAction {
                        action_name: "local_data.query".into(),
                        input: Some(json!({ "modelId": "tasks", "limit": 100 })),
                        input_from_state: None,
                        component_id: None,
                        result_key: Some("tasksResult".into()),
                    },
                ]),
                ..Default::default()
            },
            ToolComponent {
                id: "tm-done-btn".into(),
                component_type: "button".into(),
                props: Some(json!({ "label": "Mark Done" })),
                actions: Some(vec![
                    ActionDefinition::InvokeRegisteredAction {
                        action_name: "local_data.write".into(),
                        input: Some(json!({
                            "modelId": "tasks",
                            "data": { "status": "done", "completed": true }
                        })),
                        input_from_state: Some(
                            [("recordId".into(), "selectedTaskId".into())]
                                .into_iter()
                                .collect(),
                        ),
                        component_id: None,
                        result_key: None,
                    },
                    ActionDefinition::InvokeRegisteredAction {
                        action_name: "local_data.query".into(),
                        input: Some(json!({ "modelId": "tasks", "limit": 100 })),
                        input_from_state: None,
                        component_id: None,
                        result_key: Some("tasksResult".into()),
                    },
                ]),
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}

fn task_tracker_tool_v2_due_dates() -> ToolDefinition {
    let mut tool = task_tracker_tool_v1();
    // Preserve stable component IDs; add due-date column + input.
    if let Some(list) = tool.components.iter_mut().find(|c| c.id == "tm-list") {
        if let Some(props) = list.props.as_mut() {
            if let Some(cols) = props.get_mut("columns").and_then(|c| c.as_array_mut()) {
                cols.push(json!({
                    "id": "dueDate",
                    "accessor": "dueDate",
                    "header": "Due"
                }));
            }
        }
    }
    let due_input = ToolComponent {
        id: "tm-new-due".into(),
        component_type: "textInput".into(),
        value_key: Some("newTaskDueDate".into()),
        props: Some(json!({
            "label": "Due Date",
            "placeholder": "YYYY-MM-DD"
        })),
        ..Default::default()
    };
    // Insert before add button.
    if let Some(idx) = tool.components.iter().position(|c| c.id == "tm-add-btn") {
        tool.components.insert(idx, due_input);
    } else {
        tool.components.push(due_input);
    }
    if let Some(add) = tool.components.iter_mut().find(|c| c.id == "tm-add-btn") {
        if let Some(actions) = add.actions.as_mut() {
            if let Some(ActionDefinition::InvokeRegisteredAction {
                input_from_state, ..
            }) = actions.iter_mut().find(|a| {
                matches!(
                    a,
                    ActionDefinition::InvokeRegisteredAction { action_name, .. }
                        if action_name == "local_data.write"
                )
            }) {
                if let Some(map) = input_from_state.as_mut() {
                    map.insert("data.dueDate".into(), "newTaskDueDate".into());
                }
            }
        }
    }
    tool
}

fn habits_model_v1() -> DataModelDefinition {
    DataModelDefinition {
        model_id: "habits".into(),
        display_name: "Habits".into(),
        schema_version: 1,
        fields: vec![
            DataField {
                field_id: "title".into(),
                field_type: "text".into(),
                required: true,
                default: None,
                enum_values: None,
            },
            DataField {
                field_id: "streak".into(),
                field_type: "integer".into(),
                required: false,
                default: Some(json!(0)),
                enum_values: None,
            },
            DataField {
                field_id: "lastDone".into(),
                field_type: "date".into(),
                required: false,
                default: None,
                enum_values: None,
            },
        ],
    }
}

fn habit_tracker_tool_v1() -> ToolDefinition {
    ToolDefinition {
        id: "tool-habit-tracker".into(),
        name: "Habit Tracker".into(),
        description: "Track habits with streaks and last completion date.".into(),
        layout: json!({ "type": "single-column", "density": "normal" }),
        components: vec![
            ToolComponent {
                id: "hb-heading".into(),
                component_type: "heading".into(),
                props: Some(json!({ "text": "Habit Tracker", "level": 1 })),
                ..Default::default()
            },
            ToolComponent {
                id: "hb-list".into(),
                component_type: "dataTable".into(),
                props: Some(json!({
                    "columns": [
                        { "id": "title", "accessor": "title", "header": "Habit" },
                        { "id": "streak", "accessor": "streak", "header": "Streak" },
                        { "id": "lastDone", "accessor": "lastDone", "header": "Last Done" }
                    ],
                    "rowsKey": "habitsResult",
                    "selectionKey": "selectedHabitId",
                    "dataSource": {
                        "actionName": "local_data.query",
                        "input": { "modelId": "habits", "limit": 100 },
                        "resultKey": "habitsResult"
                    }
                })),
                ..Default::default()
            },
            ToolComponent {
                id: "hb-new-title".into(),
                component_type: "textInput".into(),
                value_key: Some("newHabitTitle".into()),
                props: Some(json!({
                    "label": "New Habit",
                    "placeholder": "Drink water"
                })),
                ..Default::default()
            },
            ToolComponent {
                id: "hb-add-btn".into(),
                component_type: "button".into(),
                props: Some(json!({ "label": "Add Habit" })),
                actions: Some(vec![
                    ActionDefinition::InvokeRegisteredAction {
                        action_name: "local_data.write".into(),
                        input: Some(json!({ "modelId": "habits", "data": { "streak": 0 } })),
                        input_from_state: Some(
                            [("data.title".into(), "newHabitTitle".into())]
                                .into_iter()
                                .collect(),
                        ),
                        component_id: None,
                        result_key: None,
                    },
                    ActionDefinition::InvokeRegisteredAction {
                        action_name: "local_data.query".into(),
                        input: Some(json!({ "modelId": "habits", "limit": 100 })),
                        input_from_state: None,
                        component_id: None,
                        result_key: Some("habitsResult".into()),
                    },
                    ActionDefinition::SetValue {
                        target: "newHabitTitle".into(),
                        value: json!(""),
                    },
                ]),
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}

/// Canonical create plan for Habit Tracker (provider-contract fixture).
pub fn habit_tracker_create_plan() -> ApplicationPlan {
    ApplicationPlan {
        schema_version: APPLICATION_PLAN_SCHEMA_VERSION.into(),
        plan_id: "plan-habit-tracker-create".into(),
        kind: ApplicationPlanKind::Create,
        summary: "Create Habit Tracker with habits model and list UI".into(),
        application_id: Some("tool-habit-tracker".into()),
        base_revision: None,
        steps: vec![
            PlanStep {
                id: "1".into(),
                description: "Upsert habits data model (title, streak, lastDone)".into(),
            },
            PlanStep {
                id: "2".into(),
                description: "Create Habit Tracker surface with table and add form".into(),
            },
        ],
        intents: vec![
            ChangeIntent::CreateSurface {
                tool: habit_tracker_tool_v1(),
                change_summary: Some("Create Habit Tracker with habits model".into()),
            },
            ChangeIntent::UpsertDataModel {
                application_id: "tool-habit-tracker".into(),
                model: habits_model_v1(),
            },
        ],
        tests: vec![],
        diagnostics: Some(json!({ "fixture": "habit_tracker_plan" })),
    }
}

fn expenses_model_v1() -> DataModelDefinition {
    DataModelDefinition {
        model_id: "expenses".into(),
        display_name: "Expenses".into(),
        schema_version: 1,
        fields: vec![
            DataField {
                field_id: "amount".into(),
                field_type: "decimal".into(),
                required: true,
                default: None,
                enum_values: None,
            },
            DataField {
                field_id: "category".into(),
                field_type: "enum".into(),
                required: false,
                default: Some(json!("other")),
                enum_values: Some(vec![
                    "food".into(),
                    "travel".into(),
                    "supplies".into(),
                    "services".into(),
                    "other".into(),
                ]),
            },
            DataField {
                field_id: "note".into(),
                field_type: "text".into(),
                required: false,
                default: None,
                enum_values: None,
            },
        ],
    }
}

fn budget_tracker_tool_v1() -> ToolDefinition {
    ToolDefinition {
        id: "tool-budget-tracker".into(),
        name: "Budget Tracker".into(),
        description: "Log expenses with amount, category, and notes.".into(),
        layout: json!({ "type": "single-column", "density": "normal" }),
        components: vec![
            ToolComponent {
                id: "bg-heading".into(),
                component_type: "heading".into(),
                props: Some(json!({ "text": "Budget Tracker", "level": 1 })),
                ..Default::default()
            },
            ToolComponent {
                id: "bg-table".into(),
                component_type: "dataTable".into(),
                props: Some(json!({
                    "columns": [
                        { "id": "amount", "accessor": "amount", "header": "Amount" },
                        { "id": "category", "accessor": "category", "header": "Category" },
                        { "id": "note", "accessor": "note", "header": "Note" }
                    ],
                    "rowsKey": "expensesResult",
                    "selectionKey": "selectedExpenseId",
                    "dataSource": {
                        "actionName": "local_data.query",
                        "input": { "modelId": "expenses", "limit": 200 },
                        "resultKey": "expensesResult"
                    }
                })),
                ..Default::default()
            },
            ToolComponent {
                id: "bg-amount".into(),
                component_type: "textInput".into(),
                value_key: Some("newAmount".into()),
                props: Some(json!({
                    "label": "Amount",
                    "placeholder": "25.00"
                })),
                ..Default::default()
            },
            ToolComponent {
                id: "bg-category".into(),
                component_type: "select".into(),
                value_key: Some("newCategory".into()),
                props: Some(json!({
                    "label": "Category",
                    "options": [
                        { "label": "Food", "value": "food" },
                        { "label": "Travel", "value": "travel" },
                        { "label": "Supplies", "value": "supplies" },
                        { "label": "Services", "value": "services" },
                        { "label": "Other", "value": "other" }
                    ]
                })),
                ..Default::default()
            },
            ToolComponent {
                id: "bg-note".into(),
                component_type: "textInput".into(),
                value_key: Some("newNote".into()),
                props: Some(json!({
                    "label": "Note",
                    "placeholder": "Optional description"
                })),
                ..Default::default()
            },
            ToolComponent {
                id: "bg-add-btn".into(),
                component_type: "button".into(),
                props: Some(json!({ "label": "Record Expense" })),
                actions: Some(vec![
                    ActionDefinition::InvokeRegisteredAction {
                        action_name: "local_data.write".into(),
                        input: Some(json!({ "modelId": "expenses", "data": {} })),
                        input_from_state: Some(
                            [
                                ("data.amount".into(), "newAmount".into()),
                                ("data.category".into(), "newCategory".into()),
                                ("data.note".into(), "newNote".into()),
                            ]
                            .into_iter()
                            .collect(),
                        ),
                        component_id: None,
                        result_key: None,
                    },
                    ActionDefinition::InvokeRegisteredAction {
                        action_name: "local_data.query".into(),
                        input: Some(json!({ "modelId": "expenses", "limit": 200 })),
                        input_from_state: None,
                        component_id: None,
                        result_key: Some("expensesResult".into()),
                    },
                    ActionDefinition::SetValue {
                        target: "newAmount".into(),
                        value: json!(""),
                    },
                    ActionDefinition::SetValue {
                        target: "newNote".into(),
                        value: json!(""),
                    },
                ]),
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}

/// Canonical create plan for Budget Tracker (provider-contract fixture).
pub fn budget_tracker_create_plan() -> ApplicationPlan {
    ApplicationPlan {
        schema_version: APPLICATION_PLAN_SCHEMA_VERSION.into(),
        plan_id: "plan-budget-tracker-create".into(),
        kind: ApplicationPlanKind::Create,
        summary: "Create Budget Tracker with expenses model and logging UI".into(),
        application_id: Some("tool-budget-tracker".into()),
        base_revision: None,
        steps: vec![
            PlanStep {
                id: "1".into(),
                description: "Upsert expenses data model (amount, category, note)".into(),
            },
            PlanStep {
                id: "2".into(),
                description: "Create Budget Tracker surface with table and entry form".into(),
            },
        ],
        intents: vec![
            ChangeIntent::CreateSurface {
                tool: budget_tracker_tool_v1(),
                change_summary: Some("Create Budget Tracker with expenses model".into()),
            },
            ChangeIntent::UpsertDataModel {
                application_id: "tool-budget-tracker".into(),
                model: expenses_model_v1(),
            },
        ],
        tests: vec![],
        diagnostics: Some(json!({ "fixture": "budget_tracker_plan" })),
    }
}

fn journal_entries_model_v1() -> DataModelDefinition {
    DataModelDefinition {
        model_id: "entries".into(),
        display_name: "Journal Entries".into(),
        schema_version: 1,
        fields: vec![
            DataField {
                field_id: "body".into(),
                field_type: "text".into(),
                required: true,
                default: None,
                enum_values: None,
            },
            DataField {
                field_id: "mood".into(),
                field_type: "enum".into(),
                required: false,
                default: None,
                enum_values: Some(vec![
                    "great".into(),
                    "good".into(),
                    "okay".into(),
                    "low".into(),
                    "rough".into(),
                ]),
            },
        ],
    }
}

fn journal_tool_v1() -> ToolDefinition {
    ToolDefinition {
        id: "tool-journal".into(),
        name: "Journal".into(),
        description: "Personal journal with optional mood tags.".into(),
        layout: json!({ "type": "single-column", "density": "comfortable" }),
        components: vec![
            ToolComponent {
                id: "jn-heading".into(),
                component_type: "heading".into(),
                props: Some(json!({ "text": "Journal", "level": 1 })),
                ..Default::default()
            },
            ToolComponent {
                id: "jn-list".into(),
                component_type: "dataTable".into(),
                props: Some(json!({
                    "columns": [
                        { "id": "body", "accessor": "body", "header": "Entry" },
                        { "id": "mood", "accessor": "mood", "header": "Mood" }
                    ],
                    "rowsKey": "entriesResult",
                    "selectionKey": "selectedEntryId",
                    "dataSource": {
                        "actionName": "local_data.query",
                        "input": { "modelId": "entries", "limit": 100 },
                        "resultKey": "entriesResult"
                    }
                })),
                ..Default::default()
            },
            ToolComponent {
                id: "jn-body".into(),
                component_type: "textInput".into(),
                value_key: Some("newEntryBody".into()),
                props: Some(json!({
                    "label": "New Entry",
                    "placeholder": "What happened today?"
                })),
                ..Default::default()
            },
            ToolComponent {
                id: "jn-mood".into(),
                component_type: "select".into(),
                value_key: Some("newEntryMood".into()),
                props: Some(json!({
                    "label": "Mood (optional)",
                    "options": [
                        { "label": "—", "value": "" },
                        { "label": "Great", "value": "great" },
                        { "label": "Good", "value": "good" },
                        { "label": "Okay", "value": "okay" },
                        { "label": "Low", "value": "low" },
                        { "label": "Rough", "value": "rough" }
                    ]
                })),
                ..Default::default()
            },
            ToolComponent {
                id: "jn-add-btn".into(),
                component_type: "button".into(),
                props: Some(json!({ "label": "Save Entry" })),
                actions: Some(vec![
                    ActionDefinition::InvokeRegisteredAction {
                        action_name: "local_data.write".into(),
                        input: Some(json!({ "modelId": "entries", "data": {} })),
                        input_from_state: Some(
                            [
                                ("data.body".into(), "newEntryBody".into()),
                                ("data.mood".into(), "newEntryMood".into()),
                            ]
                            .into_iter()
                            .collect(),
                        ),
                        component_id: None,
                        result_key: None,
                    },
                    ActionDefinition::InvokeRegisteredAction {
                        action_name: "local_data.query".into(),
                        input: Some(json!({ "modelId": "entries", "limit": 100 })),
                        input_from_state: None,
                        component_id: None,
                        result_key: Some("entriesResult".into()),
                    },
                    ActionDefinition::SetValue {
                        target: "newEntryBody".into(),
                        value: json!(""),
                    },
                ]),
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}

/// Canonical create plan for Journal (provider-contract fixture).
pub fn journal_create_plan() -> ApplicationPlan {
    ApplicationPlan {
        schema_version: APPLICATION_PLAN_SCHEMA_VERSION.into(),
        plan_id: "plan-journal-create".into(),
        kind: ApplicationPlanKind::Create,
        summary: "Create Journal with entries model and save UI".into(),
        application_id: Some("tool-journal".into()),
        base_revision: None,
        steps: vec![
            PlanStep {
                id: "1".into(),
                description: "Upsert entries data model (body, optional mood)".into(),
            },
            PlanStep {
                id: "2".into(),
                description: "Create Journal surface with list and compose form".into(),
            },
        ],
        intents: vec![
            ChangeIntent::CreateSurface {
                tool: journal_tool_v1(),
                change_summary: Some("Create Journal with entries model".into()),
            },
            ChangeIntent::UpsertDataModel {
                application_id: "tool-journal".into(),
                model: journal_entries_model_v1(),
            },
        ],
        tests: vec![],
        diagnostics: Some(json!({ "fixture": "journal_plan" })),
    }
}

fn study_planner_tool_v1() -> ToolDefinition {
    ToolDefinition {
        id: "tool-study-planner".into(),
        name: "Study Planner".into(),
        description: "Dashboard and tasks in one surface (multi-surface represented as sections)."
            .into(),
        layout: json!({ "type": "dashboard", "columns": 2, "density": "normal" }),
        components: vec![
            ToolComponent {
                id: "sp-dash-heading".into(),
                component_type: "heading".into(),
                props: Some(json!({ "text": "Dashboard", "level": 2 })),
                ..Default::default()
            },
            ToolComponent {
                id: "sp-dash-summary".into(),
                component_type: "text".into(),
                props: Some(json!({
                    "text": "Overview of study sessions and upcoming focus blocks.",
                    "tone": "muted"
                })),
                ..Default::default()
            },
            ToolComponent {
                id: "sp-tasks-heading".into(),
                component_type: "heading".into(),
                props: Some(json!({ "text": "Tasks", "level": 2 })),
                ..Default::default()
            },
            ToolComponent {
                id: "sp-tasks-table".into(),
                component_type: "dataTable".into(),
                props: Some(json!({
                    "columns": [
                        { "id": "title", "accessor": "title", "header": "Task" },
                        { "id": "status", "accessor": "status", "header": "Status" }
                    ],
                    "rowsKey": "studyTasksResult",
                    "selectionKey": "selectedStudyTaskId",
                    "dataSource": {
                        "actionName": "local_data.query",
                        "input": { "modelId": "studyTasks", "limit": 100 },
                        "resultKey": "studyTasksResult"
                    }
                })),
                ..Default::default()
            },
            ToolComponent {
                id: "sp-new-title".into(),
                component_type: "textInput".into(),
                value_key: Some("newStudyTaskTitle".into()),
                props: Some(json!({
                    "label": "New Study Task",
                    "placeholder": "Read chapter 3"
                })),
                ..Default::default()
            },
            ToolComponent {
                id: "sp-add-btn".into(),
                component_type: "button".into(),
                props: Some(json!({ "label": "Add Task" })),
                actions: Some(vec![
                    ActionDefinition::InvokeRegisteredAction {
                        action_name: "local_data.write".into(),
                        input: Some(json!({
                            "modelId": "studyTasks",
                            "data": { "status": "todo" }
                        })),
                        input_from_state: Some(
                            [("data.title".into(), "newStudyTaskTitle".into())]
                                .into_iter()
                                .collect(),
                        ),
                        component_id: None,
                        result_key: None,
                    },
                    ActionDefinition::InvokeRegisteredAction {
                        action_name: "local_data.query".into(),
                        input: Some(json!({ "modelId": "studyTasks", "limit": 100 })),
                        input_from_state: None,
                        component_id: None,
                        result_key: Some("studyTasksResult".into()),
                    },
                    ActionDefinition::SetValue {
                        target: "newStudyTaskTitle".into(),
                        value: json!(""),
                    },
                ]),
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}

fn study_tasks_model_v1() -> DataModelDefinition {
    DataModelDefinition {
        model_id: "studyTasks".into(),
        display_name: "Study Tasks".into(),
        schema_version: 1,
        fields: vec![
            DataField {
                field_id: "title".into(),
                field_type: "text".into(),
                required: true,
                default: None,
                enum_values: None,
            },
            DataField {
                field_id: "status".into(),
                field_type: "enum".into(),
                required: false,
                default: Some(json!("todo")),
                enum_values: Some(vec!["todo".into(), "in_progress".into(), "done".into()]),
            },
        ],
    }
}

/// Study planner fixture: one CreateSurface with Dashboard + Tasks sections until multi-surface plans are supported.
pub fn multi_surface_planner_create_plan() -> ApplicationPlan {
    ApplicationPlan {
        schema_version: APPLICATION_PLAN_SCHEMA_VERSION.into(),
        plan_id: "plan-study-planner-create".into(),
        kind: ApplicationPlanKind::Create,
        summary: "Create Study Planner with dashboard and tasks sections on one surface".into(),
        application_id: Some("tool-study-planner".into()),
        base_revision: None,
        steps: vec![
            PlanStep {
                id: "1".into(),
                description: "Upsert studyTasks model for the tasks panel".into(),
            },
            PlanStep {
                id: "2".into(),
                description: "Create single surface with Dashboard and Tasks sections (multi-surface deferred)"
                    .into(),
            },
        ],
        intents: vec![
            ChangeIntent::CreateSurface {
                tool: study_planner_tool_v1(),
                change_summary: Some(
                    "Create Study Planner with dashboard and tasks sections".into(),
                ),
            },
            ChangeIntent::UpsertDataModel {
                application_id: "tool-study-planner".into(),
                model: study_tasks_model_v1(),
            },
        ],
        tests: vec![],
        diagnostics: Some(json!({
            "fixture": "multi_surface_planner_plan",
            "multiSurfaceNote": "Dashboard and Tasks are separate sections on one surface; multiple CreateSurface intents per application are not used in this fixture."
        })),
    }
}

/// Canonical create plan for Task Tracker (provider-contract fixture).
pub fn task_tracker_create_plan() -> ApplicationPlan {
    ApplicationPlan {
        schema_version: APPLICATION_PLAN_SCHEMA_VERSION.into(),
        plan_id: "plan-task-tracker-create".into(),
        kind: ApplicationPlanKind::Create,
        summary: "Create Task Tracker with local_data model and CRUD actions".into(),
        application_id: Some("tool-task-tracker".into()),
        base_revision: None,
        steps: vec![
            PlanStep {
                id: "1".into(),
                description: "Upsert tasks data model (title, priority, status)".into(),
            },
            PlanStep {
                id: "2".into(),
                description: "Create Task Tracker surface with list and add form".into(),
            },
        ],
        intents: vec![
            // Surface/manifest must exist before data.model_upsert accepts mutations.
            ChangeIntent::CreateSurface {
                tool: task_tracker_tool_v1(),
                change_summary: Some(
                    "Create Task Tracker with local_data model and CRUD actions".into(),
                ),
            },
            ChangeIntent::UpsertDataModel {
                application_id: "tool-task-tracker".into(),
                model: tasks_model_v1(),
            },
        ],
        tests: vec![],
        diagnostics: Some(json!({ "fixture": "task_tracker_plan" })),
    }
}

/// Evolve plan: add optional dueDate field + UI without destroying stable IDs.
pub fn task_tracker_add_due_dates_plan(base_revision: Option<i64>) -> ApplicationPlan {
    ApplicationPlan {
        schema_version: APPLICATION_PLAN_SCHEMA_VERSION.into(),
        plan_id: "plan-task-tracker-due-dates".into(),
        kind: ApplicationPlanKind::Evolve,
        summary: "Add due dates to Task Tracker".into(),
        application_id: Some("tool-task-tracker".into()),
        base_revision,
        steps: vec![
            PlanStep {
                id: "1".into(),
                description: "Migrate tasks model to schemaVersion 2 with optional dueDate".into(),
            },
            PlanStep {
                id: "2".into(),
                description: "Add due date column and input; preserve existing component IDs"
                    .into(),
            },
            PlanStep {
                id: "3".into(),
                description: "Wire dueDate into create action inputFromState".into(),
            },
        ],
        intents: vec![
            ChangeIntent::MigrateDataModel {
                application_id: "tool-task-tracker".into(),
                model: tasks_model_v2_with_due_date(),
                migration_strategy: Some("add_optional_fields".into()),
                require_approval: None,
            },
            ChangeIntent::UpdateSurface {
                tool_id: "tool-task-tracker".into(),
                tool: task_tracker_tool_v2_due_dates(),
                change_summary: Some("Add due dates to Task Tracker".into()),
                base_revision,
            },
        ],
        tests: vec![],
        diagnostics: Some(json!({ "fixture": "task_tracker_due_dates_plan" })),
    }
}

/// Serialize a plan as a full agent response JSON string (schema v1 + applicationPlan).
pub fn plan_response_json(assistant_message: &str, plan: &ApplicationPlan) -> String {
    let tool_change = crate::application_kernel::application_plan::derive_tool_change(plan);
    json!({
        "schemaVersion": crate::ai::response_schema::SCHEMA_VERSION,
        "assistantMessage": assistant_message,
        "responseType": "tool_change",
        "applicationPlan": plan,
        "toolChange": tool_change,
        "diagnostics": plan.diagnostics.clone().unwrap_or(json!({ "fixture": "application_plan" })),
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::parse_agent_response;
    use crate::application_kernel::application_plan::compile_plan;

    fn assert_create_plan_compiles(plan: &ApplicationPlan) {
        let v = compile_plan(plan).unwrap();
        assert!(v
            .compiled
            .operations
            .iter()
            .any(|o| o.op_type == "data.model_upsert"));
        assert!(v
            .compiled
            .operations
            .iter()
            .any(|o| o.op_type == "surface.create"));
    }

    #[test]
    fn create_plan_compiles() {
        assert_create_plan_compiles(&task_tracker_create_plan());
    }

    #[test]
    fn habit_tracker_create_plan_compiles() {
        assert_create_plan_compiles(&habit_tracker_create_plan());
    }

    #[test]
    fn budget_tracker_create_plan_compiles() {
        assert_create_plan_compiles(&budget_tracker_create_plan());
    }

    #[test]
    fn journal_create_plan_compiles() {
        assert_create_plan_compiles(&journal_create_plan());
    }

    #[test]
    fn multi_surface_planner_create_plan_compiles() {
        assert_create_plan_compiles(&multi_surface_planner_create_plan());
    }

    #[test]
    fn evolve_plan_preserves_stable_ids() {
        let plan = task_tracker_add_due_dates_plan(Some(1));
        let v = compile_plan(&plan).unwrap();
        let update = v
            .compiled
            .operations
            .iter()
            .find(|o| o.payload.get("action") == Some(&json!("update")))
            .expect("update surface op");
        let comps = update.payload["tool"]["components"].as_array().unwrap();
        assert!(comps.iter().any(|c| c["id"] == "tm-new-title"));
        assert!(comps.iter().any(|c| c["id"] == "tm-new-due"));
        assert!(comps.iter().any(|c| c["id"] == "tm-add-btn"));
    }

    /// Provider-contract: recorded JSON → parse → ApplicationPlan → compile → ops.
    #[test]
    fn recorded_create_json_parses_compiles_to_operations() {
        let plan = task_tracker_create_plan();
        let raw = plan_response_json("Creating Task Tracker.", &plan);
        let parsed = parse_agent_response(&raw).expect("recorded fixture must parse");
        let recovered = parsed
            .payload
            .application_plan
            .as_ref()
            .expect("applicationPlan required");
        assert_eq!(recovered.kind, ApplicationPlanKind::Create);
        let ops = parsed
            .payload
            .normalized_operations()
            .expect("plan must compile to operations");
        assert!(ops.iter().any(|o| o.op_type == "data.model_upsert"));
        assert!(ops.iter().any(|o| o.op_type == "surface.create"));
        assert_eq!(
            ops.len(),
            compile_plan(&plan).unwrap().compiled.operations.len()
        );
    }

    #[test]
    fn recorded_evolve_json_parses_compiles_to_operations() {
        let plan = task_tracker_add_due_dates_plan(Some(2));
        let raw = plan_response_json("Adding due dates.", &plan);
        let parsed = parse_agent_response(&raw).expect("recorded evolve fixture must parse");
        let recovered = parsed
            .payload
            .application_plan
            .as_ref()
            .expect("applicationPlan required");
        assert_eq!(recovered.kind, ApplicationPlanKind::Evolve);
        let ops = parsed
            .payload
            .normalized_operations()
            .expect("evolve plan must compile");
        assert!(ops.iter().any(|o| o.op_type == "data.model_upsert"));
        assert!(ops
            .iter()
            .any(|o| o.payload.get("action") == Some(&json!("update"))));
        let migrate = ops
            .iter()
            .find(|o| o.op_type == "data.model_upsert")
            .expect("migrate model op");
        assert_eq!(migrate.payload["model"]["schemaVersion"], 2);
        assert!(migrate.payload["model"]["fields"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["fieldId"] == "dueDate"));
    }

    #[test]
    fn malformed_application_plan_in_response_is_rejected() {
        let raw = json!({
            "schemaVersion": crate::ai::response_schema::SCHEMA_VERSION,
            "assistantMessage": "bad plan",
            "responseType": "tool_change",
            "applicationPlan": {
                "schemaVersion": "1",
                "kind": "create",
                "summary": "broken",
                "intents": []
            }
        })
        .to_string();
        let parsed = parse_agent_response(&raw).expect("payload shape parses");
        let err = parsed
            .payload
            .normalized_operations()
            .expect_err("empty intents must fail compile");
        assert!(
            err.contains("applicationPlan rejected"),
            "unexpected err: {err}"
        );
    }
}
