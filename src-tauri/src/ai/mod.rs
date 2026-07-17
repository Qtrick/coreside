//! AI layer: providers, prompts, and response parsing.

mod errors;
mod gemini;
mod mock;
mod prompt_builder;
mod provider;
mod response_parser;
mod response_schema;

pub use errors::AiError;
pub use gemini::GeminiProvider;
pub use mock::MockAiProvider;
pub use prompt_builder::{build_agent_prompt, PROMPT_VERSION};
pub use provider::{AgentMessage, AgentRequest, AiProvider, ProviderHealth};
pub use response_parser::{parse_agent_response, ParsedAgentResponse};
pub use response_schema::{
    layout_type_string, ResponseType, ToolChangePayload, ToolDefinition,
};

use crate::config::AppConfig;
use std::sync::Arc;

/// Create an AI provider from config. Uses mock when `AI_PROVIDER=mock`.
pub fn create_provider(config: &AppConfig) -> Result<Arc<dyn AiProvider>, AiError> {
    match config.provider.to_lowercase().as_str() {
        "mock" => Ok(Arc::new(MockAiProvider::new())),
        "gemini" => {
            let key = config
                .api_key
                .clone()
                .filter(|k| !k.trim().is_empty())
                .ok_or(AiError::NotConfigured(
                    "AI API key not configured. Set AI_API_KEY or GEMINI_API_KEY.".into(),
                ))?;
            Ok(Arc::new(GeminiProvider::new(
                key,
                config.model.clone(),
                config.base_url.clone(),
            )))
        }
        "openai" => Err(AiError::Validation(
            "OpenAI provider is not implemented yet. Set AI_PROVIDER=gemini for now, or keep OPENAI_API_KEY in .env for a future adapter.".into(),
        )),
        "anthropic" | "claude" => Err(AiError::Validation(
            "Anthropic/Claude provider is not implemented yet. Set AI_PROVIDER=gemini for now, or keep ANTHROPIC_API_KEY in .env for a future adapter.".into(),
        )),
        other => Err(AiError::Validation(format!(
            "Unsupported AI provider: {other}. Supported today: gemini, mock. Reserved: openai, anthropic."
        ))),
    }
}
