//! Protected Coreside branding and macOS Dock icon authority.
//!
//! Follow macOS clears the temporary AppKit override (`applicationIconImage(None)`)
//! so the packaged application icon is authoritative again. Adaptive Icon & Widget
//! Style requires a genuine `Assets.car` in the installed app bundle (`CFBundleIconName`).
//! Manual mode installs a temporary PNG override for Classic or Split artwork.
//!
//! Product note: `MANUAL_DOCK_ICON_SELECTION_ENABLED` defaults false — manual
//! selection is dormant in the consumer product but the implementation remains.

use std::path::PathBuf;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// Public product name shown in Dock, menus, and About surfaces.
pub const PRODUCT_NAME: &str = "Coreside";

/// Source-level product capability. When false, runtime authority is always
/// Follow macOS regardless of any stored manual preference. Not user-configurable.
pub const MANUAL_DOCK_ICON_SELECTION_ENABLED: bool = false;

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
            if !MANUAL_DOCK_ICON_SELECTION_ENABLED {
                // Product dormancy: normalize manual requests to Follow macOS so
                // hidden IPC cannot activate a PNG override while the capability
                // is off. Persist path then stores Follow macOS.
                return Ok(DockIconConfig::follow_macos());
            }
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

/// Runtime authority while the product capability may be dormant.
/// Stored prefs may still parse as manual for dormant-system integrity tests;
/// effective presentation always follows macOS when the capability is off.
pub fn effective_dock_config_for_runtime(cfg: DockIconConfig) -> DockIconConfig {
    if !MANUAL_DOCK_ICON_SELECTION_ENABLED {
        return DockIconConfig::follow_macos();
    }
    normalize_dock_config(cfg)
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
    // Packaged/release builds must not fall back to CARGO_MANIFEST_DIR — that
    // path is a build-machine artifact and can mask missing bundled resources.
    if cfg!(debug_assertions) {
        candidates.push(source_tree_branding_dir().join(filename));
    }
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

/// `…/Something.app/Contents/Resources` when `exe` is the bundle binary.
pub fn macos_app_bundle_resources_dir(exe: &std::path::Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    if macos.file_name()?.to_str()? != "MacOS" {
        return None;
    }
    let contents = macos.parent()?;
    if contents.file_name()?.to_str()? != "Contents" {
        return None;
    }
    let bundle = contents.parent()?;
    if bundle.extension()?.to_str()? != "app" {
        return None;
    }
    Some(contents.join("Resources"))
}

/// True when this process is a real `.app` with a packaged Dock icon resource.
/// Unpackaged `tauri dev` / `npm run dev:raw` binaries have none — clearing
/// AppKit then shows `exec`.
pub fn packaged_macos_icon_available() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let Some(resources) = macos_app_bundle_resources_dir(&exe) else {
        return false;
    };
    ["Assets.car", "icon.icns", "AppIcon.icns"]
        .iter()
        .any(|name| resources.join(name).is_file())
}

/// True when this process is inside a real `.app` that has adaptive `Assets.car`
/// and `CFBundleIconName=Icon` in the adjacent Info.plist.
///
/// Distinct from [`packaged_macos_icon_available`]: static icns-only bundles are
/// packaged but not adaptive. macOS still owns adaptive rendering; this is for
/// diagnostics / evidence classification.
pub fn packaged_macos_adaptive_icon_available() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    packaged_macos_adaptive_icon_available_for_exe(&exe)
}

/// Testable form of [`packaged_macos_adaptive_icon_available`].
pub fn packaged_macos_adaptive_icon_available_for_exe(exe: &std::path::Path) -> bool {
    let Some(resources) = macos_app_bundle_resources_dir(exe) else {
        return false;
    };
    if !resources.join("Assets.car").is_file() {
        return false;
    }
    let Some(contents) = resources.parent() else {
        return false;
    };
    let plist_path = contents.join("Info.plist");
    let Ok(plist) = std::fs::read_to_string(&plist_path) else {
        return false;
    };
    plist_has_cf_bundle_icon_name_icon(&plist)
}

/// True when Info.plist sets `CFBundleIconName` to exactly `Icon`.
fn plist_has_cf_bundle_icon_name_icon(plist: &str) -> bool {
    let Some(after_key) = plist.split("<key>CFBundleIconName</key>").nth(1) else {
        return false;
    };
    after_key
        .trim_start()
        .starts_with("<string>Icon</string>")
}

