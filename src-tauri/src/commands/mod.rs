//! Tauri command handlers.

mod ai_cmds;
mod conversation_cmds;
mod message_cmds;
mod settings_cmds;
mod tool_cmds;

pub use ai_cmds::*;
pub use conversation_cmds::*;
pub use message_cmds::*;
pub use settings_cmds::*;
pub use tool_cmds::*;

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

    pub fn sanitized(code: impl Into<String>, err: impl std::fmt::Display, key: Option<&str>) -> Self {
        Self {
            code: code.into(),
            message: sanitize_error(&err.to_string(), key),
        }
    }
}

impl From<crate::db::DbError> for CommandError {
    fn from(value: crate::db::DbError) -> Self {
        let code = match &value {
            crate::db::DbError::NotFound(_) => "not_found",
            crate::db::DbError::Invalid(_) => "invalid",
            _ => "db",
        };
        Self::new(code, value.to_string())
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
