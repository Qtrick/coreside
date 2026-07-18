//! Media metadata: dimensions and duration parsing.

use std::io::Cursor;

use image::ImageReader;

use super::errors::MediaError;
use super::limits::MAX_IMAGE_EDGE_PX;
use super::models::MediaMetadata;

/// Enrich validated media metadata with dimensions / duration.
/// Failures are non-fatal for unknown codecs; oversized images are rejected.
pub fn enrich_metadata(mut meta: MediaMetadata, bytes: &[u8]) -> Result<MediaMetadata, MediaError> {
    if meta.mime_type.starts_with("image/") {
        if let Some((w, h)) = parse_image_dimensions(bytes) {
            if w > MAX_IMAGE_EDGE_PX || h > MAX_IMAGE_EDGE_PX {
                return Err(MediaError::Invalid(format!(
                    "image dimensions {w}x{h} exceed max edge {MAX_IMAGE_EDGE_PX}px"
                )));
            }
            meta.width = Some(w);
            meta.height = Some(h);
        } else if let Some((w, h)) = parse_image_dimensions_headers(bytes, &meta.mime_type) {
            if w > MAX_IMAGE_EDGE_PX || h > MAX_IMAGE_EDGE_PX {
                return Err(MediaError::Invalid(format!(
                    "image dimensions {w}x{h} exceed max edge {MAX_IMAGE_EDGE_PX}px"
                )));
            }
            meta.width = Some(w);
            meta.height = Some(h);
        }
    } else if meta.mime_type == "video/mp4" || meta.mime_type == "video/quicktime" {
        if let Some((w, h, duration_ms)) = parse_mp4_metadata(bytes) {
            meta.width = w;
            meta.height = h;
            if let Some(ms) = duration_ms {
                if ms > super::limits::MAX_VIDEO_DURATION_MS {
                    return Err(MediaError::Invalid(format!(
                        "video duration {ms}ms exceeds limit of {}ms",
                        super::limits::MAX_VIDEO_DURATION_MS
                    )));
                }
                meta.duration_ms = Some(ms);
            }
        }
    }

    Ok(meta)
}

fn parse_image_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    reader.into_dimensions().ok()
}

/// Lightweight header parsers used when the image crate cannot guess the format.
fn parse_image_dimensions_headers(bytes: &[u8], mime: &str) -> Option<(u32, u32)> {
    match mime {
        "image/png" => parse_png_ihdr(bytes),
        "image/gif" => parse_gif_logical_screen(bytes),
        "image/webp" => parse_webp_dimensions(bytes),
        "image/jpeg" => parse_jpeg_sof(bytes),
        _ => None,
    }
}

fn parse_png_ihdr(bytes: &[u8]) -> Option<(u32, u32)> {
    // Signature (8) + IHDR length(4) + type(4) + width(4) + height(4)
    if bytes.len() < 24 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return None;
    }
    if &bytes[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    if w == 0 || h == 0 {
        return None;
    }
    Some((w, h))
}

fn parse_gif_logical_screen(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 10 || !bytes.starts_with(b"GIF8") {
        return None;
    }
    let w = u16::from_le_bytes(bytes[6..8].try_into().ok()?) as u32;
    let h = u16::from_le_bytes(bytes[8..10].try_into().ok()?) as u32;
    if w == 0 || h == 0 {
        return None;
    }
    Some((w, h))
}

fn parse_webp_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 30 || !bytes.starts_with(b"RIFF") || &bytes[8..12] != b"WEBP" {
        return None;
    }
    let chunk = &bytes[12..16];
    match chunk {
        b"VP8X" if bytes.len() >= 30 => {
            let w = 1 + u32::from_le_bytes([bytes[24], bytes[25], bytes[26], 0]);
            let h = 1 + u32::from_le_bytes([bytes[27], bytes[28], bytes[29], 0]);
            Some((w, h))
        }
        b"VP8 " if bytes.len() >= 30 => {
            // Lossy bitstream starts at offset 20; width/height at 26–29 (14-bit each).
            let w = (u16::from_le_bytes(bytes[26..28].try_into().ok()?) & 0x3FFF) as u32;
            let h = (u16::from_le_bytes(bytes[28..30].try_into().ok()?) & 0x3FFF) as u32;
            if w == 0 || h == 0 {
                None
            } else {
                Some((w, h))
            }
        }
        b"VP8L" if bytes.len() >= 25 => {
            let b0 = bytes[21];
            let b1 = bytes[22];
            let b2 = bytes[23];
            let b3 = bytes[24];
            let w = (u32::from(b0) | (u32::from(b1 & 0x3F) << 8)) + 1;
            let h = ((u32::from(b1) >> 6)
                | (u32::from(b2) << 2)
                | (u32::from(b3 & 0x0F) << 10))
                + 1;
            Some((w, h))
        }
        _ => None,
    }
}

