//! Stage chat file attachments into managed app-data storage.

use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use dirs::data_dir;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::CommandError;
use crate::db::product_data_dir;
use crate::security::sanitize_error;

/// Max bytes per attached file (12 MiB).
const MAX_ATTACHMENT_BYTES: usize = 12 * 1024 * 1024;
/// Max attachments per message.
pub const MAX_ATTACHMENTS_PER_MESSAGE: usize = 5;

const ALLOWED_PREFIXES: &[&str] = &["image/", "text/", "application/pdf", "application/json"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StagedAttachment {
    pub id: String,
    pub name: String,
    pub mime_type: String,
    pub byte_size: i64,
    pub local_filename: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageAttachmentInput {
    pub name: String,
    pub mime_type: String,
    pub data_base64: String,
}

fn attachments_root() -> Result<PathBuf, CommandError> {
    let base =
        data_dir().ok_or_else(|| CommandError::new("storage", "App data directory unavailable"))?;
    let root = product_data_dir(&base).join("chat-attachments");
    std::fs::create_dir_all(&root)
        .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;
    Ok(root)
}

fn sanitize_filename(name: &str) -> String {
    let base = Path::new(name)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.');
    if trimmed.is_empty() {
        "file".into()
    } else {
        trimmed.chars().take(120).collect()
    }
}

fn mime_allowed(mime: &str) -> bool {
    let m = mime.trim().to_ascii_lowercase();
    ALLOWED_PREFIXES.iter().any(|p| {
        if p.ends_with('/') {
            m.starts_with(p)
        } else {
            m == *p
        }
    }) || m == "application/json"
        || m.starts_with("text/")
}

#[tauri::command]
pub fn stage_chat_attachment(
    input: StageAttachmentInput,
) -> Result<StagedAttachment, CommandError> {
    let mime = input.mime_type.trim().to_ascii_lowercase();
    if !mime_allowed(&mime) {
        return Err(CommandError::new(
            "invalid",
            format!("Unsupported attachment type: {mime}"),
        ));
    }

    let bytes = B64
        .decode(input.data_base64.trim())
        .map_err(|_| CommandError::new("invalid", "Attachment data is not valid base64"))?;
    if bytes.is_empty() {
        return Err(CommandError::new("invalid", "Attachment is empty"));
    }
    if bytes.len() > MAX_ATTACHMENT_BYTES {
        return Err(CommandError::new(
            "invalid",
            format!(
                "Attachment exceeds {} MB limit",
                MAX_ATTACHMENT_BYTES / (1024 * 1024)
            ),
        ));
    }

    // Reject obvious executables / HTML payloads by magic bytes.
    if bytes.starts_with(b"MZ")
        || bytes.starts_with(b"<!DOCTYPE")
        || bytes.starts_with(b"<html")
        || bytes.starts_with(b"#!/")
    {
        return Err(CommandError::new(
            "invalid",
            "Executable or HTML attachments are not allowed",
        ));
    }

    let id = Uuid::new_v4().to_string();
    let safe_name = sanitize_filename(&input.name);
    let local_filename = format!("{id}_{safe_name}");
    let path = attachments_root()?.join(&local_filename);
    std::fs::write(&path, &bytes)
        .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;

    Ok(StagedAttachment {
        id,
        name: safe_name,
        mime_type: mime,
        byte_size: bytes.len() as i64,
        local_filename,
    })
}

#[tauri::command]
pub fn get_chat_attachment_src(local_filename: String) -> Result<String, CommandError> {
    let name = Path::new(local_filename.trim())
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| CommandError::new("invalid", "Invalid attachment filename"))?;
    if name.contains("..") || name.contains('/') || name.contains('\\') {
        return Err(CommandError::new("invalid", "Path traversal rejected"));
    }
    let path = attachments_root()?.join(name);
    if !path.exists() {
        return Err(CommandError::new("not_found", "Attachment not found"));
    }
    let canonical = path
        .canonicalize()
        .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;
    let root = attachments_root()?
        .canonicalize()
        .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;
    if !canonical.starts_with(&root) {
        return Err(CommandError::new(
            "invalid",
            "Path outside attachments directory",
        ));
    }
    Ok(canonical.to_string_lossy().into_owned())
}
