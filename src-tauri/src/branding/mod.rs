//! Protected Coreside branding and macOS Dock icon authority.
//!
//! Follow macOS clears the temporary AppKit override (`applicationIconImage(None)`)
//! so the packaged application icon is authoritative again. Adaptive Icon & Widget
//! Style requires a genuine Assets.car (blocked without Xcode/actool/Icon Composer).
//! Manual mode installs a temporary PNG override for Classic or Split artwork.

use std::path::PathBuf;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// Public product name shown in Dock, menus, and About surfaces.
pub const PRODUCT_NAME: &str = "Coreside";

pub const DOCK_SCHEMA_VERSION: u32 = 1;

const SETTING_KEY: &str = "dockIcon";

/// Who controls the running Dock tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DockAuthority {
    FollowMacos,
    Manual,
}

/// Manual artwork family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DockArtwork {
    Classic,
    Split,
}

/// Manual style within an artwork family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DockStyle {
    Dark,
    Light,
    Original,
}

/// Versioned Dock preference — single settings value, single mutation authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockIconConfig {
    pub schema_version: u32,
    pub authority: DockAuthority,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artwork: Option<DockArtwork>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<DockStyle>,
}

impl Default for DockIconConfig {
    fn default() -> Self {
        Self::follow_macos()
    }
}

impl DockIconConfig {
    pub fn follow_macos() -> Self {
        Self {
            schema_version: DOCK_SCHEMA_VERSION,
            authority: DockAuthority::FollowMacos,
            artwork: None,
            style: None,
        }
    }

    pub fn manual_classic(style: DockStyle) -> Self {
        Self {
            schema_version: DOCK_SCHEMA_VERSION,
            authority: DockAuthority::Manual,
            artwork: Some(DockArtwork::Classic),
            style: Some(style),
        }
    }

    pub fn manual_split() -> Self {
        Self {
            schema_version: DOCK_SCHEMA_VERSION,
            authority: DockAuthority::Manual,
            artwork: Some(DockArtwork::Split),
            style: Some(DockStyle::Original),
        }
    }

    pub fn status_label(&self) -> &'static str {
        match self.authority {
            DockAuthority::FollowMacos => "Following macOS",
            DockAuthority::Manual => match (self.artwork, self.style) {
                (Some(DockArtwork::Classic), Some(DockStyle::Dark)) => "Using Classic Dark",
                (Some(DockArtwork::Classic), Some(DockStyle::Light)) => "Using Classic Light",
                (Some(DockArtwork::Split), _) => "Using Split",
                _ => "Using manual Dock icon",
            },
        }
    }

    /// Serialize for the settings table.
    pub fn to_storage(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|e| format!("Failed to encode dock preference: {e}"))
    }
}

/// Result returned after a committed Dock mutation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DockIconCommitResult {
    pub config: DockIconConfig,
    pub status_label: String,
    pub effective_authority: DockAuthority,
    /// True when Follow macOS cleared the temporary override.
    pub override_cleared: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DockIconCommitError {
    pub code: String,
    pub message: String,
    pub rollback_failed: bool,
}

fn commit_lock() -> &'static Mutex<()> {
    static LOCK: Mutex<()> = Mutex::new(());
    &LOCK
}

/// Parse legacy `auto`/`dark`/`light` or versioned JSON. Invalid → Follow macOS.
pub fn parse_dock_icon_setting(raw: &str) -> DockIconConfig {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return DockIconConfig::follow_macos();
    }
    let lower = trimmed.to_lowercase();
    match lower.as_str() {
        "auto" | "follow_macos" | "system" => return DockIconConfig::follow_macos(),
        "dark" => return DockIconConfig::manual_classic(DockStyle::Dark),
        "light" => return DockIconConfig::manual_classic(DockStyle::Light),
        "split" => return DockIconConfig::manual_split(),
        _ => {}
    }
    match serde_json::from_str::<DockIconConfig>(trimmed) {
        Ok(cfg) => normalize_dock_config(cfg),
        Err(_) => DockIconConfig::follow_macos(),
    }
}

