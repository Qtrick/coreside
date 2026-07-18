use thiserror::Error;

#[derive(Debug, Error)]
pub enum SearchError {
    #[error("search not configured: {0}")]
    NotConfigured(String),
    #[error("invalid request: {0}")]
    Invalid(String),
    #[error("provider error: {0}")]
    Provider(String),
    #[error("rate limited")]
    RateLimited,
    #[error("request timed out")]
    Timeout,
    #[error("SSRF blocked: {0}")]
    SsrfBlocked(String),
    #[error("fetch error: {0}")]
    Fetch(String),
    #[error("credential error: {0}")]
    Credential(String),
}

impl SearchError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotConfigured(_) => "not_configured",
            Self::Invalid(_) => "invalid",
            Self::Provider(_) => "provider",
            Self::RateLimited => "rate_limited",
            Self::Timeout => "timeout",
            Self::SsrfBlocked(_) => "ssrf_blocked",
            Self::Fetch(_) => "fetch",
            Self::Credential(_) => "credential",
        }
    }
}
