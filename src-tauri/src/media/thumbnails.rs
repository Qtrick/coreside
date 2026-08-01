//! Generate bounded JPEG thumbnails for imported still images / first-frame GIFs.

use std::io::Cursor;

use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat, ImageReader};

use super::errors::MediaError;
use super::limits::THUMB_MAX_EDGE_PX;
use super::storage::{delete_asset_file, write_asset_bytes};

/// Write `{asset_id}.thumb.jpg` under the managed media directory.
/// Returns the local filename on success. Non-image / decode failures return `Ok(None)`.
pub fn generate_thumbnail(
    asset_id: &str,
    mime: &str,
    bytes: &[u8],
) -> Result<Option<String>, MediaError> {
    let Some(out) = encode_thumbnail_jpeg(mime, bytes)? else {
        return Ok(None);
    };
    let filename = format!("{asset_id}.thumb.jpg");
    write_asset_bytes(&filename, &out)?;
    Ok(Some(filename))
}

/// Encode a JPEG thumbnail in memory (used by import and unit tests).
pub fn encode_thumbnail_jpeg(mime: &str, bytes: &[u8]) -> Result<Option<Vec<u8>>, MediaError> {
    if !mime.starts_with("image/") {
        return Ok(None);
    }

    let img = match decode_image(bytes) {
        Some(img) => img,
        None => return Ok(None),
    };

    let thumb = resize_to_thumb(img);
    let mut out = Vec::new();
    thumb
        .write_to(&mut Cursor::new(&mut out), ImageFormat::Jpeg)
        .map_err(|e| MediaError::Storage(format!("thumbnail encode failed: {e}")))?;
    Ok(Some(out))
}

pub fn delete_thumbnail(thumbnail_filename: Option<&str>) {
    if let Some(name) = thumbnail_filename.filter(|s| !s.is_empty()) {
        let _ = delete_asset_file(name);
    }
}

fn decode_image(bytes: &[u8]) -> Option<DynamicImage> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    reader.decode().ok()
}

fn resize_to_thumb(img: DynamicImage) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return img;
    }
    let max_edge = THUMB_MAX_EDGE_PX;
    if w <= max_edge && h <= max_edge {
        return img;
    }
    let scale = (max_edge as f32 / w as f32).min(max_edge as f32 / h as f32);
    let nw = ((w as f32) * scale).round().max(1.0) as u32;
    let nh = ((h as f32) * scale).round().max(1.0) as u32;
    img.resize(nw, nh, FilterType::Triangle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};

    fn tiny_png() -> Vec<u8> {
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_fn(400, 200, |x, y| {
            Rgb([(x % 255) as u8, (y % 255) as u8, 120])
        });
        let mut bytes = Vec::new();
        DynamicImage::ImageRgb8(img)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn encodes_jpeg_thumb_for_png() {
        let jpeg = encode_thumbnail_jpeg("image/png", &tiny_png())
            .unwrap()
            .expect("thumb bytes");
        assert!(jpeg.starts_with(&[0xFF, 0xD8, 0xFF]));
        let dims = ImageReader::new(Cursor::new(&jpeg))
            .with_guessed_format()
            .unwrap()
            .into_dimensions()
            .unwrap();
        assert!(dims.0 <= THUMB_MAX_EDGE_PX);
        assert!(dims.1 <= THUMB_MAX_EDGE_PX);
    }

    #[test]
    fn skips_video_mime() {
        assert!(encode_thumbnail_jpeg("video/mp4", b"not-a-video")
            .unwrap()
            .is_none());
    }
}
