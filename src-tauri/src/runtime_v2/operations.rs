//! Application Operation Protocol V2 types and validation.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::limits::{
    MAX_DEFINITION_JSON_BYTES, MAX_OPERATIONS_PER_TURN, MAX_TRANSACTION_GROUPS_PER_TURN,
};
use crate::ai::SettingsChangePayload;
use crate::ai::SourceCitation;
use crate::ai::ToolCallRequest;
use crate::ai::{ToolAction, ToolChangePayload, ToolDefinition};

pub const SCHEMA_VERSION_V2: &str = "2";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageVisibility {
    Visible,
    Silent,
    Status,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AssistantMessageV2 {
    pub id: String,
    pub content: String,
    #[serde(default = "default_visible")]
    pub visibility: MessageVisibility,
}

fn default_visible() -> MessageVisibility {
    MessageVisibility::Visible
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Audience {
    CurrentUser,
    CurrentSurface,
    CurrentChat,
    CurrentProject,
    FutureParticipants,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct OperationTarget {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panel_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppOperation {
    pub id: String,
    #[serde(rename = "type")]
    pub op_type: String,
    #[serde(default)]
    pub target: OperationTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_revision: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transaction_group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    #[serde(default, alias = "depends_on", skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<Vec<String>>,
    #[serde(default)]
    pub payload: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_approval: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destructive: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audience: Option<Audience>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentResponseV2 {
    #[serde(default = "default_schema_v2")]
    pub schema_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default)]
    pub assistant_messages: Vec<AssistantMessageV2>,
    /// Backward-compatible single message field (v1 / adapters).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assistant_message: Option<String>,
    #[serde(default)]
    pub operations: Vec<AppOperation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_change: Option<ToolChangePayload>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings_change: Option<SettingsChangePayload>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallRequest>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub citations: Option<Vec<SourceCitation>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub silent: Option<bool>,
}

fn default_schema_v2() -> String {
    SCHEMA_VERSION_V2.to_string()
}

/// Fully implemented and tested model-facing operations supported by the transaction runtime.
pub const SUPPORTED_MODEL_OPERATIONS: &[&str] = &[
    "surface.create",
    "surface.promote",
    "surface.archive",
    "surface.restore",
    "surface.delete",
    "tool.full_replace",
    "component.insert",
    "component.remove",
    "component.move",
    "component.replace",
    "component.update_props",
    "component.update_actions",
    "component.update_children",
    "component.update_visibility",
    "state.set",
    "state.patch",
    "layout.update",
    "wallpaper.apply",
    "data.model_upsert",
    "data.record_create",
    "data.record_update",
    "data.record_delete",
    "chat.inline_surface_create",
    "chat.inline_surface_update",
    "chat.inline_surface_remove",
    "chat.status",
    "chat.notification",
    "surface.add_section",
    "surface.remove_section",
    "surface.update_section",
    "component.bind_state",
    "component.bind_action",
    "component.set_style_token",
];

/// Internal operations used by host subsystems and commands (not directly model-facing).
pub const INTERNAL_OPERATIONS: &[&str] = &[
    "data.migrate",
    "surface.update_metadata",
    "surface.move",
    "surface.duplicate",
    "state.reset",
    "state.delete_key",
    "manifest.upsert",
    "manifest.set_health",
    "manifest.disable",
    "manifest.restore_last_known_good",
    "project.panel_create",
    "project.panel_update",
    "export.prepare",
    "package.export",
    "package.import",
    "route.navigate",
    "permission.request",
    "test.upsert",
    "test.run",
];

/// Reserved / future operations recognized by the schema but not advertised to the model.
pub const RESERVED_OPERATIONS: &[&str] = &[
    "setting.create",
    "setting.update",
    "setting.delete",
    "layout.add_panel",
    "layout.move_panel",
    "layout.resize_panel",
    "layout.remove_panel",
    "layout.set_visibility",
    "event.dispatch",
    "subscription.create",
    "subscription.update",
    "subscription.delete",
    "chat.branch_create",
    "automation.create",
    "automation.update",
    "automation.pause",
    "automation.resume",
    "wallpaper.create",
    "wallpaper.delete",
];

pub fn is_supported_model_operation(op: &str) -> bool {
    SUPPORTED_MODEL_OPERATIONS.contains(&op)
}

pub fn is_internal_operation(op: &str) -> bool {
    INTERNAL_OPERATIONS.contains(&op)
}

pub fn is_reserved_operation(op: &str) -> bool {
    RESERVED_OPERATIONS.contains(&op)
}

/// Allowed operation type prefixes / exact names.
pub fn is_known_operation_type(op: &str) -> bool {
    is_supported_model_operation(op) || is_internal_operation(op) || is_reserved_operation(op)
}

/// Shared validation core for operation lists.
/// `require_model_ops` restricts to SUPPORTED_MODEL_OPERATIONS only (rejecting internal/reserved).
fn validate_operations_inner(
    operations: &[AppOperation],
    require_model_ops: bool,
) -> Result<(), String> {
    if operations.len() > MAX_OPERATIONS_PER_TURN {
        return Err(format!(
            "operations exceeds max of {MAX_OPERATIONS_PER_TURN}"
        ));
    }
    let mut groups = std::collections::HashSet::new();
    let mut seen_ids = std::collections::HashSet::new();
    for op in operations {
        if op.id.trim().is_empty() {
            return Err("operation.id must be non-empty".into());
        }
        if !seen_ids.insert(op.id.clone()) {
            return Err(format!("duplicate operation.id: {}", op.id));
        }
        if require_model_ops {
            if !is_supported_model_operation(&op.op_type) {
                if is_internal_operation(&op.op_type) {
                    return Err(format!(
                        "internal operation '{}' is not allowed in model responses",
                        op.op_type
                    ));
                }
                if is_reserved_operation(&op.op_type) {
                    return Err(format!(
                        "reserved operation '{}' is not allowed in model responses",
                        op.op_type
                    ));
                }
                return Err(format!("unknown operation type: {}", op.op_type));
            }
        } else if !is_known_operation_type(&op.op_type) {
            return Err(format!("unknown operation type: {}", op.op_type));
        }
        if let Some(g) = &op.transaction_group {
            groups.insert(g.clone());
        }
        if let Some(sid) = op.target.surface_id.as_ref() {
            crate::security::assert_not_protected(sid)?;
        }
        if let Some(tid) = op.target.tool_id.as_ref() {
            crate::security::assert_not_protected(tid)?;
        }
        if let Some(tool) = op.payload.get("tool") {
            if let Some(id) = tool.get("id").and_then(|v| v.as_str()) {
                crate::security::assert_not_protected(id)?;
            }
        }
        if let Some(aud) = &op.audience {
            match aud {
                Audience::CurrentSurface => {
                    if op.target.surface_id.is_none() && op.target.tool_id.is_none() {
                        return Err(format!(
                            "operation '{}' with audience CurrentSurface must specify surfaceId or toolId target",
                            op.id
                        ));
                    }
                }
                Audience::CurrentChat => {
                    if op.target.conversation_id.is_none()
                        && op.target.surface_id.is_none()
                        && op.target.message_id.is_none()
                    {
                        return Err(format!(
                            "operation '{}' with audience CurrentChat must specify conversationId, surfaceId, or messageId target",
                            op.id
                        ));
                    }
                }
                Audience::CurrentProject => {
                    if op.target.project_id.is_none()
                        && op.target.surface_id.is_none()
                        && op.target.tool_id.is_none()
                    {
                        return Err(format!(
                            "operation '{}' with audience CurrentProject must specify projectId, surfaceId, or toolId target",
                            op.id
                        ));
                    }
                }
                Audience::CurrentUser | Audience::FutureParticipants => {}
            }
        }
        // Semantic validation of required target fields and schemas (model-facing only)
        if require_model_ops {
            match op.op_type.as_str() {
                "component.insert" => {
                    if op.target.surface_id.is_none() && op.target.tool_id.is_none() {
                        return Err(format!(
                            "operation '{}' of type '{}' requires surfaceId or toolId target",
                            op.id, op.op_type
                        ));
                    }
                    if !op
                        .payload
                        .get("component")
                        .map(|c| c.is_object())
                        .unwrap_or(false)
                    {
                        return Err(format!(
                            "operation '{}' of type 'component.insert' requires a 'component' object payload",
                            op.id
                        ));
                    }
                }
                "component.remove"
                | "component.replace"
                | "component.update_actions"
                | "component.update_children"
                | "component.update_visibility" => {
                    if op.target.component_id.is_none() && op.payload.get("componentId").is_none() {
                        return Err(format!(
                            "operation '{}' of type '{}' requires componentId target or payload",
                            op.id, op.op_type
                        ));
                    }
                }
                "component.update_props" => {
                    if op.target.component_id.is_none()
                        && op.target.surface_id.is_none()
                        && op.payload.get("componentId").is_none()
                    {
                        return Err(format!(
                            "operation '{}' of type 'component.update_props' requires target",
                            op.id
                        ));
                    }
                }
                "chat.inline_surface_create" => {
                    if !op
                        .payload
                        .get("definition")
                        .map(|d| d.is_object())
                        .unwrap_or(false)
                        && !op
                            .payload
                            .get("tool")
                            .map(|t| t.is_object())
                            .unwrap_or(false)
                    {
                        return Err(format!(
                            "operation '{}' of type 'chat.inline_surface_create' requires a 'definition' or 'tool' object payload",
                            op.id
                        ));
                    }
                }
                _ => {}
            }
        }
        if let Ok(bytes) = serde_json::to_vec(&op.payload) {
            if bytes.len() > MAX_DEFINITION_JSON_BYTES {
                return Err("operation payload too large".into());
            }
        }
    }
    if groups.len() > MAX_TRANSACTION_GROUPS_PER_TURN {
        return Err(format!(
            "transaction groups exceed max of {MAX_TRANSACTION_GROUPS_PER_TURN}"
        ));
    }
    Ok(())
}

/// Validate a list of operations for model-facing responses.
/// Only operations in SUPPORTED_MODEL_OPERATIONS are allowed.
/// Internal and reserved operations must only come from host subsystems.
pub fn validate_model_operations(operations: &[AppOperation]) -> Result<(), String> {
    validate_operations_inner(operations, true)
}

/// Validate a list of application operations (trusted boundary).
/// Accepts all known operation types including internal and reserved.
pub fn validate_operations(operations: &[AppOperation]) -> Result<(), String> {
    validate_operations_inner(operations, false)
}

impl AgentResponseV2 {
    pub fn visible_assistant_text(&self) -> String {
        if self.silent.unwrap_or(false) {
            return String::new();
        }
        let from_msgs: Vec<&str> = self
            .assistant_messages
            .iter()
            .filter(|m| matches!(m.visibility, MessageVisibility::Visible))
            .map(|m| m.content.as_str())
            .filter(|s| !s.trim().is_empty())
            .collect();
        if !from_msgs.is_empty() {
            return from_msgs.join("\n\n");
        }
        self.assistant_message.clone().unwrap_or_default()
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_operations(&self.operations)?;

        let has_visible = !self.visible_assistant_text().trim().is_empty();
        let silent = self.silent.unwrap_or(false) || (!has_visible && !self.operations.is_empty());
        if !silent
            && !has_visible
            && self.operations.is_empty()
            && self.tool_change.is_none()
            && self.settings_change.is_none()
            && self
                .tool_calls
                .as_ref()
                .map(|c| c.is_empty())
                .unwrap_or(true)
        {
            return Err(
                "response must include a message, operations, or tool/settings change".into(),
            );
        }
        Ok(())
    }
}

/// Adapt a v1 tool_change into a v2 operation list.
pub fn tool_change_to_operations(tc: &ToolChangePayload) -> Vec<AppOperation> {
    let tool = match &tc.tool {
        Some(t) => t,
        None => return vec![],
    };
    let op_type = match tc.action.as_str() {
        "create" => "surface.create",
        "update" | "replace" => "tool.full_replace",
        _ => "tool.full_replace",
    };
    // A v1 update/replace persists under targetToolId, regardless of the ID in
    // the proposed definition. Keep the v2 target aligned so preview and apply
    // address the same surface and capability boundary.
    let target_tool_id = if matches!(&tc.action, ToolAction::Update | ToolAction::Replace) {
        tc.target_tool_id
            .as_deref()
            .filter(|id| !id.trim().is_empty())
            .unwrap_or(&tool.id)
    } else {
        &tool.id
    };
    vec![AppOperation {
        id: format!("op-v1-{}", tool.id),
        op_type: op_type.into(),
        target: OperationTarget {
            surface_id: Some(format!("surf-{target_tool_id}")),
            tool_id: Some(target_tool_id.to_string()),
            surface_type: Some("tool".into()),
            placement: Some("tool_canvas".into()),
            ..Default::default()
        },
        base_revision: None,
        transaction_group: Some("v1-tool-change".into()),
        idempotency_key: None,
        depends_on: None,
        payload: json!({
            "action": tc.action.as_str(),
            "tool": tool,
            "changeSummary": tc.change_summary,
            "targetToolId": tc.target_tool_id,
        }),
        requires_approval: Some(true),
        destructive: Some(matches!(tc.action.as_str(), "replace")),
        audience: Some(Audience::CurrentUser),
    }]
}

pub fn surface_definition_from_tool(tool: &ToolDefinition) -> Value {
    json!({
        "id": tool.id,
        "name": tool.name,
        "description": tool.description,
        "layout": tool.layout,
        "components": tool.components,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::{ToolAction, ToolChangePayload, ToolDefinition};

    #[test]
    fn validates_empty_silent_ops() {
        let resp = AgentResponseV2 {
            schema_version: "2".into(),
            turn_id: Some("t1".into()),
            assistant_messages: vec![],
            assistant_message: None,
            operations: vec![AppOperation {
                id: "op1".into(),
                op_type: "state.set".into(),
                target: OperationTarget {
                    surface_id: Some("surf-test".into()),
                    ..Default::default()
                },
                base_revision: None,
                transaction_group: None,
                idempotency_key: None,
                depends_on: None,
                payload: json!({"state": {"key": "value"}}),
                requires_approval: None,
                destructive: None,
                audience: None,
            }],
            tool_change: None,
            settings_change: None,
            tool_calls: None,
            citations: None,
            diagnostics: None,
            silent: Some(true),
        };
        assert!(resp.validate().is_ok());
        assert!(resp.visible_assistant_text().is_empty());
    }

    #[test]
    fn rejects_unknown_op() {
        let resp = AgentResponseV2 {
            schema_version: "2".into(),
            turn_id: None,
            assistant_messages: vec![AssistantMessageV2 {
                id: "m1".into(),
                content: "hi".into(),
                visibility: MessageVisibility::Visible,
            }],
            assistant_message: None,
            operations: vec![AppOperation {
                id: "op1".into(),
                op_type: "shell.exec".into(),
                target: OperationTarget::default(),
                base_revision: None,
                transaction_group: None,
                idempotency_key: None,
                depends_on: None,
                payload: json!({}),
                requires_approval: None,
                destructive: None,
                audience: None,
            }],
            tool_change: None,
            settings_change: None,
            tool_calls: None,
            citations: None,
            diagnostics: None,
            silent: None,
        };
        assert!(resp.validate().is_err());
    }

    #[test]
    fn adapts_v1_tool_change() {
        let tc = ToolChangePayload {
            action: ToolAction::Create,
            target_tool_id: None,
            tool: Some(ToolDefinition {
                id: "water".into(),
                name: "Water".into(),
                description: "".into(),
                layout: json!({"type":"single-column"}),
                components: vec![],
            }),
            change_summary: "create".into(),
        };
        let ops = tool_change_to_operations(&tc);
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].op_type, "surface.create");
    }

    #[test]
    fn v1_replace_targets_the_persisted_tool_surface() {
        let tc = ToolChangePayload {
            action: ToolAction::Replace,
            target_tool_id: Some("existing-tool".into()),
            tool: Some(ToolDefinition {
                id: "proposed-tool".into(),
                name: "Replacement".into(),
                description: "".into(),
                layout: json!({"type":"single-column"}),
                components: vec![],
            }),
            change_summary: "replace".into(),
        };
        let op = tool_change_to_operations(&tc).pop().unwrap();
        assert_eq!(op.op_type, "tool.full_replace");
        assert_eq!(op.target.tool_id.as_deref(), Some("existing-tool"));
        assert_eq!(op.target.surface_id.as_deref(), Some("surf-existing-tool"));
    }

    #[test]
    fn rejects_internal_operations_in_model_responses() {
        let ops = vec![AppOperation {
            id: "op1".into(),
            op_type: "state.reset".into(),
            target: OperationTarget::default(),
            base_revision: None,
            transaction_group: None,
            idempotency_key: None,
            depends_on: None,
            payload: json!({}),
            requires_approval: None,
            destructive: None,
            audience: None,
        }];
        let err = validate_model_operations(&ops).unwrap_err();
        assert!(err.contains("internal operation"), "{err}");
    }

    #[test]
    fn rejects_reserved_operations_in_model_responses() {
        let ops = vec![AppOperation {
            id: "op1".into(),
            op_type: "layout.move_panel".into(),
            target: OperationTarget::default(),
            base_revision: None,
            transaction_group: None,
            idempotency_key: None,
            depends_on: None,
            payload: json!({}),
            requires_approval: None,
            destructive: None,
            audience: None,
        }];
        let err = validate_model_operations(&ops).unwrap_err();
        assert!(err.contains("reserved operation"), "{err}");
    }

    #[test]
    fn accepts_model_facing_operations() {
        let ops = vec![AppOperation {
            id: "op1".into(),
            op_type: "component.update_props".into(),
            target: OperationTarget {
                surface_id: Some("surf-test".into()),
                ..Default::default()
            },
            base_revision: None,
            transaction_group: None,
            idempotency_key: None,
            depends_on: None,
            payload: json!({"props": {"text": "hello"}}),
            requires_approval: None,
            destructive: None,
            audience: None,
        }];
        assert!(validate_model_operations(&ops).is_ok());
    }
}
