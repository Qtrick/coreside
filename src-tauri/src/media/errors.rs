use thiserror::Error;

#[derive(Debug, Error)]
pub enum MediaError {
    #[error("invalid media: {0}")]
    Invalid(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("download failed: {0}")]
    Download(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("SSRF blocked: {0}")]
    SsrfBlocked(String),
    #[error("duplicate asset")]
    Duplicate,
    #[error("database error: {0}")]
    Database(String),
}

impl MediaError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid(_) => "invalid",
            Self::NotFound(_) => "not_found",
            Self::Download(_) => "download",
            Self::Storage(_) => "storage",
            Self::SsrfBlocked(_) => "ssrf_blocked",
            Self::Duplicate => "duplicate",
            Self::Database(_) => "database",
        }
    }
}
