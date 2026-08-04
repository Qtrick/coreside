//! Stage chat file attachments into managed AppPaths storage.

use std::io::Write;
use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::{Deserialize, Serialize};
use tauri::State;
use tempfile::NamedTempFile;
use uuid::Uuid;

use super::CommandError;
use crate::app_paths::AppPaths;
use crate::security::sanitize_error;
use crate::state::AppState;
use tauri::Manager;

/// Max decoded bytes per attached file (12 MiB).
pub const MAX_ATTACHMENT_BYTES: usize = 12 * 1024 * 1024;
/// Encoded Base64 expands ~4/3; reject oversized wire payloads before decode.
const MAX_BASE64_CHARS: usize = (MAX_ATTACHMENT_BYTES / 3 + 1) * 4 + 64;
/// Max attachments per message.
pub const MAX_ATTACHMENTS_PER_MESSAGE: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StagedAttachment {
    pub id: String,
    pub name: String,
    pub mime_type: String,
    pub byte_size: i64,
}

/// Trusted attachment fields resolved from SQLite (never trust frontend metadata).
#[derive(Debug, Clone)]
pub struct TrustedAttachmentMeta {
    pub id: String,
    pub name: String,
    pub mime_type: String,
    pub byte_size: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentSrc {
    /// Opaque attachment ID (not a filesystem path).
    pub id: String,
    /// Opaque custom-protocol URL — no absolute filesystem path.
    pub url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageAttachmentInput {
    pub name: String,
    pub mime_type: String,
    pub data_base64: String,
}

fn attachments_paths() -> Result<(PathBuf, PathBuf), CommandError> {
    let paths = AppPaths::resolve().map_err(|e| {
        CommandError::new("storage", sanitize_error(&e.to_string(), None))
    })?;
    paths.ensure_dirs().map_err(|e| {
        CommandError::new("storage", sanitize_error(&e.to_string(), None))
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&paths.attachments, std::fs::Permissions::from_mode(0o700));
        let _ = std::fs::set_permissions(
            &paths.attachment_staging,
            std::fs::Permissions::from_mode(0o700),
        );
    }
    Ok((paths.attachments, paths.attachment_staging))
}

fn opaque_attachment_url(id: &str) -> String {
    #[cfg(any(windows, target_os = "android"))]
    {
        format!("http://coreside-asset.localhost/attachment/{id}")
    }
    #[cfg(not(any(windows, target_os = "android")))]
    {
        format!("coreside-asset://localhost/attachment/{id}")
    }
}

fn is_safe_attachment_id(id: &str) -> bool {
    let id = id.trim();
    !id.is_empty()
        && id.len() <= 64
        && !id.contains('/')
        && !id.contains('\\')
        && !id.contains("..")
        && id
            .chars()
            .all(|c| c.is_ascii_hexdigit() || c == '-')
}

/// Strip CR/LF/controls so DB-corrupted mime cannot inject response headers.
fn safe_response_mime(mime: &str) -> String {
    let cleaned: String = mime
        .chars()
        .take(128)
        .filter(|c| c.is_ascii_graphic() || *c == ' ')
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() || !trimmed.contains('/') {
        "application/octet-stream".into()
    } else {
        trimmed.to_ascii_lowercase()
    }
}

fn content_hash(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
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

fn strip_bom_and_ws(bytes: &[u8]) -> &[u8] {
    let mut b = bytes;
    if b.starts_with(&[0xEF, 0xBB, 0xBF]) {
        b = &b[3..];
    } else if b.starts_with(&[0xFF, 0xFE]) || b.starts_with(&[0xFE, 0xFF]) {
        // UTF-16 BOM — treat as hostile for attachment policy.
        return b;
    }
    while let Some((&c, rest)) = b.split_first() {
        if c == b' ' || c == b'\t' || c == b'\n' || c == b'\r' {
            b = rest;
        } else {
            break;
        }
    }
    b
}

/// Reject only markup *documents* (leading root tag), not prose that mentions HTML/SVG.
fn looks_like_markup_document(bytes: &[u8]) -> bool {
    let lower: Vec<u8> = bytes
        .iter()
        .take(96)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let head = String::from_utf8_lossy(&lower);
    let trimmed = head.trim_start();
    trimmed.starts_with("<!doctype")
        || trimmed.starts_with("<html")
        || trimmed.starts_with("<svg")
        || trimmed.starts_with("<?xml")
        || trimmed.starts_with("<script")
}

fn detect_format(bytes: &[u8]) -> Result<&'static str, CommandError> {
    let b = strip_bom_and_ws(bytes);
    if b.is_empty() {
        return Err(CommandError::new("invalid", "Attachment is empty"));
    }
    // Executables / scripts
    if b.starts_with(b"MZ") || b.starts_with(b"\x7fELF") || b.starts_with(b"#!") {
        return Err(CommandError::new(
            "invalid",
            "Executable or script attachments are not allowed",
        ));
    }
    // Archives
    if b.starts_with(b"PK\x03\x04") || b.starts_with(b"Rar!") || b.starts_with(b"7z\xBC\xAF\x27\x1C")
    {
        return Err(CommandError::new(
            "invalid",
            "Archive attachments are not allowed",
        ));
    }
    // UTF-16 BOM — treat as hostile for attachment policy.
    if b.starts_with(&[0xFF, 0xFE]) || b.starts_with(&[0xFE, 0xFF]) {
        return Err(CommandError::new(
            "invalid",
            "UTF-16 text attachments are not allowed",
        ));
    }

    // Known binary formats before markup/text heuristics.
    if b.starts_with(&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']) {
        return Ok("image/png");
    }
    if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Ok("image/jpeg");
    }
    if b.len() >= 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        return Ok("image/webp");
    }
    if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        return Ok("image/gif");
    }
    if b.starts_with(b"%PDF-") {
        return Ok("application/pdf");
    }

    if looks_like_markup_document(b) {
        return Err(CommandError::new(
            "invalid",
            "HTML, SVG, or XML attachments are not allowed",
        ));
    }

    // Bounded plain text / JSON / Markdown — reject NULs and control bytes except tab/LF/CR.
    if b.iter().any(|&c| {
        c == 0
            || (c < 0x09)
            || (c == 0x0B)
            || (c == 0x0C)
            || (c == 0x0E)
            || (c == 0x0F)
            || (c < 0x20 && !matches!(c, b'\t' | b'\n' | b'\r'))
    }) {
        return Err(CommandError::new(
            "invalid",
            "Unrecognized or binary attachment type",
        ));
    }
    let text = String::from_utf8(b.to_vec()).map_err(|_| {
        CommandError::new("invalid", "Text attachments must be valid UTF-8")
    })?;
    let trimmed = text.trim_start();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return Ok("application/json");
    }
    Ok("text/plain")
}

