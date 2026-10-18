//! User-saved theme presets — protected settings authority.
//!
//! Snapshots capture committed workspace appearance (theme preference, full
//! palette, wallpaper pair, interface transparency). Apply is one SQLite
//! transaction. Quota is enforced inside insert transactions.

pub mod policy;
pub mod repository;
pub mod snapshot;

pub use policy::{
    max_presets_for_plan, FREE_THEME_PRESET_LIMIT, PERSONAL_THEME_PRESET_LIMIT,
    PRO_THEME_PRESET_LIMIT,
};
pub use repository::{
    count_theme_presets, delete_theme_preset, find_theme_presets_referencing_asset,
    get_theme_preset, insert_theme_preset, list_theme_presets, rename_theme_preset,
    update_theme_preset_snapshot, ThemePresetCapabilities, ThemePresetRecord,
};
pub use snapshot::{
    apply_appearance_changes, apply_appearance_snapshot, assert_wallpaper_media_asset_ids_on_conn,
    asset_ids_from_wallpaper_json, AppearanceSnapshot, APPEARANCE_SNAPSHOT_SCHEMA_VERSION,
};

#[cfg(test)]
mod tests;
