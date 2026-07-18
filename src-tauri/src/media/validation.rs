use sha2::{Digest, Sha256};

use super::errors::MediaError;
use super::limits::{ALLOWED_IMAGE_MIMES, ALLOWED_VIDEO_MIMES, max_bytes_for_category};
use super::models::MediaMetadata;

const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";
const JPEG_MAGIC: &[u8] = &[0xFF, 0xD8, 0xFF];
const GIF_MAGIC: &[u8] = b"GIF8";
const WEBP_MAGIC: &[u8] = b"RIFF";
const WEBP_FMT: &[u8] = b"WEBP";
const MP4_FTYP: &[u8] = b"ftyp";
const HTML_MAGIC: &[u8] = b"<!DOCTYPE";
const HTML_MAGIC2: &[u8] = b"<html";
const EXE_MZ: &[u8] = b"MZ";

pub fn content_hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    hex::encode(digest)
}

pub fn detect_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(PNG_MAGIC) {
        Some("image/png")
    } else if bytes.starts_with(JPEG_MAGIC) {
        Some("image/jpeg")
    } else if bytes.starts_with(GIF_MAGIC) {
        Some("image/gif")
    } else if bytes.len() >= 12
        && bytes.starts_with(WEBP_MAGIC)
        && &bytes[8..12] == WEBP_FMT
    {
        Some("image/webp")
    } else if bytes.len() >= 12 && bytes[4..8] == *MP4_FTYP {
        let brand = &bytes[8..12];
        if brand == b"avif" || brand == b"avis" {
            Some("image/avif")
        } else if brand == b"qt  " {
            Some("video/quicktime")
        } else {
            Some("video/mp4")
        }
    } else if bytes.starts_with(b"\x1a\x45\xdf\xa3") {
        Some("video/webm")
    } else {
        None
    }
}

pub fn is_rejected_payload(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(512)];
    let lower = String::from_utf8_lossy(head).to_ascii_lowercase();
    head.starts_with(EXE_MZ)
        || head.starts_with(HTML_MAGIC)
        || head.starts_with(HTML_MAGIC2)
        || lower.contains("<script")
        || lower.contains("<?php")
}

pub fn validate_bytes(bytes: &[u8], category_hint: Option<&str>) -> Result<MediaMetadata, MediaError> {
    if bytes.is_empty() {
        return Err(MediaError::Invalid("empty payload".into()));
    }
    if is_rejected_payload(bytes) {
        return Err(MediaError::Invalid(
            "executable or HTML payloads are not allowed".into(),
        ));
    }

    let mime = detect_mime(bytes)
        .ok_or_else(|| MediaError::Invalid("unrecognized media format".into()))?;

    let category = if ALLOWED_VIDEO_MIMES.contains(&mime) {
        "video"
    } else if mime == "image/gif" || mime == "image/webp" {
        // WebP may be animated; treat GIF as animated; animated WebP still "animated-image".
        if mime == "image/gif" {
            "animated-image"
        } else {
            "image"
        }
    } else if ALLOWED_IMAGE_MIMES.contains(&mime) {
        "image"
    } else {
        return Err(MediaError::Invalid(format!("MIME not allowed: {mime}")));
    };

    if let Some(hint) = category_hint {
        if hint == "video" && category != "video" {
            return Err(MediaError::Invalid("expected video asset".into()));
        }
        if hint == "image" && category == "video" {
            return Err(MediaError::Invalid("expected image asset".into()));
        }
    }

    let max = max_bytes_for_category(category);
    if bytes.len() > max {
        return Err(MediaError::Invalid(format!(
            "file exceeds limit of {max} bytes"
        )));
    }

    Ok(MediaMetadata {
        mime_type: mime.to_string(),
        category: category.to_string(),
        width: None,
        height: None,
        duration_ms: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_html_and_exe() {
        assert!(is_rejected_payload(b"<!DOCTYPE html><html></html>"));
        assert!(is_rejected_payload(b"MZ\x90\x00"));
        assert!(!is_rejected_payload(PNG_MAGIC));
    }

    #[test]
    fn accepts_png_magic() {
        let mut bytes = PNG_MAGIC.to_vec();
        bytes.extend_from_slice(&[0u8; 64]);
        let meta = validate_bytes(&bytes, None).unwrap();
        assert_eq!(meta.mime_type, "image/png");
    }
}