/// Apply AppKit mutation only (no persistence). Returns whether the temporary
/// override was cleared (Follow macOS inside a packaged `.app`).
///
/// Caller must hold `commit_lock` when serializing against concurrent
/// commit/apply (see `apply_dock_native_serialized`).
///
/// While `MANUAL_DOCK_ICON_SELECTION_ENABLED` is false, any request is forced
/// to Follow macOS so stale persisted manual prefs cannot keep a PNG override.
pub fn apply_dock_native(app: &AppHandle, cfg: &DockIconConfig) -> Result<bool, String> {
    let cfg = effective_dock_config_for_runtime(cfg.clone());
    match cfg.authority {
        DockAuthority::FollowMacos => {
            #[cfg(target_os = "macos")]
            {
                // ponytail: unpackaged binaries have no CFBundle icon — clearing
                // AppKit yields the generic `exec` Dock tile. Stand in Classic Dark
                // (same primary as packaged icns) until a real .app is launched.
                // Default `npm run dev` on macOS uses the packaged development
                // runner so this stand-in should not activate there.
                if !packaged_macos_icon_available() {
                    let stand_in = DockIconConfig::manual_classic(DockStyle::Dark);
                    let bytes = read_and_validate_manual(app, &stand_in)?;
                    run_appkit_on_main(app, move || set_macos_application_icon(&bytes))?;
                    tracing::info!(
                        "Follow macOS: unpackaged process — Classic Dark stand-in (avoid exec icon)"
                    );
                    return Ok(false);
                }
                let adaptive = packaged_macos_adaptive_icon_available();
                run_appkit_on_main(app, || clear_macos_application_icon())?;
                if adaptive {
                    tracing::info!(
                        "Cleared macOS Dock override (Follow macOS, packaged adaptive Assets.car)"
                    );
                } else {
                    tracing::info!(
                        "Cleared macOS Dock override (Follow macOS, packaged static icon)"
                    );
                }
                Ok(true)
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = app;
                Ok(true)
            }
        }
        DockAuthority::Manual => {
            let bytes = read_and_validate_manual(app, &cfg)?;
            #[cfg(target_os = "macos")]
            {
                run_appkit_on_main(app, move || set_macos_application_icon(&bytes))?;
                tracing::info!(
                    artwork = ?cfg.artwork,
                    style = ?cfg.style,
                    "Applied manual macOS Dock override"
                );
                Ok(false)
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = bytes;
                Ok(false)
            }
        }
    }
}

