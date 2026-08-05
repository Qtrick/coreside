//! Provider send-path validation against capability profiles.

use super::errors::AiError;
use super::platform::{self, CapabilityFlag, CapabilityProfile};
use super::provider::AgentMessage;
use super::structured_user_input::{
    validate_image_part_for_provider_send, AgentContentPart, AgentRole,
};

/// Validate that the outgoing message batch is compatible with the active provider.
///
/// Returns a clear validation error when image or tool-result parts are present but
/// the provider preset does not advertise support — callers should roll back the
/// persisted user message so the composer draft is preserved.
pub fn validate_provider_send(provider_id: &str, messages: &[AgentMessage]) -> Result<(), AiError> {
    let profile = platform::descriptor_by_id(provider_id)
        .map(|d| d.capability_profile.clone())
        .unwrap_or_else(CapabilityProfile::empty_unknown);

    let has_images = messages.iter().any(|m| {
        m.parts
            .iter()
            .any(|p| matches!(p, AgentContentPart::Image { .. }))
    });
    if has_images && !profile.supports(CapabilityFlag::ImageInput) {
        let display = platform::descriptor_by_id(provider_id)
            .map(|d| d.display_name.to_string())
            .unwrap_or_else(|| provider_id.to_string());
        return Err(AiError::Validation(format!(
            "{display} does not support image attachments. Remove images or choose a multimodal provider."
        )));
    }

    if profile.supports(CapabilityFlag::ImageInput) {
        for message in messages {
            for part in &message.parts {
                if matches!(part, AgentContentPart::Image { .. }) {
                    if let Err(reason) = validate_image_part_for_provider_send(part) {
                        return Err(AiError::Validation(format!(
                            "Image attachment could not be sent: {reason}. Remove the image or try again."
                        )));
                    }
                }
            }
        }
    }

    let has_tool_results = messages.iter().any(|m| m.role == AgentRole::ToolResult);
    if has_tool_results && !profile.supports(CapabilityFlag::NativeToolResults) {
        let display = platform::descriptor_by_id(provider_id)
            .map(|d| d.display_name.to_string())
            .unwrap_or_else(|| provider_id.to_string());
        return Err(AiError::Validation(format!(
            "{display} does not support tool-result follow-ups for this conversation. Choose a provider with tool support."
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::structured_user_input::AgentContentPart;

    #[test]
    fn blocks_images_for_mock_provider() {
        let messages = vec![AgentMessage::with_parts(
            AgentRole::User,
            "see image",
            vec![AgentContentPart::Image {
                attachment_id: "att-1".into(),
                mime_type: "image/png".into(),
                data_base64: Some("abc".into()),
            }],
        )];
        let err = validate_provider_send("mock", &messages).unwrap_err();
        assert!(matches!(err, AiError::Validation(_)));
        assert!(err.to_string().contains("image"));
    }

    #[test]
    fn allows_images_for_anthropic_preset() {
        let messages = vec![AgentMessage::with_parts(
            AgentRole::User,
            "see image",
            vec![AgentContentPart::Image {
                attachment_id: "att-1".into(),
                mime_type: "image/png".into(),
                data_base64: Some("abc".into()),
            }],
        )];
        assert!(validate_provider_send("anthropic", &messages).is_ok());
    }

    #[test]
    fn blocks_unsendable_images_when_preset_claims_image_input() {
        let messages = vec![AgentMessage::with_parts(
            AgentRole::User,
            "see image",
            vec![AgentContentPart::Image {
                attachment_id: "att-missing".into(),
                mime_type: "image/png".into(),
                data_base64: None,
            }],
        )];
        let err = validate_provider_send("anthropic", &messages).unwrap_err();
        assert!(matches!(err, AiError::Validation(_)));
        assert!(err.to_string().contains("could not be sent"));
    }

    #[test]
    fn blocks_tool_results_for_text_only_preset() {
        let messages = vec![AgentMessage::with_parts(
            AgentRole::ToolResult,
            "Tool results (1)",
            vec![AgentContentPart::Text {
                text: "legacy".into(),
            }],
        )];
        let err = validate_provider_send("mock", &messages).unwrap_err();
        assert!(matches!(err, AiError::Validation(_)));
        assert!(err.to_string().contains("tool"));
    }
}
