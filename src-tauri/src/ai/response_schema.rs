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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
            crate::runtime_v2::validate_operations(&ops)?;
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
}
