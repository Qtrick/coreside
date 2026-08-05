use super::models::{WallpaperConfig, WallpaperType};

/// Trusted preset definitions for project / workspace wallpapers.
pub fn trusted_presets() -> Vec<WallpaperConfig> {
    vec![
        WallpaperConfig {
            schema_version: super::models::WALLPAPER_SCHEMA_VERSION.into(),
            wallpaper_type: WallpaperType::StaticColor,
            color: Some("#0b0f14".into()),
            secondary_color: None,
            gradient_angle: None,
            asset_id: None,
            preset: None,
            slide_asset_ids: None,
            interval_ms: None,
            muted: None,
            reduced_motion_fallback: Some("#0b0f14".into()),
            opacity: None,
            filter: None,
            extra: None,
        },
        WallpaperConfig {
            schema_version: super::models::WALLPAPER_SCHEMA_VERSION.into(),
            wallpaper_type: WallpaperType::LinearGradient,
            color: Some("#0b0f14".into()),
            secondary_color: Some("#1a2a3a".into()),
            gradient_angle: Some(135.0),
            asset_id: None,
            preset: None,
            slide_asset_ids: None,
            interval_ms: None,
            muted: None,
            reduced_motion_fallback: Some("#0b0f14".into()),
            opacity: Some(1.0),
            filter: None,
            extra: None,
        },
        WallpaperConfig {
            schema_version: super::models::WALLPAPER_SCHEMA_VERSION.into(),
            wallpaper_type: WallpaperType::AmbientGradient,
            color: Some("#14202e".into()),
            secondary_color: Some("#2d4a6a".into()),
            gradient_angle: Some(45.0),
            asset_id: None,
            preset: None,
            slide_asset_ids: None,
            interval_ms: None,
            muted: None,
            reduced_motion_fallback: Some("#0b0f14".into()),
            opacity: Some(0.9),
            filter: None,
            extra: None,
        },
        WallpaperConfig {
            schema_version: super::models::WALLPAPER_SCHEMA_VERSION.into(),
            wallpaper_type: WallpaperType::FloatingParticles,
            color: Some("#6ab0d4".into()),
            secondary_color: Some("#33ff66".into()),
            gradient_angle: None,
            asset_id: None,
            preset: Some("particles".into()),
            slide_asset_ids: None,
            interval_ms: None,
            muted: None,
            reduced_motion_fallback: Some("#0b0f14".into()),
            opacity: Some(0.35),
            filter: None,
            extra: None,
        },
        WallpaperConfig {
            schema_version: super::models::WALLPAPER_SCHEMA_VERSION.into(),
            wallpaper_type: WallpaperType::CanvasPreset,
            color: Some("#33ff66".into()),
            secondary_color: Some("#6ab0d4".into()),
            gradient_angle: None,
            asset_id: None,
            preset: Some("matrix".into()),
            slide_asset_ids: None,
            interval_ms: None,
            muted: None,
            reduced_motion_fallback: Some("#050805".into()),
            opacity: Some(0.42),
            filter: None,
            extra: None,
        },
    ]
}

pub fn preset_by_name(name: &str) -> Option<WallpaperConfig> {
    let key = name.trim().to_lowercase();
    trusted_presets()
        .into_iter()
        .find(|p| p.preset.as_deref().map(|s| s.to_lowercase()) == Some(key.clone()))
}
