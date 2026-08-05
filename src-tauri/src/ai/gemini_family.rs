//! Google Gemini generateContent mapping.
//!
//! Multimodal Image parts map to native `inlineData` parts. Tool results use
//! `functionResponse` wrapping the sealed envelope object (trust + envelopeHash).

use serde_json::{json, Value};

use super::provider::AgentMessage;
use super::structured_user_input::{
    flatten_parts_for_provider, is_provider_image_mime, AgentContentPart, AgentRole,
};

/// Map one agent message to a Gemini `contents` entry.
pub fn gemini_content_entry(message: &AgentMessage) -> Value {
    json!({
        "role": message.role.as_gemini_role(),
        "parts": gemini_message_parts(message),
    })
}

/// Build Gemini `parts` for one message.
pub fn gemini_message_parts(message: &AgentMessage) -> Value {
    if message.role == AgentRole::ToolResult {
        return gemini_tool_result_parts(message);
    }
    if message.parts.is_empty() {
        return json!([{ "text": message.content }]);
    }
    let has_image = message
        .parts
        .iter()
        .any(|p| matches!(p, AgentContentPart::Image { .. }));
    if !has_image {
        let text = text_parts_for_gemini(&message.parts);
        if text.is_empty() {
            return json!([{ "text": message.content }]);
        }
        return json!([{ "text": text }]);
    }

    let mut parts: Vec<Value> = Vec::new();
    let text = text_parts_for_gemini(
        &message
            .parts
            .iter()
            .filter(|p| !matches!(p, AgentContentPart::Image { .. }))
            .cloned()
            .collect::<Vec<_>>(),
    );
    if !text.trim().is_empty() {
        parts.push(json!({ "text": text }));
    }
    for part in &message.parts {
        if matches!(part, AgentContentPart::Image { .. }) {
            match map_image_part_to_gemini_inline(part) {
                Ok(v) => parts.push(v),
                Err(reason) => {
                    tracing::info!(target: "coreside::ai::gemini_family", "{reason}");
                    parts.push(json!({
                        "text": format!("[image not sent to model: {reason}]"),
                    }));
                }
            }
        }
    }
    if parts.is_empty() {
        return json!([{ "text": message.content }]);
    }
    if parts.len() == 1 && parts[0].get("text").is_some() && parts[0].as_object().map(|o| o.len()) == Some(1) {
        return Value::Array(parts);
    }
    Value::Array(parts)
}

/// Tool-result turns: native `functionResponse` with parsed envelope object.
fn gemini_tool_result_parts(message: &AgentMessage) -> Value {
    let envelope_text = message.tool_result_upstream_content();
    if let Ok(parsed) = serde_json::from_str::<Value>(&envelope_text) {
        if parsed.get("coresideEnvelope").and_then(|v| v.as_str()) == Some("tool_result") {
            return json!([{
                "functionResponse": {
                    "name": "coreside_tool_result",
                    "response": parsed
                }
            }]);
        }
    }
    json!([{ "text": envelope_text }])
}

fn text_parts_for_gemini(parts: &[AgentContentPart]) -> String {
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

/// Map a trusted Image part to a Gemini `inlineData` part.
pub fn map_image_part_to_gemini_inline(part: &AgentContentPart) -> Result<Value, String> {
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
    let Some(b64) = data_base64.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) else {
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
    let mime = mime_type.trim().to_ascii_lowercase();
    Ok(json!({
        "inlineData": {
            "mimeType": mime,
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
    fn tool_result_uses_function_response_with_envelope_hash() {
        let results = vec![ToolCallResult {
            capability: "web_search".into(),
            ok: true,
            output: json!({"hits": 1}),
            error: None,
            pending_approval: None,
        }];
        let envelope = seal_tool_result_envelope(&results);
        let msg = AgentMessage::with_parts(
            AgentRole::ToolResult,
            "Tool results (1)",
            vec![envelope],
        );
        let parts = gemini_message_parts(&msg);
        let arr = parts.as_array().expect("parts array");
        let fr = &arr[0]["functionResponse"];
        assert_eq!(fr["name"], "coreside_tool_result");
        assert_eq!(
            fr["response"]["trust"].as_str(),
            Some("untrusted_tool_output")
        );
        assert!(fr["response"]["envelopeHash"].is_string());
        assert!(!arr[0].to_string().contains("[image attachmentId="));
    }

    #[test]
    fn multimodal_user_message_emits_inline_data() {
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
        let parts = gemini_message_parts(&msg);
        let arr = parts.as_array().expect("parts");
        assert!(arr.iter().any(|p| p.get("inlineData").is_some()));
        assert!(!arr.iter().any(|p| {
            p["text"]
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
        let parts = gemini_message_parts(&msg);
        let arr = parts.as_array().expect("parts");
        let text = arr[0]["text"].as_str().expect("skip note");
        assert!(text.contains("not sent to model"));
    }
}
