//! Allowlisted appearance preference changes (theme, colors, backgrounds, live wallpapers).

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use once_cell::sync::Lazy;

static HEX_COLOR: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^#([0-9a-fA-F]{6}|[0-9a-fA-F]{3})$").expect("hex color regex")
});

/// Allowed live wallpaper kinds (declarative presets — no arbitrary code).
pub const WALLPAPER_KINDS: &[&str] = &[
    "none",
    "matrix",
    "aurora",
    "particles",
    "rain",
    "pulse",
];

/// Allowed KV keys the agent may set.
pub const ALLOWED_SETTING_KEYS: &[&str] = &[
    "theme",
    "accentPrimaryLight",
    "accentPrimaryDark",
    "accentSecondaryLight",
    "accentSecondaryDark",
    "backgroundLight",
    "backgroundDark",
    "surfaceLight",
    "surfaceDark",
    "surfaceMutedLight",
    "surfaceMutedDark",
    "borderLight",
    "borderDark",
    "textPrimaryLight",
    "textPrimaryDark",
    "textSecondaryLight",
    "textSecondaryDark",
    "wallpaper",
    "wallpaperJson",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ColorVariants {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dark: Option<String>,
}

/// Back-compat alias used in older call sites / docs.
#[allow(dead_code)]
pub type AccentVariants = ColorVariants;

/// Declarative live wallpaper preference.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WallpaperConfig {
    /// One of WALLPAPER_KINDS.
    pub kind: String,
    /// Primary glyph / particle / streak color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Secondary color (aurora / pulse).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secondary_color: Option<String>,
    /// Animation speed multiplier (0.25–3). Default 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<f32>,
    /// Density / column count feel (0.1–1). Default ~0.55.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub density: Option<f32>,
    /// Overlay opacity (0.05–0.85). Default depends on kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,
}

impl Default for WallpaperConfig {
    fn default() -> Self {
        Self {
            kind: "none".into(),
            color: None,
            secondary_color: None,
            speed: None,
            density: None,
            opacity: None,
        }
    }
}

impl WallpaperConfig {
    pub fn validate(&self) -> Result<(), String> {
        let kind = self.kind.trim().to_lowercase();
        if !WALLPAPER_KINDS.contains(&kind.as_str()) {
            return Err(format!(
                "wallpaper.kind must be one of: {}",
                WALLPAPER_KINDS.join(", ")
            ));
        }
        if let Some(c) = &self.color {
            normalize_hex(c).map_err(|e| format!("wallpaper.color: {e}"))?;
        }
        if let Some(c) = &self.secondary_color {
            normalize_hex(c).map_err(|e| format!("wallpaper.secondaryColor: {e}"))?;
        }
        clamp_opt("wallpaper.speed", self.speed, 0.25, 3.0)?;
        clamp_opt("wallpaper.density", self.density, 0.1, 1.0)?;
        clamp_opt("wallpaper.opacity", self.opacity, 0.05, 0.85)?;
        Ok(())
    }

    pub fn normalized(&self) -> Result<Self, String> {
        self.validate()?;
        Ok(Self {
            kind: self.kind.trim().to_lowercase(),
            color: self
                .color
                .as_ref()
                .map(|c| normalize_hex(c))
                .transpose()?,
            secondary_color: self
                .secondary_color
                .as_ref()
                .map(|c| normalize_hex(c))
                .transpose()?,
            speed: self.speed,
            density: self.density,
            opacity: self.opacity,
        })
    }

    pub fn to_json_string(&self) -> Result<String, String> {
        let n = self.normalized()?;
        serde_json::to_string(&n).map_err(|e| e.to_string())
    }
}