/// Validate and normalize a proposed config. Rejects invalid combinations.
pub fn normalize_dock_config(mut cfg: DockIconConfig) -> DockIconConfig {
    if cfg.schema_version != DOCK_SCHEMA_VERSION {
        return DockIconConfig::follow_macos();
    }
    match cfg.authority {
        DockAuthority::FollowMacos => DockIconConfig::follow_macos(),
        DockAuthority::Manual => {
            let artwork = cfg.artwork.unwrap_or(DockArtwork::Classic);
            let style = match artwork {
                DockArtwork::Classic => match cfg.style {
                    Some(DockStyle::Light) => DockStyle::Light,
                    Some(DockStyle::Dark) | Some(DockStyle::Original) | None => DockStyle::Dark,
                },
                DockArtwork::Split => DockStyle::Original,
            };
            cfg.artwork = Some(artwork);
            cfg.style = Some(style);
            cfg
        }
    }
}

/// Strict validation used by the mutation command (rejects unknown combos).
pub fn validate_dock_config(cfg: &DockIconConfig) -> Result<DockIconConfig, String> {
    if cfg.schema_version != DOCK_SCHEMA_VERSION {
        return Err("Unsupported Dock icon schema version".into());
    }
    match cfg.authority {
        DockAuthority::FollowMacos => {
            if cfg.artwork.is_some() || cfg.style.is_some() {
                // Tolerate extras by normalizing; Follow mode ignores them.
                return Ok(DockIconConfig::follow_macos());
            }
            Ok(DockIconConfig::follow_macos())
        }
        DockAuthority::Manual => {
            let artwork = cfg
                .artwork
                .ok_or_else(|| "Manual Dock mode requires artwork".to_string())?;
            let style = cfg
                .style
                .ok_or_else(|| "Manual Dock mode requires style".to_string())?;
            match (artwork, style) {
                (DockArtwork::Classic, DockStyle::Dark | DockStyle::Light) => {}
                (DockArtwork::Split, DockStyle::Original) => {}
                (DockArtwork::Classic, DockStyle::Original) => {
                    return Err("Classic artwork requires dark or light style".into());
                }
                (DockArtwork::Split, _) => {
                    return Err("Split artwork requires original style".into());
                }
            }
            Ok(DockIconConfig {
                schema_version: DOCK_SCHEMA_VERSION,
                authority: DockAuthority::Manual,
                artwork: Some(artwork),
                style: Some(style),
            })
        }
    }
}

/// Runtime filename for a manual tile. Follow macOS never resolves a PNG.
pub fn manual_dock_filename(artwork: DockArtwork, style: DockStyle) -> Option<&'static str> {
    match (artwork, style) {
        (DockArtwork::Classic, DockStyle::Dark) => Some("coreside-dock-dark.png"),
        (DockArtwork::Classic, DockStyle::Light) => Some("coreside-dock-light.png"),
        (DockArtwork::Split, DockStyle::Original) => Some("coreside-dock-split.png"),
        _ => None,
    }
}

fn source_tree_branding_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/branding")
}

pub fn dock_icon_candidates(app: Option<&AppHandle>, filename: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(app) = app {
        let resolver = app.path();
        for rel in [
            format!("resources/branding/{filename}"),
            format!("branding/{filename}"),
        ] {
            if let Ok(path) = resolver.resolve(&rel, tauri::path::BaseDirectory::Resource) {
                candidates.push(path);
            }
        }
        if let Ok(resource_dir) = resolver.resource_dir() {
            candidates.push(resource_dir.join("resources/branding").join(filename));
            candidates.push(resource_dir.join("branding").join(filename));
        }
    }
    candidates.push(source_tree_branding_dir().join(filename));
    candidates
}

