use regex::Regex;
use std::sync::OnceLock;

use super::models::{
    WallpaperConfig, WallpaperFilterConfig, WallpaperType, WALLPAPER_SCHEMA_VERSION,
};

fn remote_url_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^https?://").expect("url regex"))
}

fn css_injection_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(url\s*\(|@import|expression\s*\(|javascript:)").expect("css regex")
    })
}

fn hex_color_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^#([0-9a-fA-F]{6}|[0-9a-fA-F]{3})$").expect("hex regex"))
}

/// Canonical `#RRGGBB` (lowercase). Expands `#RGB`.
pub fn normalize_canonical_hex(raw: &str) -> Result<String, String> {
    let s = raw.trim();
    if !hex_color_re().is_match(s) {
        return Err(format!("invalid hex color '{raw}' (expected #RRGGBB)"));
    }
    if s.len() == 4 {
        let chars: Vec<char> = s.chars().collect();
        return Ok(format!(
            "#{}{}{}{}{}{}",
            chars[1], chars[1], chars[2], chars[2], chars[3], chars[3]
        )
        .to_lowercase());
    }
    Ok(s.to_lowercase())
}

pub fn validate_wallpaper_config(raw: &str) -> Result<WallpaperConfig, String> {
    let mut config: WallpaperConfig =
        serde_json::from_str(raw).map_err(|e| format!("wallpaper JSON invalid: {e}"))?;

    if config.schema_version != WALLPAPER_SCHEMA_VERSION {
        return Err(format!(
            "unsupported schemaVersion: {}",
            config.schema_version
        ));
    }

    validate_fields(&mut config)?;
    Ok(config)
}

fn validate_filter(filter: &WallpaperFilterConfig) -> Result<(), String> {
    if let Some(preset) = &filter.preset {
        let p = preset.trim().to_lowercase();
        if !matches!(p.as_str(), "none" | "soft" | "vivid" | "muted" | "blur") {
            return Err(format!("unknown wallpaper filter preset '{preset}'"));
        }
    }
    clamp_opt("filter.brightness", filter.brightness, 0.5, 1.5)?;
    clamp_opt("filter.contrast", filter.contrast, 0.5, 1.5)?;
    clamp_opt("filter.saturate", filter.saturate, 0.0, 2.0)?;
    clamp_opt("filter.blurPx", filter.blur_px, 0.0, 20.0)?;
    Ok(())
}

fn clamp_opt(name: &str, value: Option<f64>, min: f64, max: f64) -> Result<(), String> {
    if let Some(v) = value {
        if !(min..=max).contains(&v) || !v.is_finite() {
            return Err(format!("{name} must be between {min} and {max}"));
        }
    }
    Ok(())
}

fn validate_fields(config: &mut WallpaperConfig) -> Result<(), String> {
    for field in [
        &mut config.color,
        &mut config.secondary_color,
        &mut config.reduced_motion_fallback,
    ] {
        if let Some(c) = field.as_ref() {
            if css_injection_re().is_match(c) || remote_url_re().is_match(c) {
                return Err("raw CSS or remote URLs are not allowed in wallpaper fields".into());
            }
            *field = Some(normalize_canonical_hex(c)?);
        }
    }

    if let Some(filter) = &config.filter {
        validate_filter(filter)?;
    }

    match config.wallpaper_type {
        WallpaperType::ImageCover | WallpaperType::VideoLoop | WallpaperType::AnimatedImage => {
            let id = config
                .asset_id
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| "media wallpaper requires local assetId".to_string())?;
            if remote_url_re().is_match(id) {
                return Err("remote asset URLs are not allowed; use local assetId".into());
            }
        }
        WallpaperType::Slideshow => {
            let ids = config
                .slide_asset_ids
                .as_ref()
                .filter(|v| !v.is_empty())
                .ok_or_else(|| "slideshow requires slideAssetIds".to_string())?;
            for id in ids {
                if remote_url_re().is_match(id) {
                    return Err("remote asset URLs are not allowed in slideshow".into());
                }
            }
        }
        WallpaperType::CanvasPreset if config.preset.as_deref().unwrap_or("").trim().is_empty() => {
            return Err("canvas preset requires preset name".into());
        }
        WallpaperType::StaticColor => {
            if config.color.as_deref().unwrap_or("").trim().is_empty() {
                return Err("static-color wallpaper requires color (#RRGGBB)".into());
            }
        }
        _ => {}
    }

    if let Some(extra) = &config.extra {
        let s = extra.to_string();
        if css_injection_re().is_match(&s) {
            return Err("extra fields cannot contain raw CSS".into());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallpapers::models::WallpaperType;

    #[test]
    fn rejects_remote_asset_url() {
        let raw = serde_json::json!({
            "schemaVersion": "1",
            "type": "image-cover",
            "assetId": "https://evil.com/x.png"
        });
        assert!(validate_wallpaper_config(&raw.to_string()).is_err());
    }

    #[test]
    fn accepts_local_asset() {
        let raw = serde_json::json!({
            "schemaVersion": "1",
            "type": "image-cover",
            "assetId": "abc-123"
        });
        let cfg = validate_wallpaper_config(&raw.to_string()).unwrap();
        assert_eq!(cfg.wallpaper_type, WallpaperType::ImageCover);
    }

    #[test]
    fn rejects_legacy_kind_none_without_schema_version() {
        let raw = r#"{"kind":"none"}"#;
        let err = validate_wallpaper_config(raw).unwrap_err();
        assert!(
            err.contains("schemaVersion") || err.contains("missing field"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn accepts_static_color_none_equivalent() {
        let raw = serde_json::json!({
            "schemaVersion": "1",
            "type": "static-color",
            "color": "#141714"
        });
        let cfg = validate_wallpaper_config(&raw.to_string()).unwrap();
        assert_eq!(cfg.wallpaper_type, WallpaperType::StaticColor);
    }

    #[test]
    fn normalizes_short_hex_and_rejects_invalid() {
        assert_eq!(normalize_canonical_hex("#ABC").unwrap(), "#aabbcc");
        assert!(normalize_canonical_hex("red").is_err());
        let raw = serde_json::json!({
            "schemaVersion": "1",
            "type": "static-color",
            "color": "not-a-color"
        });
        assert!(validate_wallpaper_config(&raw.to_string()).is_err());
    }

    #[test]
    fn accepts_bounded_filter_config() {
        let raw = serde_json::json!({
            "schemaVersion": "1",
            "type": "static-color",
            "color": "#141714",
            "filter": { "preset": "soft", "brightness": 1.05 }
        });
        let cfg = validate_wallpaper_config(&raw.to_string()).unwrap();
        assert!(cfg.filter.is_some());
    }

    #[test]
    fn rejects_out_of_range_filter() {
        let raw = serde_json::json!({
            "schemaVersion": "1",
            "type": "static-color",
            "color": "#141714",
            "filter": { "blurPx": 99.0 }
        });
        assert!(validate_wallpaper_config(&raw.to_string()).is_err());
    }
}