fn clamp_opt(name: &str, value: Option<f32>, min: f32, max: f32) -> Result<(), String> {
    if let Some(v) = value {
        if !(min..=max).contains(&v) || !v.is_finite() {
            return Err(format!("{name} must be between {min} and {max}"));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SettingsChangePayload {
    /// `system` | `light` | `dark`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// Accent primary with light/dark variants (hex).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent_primary: Option<ColorVariants>,
    /// Accent secondary with light/dark variants (hex).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent_secondary: Option<ColorVariants>,
    /// App canvas background with light/dark variants (hex).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<ColorVariants>,
    /// Panel / card surface with light/dark variants (hex).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface: Option<ColorVariants>,
    /// Muted surface (inputs, hover fills) with light/dark variants (hex).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_muted: Option<ColorVariants>,
    /// Borders / dividers with light/dark variants (hex).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<ColorVariants>,
    /// Primary body text with light/dark variants (hex).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_primary: Option<ColorVariants>,
    /// Secondary / muted text with light/dark variants (hex).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_secondary: Option<ColorVariants>,
    /// Live wallpaper preset (matrix, aurora, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallpaper: Option<WallpaperConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change_summary: Option<String>,
}

impl SettingsChangePayload {
    pub fn is_empty(&self) -> bool {
        self.theme.is_none()
            && self.accent_primary.is_none()
            && self.accent_secondary.is_none()
            && self.background.is_none()
            && self.surface.is_none()
            && self.surface_muted.is_none()
            && self.border.is_none()
            && self.text_primary.is_none()
            && self.text_secondary.is_none()
            && self.wallpaper.is_none()
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.is_empty() {
            return Err(
                "settingsChange must include theme, colors, borders, backgrounds, and/or wallpaper"
                    .into(),
            );
        }
        if let Some(theme) = &self.theme {
            let t = theme.trim().to_lowercase();
            if !matches!(t.as_str(), "system" | "light" | "dark") {
                return Err(format!(
                    "theme must be system, light, or dark (got '{theme}')"
                ));
            }
        }
        validate_color_pair("accentPrimary", self.accent_primary.as_ref())?;
        validate_color_pair("accentSecondary", self.accent_secondary.as_ref())?;
        validate_color_pair("background", self.background.as_ref())?;
        validate_color_pair("surface", self.surface.as_ref())?;
        validate_color_pair("surfaceMuted", self.surface_muted.as_ref())?;
        validate_color_pair("border", self.border.as_ref())?;
        validate_color_pair("textPrimary", self.text_primary.as_ref())?;
        validate_color_pair("textSecondary", self.text_secondary.as_ref())?;
        if let Some(w) = &self.wallpaper {
            w.validate()?;
        }
        Ok(())
    }

    /// Flatten into KV pairs for persistence.
    pub fn to_kv_pairs(&self) -> Result<Vec<(String, String)>, String> {
        self.validate()?;
        let mut out = Vec::new();
        if let Some(theme) = &self.theme {
            out.push(("theme".into(), theme.trim().to_lowercase()));
        }
        push_color_pair(&mut out, "accentPrimary", self.accent_primary.as_ref())?;
        push_color_pair(&mut out, "accentSecondary", self.accent_secondary.as_ref())?;
        push_color_pair(&mut out, "background", self.background.as_ref())?;
        push_color_pair(&mut out, "surface", self.surface.as_ref())?;
        push_color_pair(&mut out, "surfaceMuted", self.surface_muted.as_ref())?;
        push_color_pair(&mut out, "border", self.border.as_ref())?;
        push_color_pair(&mut out, "textPrimary", self.text_primary.as_ref())?;
        push_color_pair(&mut out, "textSecondary", self.text_secondary.as_ref())?;
        if let Some(w) = &self.wallpaper {
            out.push(("wallpaper".into(), w.to_json_string()?));
        }
        Ok(out)
    }
}

fn push_color_pair(
    out: &mut Vec<(String, String)>,
    name: &str,
    colors: Option<&ColorVariants>,
) -> Result<(), String> {
    let Some(colors) = colors else {
        return Ok(());
    };
    if let Some(v) = &colors.light {
        out.push((format!("{name}Light"), normalize_hex(v)?));
    }
    if let Some(v) = &colors.dark {
        out.push((format!("{name}Dark"), normalize_hex(v)?));
    }
    Ok(())
}

fn validate_color_pair(name: &str, colors: Option<&ColorVariants>) -> Result<(), String> {
    let Some(colors) = colors else {
        return Ok(());
    };
    let light = colors
        .light
        .as_ref()
        .ok_or_else(|| format!("{name} must include both light and dark hex values"))?;
    let dark = colors
        .dark
        .as_ref()
        .ok_or_else(|| format!("{name} must include both light and dark hex values"))?;
    normalize_hex(light).map_err(|e| format!("{name}.light: {e}"))?;
    normalize_hex(dark).map_err(|e| format!("{name}.dark: {e}"))?;
    Ok(())
}

fn normalize_hex(raw: &str) -> Result<String, String> {
    let s = raw.trim();
    if !HEX_COLOR.is_match(s) {
        return Err(format!("invalid hex color '{raw}' (expected #RGB or #RRGGBB)"));
    }
    if s.len() == 4 {
        // #abc → #aabbcc
        let chars: Vec<char> = s.chars().collect();
        return Ok(format!(
            "#{}{}{}{}{}{}",
            chars[1], chars[1], chars[2], chars[2], chars[3], chars[3]
        )
        .to_lowercase());
    }
    Ok(s.to_lowercase())
}

/// Normalize a hex color or return `None` when invalid (for tolerant reads).
pub fn normalize_hex_or_none(raw: &str) -> Option<String> {
    normalize_hex(raw).ok()
}

/// Strict wallpaper parse for write paths (rejects unknown kinds / bad ranges).
pub fn parse_wallpaper_setting_strict(raw: &str) -> Result<WallpaperConfig, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(WallpaperConfig::default());
    }
    let parsed: WallpaperConfig = serde_json::from_str(trimmed)
        .map_err(|e| format!("wallpaper must be JSON: {e}"))?;
    parsed.normalized()
}

/// Validate + normalize a Base Settings KV write (appearance allowlist + user prefs).
pub fn normalize_setting_kv(key: &str, value: &str) -> Result<String, String> {
    let key = key.trim();
    match key {
        "theme" => {
            let t = value.trim().to_lowercase();
            if matches!(t.as_str(), "system" | "light" | "dark") {
                Ok(t)
            } else {
                Err("theme must be system, light, or dark".into())
            }
        }
        "dockIcon" | "dock_icon" => {
            let t = value.trim().to_lowercase();
            if matches!(t.as_str(), "auto" | "dark" | "light") {
                Ok(t)
            } else {
                Err("dockIcon must be auto, dark, or light".into())
            }
        }
        "sidebarCollapsed" | "sidebar_collapsed" => {
            let t = value.trim().to_lowercase();
            if matches!(t.as_str(), "true" | "false") {
                Ok(t)
            } else {
                Err("sidebarCollapsed must be true or false".into())
            }
        }
        "preferredModel" | "preferred_model" => {
            let t = value.trim();
            if t.is_empty() {
                Ok("auto".into())
            } else if t.len() > 200 {
                Err("preferredModel is too long".into())
            } else {
                Ok(t.to_string())
            }
        }
        "actionLogEnabled" | "action_log_enabled" => {
            let t = value.trim().to_lowercase();
            if matches!(t.as_str(), "true" | "1" | "yes") {
                Ok("true".into())
            } else if matches!(t.as_str(), "false" | "0" | "no") {
                Ok("false".into())
            } else {
                Err("actionLogEnabled must be true or false".into())
            }
        }
        "actionLogMode" | "action_log_mode" => {
            let mode = match value.trim().to_lowercase().as_str() {
                "always" | "on" | "true" | "1" | "yes" => "always",
                "intelligent" | "auto" | "smart" => "intelligent",
                "off" | "false" | "0" | "no" => "off",
                _ => {
                    return Err(
                        "actionLogMode must be off, always, or intelligent".into(),
                    )
                }
            };
            Ok(mode.into())
        }
        "safeSearch" | "safe_search" => {
            let t = value.trim().to_lowercase();
            if matches!(t.as_str(), "strict" | "standard" | "off") {
                Ok(t)
            } else {
                Err("safeSearch must be strict, standard, or off".into())
            }
        }
        "wallpaper" => Ok(parse_wallpaper_setting_strict(value)?.to_json_string()?),
        "wallpaperJson" | "wallpaper_json" => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                Ok(String::new())
            } else {
                crate::wallpapers::validate_wallpaper_config(trimmed)?;
                Ok(trimmed.to_string())
            }
        }
        k if k.starts_with("accent")
            || k.starts_with("background")
            || k.starts_with("surface")
            || k.starts_with("border")
            || k.starts_with("text") =>
        {
            // Accept camelCase appearance color keys only when they are allowlisted.
            if ALLOWED_SETTING_KEYS.contains(&k) || is_appearance_color_key(k) {
                normalize_hex(value)
            } else {
                Ok(value.to_string())
            }
        }
        _ => Ok(value.to_string()),
    }
}

