//! Protected Coreside branding and macOS Dock icon authority.
//!
//! Follow macOS clears the temporary AppKit override (`applicationIconImage(None)`)
//! inside a real `.app` bundle so the packaged application icon is authoritative.
//! In an unbundled development environment (`tauri dev`), clearing AppKit exposes
//! macOS's generic Unix-executable ("exec") icon; Follow macOS therefore applies
//! a protected development preview PNG (Classic Dark fallback, matching packaged ICNS)
//! while truthfully reporting that the runtime is degraded to a development fallback.
//!
//! Manual mode installs a validated protected PNG override for Classic Dark,
//! Classic Light, or Split artwork.

use std::path::{Path, PathBuf};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// Public product name shown in Dock, menus, and About surfaces.
pub const PRODUCT_NAME: &str = "Coreside";

/// Source-level product capability. When true, manual Dock icon selection
/// (Classic Dark, Classic Light, Split) is active in Settings and runtime.
pub const MANUAL_DOCK_ICON_SELECTION_ENABLED: bool = true;

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

/// Effective presentation mode in the macOS Dock tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveDockPresentation {
    PackagedAdaptive,
    PackagedStatic,
    DevelopmentFallback,
    Manual,
}

impl EffectiveDockPresentation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PackagedAdaptive => "packaged_adaptive",
            Self::PackagedStatic => "packaged_static",
            Self::DevelopmentFallback => "development_fallback",
            Self::Manual => "manual",
        }
    }
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

/// Result returned after a committed Dock mutation or status query.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DockIconCommitResult {
    pub config: DockIconConfig,
    pub status_label: String,
    pub effective_authority: DockAuthority,
    pub effective_presentation: EffectiveDockPresentation,
    /// True when Follow macOS cleared the temporary override (only inside a packaged .app).
    pub override_cleared: bool,
    pub adaptive_capable: bool,
    pub development_fallback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockIconCommitError {
    pub code: String,
    pub message: String,
    pub rollback_failed: bool,
}

pub fn commit_lock() -> &'static Mutex<()> {
    static LOCK: Mutex<()> = Mutex::new(());
    &LOCK
}

/// Return truthful, consumer-facing status text for a given presentation mode.
pub fn consumer_status_label(
    presentation: EffectiveDockPresentation,
    cfg: &DockIconConfig,
) -> &'static str {
    match presentation {
        EffectiveDockPresentation::PackagedAdaptive => "Following macOS Icon & Widget Style",
        EffectiveDockPresentation::PackagedStatic => {
            "Using Coreside’s packaged icon. Adaptive icon styles aren’t included in this build"
        }
        EffectiveDockPresentation::DevelopmentFallback => {
            "Follow macOS is selected. Development preview uses a fixed Coreside icon because this process isn’t running from an app bundle"
        }
        EffectiveDockPresentation::Manual => match (cfg.artwork, cfg.style) {
            (Some(DockArtwork::Classic), Some(DockStyle::Dark)) => "Using Classic Dark",
            (Some(DockArtwork::Classic), Some(DockStyle::Light)) => "Using Classic Light",
            (Some(DockArtwork::Split), _) => "Using Split",
            _ => "Using manual Dock icon",
        },
    }
}

/// Strict parse of legacy keywords or versioned JSON. Returns None if unparseable/invalid.
pub fn parse_dock_icon_setting_strict(raw: &str) -> Option<DockIconConfig> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_lowercase();
    match lower.as_str() {
        "auto" | "follow_macos" | "system" => return Some(DockIconConfig::follow_macos()),
        "dark" => return Some(DockIconConfig::manual_classic(DockStyle::Dark)),
        "light" => return Some(DockIconConfig::manual_classic(DockStyle::Light)),
        "split" => return Some(DockIconConfig::manual_split()),
        _ => {}
    }
    if let Ok(cfg) = serde_json::from_str::<DockIconConfig>(trimmed) {
        if cfg.schema_version == DOCK_SCHEMA_VERSION {
            return Some(normalize_dock_config(cfg));
        }
    }
    None
}