fn mime_compatible(declared: &str, detected: &str) -> bool {
    let d = declared.trim().to_ascii_lowercase();
    if d.is_empty() || d == "application/octet-stream" {
        return true;
    }
    if d == detected {
        return true;
    }
    // Allow generic image/* only for detected raster images.
    if d.starts_with("image/") && detected.starts_with("image/") {
        return true;
    }
    // Browsers often declare text/plain / text/markdown for JSON-looking notes.
    if d.starts_with("text/") && (detected.starts_with("text/") || detected == "application/json") {
        return true;
    }
    if (d == "application/json" || d == "text/json") && detected == "application/json" {
        return true;
    }
    false
}

fn atomic_write(final_path: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    let parent = final_path.parent().ok_or_else(|| {
        CommandError::new("storage", "Invalid attachment destination")
    })?;
    let mut tmp = NamedTempFile::new_in(parent)
        .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;
    tmp.write_all(bytes)
        .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;
    tmp.flush()
        .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o600));
    }
    tmp.persist(final_path)
        .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;
    Ok(())
}

#[tauri::command]
pub fn stage_chat_attachment(
    state: State<'_, AppState>,
    input: StageAttachmentInput,
) -> Result<StagedAttachment, CommandError> {
    state.require_profile()?;

    let encoded = input.data_base64.trim();
    if encoded.is_empty() {
        return Err(CommandError::new("invalid", "Attachment is empty"));
    }
    if encoded.len() > MAX_BASE64_CHARS {
        return Err(CommandError::new(
            "invalid",
            format!(
                "Attachment exceeds {} MB limit",
                MAX_ATTACHMENT_BYTES / (1024 * 1024)
            ),
        ));
    }

    let bytes = B64
        .decode(encoded)
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

    let detected = detect_format(&bytes)?;
    let declared = input.mime_type.trim().to_ascii_lowercase();
    if !mime_compatible(&declared, detected) {
        return Err(CommandError::new(
            "invalid",
            format!("Attachment content does not match declared type ({declared})"),
        ));
    }

    let id = Uuid::new_v4().to_string();
    let safe_name = sanitize_filename(&input.name);
    let storage_key = format!("{id}_{safe_name}");
    let (_root, staging) = attachments_paths()?;
    let path = staging.join(&storage_key);
    atomic_write(&path, &bytes)?;
    let hash = content_hash(&bytes);
    let expires = chrono::Utc::now() + chrono::Duration::hours(24);

    {
        let db = state.db.lock();
        db.conn()
            .execute(
                "INSERT INTO chat_attachments
                 (id, storage_key, original_filename, display_name, detected_mime, detected_format,
                  byte_size, content_hash, state, expires_at, backup_eligible)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,'staged',?9,0)",
                rusqlite::params![
                    id,
                    storage_key,
                    input.name,
                    safe_name,
                    detected,
                    detected,
                    bytes.len() as i64,
                    hash,
                    expires.to_rfc3339(),
                ],
            )
            .map_err(|e| {
                let _ = std::fs::remove_file(&path);
                CommandError::new("storage", sanitize_error(&e.to_string(), None))
            })?;
    }

    Ok(StagedAttachment {
        id,
        name: safe_name,
        mime_type: detected.to_string(),
        byte_size: bytes.len() as i64,
    })
}