fn is_appearance_color_key(key: &str) -> bool {
    matches!(
        key,
        "accentPrimaryLight"
            | "accentPrimaryDark"
            | "accentSecondaryLight"
            | "accentSecondaryDark"
            | "backgroundLight"
            | "backgroundDark"
            | "surfaceLight"
            | "surfaceDark"
            | "surfaceMutedLight"
            | "surfaceMutedDark"
            | "borderLight"
            | "borderDark"
            | "textPrimaryLight"
            | "textPrimaryDark"
            | "textSecondaryLight"
            | "textSecondaryDark"
            | "accent_primary_light"
            | "accent_primary_dark"
            | "accent_secondary_light"
            | "accent_secondary_dark"
            | "background_light"
            | "background_dark"
            | "surface_light"
            | "surface_dark"
            | "surface_muted_light"
            | "surface_muted_dark"
            | "border_light"
            | "border_dark"
            | "text_primary_light"
            | "text_primary_dark"
            | "text_secondary_light"
            | "text_secondary_dark"
    )
}

/// Keys the agent may persist via `settings_change` (appearance allowlist only).
/// Base Settings like `actionLogEnabled` are intentionally excluded — user toggle only.
pub fn is_allowed_setting_key(key: &str) -> bool {
    ALLOWED_SETTING_KEYS.contains(&key.trim())
}

