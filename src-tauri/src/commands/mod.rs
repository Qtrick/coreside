//! Tauri command handlers.

mod ai_cmds;
mod attachment_cmds;
mod automation_cmds;
mod conversation_cmds;
mod crawler_cmds;
mod credential_cmds;
mod exa_cmds;
mod export_cmds;
mod kernel_cmds;
mod link_cmds;
mod media_cmds;
mod message_cmds;
mod project_cmds;
mod runtime_v2_cmds;
mod search_cmds;
mod settings_cmds;
mod tool_cmds;
mod window_cmds;

pub use ai_cmds::*;
pub use attachment_cmds::*;
pub use automation_cmds::*;
pub use conversation_cmds::*;
pub use crawler_cmds::*;
pub use credential_cmds::*;
pub use exa_cmds::*;
pub use export_cmds::*;
pub use kernel_cmds::*;
pub use link_cmds::*;
pub use media_cmds::*;
pub use message_cmds::*;
pub use project_cmds::*;
pub use runtime_v2_cmds::*;
pub use search_cmds::*;
pub use settings_cmds::*;
pub use tool_cmds::*;
pub use window_cmds::*;

use serde::Serialize;

use crate::security::sanitize_error;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl CommandError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn sanitized(
        code: impl Into<String>,
        err: impl std::fmt::Display,
        key: Option<&str>,
    ) -> Self {
        Self {
            code: code.into(),
            message: sanitize_error(&err.to_string(), key),
        }
    }
}

impl From<crate::db::DbError> for CommandError {
    /// `NotFound` and `Invalid` carry Coreside-authored text and are safe to
    /// forward. Engine errors are not: raw rusqlite/serde messages name tables,
    /// columns, and constraints, so they are logged and replaced with a stable
    /// message at the IPC boundary.
    fn from(value: crate::db::DbError) -> Self {
        match &value {
            crate::db::DbError::NotFound(msg) => Self::new("not_found", msg.clone()),
            crate::db::DbError::Invalid(msg) => {
                let code = if msg.starts_with("revision_conflict") {
                    "conflict"
                } else {
                    "invalid"
                };
                Self::new(code, msg.clone())
            }
            other => {
                tracing::error!(error = %other, "database error surfaced to IPC");
                Self::new("storage_error", "Local storage could not complete that.")
            }
        }
    }
}

impl From<crate::ai::AiError> for CommandError {
    fn from(value: crate::ai::AiError) -> Self {
        Self::new(value.code(), sanitize_error(&value.to_string(), None))
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for CommandError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::DbError;

    #[test]
    fn engine_errors_do_not_reach_the_ipc_boundary() {
        let raw = rusqlite::Error::InvalidColumnName("provider_connections.keyring_account".into());
        let err = CommandError::from(DbError::Sqlite(raw));
        assert_eq!(err.code, "storage_error");
        assert!(
            !err.message.contains("keyring_account"),
            "leaked schema detail: {}",
            err.message
        );
    }

    #[test]
    fn authored_errors_keep_their_text_and_code() {
        let not_found = CommandError::from(DbError::NotFound("tool tool-1".into()));
        assert_eq!(not_found.code, "not_found");
        assert_eq!(not_found.message, "tool tool-1");

        let invalid = CommandError::from(DbError::Invalid("title is required".into()));
        assert_eq!(invalid.code, "invalid");

        let conflict = CommandError::from(DbError::Invalid(
            "revision_conflict: expected 2, found 3".into(),
        ));
        assert_eq!(conflict.code, "conflict");
    }
}
