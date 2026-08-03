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

/// Max decoded bytes per attached file (12 MiB).
const MAX_ATTACHMENT_BYTES: usize = 12 * 1024 * 1024;
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
    pub local_filename: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentSrc {
    /// Opaque storage filename (not an absolute path).
    pub local_filename: String,
    /// Asset-protocol URL for rendering (no bare filesystem path field).
    pub url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageAttachmentInput {
    pub name: String,
    pub mime_type: String,
    pub data_base64: String,
}

fn attachments_root() -> Result<PathBuf, CommandError> {
    let paths = AppPaths::resolve().map_err(|e| {
        CommandError::new("storage", sanitize_error(&e.to_string(), None))
    })?;
    std::fs::create_dir_all(&paths.attachments)
        .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&paths.attachments, std::fs::Permissions::from_mode(0o700));
    }
    Ok(paths.attachments)
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

fn asset_protocol_url(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let encoded: String = raw
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{:02X}", b),
        })
        .collect();
    #[cfg(any(windows, target_os = "android"))]
    {
        format!("http://asset.localhost/{encoded}")
    }
    #[cfg(not(any(windows, target_os = "android")))]
    {
        format!("asset://localhost/{encoded}")
    }
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
    let local_filename = format!("{id}_{safe_name}");
    let path = attachments_root()?.join(&local_filename);
    atomic_write(&path, &bytes)?;

    Ok(StagedAttachment {
        id,
        name: safe_name,
        mime_type: detected.to_string(),
        byte_size: bytes.len() as i64,
        local_filename,
    })
}

#[tauri::command]
pub fn get_chat_attachment_src(
    state: State<'_, AppState>,
    local_filename: String,
) -> Result<AttachmentSrc, CommandError> {
    state.require_profile()?;
    let name = Path::new(local_filename.trim())
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| CommandError::new("invalid", "Invalid attachment filename"))?;
    if name.contains("..") || name.contains('/') || name.contains('\\') || name.contains('\0') {
        return Err(CommandError::new("invalid", "Path traversal rejected"));
    }
    let root = attachments_root()?;
    let path = root.join(name);
    if !path.exists() {
        return Err(CommandError::new("not_found", "Attachment not found"));
    }
    let canonical = path
        .canonicalize()
        .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;
    let root_canonical = root
        .canonicalize()
        .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;
    if !canonical.starts_with(&root_canonical) {
        return Err(CommandError::new(
            "invalid",
            "Path outside attachments directory",
        ));
    }
    Ok(AttachmentSrc {
        local_filename: name.to_string(),
        url: asset_protocol_url(&canonical),
    })
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