/// Derive a slightly darker/lighter hover from a hex primary (simple blend).
#[allow(dead_code)]
pub fn derive_hover_hex(hex: &str) -> Option<String> {
    let h = normalize_hex(hex).ok()?;
    let r = u8::from_str_radix(&h[1..3], 16).ok()?;
    let g = u8::from_str_radix(&h[3..5], 16).ok()?;
    let b = u8::from_str_radix(&h[5..7], 16).ok()?;
    // Darken ~12% toward black for hover.
    let mix = |c: u8| ((c as u16 * 88) / 100) as u8;
    Some(format!("#{:02x}{:02x}{:02x}", mix(r), mix(g), mix(b)))
}

#[allow(dead_code)]
pub fn settings_change_from_value(value: &Value) -> Option<SettingsChangePayload> {
    serde_json::from_value(value.clone()).ok()
}

pub fn parse_wallpaper_setting(raw: &str) -> WallpaperConfig {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return WallpaperConfig::default();
    }
    serde_json::from_str::<WallpaperConfig>(trimmed)
        .ok()
        .and_then(|w| w.normalized().ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_theme_accents_and_backgrounds() {
        let change = SettingsChangePayload {
            theme: Some("dark".into()),
            accent_primary: Some(ColorVariants {
                light: Some("#2f8f63".into()),
                dark: Some("#69c994".into()),
            }),
            background: Some(ColorVariants {
                light: Some("#f5f6f1".into()),
                dark: Some("#0f1418".into()),
            }),
            surface: Some(ColorVariants {
                light: Some("#ffffff".into()),
                dark: Some("#1a2228".into()),
            }),
            ..Default::default()
        };
        let pairs = change.to_kv_pairs().unwrap();
        assert!(pairs.iter().any(|(k, v)| k == "theme" && v == "dark"));
        assert!(pairs
            .iter()
            .any(|(k, v)| k == "accentPrimaryLight" && v == "#2f8f63"));
        assert!(pairs
            .iter()
            .any(|(k, v)| k == "backgroundDark" && v == "#0f1418"));
        assert!(pairs
            .iter()
            .any(|(k, v)| k == "surfaceLight" && v == "#ffffff"));
    }

    #[test]
    fn accepts_matrix_wallpaper() {
        let change = SettingsChangePayload {
            theme: Some("dark".into()),
            wallpaper: Some(WallpaperConfig {
                kind: "matrix".into(),
                color: Some("#33ff66".into()),
                speed: Some(1.2),
                density: Some(0.7),
                opacity: Some(0.4),
                ..Default::default()
            }),
            background: Some(ColorVariants {
                light: Some("#f5f6f1".into()),
                dark: Some("#050805".into()),
            }),
            ..Default::default()
        };
        let pairs = change.to_kv_pairs().unwrap();
        let wallpaper = pairs
            .iter()
            .find(|(k, _)| k == "wallpaper")
            .map(|(_, v)| v.as_str())
            .unwrap();
        assert!(wallpaper.contains("matrix"));
        assert!(wallpaper.contains("#33ff66"));
    }

    #[test]
    fn rejects_unknown_wallpaper_kind() {
        let change = SettingsChangePayload {
            wallpaper: Some(WallpaperConfig {
                kind: "shadertoy".into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(change.validate().is_err());
    }

    #[test]
    fn rejects_bad_theme() {
        let change = SettingsChangePayload {
            theme: Some("neon".into()),
            ..Default::default()
        };
        assert!(change.validate().is_err());
    }

    #[test]
    fn rejects_background_without_both_variants() {
        let change = SettingsChangePayload {
            background: Some(ColorVariants {
                light: Some("#f5f6f1".into()),
                dark: None,
            }),
            ..Default::default()
        };
        assert!(change.validate().is_err());
    }

    #[test]
    fn normalize_setting_kv_rejects_bad_wallpaper_and_hex() {
        assert!(normalize_setting_kv(
            "wallpaper",
            r#"{"kind":"shadertoy"}"#
        )
        .is_err());
        assert!(normalize_setting_kv("accentPrimaryLight", "red").is_err());
        assert_eq!(
            normalize_setting_kv("accentPrimaryLight", "#ABC").unwrap(),
            "#aabbcc"
        );
        let wallpaper = normalize_setting_kv(
            "wallpaper",
            r##"{"kind":"matrix","color":"#33FF66","speed":1.2}"##,
        )
        .unwrap();
        assert!(wallpaper.contains("matrix"));
        assert!(wallpaper.contains("#33ff66"));
    }

    #[test]
    fn agent_cannot_toggle_action_log_via_allowlist() {
        assert!(!is_allowed_setting_key("actionLogEnabled"));
        assert!(!is_allowed_setting_key("action_log_enabled"));
        assert!(!is_allowed_setting_key("actionLogMode"));
        assert!(!is_allowed_setting_key("action_log_mode"));
        // User IPC may still normalize the KV; agent apply path must gate on allowlist.
        assert_eq!(
            normalize_setting_kv("actionLogEnabled", "true").unwrap(),
            "true"
        );
    }
}
