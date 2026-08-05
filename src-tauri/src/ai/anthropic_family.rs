//! Anthropic Messages API content mapping.
//!
//! Multimodal Image parts map to native `image` content blocks (base64 source).
//! Tool results use multipart `text` blocks with the sealed JSON envelope — not
//! flattened display summaries or `[image attachmentId=…]` placeholders.

use serde_json::{json, Value};

use super::provider::AgentMessage;
use super::structured_user_input::{
    flatten_parts_for_provider, is_provider_image_mime, AgentContentPart, AgentRole,
};

/// Map one agent message to an Anthropic Messages API message object.
pub fn anthropic_message_json(message: &AgentMessage) -> Value {
    json!({
        "role": message.role.as_anthropic_role(),
        "content": anthropic_message_content(message),
    })
}

/// Message `content` for Anthropic: string when text-only; array when multipart.
pub fn anthropic_message_content(message: &AgentMessage) -> Value {
    if message.role == AgentRole::ToolResult {
        return anthropic_tool_result_content(message);
    }
    if message.parts.is_empty() {
        return json!(message.content);
    }
    let has_image = message
        .parts
        .iter()
        .any(|p| matches!(p, AgentContentPart::Image { .. }));
    if !has_image {
        let text = text_parts_for_anthropic(&message.parts);
        if text.is_empty() {
            return json!(message.content);
        }
        return json!(text);
    }

    let mut blocks: Vec<Value> = Vec::new();
    let text = text_parts_for_anthropic(
        &message
            .parts
            .iter()
            .filter(|p| !matches!(p, AgentContentPart::Image { .. }))
            .cloned()
            .collect::<Vec<_>>(),
    );
    if !text.trim().is_empty() {
        blocks.push(json!({
            "type": "text",
            "text": text
        }));
    }
    for part in &message.parts {
        if matches!(part, AgentContentPart::Image { .. }) {
            match map_image_part_to_anthropic_block(part) {
                Ok(v) => blocks.push(v),
                Err(reason) => {
                    tracing::info!(target: "coreside::ai::anthropic_family", "{reason}");
                    blocks.push(json!({
                        "type": "text",
                        "text": format!("[image not sent to model: {reason}]"),
                    }));
                }
            }
        }
    }
    if blocks.is_empty() {
        return json!(message.content);
    }
    if blocks.len() == 1 && blocks[0].get("type").and_then(|t| t.as_str()) == Some("text") {
        return blocks[0]
            .get("text")
            .cloned()
            .unwrap_or_else(|| json!(message.content));
    }
    Value::Array(blocks)
}

/// Tool-result turns: native multipart text block with sealed envelope JSON.
/// Anthropic `tool_result` blocks require a matching prior `tool_use` id; Coreside
/// uses JSON-schema toolCalls, so we send the envelope as user text content blocks.
fn anthropic_tool_result_content(message: &AgentMessage) -> Value {
    let envelope = message.tool_result_upstream_content();
    json!([{
        "type": "text",
        "text": envelope
    }])
}

fn text_parts_for_anthropic(parts: &[AgentContentPart]) -> String {
    let filtered: Vec<AgentContentPart> = parts
        .iter()
        .filter(|p| !matches!(p, AgentContentPart::ToolResultEnvelope { .. }))
        .cloned()
        .collect();
    if filtered.is_empty() {
        return String::new();
    }
    flatten_parts_for_provider(&filtered)
}

/// Map a trusted Image part to an Anthropic `image` content block.
pub fn map_image_part_to_anthropic_block(part: &AgentContentPart) -> Result<Value, String> {
    let AgentContentPart::Image {
        attachment_id,
        mime_type,
        data_base64,
    } = part
    else {
        return Err("not an Image part".into());
    };
    if attachment_id.trim().is_empty() {
        return Err("image part missing attachment_id".into());
    }
    if !is_provider_image_mime(mime_type) {
        return Err(format!(
            "skip image {attachment_id}: unsupported mime {mime_type}"
        ));
    }
    let Some(b64) = data_base64
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    else {
        return Err(format!(
            "skip image {attachment_id}: no authorized bytes loaded (paths are never sent)"
        ));
    };
    if b64.starts_with('/') || (b64.contains("://") && !b64.starts_with("data:")) {
        return Err(format!(
            "skip image {attachment_id}: refusing non-base64 / path-like payload"
        ));
    }
    let approx_bytes = (b64.len() / 4).saturating_mul(3);
    if approx_bytes > super::structured_user_input::MAX_PROVIDER_IMAGE_BYTES {
        return Err(format!(
            "skip image {attachment_id}: exceeds provider byte budget (~{approx_bytes} bytes)"
        ));
    }
    let media_type = mime_type.trim().to_ascii_lowercase();
    Ok(json!({
        "type": "image",
        "source": {
            "type": "base64",
            "media_type": media_type,
            "data": b64
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::capability_registry::{seal_tool_result_envelope, ToolCallResult};
    use crate::ai::structured_user_input::image_part_from_authorized_bytes;

    #[test]
    fn tool_result_uses_multipart_text_envelope_not_display_summary() {
        let results = vec![ToolCallResult {
            capability: "web_search".into(),
            ok: true,
            output: json!({"hits": 1}),
            error: None,
            pending_approval: None,
        }];
        let envelope = seal_tool_result_envelope(&results);
        let msg =
            AgentMessage::with_parts(AgentRole::ToolResult, "Tool results (1)", vec![envelope]);
        let mapped = anthropic_message_json(&msg);
        assert_eq!(mapped["role"], "user");
        let blocks = mapped["content"].as_array().expect("multipart content");
        let text = blocks[0]["text"].as_str().expect("text block");
        assert!(text.contains("untrusted_tool_output"));
        assert!(text.contains("envelopeHash"));
        assert!(text.contains("web_search"));
        assert!(!text.contains("[image attachmentId="));
    }

    #[test]
    fn multimodal_user_message_emits_native_image_block() {
        let png: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00,
            0x00, 0x90, 0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08,
            0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x01, 0x00, 0x05, 0xFE,
            0xD4, 0xEF, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        let image = image_part_from_authorized_bytes("att-1", "image/png", png).unwrap();
        let msg = AgentMessage::with_parts(
            AgentRole::User,
            "see image",
            vec![
                AgentContentPart::Text {
                    text: "see image".into(),
                },
                image,
            ],
        );
        let content = anthropic_message_content(&msg);
        let blocks = content.as_array().expect("multipart");
        assert!(blocks.iter().any(|b| b["type"] == "image"));
        assert!(blocks
            .iter()
            .any(|b| { b.pointer("/source/type").and_then(|v| v.as_str()) == Some("base64") }));
        assert!(!blocks.iter().any(|b| {
            b["type"] == "text"
                && b["text"]
                    .as_str()
                    .unwrap_or("")
                    .contains("[image attachmentId=")
        }));
    }

    #[test]
    fn skipped_image_includes_honest_not_sent_note() {
        let msg = AgentMessage::with_parts(
            AgentRole::User,
            "see attachment",
            vec![AgentContentPart::Image {
                attachment_id: "att-missing".into(),
                mime_type: "image/png".into(),
                data_base64: None,
            }],
        );
        let content = anthropic_message_content(&msg);
        let text = content.as_str().or_else(|| {
            content
                .as_array()
                .and_then(|a| a.first())
                .and_then(|b| b["text"].as_str())
        });
        let text = text.expect("skip note");
        assert!(text.contains("not sent to model"));
        assert!(!text.contains("[image attachmentId="));
    }
}