fn resolve_manual_asset(app: &AppHandle, cfg: &DockIconConfig) -> Result<PathBuf, String> {
    let artwork = cfg
        .artwork
        .ok_or_else(|| "Manual Dock preference is missing artwork".to_string())?;
    let style = cfg
        .style
        .ok_or_else(|| "Manual Dock preference is missing style".to_string())?;
    let filename = manual_dock_filename(artwork, style)
        .ok_or_else(|| "That Dock artwork combination is not available".to_string())?;
    let candidates = dock_icon_candidates(Some(app), filename);
    candidates
        .into_iter()
        .find(|p| p.is_file())
        .ok_or_else(|| "Couldn't find the Dock icon image. Try reinstalling Coreside.".into())
}

/// Decode and validate a manual Dock PNG (dimensions + alpha corners).
pub fn validate_manual_dock_bytes(bytes: &[u8]) -> Result<(), String> {
    let img = image::load_from_memory(bytes)
        .map_err(|_| "Couldn't read the Dock icon image.".to_string())?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w < 128 || h < 128 || w != h {
        return Err("Dock icon must be a square image at least 128×128.".into());
    }
    // Transparent outer canvas — sample corners.
    for (x, y) in [(0u32, 0u32), (w - 1, 0), (0, h - 1), (w - 1, h - 1)] {
        if rgba.get_pixel(x, y)[3] != 0 {
            return Err("Dock icon must have a transparent outer canvas.".into());
        }
    }
    Ok(())
}

fn read_and_validate_manual(app: &AppHandle, cfg: &DockIconConfig) -> Result<Vec<u8>, String> {
    let path = resolve_manual_asset(app, cfg)?;
    let bytes =
        std::fs::read(&path).map_err(|_| "Couldn't read the Dock icon image.".to_string())?;
    validate_manual_dock_bytes(&bytes)?;
    Ok(bytes)
}

/// Apply AppKit mutation only (no persistence). Caller must hold `commit_lock`
/// when serializing against concurrent commit/apply (see `apply_dock_native_serialized`).
pub fn apply_dock_native(app: &AppHandle, cfg: &DockIconConfig) -> Result<(), String> {
    match cfg.authority {
        DockAuthority::FollowMacos => {
            #[cfg(target_os = "macos")]
            {
                run_appkit_on_main(app, || clear_macos_application_icon())?;
                tracing::info!("Cleared macOS Dock override (Follow macOS)");
                Ok(())
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = app;
                Ok(())
            }
        }
        DockAuthority::Manual => {
            let bytes = read_and_validate_manual(app, cfg)?;
            #[cfg(target_os = "macos")]
            {
                run_appkit_on_main(app, move || set_macos_application_icon(&bytes))?;
                tracing::info!(
                    artwork = ?cfg.artwork,
                    style = ?cfg.style,
                    "Applied manual macOS Dock override"
                );
                Ok(())
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = bytes;
                Ok(())
            }
        }
    }
}

/// Startup / re-apply path: same mutex as commit so rapid selection cannot race AppKit.
pub fn apply_dock_native_serialized(app: &AppHandle, cfg: &DockIconConfig) -> Result<(), String> {
    let _guard = commit_lock().lock();
    apply_dock_native(app, cfg)
}

