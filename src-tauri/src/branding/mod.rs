//! macOS dock icon switching and display-name branding.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

/// Public product name shown in Dock, menus, and About surfaces.
pub const PRODUCT_NAME: &str = "Coreside";

/// Dock icon variant filenames.
/// - `dark` → dark-background tile with white mark (default for OS Light)
/// - `light` → light-background tile with black mark (default for OS Dark)
pub fn dock_icon_filename(variant: &str) -> &'static str {
    match variant {
        "light" => "coreside-dock-light.png",
        _ => "coreside-dock-dark.png",
    }
}

/// Resolve which variant to use for a preference + optional OS dark flag.
/// Preference: `auto` | `dark` | `light`
pub fn resolve_dock_variant(preference: &str, os_is_dark: bool) -> &'static str {
    match preference.trim().to_lowercase().as_str() {
        "dark" => "dark",
        "light" => "light",
        _ => {
            if os_is_dark {
                "light"
            } else {
                "dark"
            }
        }
    }
}

fn source_tree_branding_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/branding")
}

/// Candidate paths: packaged resource dir first, then source-tree (dev / tests).
pub fn dock_icon_candidates(app: Option<&AppHandle>, variant: &str) -> Vec<PathBuf> {
    let filename = dock_icon_filename(variant);
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

#[cfg(test)]
fn dock_icon_path(variant: &str) -> PathBuf {
    dock_icon_candidates(None, variant)
        .into_iter()
        .find(|p| p.is_file())
        .unwrap_or_else(|| source_tree_branding_dir().join(dock_icon_filename(variant)))
}

fn resolve_dock_icon_path(app: &AppHandle, variant: &str) -> Result<PathBuf, String> {
    let candidates = dock_icon_candidates(Some(app), variant);
    if let Some(path) = candidates.iter().find(|p| p.is_file()) {
        return Ok(path.clone());
    }
    Err(format!(
        "Dock icon not found for variant '{variant}'. Tried: {}",
        candidates
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// Apply a dock icon by preference (`auto` | `dark` | `light`) and current OS appearance.
pub fn set_dock_icon(app: &AppHandle, preference: &str, os_is_dark: bool) -> Result<(), String> {
    let variant = resolve_dock_variant(preference, os_is_dark);
    let path = resolve_dock_icon_path(app, variant)?;
    let bytes = std::fs::read(&path)
        .map_err(|e| format!("Failed to read dock icon {}: {e}", path.display()))?;

    #[cfg(target_os = "macos")]
    {
        set_macos_application_icon(&bytes)?;
        tracing::info!(
            preference,
            variant,
            os_is_dark,
            path = %path.display(),
            "Updated macOS dock icon"
        );
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = bytes;
        tracing::debug!(
            preference,
            variant,
            "set_dock_icon is a no-op on this platform"
        );
        Ok(())
    }
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

/// Back-compat: treat as preference `auto`.
pub fn set_dock_icon_for_os_appearance(app: &AppHandle, is_dark: bool) -> Result<(), String> {
    set_dock_icon(app, "auto", is_dark)
}

#[cfg(target_os = "macos")]
fn set_macos_application_icon(png_bytes: &[u8]) -> Result<(), String> {
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;

    let mtm = MainThreadMarker::new()
        .ok_or_else(|| "set_dock_icon must run on the main thread".to_string())?;

    let data = NSData::with_bytes(png_bytes);
    let image = NSImage::initWithData(NSImage::alloc(), &data)
        .ok_or_else(|| "Failed to create NSImage from dock PNG bytes".to_string())?;

    let app = NSApplication::sharedApplication(mtm);
    // SAFETY: AppKit setApplicationIconImage; called on main thread with a live NSImage.
    unsafe {
        app.setApplicationIconImage(Some(&image));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filenames_for_variants() {
        assert_eq!(dock_icon_filename("dark"), "coreside-dock-dark.png");
        assert_eq!(dock_icon_filename("light"), "coreside-dock-light.png");
    }

    #[test]
    fn resolve_auto_follows_os() {
        assert_eq!(resolve_dock_variant("auto", false), "dark");
        assert_eq!(resolve_dock_variant("auto", true), "light");
        assert_eq!(resolve_dock_variant("dark", true), "dark");
        assert_eq!(resolve_dock_variant("light", false), "light");
    }

    #[test]
    fn dock_pngs_exist_on_disk() {
        assert!(dock_icon_path("dark").is_file());
        assert!(dock_icon_path("light").is_file());
    }
}