/// Parse legacy `auto`/`dark`/`light`/`split` or versioned JSON. Invalid → Follow macOS.
pub fn parse_dock_icon_setting(raw: &str) -> DockIconConfig {
    parse_dock_icon_setting_strict(raw).unwrap_or_else(DockIconConfig::follow_macos)
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

/// Strict validation used by the mutation command.
pub fn validate_dock_config(cfg: &DockIconConfig) -> Result<DockIconConfig, String> {
    if cfg.schema_version != DOCK_SCHEMA_VERSION {
        return Err("Unsupported Dock icon schema version".into());
    }
    match cfg.authority {
        DockAuthority::FollowMacos => Ok(DockIconConfig::follow_macos()),
        DockAuthority::Manual => {
            if !MANUAL_DOCK_ICON_SELECTION_ENABLED {
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

/// Runtime filename for a manual tile. Follow macOS never resolves a PNG directly.
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

/// Candidate asset paths considering packaging context.
/// Packaged `.app` execution must NEVER consult `CARGO_MANIFEST_DIR` (to prevent
/// checkout files masking missing bundle resources during testing).
pub fn dock_icon_candidates_with_context(
    app: Option<&AppHandle>,
    filename: &str,
    is_packaged: bool,
) -> Vec<PathBuf> {
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
    // Only unbundled development executables may fall back to the source tree.
    if !is_packaged {
        candidates.push(source_tree_branding_dir().join(filename));
    }
    candidates
}

pub fn dock_icon_candidates(app: Option<&AppHandle>, filename: &str) -> Vec<PathBuf> {
    dock_icon_candidates_with_context(app, filename, is_packaged_app())
}

pub fn resolve_manual_asset_with_context(
    app: Option<&AppHandle>,
    cfg: &DockIconConfig,
    is_packaged: bool,
) -> Result<PathBuf, String> {
    let artwork = cfg
        .artwork
        .ok_or_else(|| "Manual Dock preference is missing artwork".to_string())?;
    let style = cfg
        .style
        .ok_or_else(|| "Manual Dock preference is missing style".to_string())?;
    let filename = manual_dock_filename(artwork, style)
        .ok_or_else(|| "That Dock artwork combination is not available".to_string())?;
    let candidates = dock_icon_candidates_with_context(app, filename, is_packaged);
    candidates
        .into_iter()
        .find(|p| p.is_file())
        .ok_or_else(|| "Couldn't find the Dock icon image. Try reinstalling Coreside.".into())
}

/// Decode and validate a manual Dock PNG (dimensions + transparent corners).
pub fn validate_manual_dock_bytes(bytes: &[u8]) -> Result<(), String> {
    let img = image::load_from_memory(bytes)
        .map_err(|_| "Couldn't read the Dock icon image.".to_string())?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w < 128 || h < 128 || w != h {
        return Err("Dock icon must be a square image at least 128×128.".into());
    }
    // Transparent outer canvas — sample all 4 corners.
    for (x, y) in [(0u32, 0u32), (w - 1, 0), (0, h - 1), (w - 1, h - 1)] {
        if rgba.get_pixel(x, y)[3] != 0 {
            return Err("Dock icon must have a transparent outer canvas.".into());
        }
    }
    Ok(())
}

pub fn resolve_and_read_manual(
    app: Option<&AppHandle>,
    cfg: &DockIconConfig,
    is_packaged: bool,
) -> Result<Vec<u8>, String> {
    let path = resolve_manual_asset_with_context(app, cfg, is_packaged)?;
    let bytes =
        std::fs::read(&path).map_err(|_| "Couldn't read the Dock icon image.".to_string())?;
    validate_manual_dock_bytes(&bytes)?;
    Ok(bytes)
}

/// Path check for `.app` bundle resources.
pub fn macos_app_bundle_resources_dir(exe: &Path) -> Option<PathBuf> {
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

/// Helper to validate standard macOS `.app` directory structure.
pub fn is_valid_app_bundle_structure(bundle_path: &Path, exe: Option<&Path>) -> bool {
    if bundle_path.extension().and_then(|s| s.to_str()) != Some("app") {
        return false;
    }
    let contents = bundle_path.join("Contents");
    if !contents.join("Resources").is_dir() {
        return false;
    }
    if !contents.join("Info.plist").is_file() {
        return false;
    }
    if let Some(exe) = exe {
        let macos_dir = contents.join("MacOS");
        if !exe.starts_with(&macos_dir) {
            return false;
        }
    }
    true
}

/// Returns the running application's bundle URL if running inside a bundle.
#[cfg(target_os = "macos")]
pub fn running_application_bundle_url() -> Option<PathBuf> {
    use objc2_app_kit::NSRunningApplication;
    let app = NSRunningApplication::currentApplication();
    let url = app.bundleURL()?;
    let path = url.path()?;
    Some(PathBuf::from(path.to_string()))
}

#[cfg(not(target_os = "macos"))]
pub fn running_application_bundle_url() -> Option<PathBuf> {
    None
}

/// True when this process is running inside a verified `.app` bundle.
pub fn is_packaged_app() -> bool {
    #[cfg(target_os = "macos")]
    {
        let Some(bundle_path) = running_application_bundle_url() else {
            return false;
        };
        let exe = std::env::current_exe().ok();
        is_valid_app_bundle_structure(&bundle_path, exe.as_deref())
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// True when this process is a real `.app` with a packaged Dock icon resource.
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
pub fn packaged_macos_adaptive_icon_available() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    packaged_macos_adaptive_icon_available_for_exe(&exe)
}

pub fn packaged_macos_adaptive_icon_available_for_exe(exe: &Path) -> bool {
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

fn plist_has_cf_bundle_icon_name_icon(plist: &str) -> bool {
    let Some(after_key) = plist.split("<key>CFBundleIconName</key>").nth(1) else {
        return false;
    };
    after_key.trim_start().starts_with("<string>Icon</string>")
}

/// Native AppKit side-effect requested by the resolved plan.
pub enum NativeAction {
    ClearOverride,
    SetImage(Vec<u8>),
}

/// Pure runtime plan separating desired user authority from effective runtime presentation.
pub struct DockRuntimePlan {
    pub effective_config: DockIconConfig,
    pub effective_presentation: EffectiveDockPresentation,
    pub override_cleared: bool,
    pub adaptive_capable: bool,
    pub development_fallback: bool,
    pub status_label: String,
    pub action: NativeAction,
}

/// Pure plan resolution with injectable context for deterministic unit testing.
pub fn resolve_dock_runtime_plan_with_context(
    app: Option<&AppHandle>,
    proposed: &DockIconConfig,
    is_packaged: bool,
    is_adaptive: bool,
) -> Result<DockRuntimePlan, String> {
    let normalized = normalize_dock_config(proposed.clone());

    match normalized.authority {
        DockAuthority::FollowMacos => {
            if is_packaged {
                let presentation = if is_adaptive {
                    EffectiveDockPresentation::PackagedAdaptive
                } else {
                    EffectiveDockPresentation::PackagedStatic
                };
                let status_label = consumer_status_label(presentation, &normalized).to_string();
                Ok(DockRuntimePlan {
                    effective_config: normalized,
                    effective_presentation: presentation,
                    override_cleared: true,
                    adaptive_capable: is_adaptive,
                    development_fallback: false,
                    status_label,
                    action: NativeAction::ClearOverride,
                })
            } else {
                // Development fallback: apply deterministic Classic Dark preview PNG
                // without mutating the persisted Follow macOS preference.
                let stand_in = DockIconConfig::manual_classic(DockStyle::Dark);
                let bytes = resolve_and_read_manual(app, &stand_in, false)?;
                let presentation = EffectiveDockPresentation::DevelopmentFallback;
                let status_label = consumer_status_label(presentation, &normalized).to_string();
                Ok(DockRuntimePlan {
                    effective_config: normalized,
                    effective_presentation: presentation,
                    override_cleared: false,
                    adaptive_capable: false,
                    development_fallback: true,
                    status_label,
                    action: NativeAction::SetImage(bytes),
                })
            }
        }
        DockAuthority::Manual => {
            if !MANUAL_DOCK_ICON_SELECTION_ENABLED {
                // If capability were disabled, fall back to Follow macOS logic.
                return resolve_dock_runtime_plan_with_context(
                    app,
                    &DockIconConfig::follow_macos(),
                    is_packaged,
                    is_adaptive,
                );
            }
            let bytes = resolve_and_read_manual(app, &normalized, is_packaged)?;
            let presentation = EffectiveDockPresentation::Manual;
            let status_label = consumer_status_label(presentation, &normalized).to_string();
            Ok(DockRuntimePlan {
                effective_config: normalized,
                effective_presentation: presentation,
                override_cleared: false,
                adaptive_capable: false,
                development_fallback: false,
                status_label,
                action: NativeAction::SetImage(bytes),
            })
        }
    }
}

/// Resolve the active runtime plan against the live runtime environment.
pub fn resolve_dock_runtime_plan(
    app: Option<&AppHandle>,
    proposed: &DockIconConfig,
) -> Result<DockRuntimePlan, String> {
    let packaged = is_packaged_app();
    let adaptive = if packaged {
        packaged_macos_adaptive_icon_available()
    } else {
        false
    };
    resolve_dock_runtime_plan_with_context(app, proposed, packaged, adaptive)
}

/// Execute the native AppKit action on the macOS main thread.
pub fn execute_native_plan(app: &AppHandle, plan: &DockRuntimePlan) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        match &plan.action {
            NativeAction::ClearOverride => {
                run_appkit_on_main(app, || clear_macos_application_icon())
            }
            NativeAction::SetImage(bytes) => {
                let bytes_clone = bytes.clone();
                run_appkit_on_main(app, move || set_macos_application_icon(&bytes_clone))
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        let _ = plan;
        Ok(())
    }
}

/// Read persisted Dock preference directly from database under caller's lock.
pub fn read_persisted_dock_from_db(db: &rusqlite::Connection) -> Result<DockIconConfig, String> {
    let mut stmt = db
        .prepare("SELECT key, value FROM settings WHERE key IN ('dockIcon', 'dock_icon')")
        .map_err(|e| e.to_string())?;
    let mut canonical: Option<String> = None;
    let mut legacy: Option<String> = None;

    let rows = stmt
        .query_map([], |row| {
            let k: String = row.get(0)?;
            let v: String = row.get(1)?;
            Ok((k, v))
        })
        .map_err(|e| e.to_string())?;

    for row in rows {
        if let Ok((k, v)) = row {
            if k == SETTING_KEY {
                canonical = Some(v);
            } else if k == "dock_icon" {
                legacy = Some(v);
            }
        }
    }

    let cfg = match (canonical, legacy) {
        (Some(c), Some(l)) => {
            if let Some(valid_c) = parse_dock_icon_setting_strict(&c) {
                valid_c
            } else if let Some(valid_l) = parse_dock_icon_setting_strict(&l) {
                valid_l
            } else {
                parse_dock_icon_setting(&c)
            }
        }
        (Some(c), None) => parse_dock_icon_setting(&c),
        (None, Some(l)) => parse_dock_icon_setting(&l),
        (None, None) => DockIconConfig::follow_macos(),
    };
    Ok(cfg)
}

/// Commit ordering:
/// 1. Acquire coordinator lock
/// 2. Validate proposed preference
/// 3. Read prior setting under database lock
/// 4. Resolve runtime plan & preflight asset bytes
/// 5. Execute native AppKit mutation on main thread
/// 6. Persist to SQLite
/// 7. On persist error: restore AppKit state derived from prior setting
pub fn commit_dock_preference(
    app: &AppHandle,
    state: &crate::state::AppState,
    requested: DockIconConfig,
) -> Result<DockIconCommitResult, DockIconCommitError> {
    let _guard = commit_lock().lock();

    let cfg = validate_dock_config(&requested).map_err(|message| DockIconCommitError {
        code: "invalid".into(),
        message,
        rollback_failed: false,
    })?;

    let prior = {
        let db = state.db.lock();
        read_persisted_dock_from_db(db.conn()).map_err(|message| DockIconCommitError {
            code: "database".into(),
            message,
            rollback_failed: false,
        })?
    };

    let plan =
        resolve_dock_runtime_plan(Some(app), &cfg).map_err(|message| DockIconCommitError {
            code: "asset".into(),
            message,
            rollback_failed: false,
        })?;

    execute_native_plan(app, &plan).map_err(|message| DockIconCommitError {
        code: "native".into(),
        message: consumer_safe_native_error(&message),
        rollback_failed: false,
    })?;

    let stored = plan
        .effective_config
        .to_storage()
        .map_err(|message| DockIconCommitError {
            code: "persist".into(),
            message,
            rollback_failed: false,
        })?;

    {
        let mut db = state.db.lock();
        if let Err(err) = crate::db::set_setting(&mut db, SETTING_KEY, &stored) {
            tracing::error!(error = %err, "failed to persist dock preference; executing rollback");
            let rollback_result = (|| -> Result<(), String> {
                let prior_plan = resolve_dock_runtime_plan(Some(app), &prior)?;
                execute_native_plan(app, &prior_plan)
            })();
            let rollback_failed = rollback_result.is_err();
            return Err(DockIconCommitError {
                code: "persist".into(),
                message: if rollback_failed {
                    "Couldn't save the Dock icon setting, and restoring the previous Dock icon also failed.".into()
                } else {
                    format!(
                        "Couldn't save the Dock icon setting. {}",
                        consumer_safe_persist_error(&err.to_string())
                    )
                },
                rollback_failed,
            });
        }
    }

    let effective_authority = plan.effective_config.authority;
    Ok(DockIconCommitResult {
        config: plan.effective_config,
        status_label: plan.status_label,
        effective_authority,
        effective_presentation: plan.effective_presentation,
        override_cleared: plan.override_cleared,
        adaptive_capable: plan.adaptive_capable,
        development_fallback: plan.development_fallback,
    })
}

/// Process-global Dock reconciliation on startup / profile-ready.
/// Rust-owned: runs once per app launch/profile open, serializes via coordinator lock,
/// and reapplies the authoritative Dock tile before user interaction.
pub fn reconcile_dock_on_startup(
    app: &AppHandle,
    database: &parking_lot::Mutex<crate::db::Database>,
) -> Result<DockIconCommitResult, String> {
    let _guard = commit_lock().lock();

    let cfg = {
        let db = database.lock();
        read_persisted_dock_from_db(db.conn())?
    };
    let plan = resolve_dock_runtime_plan(Some(app), &cfg)?;

    execute_native_plan(app, &plan)?;

    tracing::info!(
        presentation = ?plan.effective_presentation,
        authority = ?plan.effective_config.authority,
        override_cleared = plan.override_cleared,
        "Reconciled macOS Dock icon at startup"
    );

    let effective_authority = plan.effective_config.authority;
    Ok(DockIconCommitResult {
        config: plan.effective_config,
        status_label: plan.status_label,
        effective_authority,
        effective_presentation: plan.effective_presentation,
        override_cleared: plan.override_cleared,
        adaptive_capable: plan.adaptive_capable,
        development_fallback: plan.development_fallback,
    })
}

/// Read-only status query from backend authority.
pub fn get_dock_icon_status(
    app: Option<&AppHandle>,
    state: &crate::state::AppState,
) -> Result<DockIconCommitResult, String> {
    let _guard = commit_lock().lock();

    let cfg = {
        let db = state.db.lock();
        read_persisted_dock_from_db(db.conn())?
    };
    let plan = resolve_dock_runtime_plan(app, &cfg)?;

    let effective_authority = plan.effective_config.authority;
    Ok(DockIconCommitResult {
        config: plan.effective_config,
        status_label: plan.status_label,
        effective_authority,
        effective_presentation: plan.effective_presentation,
        override_cleared: plan.override_cleared,
        adaptive_capable: plan.adaptive_capable,
        development_fallback: plan.development_fallback,
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

/// AppKit Dock mutations must run on the main thread.
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
    use objc2::AnyThread;
    use objc2::MainThreadMarker;
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
    fn manual_capability_is_enabled() {
        assert!(
            MANUAL_DOCK_ICON_SELECTION_ENABLED,
            "manual Dock selection must be enabled for Classic and Split options"
        );
    }

    #[test]
    fn plan_unbundled_follow_macos_selects_development_fallback() {
        let cfg = DockIconConfig::follow_macos();
        let plan = resolve_dock_runtime_plan_with_context(None, &cfg, false, false).unwrap();
        assert_eq!(
            plan.effective_presentation,
            EffectiveDockPresentation::DevelopmentFallback
        );
        assert_eq!(plan.effective_config.authority, DockAuthority::FollowMacos);
        assert!(
            !plan.override_cleared,
            "unbundled development must NOT clear override to nil (avoids generic exec icon)"
        );
        assert!(plan.development_fallback);
        assert!(!plan.adaptive_capable);
        assert!(
            plan.status_label
                .contains("Development preview uses a fixed Coreside icon"),
            "status label should explain development fallback truthfully"
        );
        match plan.action {
            NativeAction::SetImage(bytes) => {
                assert!(validate_manual_dock_bytes(&bytes).is_ok());
            }
            NativeAction::ClearOverride => {
                panic!("unbundled Follow macOS must not request ClearOverride");
            }
        }
    }

    #[test]
    fn plan_packaged_static_clears_override() {
        let cfg = DockIconConfig::follow_macos();
        let plan = resolve_dock_runtime_plan_with_context(None, &cfg, true, false).unwrap();
        assert_eq!(
            plan.effective_presentation,
            EffectiveDockPresentation::PackagedStatic
        );
        assert!(
            plan.override_cleared,
            "packaged static app must clear override so bundled icon is authoritative"
        );
        assert!(!plan.adaptive_capable);
        assert!(!plan.development_fallback);
        assert!(plan.status_label.contains("packaged icon"));
        match plan.action {
            NativeAction::ClearOverride => {}
            NativeAction::SetImage(_) => {
                panic!("packaged static app must request ClearOverride, not SetImage");
            }
        }
    }

    #[test]
    fn plan_packaged_adaptive_clears_override() {
        let cfg = DockIconConfig::follow_macos();
        let plan = resolve_dock_runtime_plan_with_context(None, &cfg, true, true).unwrap();
        assert_eq!(
            plan.effective_presentation,
            EffectiveDockPresentation::PackagedAdaptive
        );
        assert!(plan.override_cleared);
        assert!(plan.adaptive_capable);
        assert!(!plan.development_fallback);
        assert!(plan
            .status_label
            .contains("Following macOS Icon & Widget Style"));
        match plan.action {
            NativeAction::ClearOverride => {}
            NativeAction::SetImage(_) => {
                panic!("packaged adaptive app must request ClearOverride");
            }
        }
    }

    #[test]
    fn plan_manual_mode_requests_exact_protected_asset() {
        for (art, style, expected_name) in [
            (
                DockArtwork::Classic,
                DockStyle::Dark,
                "coreside-dock-dark.png",
            ),
            (
                DockArtwork::Classic,
                DockStyle::Light,
                "coreside-dock-light.png",
            ),
            (
                DockArtwork::Split,
                DockStyle::Original,
                "coreside-dock-split.png",
            ),
        ] {
            let cfg = DockIconConfig {
                schema_version: 1,
                authority: DockAuthority::Manual,
                artwork: Some(art),
                style: Some(style),
            };
            let plan = resolve_dock_runtime_plan_with_context(None, &cfg, false, false).unwrap();
            assert_eq!(
                plan.effective_presentation,
                EffectiveDockPresentation::Manual
            );
            assert_eq!(plan.effective_config.authority, DockAuthority::Manual);
            assert!(!plan.override_cleared);
            match plan.action {
                NativeAction::SetImage(bytes) => {
                    let expected_bytes =
                        std::fs::read(source_tree_branding_dir().join(expected_name)).unwrap();
                    assert_eq!(bytes, expected_bytes);
                }
                NativeAction::ClearOverride => panic!("manual mode must not ClearOverride"),
            }
        }
    }

    #[test]
    fn dock_icon_candidates_never_consults_cargo_manifest_dir_when_packaged() {
        let name = "coreside-dock-dark.png";
        let source = source_tree_branding_dir().join(name);

        // In packaged mode, even in debug builds, candidate list must NEVER include CARGO_MANIFEST_DIR
        let packaged_candidates = dock_icon_candidates_with_context(None, name, true);
        assert!(
            !packaged_candidates.iter().any(|p| p == &source),
            "packaged resolution must NEVER contain CARGO_MANIFEST_DIR path"
        );

        // In unbundled mode, candidate list includes CARGO_MANIFEST_DIR
        let unbundled_candidates = dock_icon_candidates_with_context(None, name, false);
        assert!(
            unbundled_candidates.iter().any(|p| p == &source),
            "unbundled resolution must allow CARGO_MANIFEST_DIR fallback"
        );
    }

    #[test]
    fn macos_bundle_resources_dir_requires_app_layout() {
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
    fn bundle_structure_validation() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("Coreside.app");
        let contents = app.join("Contents");
        let macos = contents.join("MacOS");
        let resources = contents.join("Resources");
        std::fs::create_dir_all(&macos).unwrap();
        std::fs::create_dir_all(&resources).unwrap();
        let exe = macos.join("Coreside");
        std::fs::write(&exe, b"bin").unwrap();
        let plist = contents.join("Info.plist");
        std::fs::write(&plist, b"<plist/>").unwrap();

        assert!(is_valid_app_bundle_structure(&app, Some(&exe)));

        // Bad exe path outside bundle
        assert!(!is_valid_app_bundle_structure(
            &app,
            Some(Path::new("/bin/ls"))
        ));

        // Missing Info.plist
        std::fs::remove_file(&plist).unwrap();
        assert!(!is_valid_app_bundle_structure(&app, Some(&exe)));
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

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn legacy_auto_becomes_follow_macos() {
        let cfg = parse_dock_icon_setting("auto");
        assert_eq!(cfg.authority, DockAuthority::FollowMacos);
        assert!(cfg.artwork.is_none());
    }

    #[test]
    fn legacy_split_becomes_manual_split() {
        let cfg = parse_dock_icon_setting("split");
        assert_eq!(cfg.authority, DockAuthority::Manual);
        assert_eq!(cfg.artwork, Some(DockArtwork::Split));
        assert_eq!(cfg.style, Some(DockStyle::Original));
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
    fn manual_split_resolves_dock_split_png() {
        assert_eq!(
            manual_dock_filename(DockArtwork::Split, DockStyle::Original),
            Some("coreside-dock-split.png")
        );
        let cfg = DockIconConfig::manual_split();
        let name = manual_dock_filename(cfg.artwork.unwrap(), cfg.style.unwrap()).unwrap();
        assert_eq!(name, "coreside-dock-split.png");
        assert!(source_tree_branding_dir().join(name).is_file());
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
        assert!(validate_dock_config(&split_dark).is_err());
        assert!(validate_dock_config(&classic_original).is_err());
        assert!(validate_dock_config(&missing_artwork).is_err());
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
    fn deterministic_dual_alias_db_resolution() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test_aliases.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch("CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);")
            .unwrap();

        // 1. Both exist, canonical valid -> canonical wins
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('dockIcon', 'split')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('dock_icon', 'dark')",
            [],
        )
        .unwrap();
        let resolved = read_persisted_dock_from_db(&conn).unwrap();
        assert_eq!(resolved.artwork, Some(DockArtwork::Split));

        // 2. Canonical malformed, legacy valid -> legacy wins
        conn.execute(
            "UPDATE settings SET value = '{bad-json' WHERE key = 'dockIcon'",
            [],
        )
        .unwrap();
        let resolved2 = read_persisted_dock_from_db(&conn).unwrap();
        assert_eq!(resolved2.artwork, Some(DockArtwork::Classic));
        assert_eq!(resolved2.style, Some(DockStyle::Dark));
    }
}