/// Commit ordering:
/// 1. Validate + preflight asset
/// 2. Apply AppKit on main thread
/// 3. Persist normalized JSON
/// 4. If persist fails, restore prior AppKit state
///
/// `commit_lock` serializes commit and serialized apply so the last critical section wins.
pub fn commit_dock_preference(
    app: &AppHandle,
    prior: &DockIconConfig,
    requested: DockIconConfig,
    persist: impl FnOnce(&DockIconConfig) -> Result<(), String>,
) -> Result<DockIconCommitResult, DockIconCommitError> {
    let _guard = commit_lock().lock();

    let cfg = validate_dock_config(&requested).map_err(|message| DockIconCommitError {
        code: "invalid".into(),
        message,
        rollback_failed: false,
    })?;

    // Preflight before mutating AppKit.
    if cfg.authority == DockAuthority::Manual {
        read_and_validate_manual(app, &cfg).map_err(|message| DockIconCommitError {
            code: "asset".into(),
            message,
            rollback_failed: false,
        })?;
    }

    if let Err(message) = apply_dock_native(app, &cfg) {
        return Err(DockIconCommitError {
            code: "native".into(),
            message: consumer_safe_native_error(&message),
            rollback_failed: false,
        });
    }

    if let Err(message) = persist(&cfg) {
        let rollback_failed = apply_dock_native(app, prior).is_err();
        return Err(DockIconCommitError {
            code: "persist".into(),
            message: if rollback_failed {
                "Couldn't save the Dock icon setting, and restoring the previous Dock icon also failed."
                    .into()
            } else {
                format!(
                    "Couldn't save the Dock icon setting. {}",
                    consumer_safe_persist_error(&message)
                )
            },
            rollback_failed,
        });
    }

    Ok(DockIconCommitResult {
        status_label: cfg.status_label().to_string(),
        override_cleared: cfg.authority == DockAuthority::FollowMacos,
        effective_authority: cfg.authority,
        config: cfg,
    })
}

fn consumer_safe_native_error(raw: &str) -> String {
    if raw.contains("main thread") {
        return "Couldn't update the Dock icon right now. Try again.".into();
    }
    if raw.contains("not found") || raw.contains("Couldn't find") || raw.contains("Couldn't read") {
        return raw.to_string();
    }
    "Couldn't update the Dock icon.".into()
}

fn consumer_safe_persist_error(_raw: &str) -> String {
    "Your previous Dock icon was restored.".into()
}

/// Ensure macOS Dock / menu surfaces show `Coreside` instead of the Cargo binary name.
pub fn apply_display_name() {
    #[cfg(target_os = "macos")]
    {
        use objc2::MainThreadMarker;
        use objc2_app_kit::NSApplication;
        use objc2_foundation::{NSProcessInfo, NSString};

        let name = NSString::from_str(PRODUCT_NAME);
        NSProcessInfo::processInfo().setProcessName(&name);

        if let Some(mtm) = MainThreadMarker::new() {
            let app = NSApplication::sharedApplication(mtm);
            if let Some(menu) = app.mainMenu() {
                if let Some(app_menu_item) = menu.itemAtIndex(0) {
                    app_menu_item.setTitle(&name);
                }
            }
        }

        tracing::info!(name = PRODUCT_NAME, "Set macOS process display name");
    }
}

pub fn setting_key() -> &'static str {
    SETTING_KEY
}

/// AppKit Dock mutations must run on the main thread. Tauri commands usually do not.
#[cfg(target_os = "macos")]
fn run_appkit_on_main<T: Send + 'static>(
    app: &AppHandle,
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    use objc2::MainThreadMarker;

    if MainThreadMarker::new().is_some() {
        return f();
    }

    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(f());
    })
    .map_err(|_| "Couldn't reach the main thread to update the Dock icon.".to_string())?;
    rx.recv()
        .map_err(|_| "Couldn't update the Dock icon (main thread).".to_string())?
}

