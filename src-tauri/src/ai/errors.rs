//! AI error types.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AiError {
    #[error("AI not configured: {0}")]
    NotConfigured(String),

    #[error("AI request timed out")]
    Timeout,

    #[error("AI request cancelled")]
    Cancelled,

    #[error("HTTP error: {0}")]
    Http(String),

    #[error("Failed to parse AI response: {0}")]
    Parse(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Provider error: {0}")]
    Provider(String),
}

impl AiError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotConfigured(_) => "not_configured",
            Self::Timeout => "timeout",
            Self::Cancelled => "cancelled",
            Self::Http(_) => "http",
            Self::Parse(_) => "parse",
            Self::Validation(_) => "validation",
            Self::Provider(_) => "provider",
        }
    }
}
