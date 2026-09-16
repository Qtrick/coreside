//! Serde types for structured agent JSON responses.

use super::capability_registry::ToolCallRequest;
use super::settings_change::SettingsChangePayload;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const SCHEMA_VERSION: &str = "1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ResponseType {
    #[default]
    Message,
    ToolChange,
    ToolUse,
    SettingsChange,
    Noop,
}

impl ResponseType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Message => "message",
            Self::ToolChange => "tool_change",
            Self::ToolUse => "tool_use",
            Self::SettingsChange => "settings_change",
            Self::Noop => "noop",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolAction {
    Create,
    Update,
    Replace,
}

impl ToolAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Replace => "replace",
        }
    }
}

use std::collections::HashMap;

pub const MAX_ACTIONS_PER_COMPONENT: usize = 16;
pub const MAX_ACTION_TARGET_LEN: usize = 128;
pub const MAX_ACTION_PAYLOAD_BYTES: usize = 32_768;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ActionDefinition {
    SetValue {
        target: String,
        value: Value,
    },
    Toggle {
        target: String,
    },
    Increment {
        target: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        amount: Option<f64>,
    },
    Decrement {
        target: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        amount: Option<f64>,
    },
    Reset {
        target: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<Value>,
    },
    AppendItem {
        target: String,
        item: Value,
    },
    RemoveItem {
        target: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },
    UpdateItem {
        target: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        patch: Value,
    },
    SelectTab {
        target: String,
        #[serde(rename = "tabId")]
        tab_id: String,
    },
    SubmitToAgent {
        #[serde(rename = "eventName")]
        event_name: String,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            rename = "includeFields"
        )]
        include_fields: Option<Vec<String>>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            rename = "componentId"
        )]
        component_id: Option<String>,
    },
    InvokeRegisteredAction {
        #[serde(rename = "actionName")]
        action_name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        input: Option<Value>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            rename = "inputFromState"
        )]
        input_from_state: Option<HashMap<String, String>>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            rename = "componentId"
        )]
        component_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none", rename = "resultKey")]
        result_key: Option<String>,
    },
}

impl ActionDefinition {
    pub fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::SetValue { target, value } => {
                validate_action_target(target)?;
                validate_action_payload_size(value)?;
            }
            Self::Toggle { target } => {
                validate_action_target(target)?;
            }
            Self::Increment { target, .. } | Self::Decrement { target, .. } => {
                validate_action_target(target)?;
            }
            Self::Reset { target, value } => {
                validate_action_target(target)?;
                if let Some(v) = value {
                    validate_action_payload_size(v)?;
                }
            }
            Self::AppendItem { target, item } => {
                validate_action_target(target)?;
                validate_action_payload_size(item)?;
            }
            Self::RemoveItem { target, id, .. } => {
                validate_action_target(target)?;
                if let Some(id_str) = id {
                    if id_str.len() > MAX_ACTION_TARGET_LEN {
                        return Err("Item ID too long");
                    }
                }
            }
            Self::UpdateItem {
                target, id, patch, ..
            } => {
                validate_action_target(target)?;
                if let Some(id_str) = id {
                    if id_str.len() > MAX_ACTION_TARGET_LEN {
                        return Err("Item ID too long");
                    }
                }
                validate_action_payload_size(patch)?;
            }
            Self::SelectTab { target, tab_id } => {
                validate_action_target(target)?;
                if tab_id.trim().is_empty() || tab_id.len() > MAX_ACTION_TARGET_LEN {
                    return Err("Invalid tab ID");
                }
            }
            Self::SubmitToAgent {
                event_name,
                include_fields,
                component_id,
            } => {
                if event_name.trim().is_empty() || event_name.len() > MAX_ACTION_TARGET_LEN {
                    return Err("Invalid eventName in submitToAgent");
                }
                match include_fields {
                    None => return Err("submitToAgent requires explicit includeFields"),
                    Some(fields) if fields.is_empty() => {
                        return Err("submitToAgent requires non-empty includeFields");
                    }
                    Some(fields) => {
                        if fields.len() > 64 {
                            return Err("Too many includeFields in submitToAgent");
                        }
                        for f in fields {
                            if f.trim().is_empty() || f.len() > MAX_ACTION_TARGET_LEN {
                                return Err("Invalid field name in includeFields");
                            }
                        }
                    }
                }
                if let Some(cid) = component_id {
                    if cid.len() > MAX_ACTION_TARGET_LEN {
                        return Err("componentId too long in submitToAgent");
                    }
                }
            }
            Self::InvokeRegisteredAction {
                action_name,
                input,
                input_from_state,
                component_id,
                result_key,
            } => {
                if action_name.trim().is_empty() || action_name.len() > MAX_ACTION_TARGET_LEN {
                    return Err("Invalid actionName in invokeRegisteredAction");
                }
                if let Some(inp) = input {
                    validate_action_payload_size(inp)?;
                }
                if let Some(ifs) = input_from_state {
                    if ifs.len() > 32 {
                        return Err("Too many inputFromState mappings in invokeRegisteredAction");
                    }
                    for (k, v) in ifs {
                        if k.len() > MAX_ACTION_TARGET_LEN || v.len() > MAX_ACTION_TARGET_LEN {
                            return Err("inputFromState key/value exceeds maximum length");
                        }
                    }
                }
                if let Some(cid) = component_id {
                    if cid.len() > MAX_ACTION_TARGET_LEN {
                        return Err("componentId too long in invokeRegisteredAction");
                    }
                }
                if let Some(rk) = result_key {
                    if rk.trim().is_empty() || rk.len() > MAX_ACTION_TARGET_LEN {
                        return Err("Invalid resultKey in invokeRegisteredAction");
                    }
                }
            }
        }
        Ok(())
    }
}