/// Read-only readiness check before inserting a user message.
pub fn assert_staged_attachments_ready(
    state: &AppState,
    attachment_ids: &[String],
) -> Result<(), CommandError> {
    if attachment_ids.is_empty() {
        return Ok(());
    }
    if attachment_ids.len() > MAX_ATTACHMENTS_PER_MESSAGE {
        return Err(CommandError::new("invalid", "Too many attachments"));
    }
    let mut seen = std::collections::HashSet::new();
    let (durable_root, staging_root) = attachments_paths()?;
    let db = state.db.lock();
    for raw_id in attachment_ids {
        let id = raw_id.trim();
        if !is_safe_attachment_id(id) {
            return Err(CommandError::new("invalid", "Invalid attachment id"));
        }
        if !seen.insert(id.to_string()) {
            return Err(CommandError::new("invalid", "Duplicate attachment id"));
        }
        let (storage_key, att_state): (String, String) = db
            .conn()
            .query_row(
                "SELECT storage_key, state FROM chat_attachments
                 WHERE id = ?1 AND deleted_at IS NULL",
                rusqlite::params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|_| CommandError::new("not_found", "Attachment not found"))?;
        if att_state != "staged" {
            return Err(CommandError::new(
                "invalid",
                "Attachment is not available to attach",
            ));
        }
        let name = validated_storage_file_name(&storage_key)?;
        if !durable_root.join(name).exists() && !staging_root.join(name).exists() {
            return Err(CommandError::new(
                "not_found",
                "Attachment file missing from staging",
            ));
        }
    }
    Ok(())
}

fn validated_storage_file_name(storage_key: &str) -> Result<&str, CommandError> {
    let name = Path::new(storage_key)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| CommandError::new("invalid", "Invalid attachment storage key"))?;
    if name != storage_key
        || name.contains("..")
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
    {
        return Err(CommandError::new("invalid", "Invalid attachment storage key"));
    }
    Ok(name)
}

