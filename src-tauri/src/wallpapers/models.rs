use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const WALLPAPER_SCHEMA_VERSION: &str = "1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum WallpaperType {
    StaticColor,
    LinearGradient,
    ImageCover,
    VideoLoop,
    AnimatedImage,
    Slideshow,
    AmbientGradient,
    FloatingParticles,
    /// Legacy canvas presets (matrix, aurora, …) from global settings.
    CanvasPreset,
}

/// Typed wallpaper-layer filter — never a raw CSS filter string.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct WallpaperFilterConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brightness: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contrast: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saturate: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blur_px: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WallpaperConfig {
    pub schema_version: String,
    #[serde(rename = "type")]
    pub wallpaper_type: WallpaperType,
    pub color: Option<String>,
    pub secondary_color: Option<String>,
    pub gradient_angle: Option<f64>,
    pub asset_id: Option<String>,
    pub preset: Option<String>,
    pub slide_asset_ids: Option<Vec<String>>,
    pub interval_ms: Option<u64>,
    pub muted: Option<bool>,
    pub reduced_motion_fallback: Option<String>,
    pub opacity: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<WallpaperFilterConfig>,
    pub extra: Option<Value>,
}

impl WallpaperConfig {
    pub fn static_color(color: &str) -> Self {
        Self {
            schema_version: WALLPAPER_SCHEMA_VERSION.into(),
            wallpaper_type: WallpaperType::StaticColor,
            color: Some(color.into()),
            secondary_color: None,
            gradient_angle: None,
            asset_id: None,
            preset: None,
            slide_asset_ids: None,
            interval_ms: None,
            muted: None,
            reduced_motion_fallback: None,
            opacity: None,
            filter: None,
            extra: None,
        }
    }
}