fn validate_action_target(target: &str) -> Result<(), &'static str> {
    let t = target.trim();
    if t.is_empty() {
        return Err("Action target cannot be empty");
    }
    if t.len() > MAX_ACTION_TARGET_LEN {
        return Err("Action target exceeds maximum length");
    }
    Ok(())
}

fn validate_action_payload_size(val: &Value) -> Result<(), &'static str> {
    if val.to_string().len() > MAX_ACTION_PAYLOAD_BYTES {
        return Err("Action payload exceeds maximum size limit");
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ToolComponent {
    pub id: String,
    #[serde(rename = "type")]
    pub component_type: String,
    /// Stable state binding key (preferred over embedding valueKey only in props).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub props: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<ToolComponent>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actions: Option<Vec<ActionDefinition>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout_role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub col_span: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row_span: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Accepts a string layout type or an object like `{ "type": "single-column" }`.
    #[serde(default = "default_layout")]
    pub layout: Value,
    #[serde(default)]
    pub components: Vec<ToolComponent>,
}

fn default_layout() -> Value {
    json!({ "type": "single-column" })
}

/// Normalize layout to the object form preferred by the frontend.
pub fn normalize_layout(layout: &Value) -> Value {
    match layout {
        Value::String(s) => json!({ "type": s }),
        Value::Object(obj) if obj.contains_key("type") => layout.clone(),
        Value::Object(obj) if !obj.is_empty() => layout.clone(),
        _ => default_layout(),
    }
}

/// Extract a string layout type for the SQLite `layout` column.
pub fn layout_type_string(layout: &Value) -> String {
    match layout {
        Value::String(s) => {
            if s.trim().is_empty() {
                "single-column".into()
            } else {
                s.clone()
            }
        }
        Value::Object(obj) => obj
            .get("type")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("single-column")
            .to_string(),
        _ => "single-column".into(),
    }
}

pub const VALID_LAYOUT_TYPES: &[&str] = &[
    "stack",
    "split",
    "grid",
    "dashboard",
    "form",
    "content",
    "full",
    "single-column",
];

pub fn validate_layout(layout: &Value) -> Result<(), String> {
    let layout_type = match layout {
        Value::String(s) => s.as_str(),
        Value::Object(obj) => {
            if let Some(t) = obj.get("type") {
                t.as_str()
                    .ok_or_else(|| "layout.type must be a string".to_string())?
            } else {
                "single-column"
            }
        }
        _ => return Err("layout must be a string or object".to_string()),
    };

    if !VALID_LAYOUT_TYPES.contains(&layout_type) {
        return Err(format!(
            "invalid layout type: '{layout_type}', expected one of: {:?}",
            VALID_LAYOUT_TYPES
        ));
    }

    if let Value::Object(obj) = layout {
        if let Some(cols) = obj.get("columns") {
            let c = cols
                .as_i64()
                .ok_or_else(|| "layout.columns must be an integer".to_string())?;
            if !(1..=6).contains(&c) {
                return Err(format!("layout.columns must be between 1 and 6, got {c}"));
            }
        }
        if let Some(gap) = obj.get("gap") {
            let g = gap
                .as_str()
                .ok_or_else(|| "layout.gap must be a string".to_string())?;
            if !["none", "xs", "sm", "md", "lg", "xl"].contains(&g) {
                return Err(format!("invalid layout.gap: '{g}'"));
            }
        }
        if let Some(max_width) = obj.get("maxWidth") {
            let mw = max_width
                .as_str()
                .ok_or_else(|| "layout.maxWidth must be a string".to_string())?;
            if !["sm", "md", "lg", "xl", "full"].contains(&mw) {
                return Err(format!("invalid layout.maxWidth: '{mw}'"));
            }
        }
        if let Some(density) = obj.get("density") {
            let d = density
                .as_str()
                .ok_or_else(|| "layout.density must be a string".to_string())?;
            if !["compact", "normal", "comfortable"].contains(&d) {
                return Err(format!("invalid layout.density: '{d}'"));
            }
        }
        if let Some(align) = obj.get("align") {
            let a = align
                .as_str()
                .ok_or_else(|| "layout.align must be a string".to_string())?;
            if !["start", "center", "end", "stretch"].contains(&a) {
                return Err(format!("invalid layout.align: '{a}'"));
            }
        }
        if let Some(split_ratio) = obj.get("splitRatio") {
            let sr = split_ratio
                .as_str()
                .ok_or_else(|| "layout.splitRatio must be a string".to_string())?;
            if !["1:1", "1:2", "1:3", "2:1", "3:1", "1:4", "4:1"].contains(&sr) {
                return Err(format!("invalid layout.splitRatio: '{sr}'"));
            }
        }
        if let Some(collapse_at) = obj.get("collapseAt") {
            let ca = collapse_at
                .as_str()
                .ok_or_else(|| "layout.collapseAt must be a string".to_string())?;
            if !["mobile", "tablet", "never"].contains(&ca) {
                return Err(format!("invalid layout.collapseAt: '{ca}'"));
            }
        }
    }

    Ok(())
}

impl ToolDefinition {
    /// Normalize `layout` to `{ "type": "..." }` for frontend IPC.
    pub fn normalize_for_frontend(&mut self) {
        self.layout = normalize_layout(&self.layout);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolChangePayload {
    pub action: ToolAction,
    #[serde(default)]
    pub target_tool_id: Option<String>,
    #[serde(default)]
    pub tool: Option<ToolDefinition>,
    #[serde(default)]
    pub change_summary: String,
}

impl ToolChangePayload {
    pub fn normalize_for_frontend(&mut self) {
        if let Some(tool) = self.tool.as_mut() {
            tool.normalize_for_frontend();
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceCitation {
    pub id: String,
    pub title: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_domain: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct AgentResponsePayload {
    #[serde(default = "default_schema_version")]
    pub schema_version: String,
    #[serde(default)]
    pub assistant_message: String,
    #[serde(default = "default_response_type")]
    pub response_type: ResponseType,
    #[serde(default)]
    pub tool_change: Option<ToolChangePayload>,
    #[serde(default)]
    pub settings_change: Option<SettingsChangePayload>,
    #[serde(default)]
    pub tool_calls: Option<Vec<ToolCallRequest>>,
    #[serde(default)]
    pub citations: Option<Vec<SourceCitation>>,
    #[serde(default)]
    pub diagnostics: Option<Value>,
    /// Runtime V2 multi-operation list (schemaVersion "2").
    #[serde(default)]
    pub operations: Option<Vec<Value>>,
    #[serde(default)]
    pub silent: Option<bool>,
    #[serde(default)]
    pub turn_id: Option<String>,
    #[serde(default)]
    pub assistant_messages: Option<Vec<Value>>,
}

fn default_response_type() -> ResponseType {
    ResponseType::Message
}

fn default_schema_version() -> String {
    SCHEMA_VERSION.to_string()
}

impl AgentResponsePayload {
    pub fn normalize_for_frontend(&mut self) {
        if let Some(tc) = self.tool_change.as_mut() {
            tc.normalize_for_frontend();
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let has_ops = self
            .operations
            .as_ref()
            .map(|o| !o.is_empty())
            .unwrap_or(false);
        let silent = self.silent.unwrap_or(false);
        let has_v2_visible = self
            .assistant_messages
            .as_ref()
            .map(|msgs| {
                msgs.iter().any(|m| {
                    let vis = m
                        .get("visibility")
                        .and_then(|v| v.as_str())
                        .unwrap_or("visible");
                    vis != "silent"
                        && m.get("content")
                            .and_then(|v| v.as_str())
                            .map(|s| !s.trim().is_empty())
                            .unwrap_or(false)
                })
            })
            .unwrap_or(false);

        if has_ops {
            let ops: Vec<crate::runtime_v2::AppOperation> =
                serde_json::from_value(Value::Array(self.operations.clone().unwrap_or_default()))
                    .map_err(|e| format!("invalid operations: {e}"))?;
            crate::runtime_v2::validate_model_operations(&ops)?;
        }

        if self.assistant_message.trim().is_empty()
            && !matches!(
                self.response_type,
                ResponseType::Noop | ResponseType::ToolUse
            )
            && !has_ops
            && !silent
            && !has_v2_visible
        {
            return Err("assistantMessage must be non-empty".into());
        }

        match self.response_type {
            ResponseType::ToolUse => {
                let calls = self
                    .tool_calls
                    .as_ref()
                    .filter(|c| !c.is_empty())
                    .ok_or_else(|| {
                        "toolCalls required and must be non-empty for responseType tool_use"
                            .to_string()
                    })?;
                if calls.len() > super::capability_registry::TOOL_LOOP_MAX_STEPS {
                    return Err(format!(
                        "toolCalls exceeds max of {}",
                        super::capability_registry::TOOL_LOOP_MAX_STEPS
                    ));
                }
                for call in calls {
                    super::capability_registry::validate_tool_call(call)?;
                }
            }
            ResponseType::ToolChange => {
                let tc = self.tool_change.as_ref().ok_or_else(|| {
                    "toolChange required for responseType tool_change".to_string()
                })?;
                let tool = tc
                    .tool
                    .as_ref()
                    .ok_or_else(|| "toolChange.tool required".to_string())?;
                if tool.id.trim().is_empty() {
                    return Err("tool.id must be non-empty".into());
                }
                if tool.name.trim().is_empty() {
                    return Err("tool.name must be non-empty".into());
                }
                crate::security::assert_not_protected(&tool.id)?;
                if let Some(target) = tc.target_tool_id.as_ref() {
                    if !target.trim().is_empty() {
                        crate::security::assert_not_protected(target)?;
                    }
                }
                if matches!(tc.action, ToolAction::Update | ToolAction::Replace)
                    && tc
                        .target_tool_id
                        .as_ref()
                        .map(|s| s.trim().is_empty())
                        .unwrap_or(true)
                {
                    return Err("targetToolId required for update/replace".into());
                }
                if matches!(tc.action, ToolAction::Create) && tool.components.is_empty() {
                    return Err("tool.components must not be empty when creating a tool".into());
                }
                if !tool.components.is_empty() {
                    crate::runtime_v2::packs::validate_tool_components(&tool.components)?;
                }
                validate_layout(&tool.layout)?;
            }
            ResponseType::SettingsChange => {
                let sc = self.settings_change.as_ref().ok_or_else(|| {
                    "settingsChange required for responseType settings_change".to_string()
                })?;
                sc.validate()?;
            }
            ResponseType::Message | ResponseType::Noop => {
                if let Some(sc) = &self.settings_change {
                    sc.validate()?;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tool_use_requires_non_empty_tool_calls() {
        let missing = AgentResponsePayload {
            schema_version: "1".into(),
            assistant_message: "Searching…".into(),
            response_type: ResponseType::ToolUse,
            tool_change: None,
            settings_change: None,
            tool_calls: None,
            citations: None,
            diagnostics: None,
            operations: None,
            silent: None,
            turn_id: None,
            assistant_messages: None,
        };
        assert!(missing.validate().is_err());

        let valid = AgentResponsePayload {
            tool_calls: Some(vec![ToolCallRequest {
                capability: "web_search".into(),
                arguments: json!({ "query": "rust async" }),
            }]),
            ..missing
        };
        assert!(valid.validate().is_ok());
    }

    #[test]
    fn action_definitions_roundtrip_all_types() {
        let actions = vec![
            ActionDefinition::SetValue {
                target: "count".into(),
                value: json!(42),
            },
            ActionDefinition::Toggle {
                target: "isActive".into(),
            },
            ActionDefinition::Increment {
                target: "count".into(),
                amount: Some(5.0),
            },
            ActionDefinition::Decrement {
                target: "count".into(),
                amount: None,
            },
            ActionDefinition::Reset {
                target: "form".into(),
                value: Some(json!({ "field": "" })),
            },
            ActionDefinition::AppendItem {
                target: "items".into(),
                item: json!({ "title": "Buy milk" }),
            },
            ActionDefinition::RemoveItem {
                target: "items".into(),
                index: Some(2),
                id: Some("item-123".into()),
            },
            ActionDefinition::UpdateItem {
                target: "items".into(),
                index: None,
                id: Some("item-123".into()),
                patch: json!({ "done": true }),
            },
            ActionDefinition::SelectTab {
                target: "activeTab".into(),
                tab_id: "tab-settings".into(),
            },
            ActionDefinition::SubmitToAgent {
                event_name: "submitForm".into(),
                include_fields: Some(vec!["name".into(), "email".into()]),
                component_id: Some("btn-submit".into()),
            },
            ActionDefinition::InvokeRegisteredAction {
                action_name: "local_data.query".into(),
                input: Some(json!({ "modelId": "habits" })),
                input_from_state: Some([("queryText".into(), "searchField".into())].into()),
                component_id: Some("query-btn".into()),
                result_key: Some("habitRecords".into()),
            },
        ];

        for action in &actions {
            assert!(action.validate().is_ok());
            let serialized = serde_json::to_value(action).expect("serialize action");
            let deserialized: ActionDefinition =
                serde_json::from_value(serialized.clone()).expect("deserialize action");
            assert_eq!(&deserialized, action);
        }
    }

    #[test]
    fn unknown_action_type_fails_closed() {
        let raw = json!({
            "type": "executeArbitraryScript",
            "target": "window.location"
        });
        let res: Result<ActionDefinition, _> = serde_json::from_value(raw);
        assert!(res.is_err());
    }

    #[test]
    fn invalid_action_target_fails_validation() {
        let empty_target = ActionDefinition::SetValue {
            target: "".into(),
            value: json!(1),
        };
        assert!(empty_target.validate().is_err());

        let oversized_target = ActionDefinition::Toggle {
            target: "a".repeat(129),
        };
        assert!(oversized_target.validate().is_err());
    }

    #[test]
    fn tool_component_serializes_and_deserializes_actions() {
        let comp = ToolComponent {
            id: "button-1".into(),
            component_type: "button".into(),
            value_key: None,
            props: Some(json!({ "label": "Click Me" })),
            children: None,
            actions: Some(vec![
                ActionDefinition::Increment {
                    target: "counter".into(),
                    amount: Some(1.0),
                },
                ActionDefinition::InvokeRegisteredAction {
                    action_name: "local_data.write".into(),
                    input: Some(json!({ "key": "saved" })),
                    input_from_state: None,
                    component_id: Some("button-1".into()),
                    result_key: Some("saveResult".into()),
                },
            ]),
            layout_role: None,
            col_span: None,
            row_span: None,
        };

        let val = serde_json::to_value(&comp).expect("serialize component");
        assert!(val.get("actions").is_some());
        let roundtrip: ToolComponent = serde_json::from_value(val).expect("deserialize component");
        assert_eq!(roundtrip, comp);
        assert_eq!(roundtrip.actions.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn validate_layout_accepts_valid_layouts() {
        assert!(validate_layout(&json!("stack")).is_ok());
        assert!(validate_layout(&json!("single-column")).is_ok());
        assert!(validate_layout(&json!({
            "type": "grid",
            "columns": 3,
            "gap": "md",
            "maxWidth": "lg",
            "density": "comfortable",
            "align": "stretch"
        }))
        .is_ok());
        assert!(validate_layout(&json!({
            "type": "split",
            "splitRatio": "1:2",
            "collapseAt": "mobile"
        }))
        .is_ok());
    }

    #[test]
    fn validate_layout_rejects_invalid_values() {
        assert!(validate_layout(&json!("unknown_layout")).is_err());
        assert!(validate_layout(&json!(123)).is_err());
        assert!(validate_layout(&json!({ "type": "grid", "columns": 0 })).is_err());
        assert!(validate_layout(&json!({ "type": "grid", "columns": 7 })).is_err());
        assert!(validate_layout(&json!({ "type": "stack", "gap": "huge" })).is_err());
        assert!(validate_layout(&json!({ "type": "split", "splitRatio": "5:1" })).is_err());
    }
}