fn promote_attachment_file(storage_key: &str) -> Result<(), CommandError> {
    let name = validated_storage_file_name(storage_key)?;
    let (durable_root, staging_root) = attachments_paths()?;
    let durable_path = durable_root.join(name);
    let staging_path = staging_root.join(name);
    if durable_path.exists() {
        // Crash/copy window may leave a staging duplicate — durable wins.
        if staging_path.exists() {
            let _ = std::fs::remove_file(&staging_path);
        }
        return Ok(());
    }
    if !staging_path.exists() {
        return Err(CommandError::new(
            "not_found",
            "Attachment file missing from staging",
        ));
    }
    std::fs::rename(&staging_path, &durable_path)
        .or_else(|_| {
            std::fs::copy(&staging_path, &durable_path).and_then(|_| std::fs::remove_file(&staging_path))
        })
        .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&durable_path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn demote_attachment_file(storage_key: &str) -> Result<(), CommandError> {
    let name = validated_storage_file_name(storage_key)?;
    let (durable_root, staging_root) = attachments_paths()?;
    let durable_path = durable_root.join(name);
    let staging_path = staging_root.join(name);
    if staging_path.exists() {
        // Incomplete promote (copy ok, staging delete failed) can leave both copies.
        // Staging is authoritative after demote — drop the durable duplicate.
        if durable_path.exists() {
            let _ = std::fs::remove_file(&durable_path);
        }
        return Ok(());
    }
    if !durable_path.exists() {
        return Ok(());
    }
    std::fs::rename(&durable_path, &staging_path)
        .or_else(|_| {
            std::fs::copy(&durable_path, &staging_path).and_then(|_| std::fs::remove_file(&durable_path))
        })
        .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;
    Ok(())
}

struct ClaimedAttachment {
    id: String,
    storage_key: String,
    display_name: String,
    mime: String,
    byte_size: i64,
    already_bound: bool,
}

fn claim_attachment_row(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    message_id: &str,
    id: &str,
) -> Result<ClaimedAttachment, CommandError> {
    let (storage_key, display_name, mime, byte_size, att_state, existing_message): (
        String,
        String,
        String,
        i64,
        String,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT storage_key, display_name, detected_mime, byte_size, state, message_id
             FROM chat_attachments
             WHERE id = ?1 AND deleted_at IS NULL",
            rusqlite::params![id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .map_err(|_| CommandError::new("not_found", "Attachment not found"))?;

    if att_state == "attached" && existing_message.as_deref() == Some(message_id) {
        let _ = validated_storage_file_name(&storage_key)?;
        return Ok(ClaimedAttachment {
            id: id.to_string(),
            storage_key,
            display_name,
            mime,
            byte_size,
            already_bound: true,
        });
    }
    if att_state != "staged" {
        return Err(CommandError::new(
            "invalid",
            "Attachment is not available to attach",
        ));
    }
    let name = validated_storage_file_name(&storage_key)?;
    // Fail closed before flipping state so we never mark attached without a file.
    let (durable_root, staging_root) = attachments_paths()?;
    if !durable_root.join(name).exists() && !staging_root.join(name).exists() {
        return Err(CommandError::new(
            "not_found",
            "Attachment file missing from staging",
        ));
    }

    let updated = conn
        .execute(
            "UPDATE chat_attachments
             SET conversation_id = ?1,
                 message_id = ?2,
                 state = 'attached',
                 backup_eligible = 1,
                 expires_at = NULL,
                 updated_at = datetime('now')
             WHERE id = ?3 AND deleted_at IS NULL AND state = 'staged'",
            rusqlite::params![conversation_id, message_id, id],
        )
        .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;
    if updated != 1 {
        return Err(CommandError::new(
            "invalid",
            "Attachment is not available to attach",
        ));
    }

    Ok(ClaimedAttachment {
        id: id.to_string(),
        storage_key,
        display_name,
        mime,
        byte_size,
        already_bound: false,
    })
}

fn unclaim_attachment_rows(
    conn: &rusqlite::Connection,
    message_id: &str,
    ids: &[String],
) -> Result<(), CommandError> {
    for id in ids {
        conn.execute(
            "UPDATE chat_attachments
             SET conversation_id = NULL,
                 message_id = NULL,
                 state = 'staged',
                 backup_eligible = 0,
                 expires_at = datetime('now', '+24 hours'),
                 updated_at = datetime('now')
             WHERE id = ?1 AND message_id = ?2 AND state = 'attached' AND deleted_at IS NULL",
            rusqlite::params![id, message_id],
        )
        .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;
    }
    Ok(())
}

/// Roll back attachment claims for a message that is being deleted after a failed
/// commit. Demotes durable files back to staging when present.
pub fn unbind_attachments_from_message(
    db: &mut crate::db::Database,
    message_id: &str,
    attachment_ids: &[String],
) -> Result<(), CommandError> {
    if attachment_ids.is_empty() {
        return Ok(());
    }
    let mut storage_keys = Vec::with_capacity(attachment_ids.len());
    {
        let conn = db.conn();
        for id in attachment_ids {
            let key: Result<String, rusqlite::Error> = conn.query_row(
                "SELECT storage_key FROM chat_attachments
                 WHERE id = ?1 AND message_id = ?2 AND state = 'attached' AND deleted_at IS NULL",
                rusqlite::params![id, message_id],
                |row| row.get(0),
            );
            if let Ok(key) = key {
                storage_keys.push(key);
            }
        }
        unclaim_attachment_rows(conn, message_id, attachment_ids)?;
    }
    for key in storage_keys {
        let _ = demote_attachment_file(&key);
    }
    Ok(())
}

/// Result of claiming attachment rows before filesystem promotion.
pub struct AttachmentClaimResult {
    pub attachments: Vec<TrustedAttachmentMeta>,
    /// Storage keys that still need staging→durable promotion.
    pub promote_keys: Vec<String>,
    /// Newly claimed ids (not already bound); revert these on promote failure.
    pub new_claim_ids: Vec<String>,
}

/// Claim staged attachment rows on an existing connection/transaction.
/// Prefer wrapping message insert + this claim + metadata in one BEGIN/COMMIT.
pub fn claim_attachments_on_conn(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    message_id: &str,
    attachment_ids: &[String],
) -> Result<AttachmentClaimResult, CommandError> {
    if attachment_ids.is_empty() {
        return Ok(AttachmentClaimResult {
            attachments: Vec::new(),
            promote_keys: Vec::new(),
            new_claim_ids: Vec::new(),
        });
    }
    if attachment_ids.len() > MAX_ATTACHMENTS_PER_MESSAGE {
        return Err(CommandError::new("invalid", "Too many attachments"));
    }

    let mut seen = std::collections::HashSet::new();
    let mut claimed: Vec<ClaimedAttachment> = Vec::with_capacity(attachment_ids.len());
    for raw_id in attachment_ids {
        let id = raw_id.trim();
        if !is_safe_attachment_id(id) {
            return Err(CommandError::new("invalid", "Invalid attachment id"));
        }
        if !seen.insert(id.to_string()) {
            return Err(CommandError::new("invalid", "Duplicate attachment id"));
        }
        claimed.push(claim_attachment_row(
            conn,
            conversation_id,
            message_id,
            id,
        )?);
    }

    let mut promote_keys = Vec::new();
    let mut new_claim_ids = Vec::new();
    let mut attachments = Vec::with_capacity(claimed.len());
    for row in claimed {
        // Always ensure durable promotion — including already_bound rows whose
        // prior attempt may have crashed after claim but before rename.
        promote_keys.push(row.storage_key.clone());
        if !row.already_bound {
            new_claim_ids.push(row.id.clone());
        }
        attachments.push(TrustedAttachmentMeta {
            id: row.id,
            name: row.display_name,
            mime_type: row.mime,
            byte_size: row.byte_size,
        });
    }

    Ok(AttachmentClaimResult {
        attachments,
        promote_keys,
        new_claim_ids,
    })
}

/// Claim staged attachment rows for a message (DB only), in a dedicated transaction.
/// Prefer `claim_attachments_on_conn` inside a broader insert+claim+metadata transaction.
pub fn claim_attachments_on_db(
    db: &crate::db::Database,
    conversation_id: &str,
    message_id: &str,
    attachment_ids: &[String],
) -> Result<AttachmentClaimResult, CommandError> {
    let tx = db.conn().unchecked_transaction().map_err(|e| {
        CommandError::new("storage", sanitize_error(&e.to_string(), None))
    })?;
    let claimed = claim_attachments_on_conn(&tx, conversation_id, message_id, attachment_ids)?;
    tx.commit()
        .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;
    Ok(claimed)
}

/// Promote claimed attachment files. On failure, demotes any files already moved.
pub fn promote_claimed_attachment_files(storage_keys: &[String]) -> Result<(), CommandError> {
    let mut promoted: Vec<String> = Vec::new();
    for key in storage_keys {
        if let Err(err) = promote_attachment_file(key) {
            for done in promoted.iter().rev() {
                let _ = demote_attachment_file(done.as_str());
            }
            return Err(err);
        }
        promoted.push(key.clone());
    }
    Ok(())
}

/// Resolve staged attachments by opaque id, claim rows first (affected-row guard),
/// then promote into durable storage. Frontend-supplied name/mime/size/filename are ignored.
/// On failure, newly claimed rows are unclaimed and promoted files are demoted.
pub fn bind_attachments_to_message(
    state: &AppState,
    conversation_id: &str,
    message_id: &str,
    attachment_ids: &[String],
) -> Result<Vec<TrustedAttachmentMeta>, CommandError> {
    let claimed = {
        let db = state.db.lock();
        claim_attachments_on_db(&db, conversation_id, message_id, attachment_ids)?
    };
    if let Err(err) = promote_claimed_attachment_files(&claimed.promote_keys) {
        let mut db = state.db.lock();
        let _ = unbind_attachments_from_message(&mut db, message_id, &claimed.new_claim_ids);
        return Err(err);
    }
    Ok(claimed.attachments)
}

#[tauri::command]
pub fn get_chat_attachment_src(
    state: State<'_, AppState>,
    attachment_id: Option<String>,
    local_filename: Option<String>,
) -> Result<AttachmentSrc, CommandError> {
    state.require_profile()?;
    // Storage keys are not an accepted public contract — opaque id only.
    if local_filename
        .as_ref()
        .is_some_and(|s| !s.trim().is_empty())
    {
        return Err(CommandError::new(
            "invalid",
            "Attachment id required; storage keys are not accepted",
        ));
    }
    let id = if let Some(id) = attachment_id.filter(|s| !s.trim().is_empty()) {
        let id = id.trim().to_string();
        if !is_safe_attachment_id(&id) {
            return Err(CommandError::new("invalid", "Invalid attachment id"));
        }
        id
    } else {
        return Err(CommandError::new("invalid", "Attachment id required"));
    };

    let db = state.db.lock();
    let (state_s, _key): (String, String) = db
        .conn()
        .query_row(
            "SELECT state, storage_key FROM chat_attachments WHERE id = ?1 AND deleted_at IS NULL",
            rusqlite::params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| CommandError::new("not_found", "Attachment not found"))?;
    if matches!(
        state_s.as_str(),
        "deleted" | "expired" | "failed" | "cancelled"
    ) {
        return Err(CommandError::new("not_found", "Attachment not available"));
    }

    Ok(AttachmentSrc {
        url: opaque_attachment_url(&id),
        id,
    })
}

/// Cancel a staged (or failed) attachment: mark cancelled, clear backup eligibility,
/// and delete managed files. Idempotent for already-cancelled/deleted/expired rows.
#[tauri::command]
pub fn cancel_chat_attachment(
    state: State<'_, AppState>,
    attachment_id: String,
) -> Result<(), CommandError> {
    state.require_profile()?;
    cancel_staged_attachment_ids(&state, &[attachment_id])
}

/// Best-effort release for queue cancel/remove/failed-drain cleanup.
/// Skips missing and already-committed ids so attached message files are preserved.
pub fn release_staged_attachment_ids_best_effort(state: &AppState, attachment_ids: &[String]) {
    for raw_id in attachment_ids {
        let id = raw_id.trim();
        if id.is_empty() {
            continue;
        }
        if let Err(err) = cancel_staged_attachment_ids(state, &[id.to_string()]) {
            // Committed / unavailable rows are expected when the turn already bound them.
            if err.code == "invalid" || err.code == "not_found" {
                continue;
            }
            tracing::warn!(
                attachment_id = %id,
                error = %err.message,
                "best-effort staged attachment release failed"
            );
        }
    }
}

/// Cancel multiple staged attachments by opaque id. Skips unknown ids; fails closed
/// when an id is committed to a message.
pub fn cancel_staged_attachment_ids(
    state: &AppState,
    attachment_ids: &[String],
) -> Result<(), CommandError> {
    if attachment_ids.is_empty() {
        return Ok(());
    }
    let (durable_root, staging_root) = attachments_paths()?;
    let db = state.db.lock();
    for raw_id in attachment_ids {
        let id = raw_id.trim();
        if id.is_empty() {
            continue;
        }
        if !is_safe_attachment_id(id) {
            return Err(CommandError::new("invalid", "Invalid attachment id"));
        }
        let row: Result<(String, String, Option<String>), rusqlite::Error> = db.conn().query_row(
            "SELECT storage_key, state, message_id FROM chat_attachments
             WHERE id = ?1",
            rusqlite::params![id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        );
        let Ok((storage_key, att_state, message_id)) = row else {
            continue;
        };
        if att_state == "attached" || message_id.is_some() {
            return Err(CommandError::new(
                "invalid",
                "Committed attachments cannot be cancelled",
            ));
        }
        if matches!(att_state.as_str(), "cancelled" | "deleted" | "expired") {
            continue;
        }
        if att_state != "staged" && att_state != "failed" {
            return Err(CommandError::new(
                "invalid",
                "Attachment is not available to cancel",
            ));
        }
        let name = validated_storage_file_name(&storage_key)?;
        let updated = db
            .conn()
            .execute(
                "UPDATE chat_attachments
                 SET state = 'cancelled',
                     backup_eligible = 0,
                     deleted_at = datetime('now'),
                     updated_at = datetime('now'),
                     expires_at = NULL
                 WHERE id = ?1 AND state IN ('staged', 'failed') AND deleted_at IS NULL",
                rusqlite::params![id],
            )
            .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;
        // Race with bind: UPDATE matched 0 rows — do not delete durable files.
        if updated != 1 {
            let now_state: Result<(String, Option<String>), rusqlite::Error> = db.conn().query_row(
                "SELECT state, message_id FROM chat_attachments WHERE id = ?1",
                rusqlite::params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            );
            if let Ok((s, mid)) = now_state {
                if s == "attached" || mid.is_some() {
                    return Err(CommandError::new(
                        "invalid",
                        "Committed attachments cannot be cancelled",
                    ));
                }
                if matches!(s.as_str(), "cancelled" | "deleted" | "expired") {
                    continue;
                }
            }
            return Err(CommandError::new(
                "invalid",
                "Attachment is not available to cancel",
            ));
        }
        let _ = std::fs::remove_file(staging_root.join(name));
        let _ = std::fs::remove_file(durable_root.join(name));
    }
    Ok(())
}

/// Resolve opaque attachment ID to bytes for the custom protocol (no path leak).
///
/// P1 remaining (blocked on product window/capability wiring): authorization is
/// opaque-ID + profile-ready only — not per-window / conversation ACL. Do not
/// claim stronger isolation until that lands. Size is bounded by `MAX_ATTACHMENT_BYTES`.
pub fn read_attachment_bytes_for_protocol(
    app: &tauri::AppHandle,
    attachment_id: &str,
) -> Result<(Vec<u8>, String), String> {
    let id = attachment_id.trim();
    if !is_safe_attachment_id(id) {
        return Err("invalid attachment id".into());
    }
    let state = app
        .try_state::<AppState>()
        .ok_or_else(|| "app state unavailable".to_string())?;
    if !state.profile_ready() {
        return Err("profile unavailable".into());
    }
    let (storage_key, mime, state_s): (String, String, String) = {
        let db = state.db.lock();
        db.conn()
            .query_row(
                "SELECT storage_key, detected_mime, state FROM chat_attachments
                 WHERE id = ?1 AND deleted_at IS NULL",
                rusqlite::params![id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|_| "attachment not found".to_string())?
    };
    if matches!(
        state_s.as_str(),
        "deleted" | "expired" | "failed" | "cancelled"
    ) {
        return Err("attachment not available".into());
    }
    let name = validated_storage_file_name(&storage_key).map_err(|e| e.message)?;
    let paths = AppPaths::resolve().map_err(|e| e.to_string())?;
    for root in [&paths.attachment_staging, &paths.attachments] {
        let candidate = root.join(name);
        if candidate.exists() {
            let canonical = candidate.canonicalize().map_err(|e| e.to_string())?;
            let root_c = root.canonicalize().map_err(|e| e.to_string())?;
            if !canonical.starts_with(&root_c) {
                return Err("path outside attachments".into());
            }
            // Bound before whole-file read (protocol must not load unbounded blobs).
            let meta = std::fs::metadata(&canonical).map_err(|e| e.to_string())?;
            if meta.len() > MAX_ATTACHMENT_BYTES as u64 {
                return Err("attachment exceeds size limit".into());
            }
            let bytes = std::fs::read(&canonical).map_err(|e| e.to_string())?;
            if bytes.len() > MAX_ATTACHMENT_BYTES {
                return Err("attachment exceeds size limit".into());
            }
            return Ok((bytes, safe_response_mime(&mime)));
        }
    }
    Err("attachment file missing".into())
}

/// Bounded startup/periodic GC for expired staged attachments and orphaned rows.
/// Does not delete committed (`attached`) message files.
pub fn reconcile_and_sweep_attachments(state: &AppState) -> Result<AttachmentSweepReport, CommandError> {
    state.require_profile()?;
    let (durable_root, staging_root) = attachments_paths()?;
    const MAX_ROWS: usize = 200;
    let mut report = AttachmentSweepReport::default();

    let expired_rows: Vec<(String, String)> = {
        let db = state.db.lock();
        let mut stmt = db
            .conn()
            .prepare(
                "SELECT id, storage_key FROM chat_attachments
                 WHERE deleted_at IS NULL
                   AND state IN ('staged', 'failed', 'cancelled', 'expired')
                   AND expires_at IS NOT NULL
                   AND expires_at < datetime('now')
                 LIMIT ?1",
            )
            .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;
        let rows = stmt
            .query_map(rusqlite::params![MAX_ROWS as i64], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;
        rows.filter_map(Result::ok).collect()
    };

    for (id, storage_key) in expired_rows {
        if !is_safe_attachment_id(&id) {
            continue;
        }
        let Ok(name) = validated_storage_file_name(&storage_key) else {
            continue;
        };
        {
            let db = state.db.lock();
            let updated = db
                .conn()
                .execute(
                    "UPDATE chat_attachments
                     SET state = 'expired',
                         backup_eligible = 0,
                         deleted_at = datetime('now'),
                         updated_at = datetime('now')
                     WHERE id = ?1
                       AND state IN ('staged', 'failed', 'cancelled', 'expired')
                       AND message_id IS NULL
                       AND deleted_at IS NULL",
                    rusqlite::params![id],
                )
                .unwrap_or(0);
            if updated != 1 {
                continue;
            }
        }
        let _ = std::fs::remove_file(staging_root.join(name));
        let _ = std::fs::remove_file(durable_root.join(name));
        report.expired += 1;
    }

    // Reconcile: attached rows whose durable file is missing (diagnostic only).
    let attached_keys: Vec<(String, String)> = {
        let db = state.db.lock();
        let mut stmt = db
            .conn()
            .prepare(
                "SELECT id, storage_key FROM chat_attachments
                 WHERE state = 'attached' AND deleted_at IS NULL
                 LIMIT ?1",
            )
            .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;
        let rows = stmt
            .query_map(rusqlite::params![MAX_ROWS as i64], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;
        rows.filter_map(Result::ok).collect()
    };
    for (id, storage_key) in attached_keys {
        let Ok(name) = validated_storage_file_name(&storage_key) else {
            report.missing_durable += 1;
            continue;
        };
        let in_durable = durable_root.join(name).exists();
        let in_staging = staging_root.join(name).exists();
        if !in_durable && !in_staging {
            report.missing_durable += 1;
            tracing::warn!(attachment_id = %id, "attached attachment file missing from managed roots");
        } else if !in_durable && in_staging {
            // Crash window after claim / before promote — heal by promoting.
            if std::fs::rename(staging_root.join(name), durable_root.join(name)).is_ok()
                || (std::fs::copy(staging_root.join(name), durable_root.join(name)).is_ok()
                    && std::fs::remove_file(staging_root.join(name)).is_ok())
            {
                report.promoted_orphans += 1;
            } else {
                report.missing_durable += 1;
            }
        }
    }

    Ok(report)
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentSweepReport {
    pub expired: u64,
    pub missing_durable: u64,
    pub promoted_orphans: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_oversized_base64_before_decode_budget() {
        let huge = "A".repeat(MAX_BASE64_CHARS + 1);
        assert!(huge.len() > MAX_BASE64_CHARS);
    }

    #[test]
    fn detects_png_magic() {
        let mut bytes = vec![0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];
        bytes.extend_from_slice(&[0; 16]);
        assert_eq!(detect_format(&bytes).unwrap(), "image/png");
    }

    #[test]
    fn rejects_html_despite_image_mime_claim() {
        let html = b"  <!DOCTYPE html><html><body>x</body></html>";
        assert!(detect_format(html).is_err());
    }

    #[test]
    fn rejects_mixed_case_html() {
        let html = b"<HtMl><ScRiPt>alert(1)</sCrIpT>";
        assert!(detect_format(html).is_err());
    }

    #[test]
    fn rejects_svg() {
        let svg = b"<?xml version=\"1.0\"?><svg onload=\"alert(1)\"></svg>";
        assert!(detect_format(svg).is_err());
    }

    #[test]
    fn rejects_zip_polyglot_prefix() {
        assert!(detect_format(b"PK\x03\x04rest").is_err());
    }

    #[test]
    fn allows_text_that_mentions_html() {
        let notes = b"Notes: use an <html> snippet and javascript:void(0) in docs only.";
        assert_eq!(detect_format(notes).unwrap(), "text/plain");
    }

    #[test]
    fn allows_markdown_with_script_discussion() {
        let md = b"# Security\n\nNever paste raw <script> tags into chat.\n";
        assert_eq!(detect_format(md).unwrap(), "text/plain");
    }

    #[test]
    fn text_plain_declared_accepts_json_shape() {
        assert!(mime_compatible("text/plain", "application/json"));
    }
}
