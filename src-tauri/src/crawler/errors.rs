use thiserror::Error;

#[derive(Debug, Error)]
pub enum CrawlerError {
    #[error("crawler needs setup: {0}")]
    NeedsSetup(String),
    #[error("crawler not ready: {0}")]
    NotReady(String),
    #[error("invalid request: {0}")]
    Invalid(String),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("sidecar unavailable: {0}")]
    Sidecar(String),
    #[error("request timed out")]
    Timeout,
    #[error("cancelled")]
    Cancelled,
    #[error("io error: {0}")]
    Io(String),
}

impl CrawlerError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NeedsSetup(_) => "needs_setup",
            Self::NotReady(_) => "not_ready",
            Self::Invalid(_) => "invalid",
            Self::Protocol(_) => "protocol",
            Self::Sidecar(_) => "sidecar",
            Self::Timeout => "timeout",
            Self::Cancelled => "cancelled",
            Self::Io(_) => "io",
        }
    }
}

impl From<std::io::Error> for CrawlerError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}