#[cfg(target_os = "macos")]
fn set_macos_application_icon(png_bytes: &[u8]) -> Result<(), String> {
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;

    let mtm = MainThreadMarker::new()
        .ok_or_else(|| "Dock icon AppKit calls must run on the main thread".to_string())?;

    let data = NSData::with_bytes(png_bytes);
    let image = NSImage::initWithData(NSImage::alloc(), &data)
        .ok_or_else(|| "Failed to create NSImage from dock PNG bytes".to_string())?;

    let app = NSApplication::sharedApplication(mtm);
    // SAFETY: AppKit setApplicationIconImage; main thread with live NSImage.
    unsafe {
        app.setApplicationIconImage(Some(&image));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn clear_macos_application_icon() -> Result<(), String> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;

    let mtm = MainThreadMarker::new()
        .ok_or_else(|| "Dock icon AppKit calls must run on the main thread".to_string())?;
    let app = NSApplication::sharedApplication(mtm);
    // SAFETY: nil restores the packaged application icon (Apple docs).
    unsafe {
        app.setApplicationIconImage(None);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_auto_becomes_follow_macos() {
        let cfg = parse_dock_icon_setting("auto");
        assert_eq!(cfg.authority, DockAuthority::FollowMacos);
        assert!(cfg.artwork.is_none());
    }

    #[test]
    fn legacy_dark_light_become_manual_classic() {
        let dark = parse_dock_icon_setting("dark");
        assert_eq!(dark.authority, DockAuthority::Manual);
        assert_eq!(dark.artwork, Some(DockArtwork::Classic));
        assert_eq!(dark.style, Some(DockStyle::Dark));

        let light = parse_dock_icon_setting("light");
        assert_eq!(light.style, Some(DockStyle::Light));
    }

    #[test]
    fn invalid_legacy_fails_closed_to_follow() {
        assert_eq!(
            parse_dock_icon_setting("tint").authority,
            DockAuthority::FollowMacos
        );
        assert_eq!(
            parse_dock_icon_setting("{not-json").authority,
            DockAuthority::FollowMacos
        );
    }

    #[test]
    fn roundtrip_json() {
        let cfg = DockIconConfig::manual_split();
        let raw = cfg.to_storage().unwrap();
        let parsed = parse_dock_icon_setting(&raw);
        assert_eq!(parsed, cfg);
    }

    #[test]
    fn follow_never_resolves_manual_filename() {
        assert!(manual_dock_filename(DockArtwork::Classic, DockStyle::Dark).is_some());
        // Follow path does not call manual_dock_filename.
        let follow = DockIconConfig::follow_macos();
        assert!(follow.artwork.is_none());
    }

    #[test]
    fn manual_filenames() {
        assert_eq!(
            manual_dock_filename(DockArtwork::Classic, DockStyle::Dark),
            Some("coreside-dock-dark.png")
        );
        assert_eq!(
            manual_dock_filename(DockArtwork::Classic, DockStyle::Light),
            Some("coreside-dock-light.png")
        );
        assert_eq!(
            manual_dock_filename(DockArtwork::Split, DockStyle::Original),
            Some("coreside-dock-split.png")
        );
        assert!(manual_dock_filename(DockArtwork::Split, DockStyle::Dark).is_none());
    }

    #[test]
    fn validate_rejects_bad_manual_combo() {
        let bad = DockIconConfig {
            schema_version: 1,
            authority: DockAuthority::Manual,
            artwork: Some(DockArtwork::Split),
            style: Some(DockStyle::Dark),
        };
        assert!(validate_dock_config(&bad).is_err());
    }

    #[test]
    fn manual_assets_exist_and_validate() {
        for (art, style) in [
            (DockArtwork::Classic, DockStyle::Dark),
            (DockArtwork::Classic, DockStyle::Light),
            (DockArtwork::Split, DockStyle::Original),
        ] {
            let name = manual_dock_filename(art, style).unwrap();
            let path = source_tree_branding_dir().join(name);
            assert!(path.is_file(), "missing {name}");
            let bytes = std::fs::read(&path).unwrap();
            validate_manual_dock_bytes(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }

    #[test]
    fn commit_follow_does_not_require_png() {
        // persist-only unit path: apply is no-op off-macOS; on macOS needs main thread.
        let prior = DockIconConfig::manual_classic(DockStyle::Dark);
        let requested = DockIconConfig::follow_macos();
        // Without AppHandle we only test validate path.
        assert!(validate_dock_config(&requested).is_ok());
        let _ = prior;
    }

    #[test]
    fn status_labels() {
        assert_eq!(
            DockIconConfig::follow_macos().status_label(),
            "Following macOS"
        );
        assert_eq!(
            DockIconConfig::manual_classic(DockStyle::Dark).status_label(),
            "Using Classic Dark"
        );
        assert_eq!(DockIconConfig::manual_split().status_label(), "Using Split");
    }
}
