//! Exa API and local budget errors.

use thiserror::Error;

use crate::search::SearchError;

#[derive(Debug, Error, Clone)]
pub enum ExaError {
    #[error("Exa is not configured: {0}")]
    NotConfigured(String),
    #[error("invalid Exa request: {0}")]
    Invalid(String),
    #[error("invalid Exa API key")]
    InvalidApiKey,
    #[error("Exa account credits exhausted")]
    CreditsExhausted,
    #[error("insufficient Exa permission")]
    InsufficientPermission,
    #[error("Exa rate limited")]
    RateLimited,
    #[error("Exa provider unavailable: {0}")]
    ProviderUnavailable(String),
    #[error("local Exa budget reached")]
    LocalBudgetReached,
    #[error("Exa request timed out")]
    Timeout,
    #[error("credential error: {0}")]
    Credential(String),
    #[error("Exa error: {0}")]
    Other(String),
}

impl ExaError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotConfigured(_) => "not_configured",
            Self::Invalid(_) => "invalid_request",
            Self::InvalidApiKey => "invalid_api_key",
            Self::CreditsExhausted => "credits_exhausted",
            Self::InsufficientPermission => "insufficient_permission",
            Self::RateLimited => "rate_limited",
            Self::ProviderUnavailable(_) => "provider_unavailable",
            Self::LocalBudgetReached => "local_budget_reached",
            Self::Timeout => "timeout",
            Self::Credential(_) => "credential",
            Self::Other(_) => "exa_error",
        }
    }

    pub fn from_status(status: u16, body: &str) -> Self {
        let snippet = body.chars().take(200).collect::<String>();
        match status {
            401 => Self::InvalidApiKey,
            402 => Self::CreditsExhausted,
            403 => Self::InsufficientPermission,
            422 => Self::Invalid(if snippet.is_empty() {
                "unprocessable request".into()
            } else {
                snippet
            }),
            429 => Self::RateLimited,
            500..=599 => Self::ProviderUnavailable(if snippet.is_empty() {
                format!("HTTP {status}")
            } else {
                snippet
            }),
            _ => Self::Other(format!("HTTP {status}: {snippet}")),
        }
    }
}

impl From<ExaError> for SearchError {
    fn from(value: ExaError) -> Self {
        match value {
            ExaError::NotConfigured(m) => SearchError::NotConfigured(m),
            ExaError::Invalid(m) => SearchError::Invalid(m),
            ExaError::RateLimited => SearchError::RateLimited,
            ExaError::Timeout => SearchError::Timeout,
            ExaError::Credential(m) => SearchError::Credential(m),
            ExaError::LocalBudgetReached => {
                SearchError::Provider("local Exa budget reached".into())
            }
            ExaError::CreditsExhausted => {
                SearchError::Provider("Exa credits exhausted".into())
            }
            ExaError::InvalidApiKey => SearchError::Credential("invalid Exa API key".into()),
            other => SearchError::Provider(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_http_status_codes() {
        assert_eq!(ExaError::from_status(401, "").code(), "invalid_api_key");
        assert_eq!(ExaError::from_status(402, "").code(), "credits_exhausted");
        assert_eq!(ExaError::from_status(403, "").code(), "insufficient_permission");
        assert_eq!(ExaError::from_status(422, "bad").code(), "invalid_request");
        assert_eq!(ExaError::from_status(429, "").code(), "rate_limited");
        assert_eq!(ExaError::from_status(503, "down").code(), "provider_unavailable");
    }
}
