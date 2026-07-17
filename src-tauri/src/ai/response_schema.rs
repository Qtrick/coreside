//! Serde types for structured agent JSON responses.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const SCHEMA_VERSION: &str = "1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResponseType {
    Message,
    ToolChange,
    Noop,
}

impl ResponseType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Message => "message",
            Self::ToolChange => "tool_change",
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolComponent {
    pub id: String,
    #[serde(rename = "type")]
    pub component_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub props: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<ToolComponent>>,
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
pub struct AgentResponsePayload {
    #[serde(default = "default_schema_version")]
    pub schema_version: String,
    pub assistant_message: String,
    pub response_type: ResponseType,
    #[serde(default)]
    pub tool_change: Option<ToolChangePayload>,
    #[serde(default)]
    pub diagnostics: Option<Value>,
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
        if self.assistant_message.trim().is_empty()
            && !matches!(self.response_type, ResponseType::Noop)
        {
            return Err("assistantMessage must be non-empty".into());
        }

        match self.response_type {
            ResponseType::ToolChange => {
                let tc = self
                    .tool_change
                    .as_ref()
                    .ok_or_else(|| "toolChange required for responseType tool_change".to_string())?;
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
                if matches!(tc.action, ToolAction::Update | ToolAction::Replace)
                    && tc
                        .target_tool_id
                        .as_ref()
                        .map(|s| s.trim().is_empty())
                        .unwrap_or(true)
                {
                    return Err("targetToolId required for update/replace".into());
                }
            }
            ResponseType::Message | ResponseType::Noop => {}
        }

        Ok(())
    }
}
