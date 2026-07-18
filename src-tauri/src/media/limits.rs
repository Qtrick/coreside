//! Centralized media size and duration limits.

/// Max image download size (25 MiB).
pub const MAX_IMAGE_BYTES: usize = 25 * 1024 * 1024;
/// Max video download size (200 MiB).
pub const MAX_VIDEO_BYTES: usize = 200 * 1024 * 1024;
/// Max animated image / GIF size (15 MiB).
pub const MAX_ANIMATED_IMAGE_BYTES: usize = 15 * 1024 * 1024;
/// Max video duration for imports (10 minutes).
pub const MAX_VIDEO_DURATION_MS: i64 = 10 * 60 * 1000;
/// Max image dimension (edge). Images exceeding this are rejected after parse.
pub const MAX_IMAGE_EDGE_PX: u32 = 16_384;
/// Max edge length for generated JPEG thumbnails.
pub const THUMB_MAX_EDGE_PX: u32 = 320;

pub const ALLOWED_IMAGE_MIMES: &[&str] = &[
    "image/jpeg",
    "image/png",
    "image/webp",
    "image/gif",
    "image/avif",
];

pub const ALLOWED_VIDEO_MIMES: &[&str] = &[
    "video/mp4",
    "video/webm",
    "video/quicktime",
];

pub fn max_bytes_for_category(category: &str) -> usize {
    match category {
        "video" => MAX_VIDEO_BYTES,
        "animated-image" | "gif" => MAX_ANIMATED_IMAGE_BYTES,
        _ => MAX_IMAGE_BYTES,
    }
}