/// Startup / re-apply path: same mutex as commit so rapid selection cannot race AppKit.
pub fn apply_dock_native_serialized(app: &AppHandle, cfg: &DockIconConfig) -> Result<bool, String> {
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

    let override_cleared = match apply_dock_native(app, &cfg) {
        Ok(cleared) => cleared,
        Err(message) => {
            return Err(DockIconCommitError {
                code: "native".into(),
                message: consumer_safe_native_error(&message),
                rollback_failed: false,
            });
        }
    };

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
        override_cleared,
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
    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use std::io::Cursor;

    /// Synthetic dock PNG: square canvas with controllable corner alpha.
    fn rgba_png(w: u32, h: u32, corner_alpha: u8) -> Vec<u8> {
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_fn(w, h, |x, y| {
            let at_corner = (x == 0 || x == w - 1) && (y == 0 || y == h - 1);
            Rgba([10, 20, 30, if at_corner { corner_alpha } else { 255 }])
        });
        let mut bytes = Vec::new();
        DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn manual_capability_defaults_disabled() {
        assert!(
            !MANUAL_DOCK_ICON_SELECTION_ENABLED,
            "product default must keep manual Dock selection dormant"
        );
    }

    #[test]
    fn effective_runtime_ignores_stale_manual_when_disabled() {
        let stale = DockIconConfig::manual_classic(DockStyle::Dark);
        let effective = effective_dock_config_for_runtime(stale);
        assert_eq!(effective.authority, DockAuthority::FollowMacos);
        assert!(effective.artwork.is_none());
    }

    #[test]
    fn validate_normalizes_manual_to_follow_when_capability_disabled() {
        let split = DockIconConfig::manual_split();
        let validated = validate_dock_config(&split).unwrap();
        if MANUAL_DOCK_ICON_SELECTION_ENABLED {
            assert_eq!(validated.authority, DockAuthority::Manual);
        } else {
            assert_eq!(validated.authority, DockAuthority::FollowMacos);
        }
    }

    #[test]
    fn macos_bundle_resources_dir_requires_app_layout() {
        use std::path::Path;
        assert!(macos_app_bundle_resources_dir(Path::new("/tmp/target/debug/Coreside")).is_none());
        assert_eq!(
            macos_app_bundle_resources_dir(Path::new(
                "/Applications/Coreside.app/Contents/MacOS/Coreside"
            )),
            Some(PathBuf::from(
                "/Applications/Coreside.app/Contents/Resources"
            ))
        );
    }

    #[test]
    fn adaptive_packaged_icon_requires_assets_car_and_icon_name() {
        use std::io::Write;
        let root = std::env::temp_dir().join(format!(
            "coreside-adaptive-icon-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let macos = root.join("Coreside.app/Contents/MacOS");
        let resources = root.join("Coreside.app/Contents/Resources");
        std::fs::create_dir_all(&macos).unwrap();
        std::fs::create_dir_all(&resources).unwrap();
        let exe = macos.join("Coreside");
        std::fs::write(&exe, b"x").unwrap();

        // Static icns only → packaged icon yes, adaptive no.
        std::fs::write(resources.join("icon.icns"), b"icns").unwrap();
        assert!(!packaged_macos_adaptive_icon_available_for_exe(&exe));

        // Assets.car without plist Icon name → not adaptive.
        std::fs::write(resources.join("Assets.car"), b"car").unwrap();
        let plist_path = root.join("Coreside.app/Contents/Info.plist");
        {
            let mut f = std::fs::File::create(&plist_path).unwrap();
            writeln!(
                f,
                r#"<?xml version="1.0"?><plist><dict><key>CFBundleName</key><string>Coreside</string></dict></plist>"#
            )
            .unwrap();
        }
        assert!(!packaged_macos_adaptive_icon_available_for_exe(&exe));

        // Full adaptive evidence.
        {
            let mut f = std::fs::File::create(&plist_path).unwrap();
            writeln!(
                f,
                r#"<?xml version="1.0"?><plist><dict><key>CFBundleIconName</key><string>Icon</string></dict></plist>"#
            )
            .unwrap();
        }
        assert!(packaged_macos_adaptive_icon_available_for_exe(&exe));

        // Wrong CFBundleIconName value → not adaptive (release/debug both need Icon).
        {
            let mut f = std::fs::File::create(&plist_path).unwrap();
            writeln!(
                f,
                r#"<?xml version="1.0"?><plist><dict><key>CFBundleIconName</key><string>AppIcon</string></dict></plist>"#
            )
            .unwrap();
        }
        assert!(!packaged_macos_adaptive_icon_available_for_exe(&exe));

        // Unrelated <string>Icon</string> must not satisfy CFBundleIconName=Icon.
        {
            let mut f = std::fs::File::create(&plist_path).unwrap();
            writeln!(
                f,
                r#"<?xml version="1.0"?><plist><dict><key>CFBundleName</key><string>Icon</string><key>CFBundleIconName</key><string>AppIcon</string></dict></plist>"#
            )
            .unwrap();
        }
        assert!(!packaged_macos_adaptive_icon_available_for_exe(&exe));
        assert!(!plist_has_cf_bundle_icon_name_icon(
            &std::fs::read_to_string(&plist_path).unwrap()
        ));

        // Missing Info.plist → not adaptive even with Assets.car.
        std::fs::remove_file(&plist_path).unwrap();
        assert!(!packaged_macos_adaptive_icon_available_for_exe(&exe));

        // Unpackaged binary → never adaptive.
        assert!(!packaged_macos_adaptive_icon_available_for_exe(
            std::path::Path::new("/tmp/target/debug/Coreside")
        ));

        let _ = std::fs::remove_dir_all(&root);
    }

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
    fn follow_macos_never_resolves_a_png_filename() {
        // Follow strips any leftover artwork/style — no manual PNG applies.
        let follow = validate_dock_config(&DockIconConfig {
            schema_version: 1,
            authority: DockAuthority::FollowMacos,
            artwork: Some(DockArtwork::Split),
            style: Some(DockStyle::Original),
        })
        .unwrap();
        assert_eq!(follow.authority, DockAuthority::FollowMacos);
        assert!(follow.artwork.is_none());
        assert!(follow.style.is_none());
    }

    #[test]
    fn manual_split_still_resolves_dock_split_png() {
        assert_eq!(
            manual_dock_filename(DockArtwork::Split, DockStyle::Original),
            Some("coreside-dock-split.png")
        );
        // Exercise mapping from the dormant manual config constructors directly —
        // validate_dock_config normalizes to Follow macOS while the product gate is off.
        let cfg = DockIconConfig::manual_split();
        let name = manual_dock_filename(cfg.artwork.unwrap(), cfg.style.unwrap()).unwrap();
        assert_eq!(name, "coreside-dock-split.png");
        assert!(source_tree_branding_dir().join(name).is_file());
    }

    #[test]
    fn dock_icon_candidates_source_tree_only_in_debug() {
        let name = "coreside-dock-dark.png";
        let candidates = dock_icon_candidates(None, name);
        let source = source_tree_branding_dir().join(name);
        if cfg!(debug_assertions) {
            assert!(
                candidates.iter().any(|p| p == &source),
                "debug builds may resolve repo branding resources"
            );
        } else {
            assert!(
                candidates.is_empty(),
                "release builds must not fall back to CARGO_MANIFEST_DIR without an AppHandle"
            );
        }
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
        assert!(manual_dock_filename(DockArtwork::Split, DockStyle::Dark).is_none());
    }

    #[test]
    fn validate_rejects_bad_manual_combo() {
        let split_dark = DockIconConfig {
            schema_version: 1,
            authority: DockAuthority::Manual,
            artwork: Some(DockArtwork::Split),
            style: Some(DockStyle::Dark),
        };
        let classic_original = DockIconConfig {
            schema_version: 1,
            authority: DockAuthority::Manual,
            artwork: Some(DockArtwork::Classic),
            style: Some(DockStyle::Original),
        };
        let missing_artwork = DockIconConfig {
            schema_version: 1,
            authority: DockAuthority::Manual,
            artwork: None,
            style: Some(DockStyle::Dark),
        };
        if MANUAL_DOCK_ICON_SELECTION_ENABLED {
            assert!(validate_dock_config(&split_dark).is_err());
            assert!(validate_dock_config(&classic_original).is_err());
            assert!(validate_dock_config(&missing_artwork).is_err());
        } else {
            // Product dormancy normalizes all manual requests to Follow macOS.
            assert_eq!(
                validate_dock_config(&split_dark).unwrap().authority,
                DockAuthority::FollowMacos
            );
            assert_eq!(
                validate_dock_config(&classic_original).unwrap().authority,
                DockAuthority::FollowMacos
            );
            assert_eq!(
                validate_dock_config(&missing_artwork).unwrap().authority,
                DockAuthority::FollowMacos
            );
        }
    }

    #[test]
    fn validate_manual_dock_bytes_rejects_non_square() {
        let bytes = rgba_png(128, 160, 0);
        let err = validate_manual_dock_bytes(&bytes).unwrap_err();
        assert!(err.contains("square"), "{err}");
    }

    #[test]
    fn validate_manual_dock_bytes_rejects_undersized() {
        let bytes = rgba_png(64, 64, 0);
        let err = validate_manual_dock_bytes(&bytes).unwrap_err();
        assert!(err.contains("128"), "{err}");
    }

    #[test]
    fn validate_manual_dock_bytes_rejects_opaque_corners() {
        let bytes = rgba_png(128, 128, 255);
        let err = validate_manual_dock_bytes(&bytes).unwrap_err();
        assert!(err.contains("transparent"), "{err}");
    }

    #[test]
    fn validate_manual_dock_bytes_accepts_transparent_square() {
        validate_manual_dock_bytes(&rgba_png(128, 128, 0)).unwrap();
    }

    #[test]
    fn validate_manual_dock_bytes_rejects_garbage() {
        assert!(validate_manual_dock_bytes(b"not-a-png").is_err());
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
        // Follow validates without resolving a manual PNG filename.
        let requested = DockIconConfig::follow_macos();
        assert!(validate_dock_config(&requested).is_ok());
        assert!(requested.artwork.is_none());
    }

    #[test]
    fn native_errors_are_consumer_safe() {
        assert_eq!(
            consumer_safe_native_error("Dock icon AppKit calls must run on the main thread"),
            "Couldn't update the Dock icon right now. Try again."
        );
        assert_eq!(
            consumer_safe_native_error(
                "Couldn't find the Dock icon image. Try reinstalling Coreside."
            ),
            "Couldn't find the Dock icon image. Try reinstalling Coreside."
        );
        assert_eq!(
            consumer_safe_native_error("objc2::panic: null selector 0xdead"),
            "Couldn't update the Dock icon."
        );
        assert_eq!(
            consumer_safe_persist_error("SQLITE_BUSY: database is locked"),
            "Your previous Dock icon was restored."
        );
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
