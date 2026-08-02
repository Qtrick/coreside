use regex::Regex;
use std::sync::OnceLock;

use super::models::{WallpaperConfig, WallpaperType, WALLPAPER_SCHEMA_VERSION};

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

pub fn validate_wallpaper_config(raw: &str) -> Result<WallpaperConfig, String> {
    let config: WallpaperConfig =
        serde_json::from_str(raw).map_err(|e| format!("wallpaper JSON invalid: {e}"))?;

    if config.schema_version != WALLPAPER_SCHEMA_VERSION {
        return Err(format!(
            "unsupported schemaVersion: {}",
            config.schema_version
        ));
    }

    validate_fields(&config)?;
    Ok(config)
}

fn validate_fields(config: &WallpaperConfig) -> Result<(), String> {
    for c in [
        &config.color,
        &config.secondary_color,
        &config.reduced_motion_fallback,
    ]
    .into_iter()
    .flatten()
    {
        if css_injection_re().is_match(c) || remote_url_re().is_match(c) {
            return Err("raw CSS or remote URLs are not allowed in wallpaper fields".into());
        }
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
}