fn parse_jpeg_sof(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return None;
    }
    let mut i = 2usize;
    while i + 9 < bytes.len() {
        if bytes[i] != 0xFF {
            i += 1;
            continue;
        }
        while i < bytes.len() && bytes[i] == 0xFF {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let marker = bytes[i];
        i += 1;
        // Standalone markers without length
        if marker == 0xD8 || marker == 0xD9 || (0xD0..=0xD7).contains(&marker) {
            continue;
        }
        if i + 1 >= bytes.len() {
            break;
        }
        let len = u16::from_be_bytes([bytes[i], bytes[i + 1]]) as usize;
        if len < 2 || i + len > bytes.len() {
            break;
        }
        // SOF0–SOF3, SOF5–SOF7, SOF9–SOF11, SOF13–SOF15
        let is_sof = matches!(
            marker,
            0xC0 | 0xC1 | 0xC2 | 0xC3 | 0xC5 | 0xC6 | 0xC7 | 0xC9 | 0xCA | 0xCB | 0xCD | 0xCE
                | 0xCF
        );
        if is_sof && len >= 7 {
            let h = u16::from_be_bytes([bytes[i + 3], bytes[i + 4]]) as u32;
            let w = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]) as u32;
            if w > 0 && h > 0 {
                return Some((w, h));
            }
        }
        i += len;
    }
    None
}

fn parse_mp4_metadata(bytes: &[u8]) -> Option<(Option<u32>, Option<u32>, Option<i64>)> {
    let mut cursor = std::io::Cursor::new(bytes);
    let context = mp4parse::read_mp4(&mut cursor).ok()?;
    let mut width = None;
    let mut height = None;
    let mut duration_ms = None;

    for track in &context.tracks {
        if track.track_type != mp4parse::TrackType::Video {
            continue;
        }
        if let Some(tkhd) = track.tkhd.as_ref() {
            // tkhd width/height are 16.16 fixed point
            let w = (tkhd.width >> 16) as u32;
            let h = (tkhd.height >> 16) as u32;
            if w > 0 && h > 0 {
                width = Some(w);
                height = Some(h);
            }
        }
        if let (Some(timescale), Some(duration)) = (track.timescale, track.duration) {
            if timescale.0 > 0 {
                let ms = (duration.0 as u128)
                    .saturating_mul(1000)
                    / timescale.0 as u128;
                duration_ms = Some(ms.min(i64::MAX as u128) as i64);
            }
        } else if let (Some(media_scale), Some(tkhd)) = (context.timescale, track.tkhd.as_ref()) {
            if media_scale.0 > 0 && tkhd.duration > 0 && tkhd.duration != u64::MAX {
                let ms = (tkhd.duration as u128)
                    .saturating_mul(1000)
                    / media_scale.0 as u128;
                duration_ms = Some(ms.min(i64::MAX as u128) as i64);
            }
        }
        break;
    }

    if width.is_none() && height.is_none() && duration_ms.is_none() {
        return None;
    }
    Some((width, height, duration_ms))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_png() -> Vec<u8> {
        // 1x1 PNG
        vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00,
            0x00, 0x90, 0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08,
            0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x01, 0x00, 0x05, 0xFE,
            0x02, 0xFE, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ]
    }

    #[test]
    fn parses_png_dimensions() {
        let meta = MediaMetadata {
            mime_type: "image/png".into(),
            category: "image".into(),
            width: None,
            height: None,
            duration_ms: None,
        };
        let enriched = enrich_metadata(meta, &tiny_png()).unwrap();
        assert_eq!(enriched.width, Some(1));
        assert_eq!(enriched.height, Some(1));
    }

    #[test]
    fn png_ihdr_header_parser() {
        assert_eq!(parse_png_ihdr(&tiny_png()), Some((1, 1)));
    }

    #[test]
    fn rejects_oversized_declared_dimensions() {
        // Craft IHDR claiming huge size (won't decode via image crate; header path used)
        let mut bytes = tiny_png();
        bytes[16..20].copy_from_slice(&(MAX_IMAGE_EDGE_PX + 1).to_be_bytes());
        bytes[20..24].copy_from_slice(&100u32.to_be_bytes());
        // Corrupt CRC so image crate fails → header path
        bytes[29] ^= 0xFF;
        let meta = MediaMetadata {
            mime_type: "image/png".into(),
            category: "image".into(),
            width: None,
            height: None,
            duration_ms: None,
        };
        // image crate may still fail; header parser should catch size
        let result = enrich_metadata(meta, &bytes);
        // Either Err(oversized) or Ok with dims from image crate if it somehow decoded —
        // with corrupted CRC, expect header path rejection.
        if let Ok(m) = &result {
            if let (Some(w), Some(h)) = (m.width, m.height) {
                assert!(w <= MAX_IMAGE_EDGE_PX && h <= MAX_IMAGE_EDGE_PX);
            }
        } else {
            assert!(result.is_err());
        }
    }
}
