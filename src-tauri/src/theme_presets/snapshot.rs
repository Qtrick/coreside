//! Versioned appearance snapshot for theme presets.

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ai::{normalize_setting_kv, SettingsChangePayload, WallpaperConfig};
use crate::db::{self, Database, DbError, DbResult};

pub const APPEARANCE_SNAPSHOT_SCHEMA_VERSION: u32 = 1;
const MAX_SNAPSHOT_BYTES: usize = 65_536;

/// Canonical appearance fields persisted inside a theme preset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppearanceSnapshot {
    pub schema_version: u32,
    pub theme_preference: String,
    pub accent_primary_light: String,
    pub accent_primary_dark: String,
    pub accent_secondary_light: String,
    pub accent_secondary_dark: String,
    pub background_light: String,
    pub background_dark: String,
    pub surface_light: String,
    pub surface_dark: String,
    pub surface_muted_light: String,
    pub surface_muted_dark: String,
    pub border_light: String,
    pub border_dark: String,
    pub text_primary_light: String,
    pub text_primary_dark: String,
    pub text_secondary_light: String,
    pub text_secondary_dark: String,
    /// Legacy live wallpaper KV (always present; often `{kind:"none"}`).
    pub wallpaper: WallpaperConfig,
    /// Schema wallpaper JSON when active; empty string when cleared/legacy-only.
    #[serde(default)]
    pub wallpaper_json: String,
    pub interface_transparency: u8,
}

impl AppearanceSnapshot {
    pub fn from_settings_map(
        map: &std::collections::HashMap<String, String>,
    ) -> Result<Self, String> {
        let theme = map.get("theme").cloned().unwrap_or_else(|| "system".into());
        let theme_preference = normalize_setting_kv("theme", &theme)?;

        let color = |key: &str, default: &str| -> Result<String, String> {
            let raw = map.get(key).map(|s| s.as_str()).unwrap_or(default);
            normalize_setting_kv(key, raw)
        };

        let wallpaper_raw = map
            .get("wallpaper")
            .cloned()
            .unwrap_or_else(|| r#"{"kind":"none"}"#.into());
        let wallpaper_norm = normalize_setting_kv("wallpaper", &wallpaper_raw)?;
        let wallpaper: WallpaperConfig =
            serde_json::from_str(&wallpaper_norm).map_err(|e| format!("invalid wallpaper: {e}"))?;

        let wallpaper_json_raw = map
            .get("wallpaperJson")
            .or_else(|| map.get("wallpaper_json"))
            .cloned()
            .unwrap_or_default();
        let wallpaper_json = normalize_setting_kv("wallpaperJson", &wallpaper_json_raw)?;

        let transparency_raw = map
            .get("interfaceTransparency")
            .or_else(|| map.get("interface_transparency"))
            .cloned()
            .unwrap_or_else(|| "0".into());
        let transparency = normalize_setting_kv("interfaceTransparency", &transparency_raw)?;
        let interface_transparency: u8 = transparency
            .parse()
            .map_err(|_| "interfaceTransparency must be 0–100".to_string())?;

        Ok(Self {
            schema_version: APPEARANCE_SNAPSHOT_SCHEMA_VERSION,
            theme_preference,
            accent_primary_light: color("accentPrimaryLight", "#2f8f63")?,
            accent_primary_dark: color("accentPrimaryDark", "#69c994")?,
            accent_secondary_light: color("accentSecondaryLight", "#d38b3d")?,
            accent_secondary_dark: color("accentSecondaryDark", "#e0a158")?,
            background_light: color("backgroundLight", "#f5f5f5")?,
            background_dark: color("backgroundDark", "#0a0a0a")?,
            surface_light: color("surfaceLight", "#ffffff")?,
            surface_dark: color("surfaceDark", "#171717")?,
            surface_muted_light: color("surfaceMutedLight", "#ececec")?,
            surface_muted_dark: color("surfaceMutedDark", "#262626")?,
            border_light: color("borderLight", "#e4e4e4")?,
            border_dark: color("borderDark", "#3f3f3f")?,
            text_primary_light: color("textPrimaryLight", "#171717")?,
            text_primary_dark: color("textPrimaryDark", "#fafafa")?,
            text_secondary_light: color("textSecondaryLight", "#737373")?,
            text_secondary_dark: color("textSecondaryDark", "#a3a3a3")?,
            wallpaper,
            wallpaper_json,
            interface_transparency,
        })
    }

    pub fn to_json_string(&self) -> Result<String, String> {
        let json = serde_json::to_string(self).map_err(|e| e.to_string())?;
        if json.len() > MAX_SNAPSHOT_BYTES {
            return Err("theme snapshot is too large".into());
        }
        Ok(json)
    }

    /// Stable hash of the committed appearance snapshot (CAS for Apply confirmations).
    pub fn fingerprint(&self) -> Result<String, String> {
        use sha2::{Digest, Sha256};
        let json = self.to_json_string()?;
        Ok(hex::encode(Sha256::digest(json.as_bytes())))
    }

    /// Fingerprint of the workspace's committed appearance settings (not live preview).
    pub fn committed_fingerprint_from_db(db: &Database) -> Result<String, String> {
        let map = db::get_settings(db).map_err(|e| e.to_string())?;
        Self::from_settings_map(&map)?.fingerprint()
    }

    pub fn parse_and_validate(raw: &str) -> Result<Self, String> {
        if raw.len() > MAX_SNAPSHOT_BYTES {
            return Err("theme snapshot is too large".into());
        }
        let value: Value = serde_json::from_str(raw)
            .map_err(|_| "theme snapshot must be valid JSON".to_string())?;
        let Some(obj) = value.as_object() else {
            return Err("theme snapshot must be a JSON object".into());
        };
        // Reject secret-looking keys at the boundary.
        for key in obj.keys() {
            let lower = key.to_ascii_lowercase();
            if lower.contains("api_key")
                || lower.contains("apikey")
                || lower.contains("secret")
                || lower.contains("token")
                || lower.contains("password")
            {
                return Err("theme snapshot cannot contain secrets".into());
            }
        }
        let snap: Self =
            serde_json::from_value(value).map_err(|e| format!("invalid theme snapshot: {e}"))?;
        snap.validate()?;
        Ok(snap)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != APPEARANCE_SNAPSHOT_SCHEMA_VERSION {
            return Err(format!(
                "unsupported theme snapshot schemaVersion {}",
                self.schema_version
            ));
        }
        normalize_setting_kv("theme", &self.theme_preference)?;
        for (key, value) in self.color_pairs() {
            normalize_setting_kv(key, value)?;
        }
        let wallpaper_json = serde_json::to_string(&self.wallpaper)
            .map_err(|e| format!("invalid wallpaper: {e}"))?;
        normalize_setting_kv("wallpaper", &wallpaper_json)?;
        normalize_setting_kv("wallpaperJson", &self.wallpaper_json)?;
        normalize_setting_kv(
            "interfaceTransparency",
            &self.interface_transparency.to_string(),
        )?;
        Ok(())
    }

    fn color_pairs(&self) -> [(&str, &str); 16] {
        [
            ("accentPrimaryLight", &self.accent_primary_light),
            ("accentPrimaryDark", &self.accent_primary_dark),
            ("accentSecondaryLight", &self.accent_secondary_light),
            ("accentSecondaryDark", &self.accent_secondary_dark),
            ("backgroundLight", &self.background_light),
            ("backgroundDark", &self.background_dark),
            ("surfaceLight", &self.surface_light),
            ("surfaceDark", &self.surface_dark),
            ("surfaceMutedLight", &self.surface_muted_light),
            ("surfaceMutedDark", &self.surface_muted_dark),
            ("borderLight", &self.border_light),
            ("borderDark", &self.border_dark),
            ("textPrimaryLight", &self.text_primary_light),
            ("textPrimaryDark", &self.text_primary_dark),
            ("textSecondaryLight", &self.text_secondary_light),
            ("textSecondaryDark", &self.text_secondary_dark),
        ]
    }

    /// Expand into settings KV writes (validated).
    pub fn to_settings_writes(&self) -> Result<Vec<(String, String)>, String> {
        self.validate()?;
        let mut writes = Vec::with_capacity(20);
        writes.push((
            "theme".into(),
            normalize_setting_kv("theme", &self.theme_preference)?,
        ));
        for (key, value) in self.color_pairs() {
            writes.push((key.to_string(), normalize_setting_kv(key, value)?));
        }
        let wallpaper_json = serde_json::to_string(&self.wallpaper)
            .map_err(|e| format!("invalid wallpaper: {e}"))?;
        // Reuse pair resolution semantics from settings_cmds via normalize paths:
        // when wallpaperJson is non-empty schema, legacy wallpaper resets to none;
        // when empty, store legacy wallpaper and clear wallpaperJson.
        let wallpaper_json_norm = normalize_setting_kv("wallpaperJson", &self.wallpaper_json)?;
        if wallpaper_json_norm.is_empty() {
            writes.push((
                "wallpaper".into(),
                normalize_setting_kv("wallpaper", &wallpaper_json)?,
            ));
            writes.push(("wallpaperJson".into(), String::new()));
        } else {
            writes.push(("wallpaperJson".into(), wallpaper_json_norm));
            writes.push((
                "wallpaper".into(),
                normalize_setting_kv("wallpaper", r#"{"kind":"none"}"#)?,
            ));
        }
        writes.push((
            "interfaceTransparency".into(),
            normalize_setting_kv(
                "interfaceTransparency",
                &self.interface_transparency.to_string(),
            )?,
        ));
        Ok(writes)
    }

    /// Collect media asset IDs referenced by the snapshot wallpaper.
    pub fn referenced_asset_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        if self.wallpaper_json.trim().is_empty() {
            return ids;
        }
        let Ok(value) = serde_json::from_str::<Value>(&self.wallpaper_json) else {
            return ids;
        };
        if let Some(id) = value.get("assetId").and_then(|v| v.as_str()) {
            if !id.is_empty() {
                ids.push(id.to_string());
            }
        }
        if let Some(arr) = value.get("slideAssetIds").and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(id) = item.as_str() {
                    if !id.is_empty() {
                        ids.push(id.to_string());
                    }
                }
            }
        }
        ids.sort();
        ids.dedup();
        ids
    }

    /// Ensure referenced media assets exist and are global (not project-scoped).
    /// Workspace theme presets must not adopt project-owned media.
    pub(crate) fn assert_media_assets_exist_on_conn(
        &self,
        conn: &rusqlite::Connection,
    ) -> DbResult<()> {
        for asset_id in self.referenced_asset_ids() {
            let asset = conn
                .query_row(
                    "SELECT local_filename, validation_status, project_id FROM media_assets WHERE id = ?1 LIMIT 1",
                    [&asset_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<String>>(2)?,
                        ))
                    },
                )
                .optional()?;
            let Some((local_filename, validation_status, project_id)) = asset else {
                return Err(DbError::NotFound(asset_id));
            };
            if project_id.as_ref().is_some_and(|p| !p.trim().is_empty()) {
                return Err(DbError::Invalid(format!(
                    "theme wallpaper cannot use project-scoped media asset {asset_id}"
                )));
            }
            if validation_status != "ok" {
                return Err(DbError::Invalid(format!(
                    "theme wallpaper asset {asset_id} is not valid"
                )));
            }
            crate::media::resolve_asset_file_path(&local_filename).map_err(|_| {
                DbError::NotFound(format!("theme wallpaper asset file for {asset_id}"))
            })?;
        }
        Ok(())
    }
}

/// Apply a validated appearance snapshot in one SQLite transaction.
pub fn apply_appearance_snapshot(db: &mut Database, snapshot: &AppearanceSnapshot) -> DbResult<()> {
    snapshot.validate().map_err(db::DbError::Invalid)?;
    let writes = snapshot
        .to_settings_writes()
        .map_err(db::DbError::Invalid)?;
    db.with_transaction(|conn| {
        snapshot.assert_media_assets_exist_on_conn(conn)?;
        for (key, value) in &writes {
            db::set_setting_on_conn(conn, key, value)?;
        }
        Ok(())
    })
}

/// Apply only user-approved appearance fields against committed SQLite state.
/// Unmentioned fields never come from optimistic frontend previews.
pub fn apply_appearance_changes(
    db: &mut Database,
    changes: &SettingsChangePayload,
) -> DbResult<()> {
    let mut writes = changes.to_kv_pairs().map_err(DbError::Invalid)?;
    if changes.wallpaper.is_some() {
        // Agent wallpaper proposals use the legacy live-wallpaper shape. Clear
        // schema wallpaper JSON in the same transaction when replacing it.
        writes.push(("wallpaperJson".into(), String::new()));
    }
    db.with_transaction(|conn| {
        for (key, value) in &writes {
            let normalized = normalize_setting_kv(key, value).map_err(DbError::Invalid)?;
            db::set_setting_on_conn(conn, key, &normalized)?;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_map() -> std::collections::HashMap<String, String> {
        let mut map = std::collections::HashMap::new();
        map.insert("theme".into(), "dark".into());
        map.insert("accentPrimaryLight".into(), "#2f8f63".into());
        map.insert("accentPrimaryDark".into(), "#69c994".into());
        map.insert("accentSecondaryLight".into(), "#d38b3d".into());
        map.insert("accentSecondaryDark".into(), "#e0a158".into());
        map.insert("backgroundLight".into(), "#f5f5f5".into());
        map.insert("backgroundDark".into(), "#0a0a0a".into());
        map.insert("surfaceLight".into(), "#ffffff".into());
        map.insert("surfaceDark".into(), "#171717".into());
        map.insert("surfaceMutedLight".into(), "#ececec".into());
        map.insert("surfaceMutedDark".into(), "#262626".into());
        map.insert("borderLight".into(), "#e4e4e4".into());
        map.insert("borderDark".into(), "#3f3f3f".into());
        map.insert("textPrimaryLight".into(), "#171717".into());
        map.insert("textPrimaryDark".into(), "#fafafa".into());
        map.insert("textSecondaryLight".into(), "#737373".into());
        map.insert("textSecondaryDark".into(), "#a3a3a3".into());
        map.insert("wallpaper".into(), r#"{"kind":"matrix"}"#.into());
        map.insert("wallpaperJson".into(), String::new());
        map.insert("interfaceTransparency".into(), "40".into());
        map
    }

    #[test]
    fn round_trip_from_settings_map() {
        let snap = AppearanceSnapshot::from_settings_map(&sample_map()).unwrap();
        assert_eq!(snap.theme_preference, "dark");
        assert_eq!(snap.wallpaper.kind, "matrix");
        assert_eq!(snap.interface_transparency, 40);
        let json = snap.to_json_string().unwrap();
        let parsed = AppearanceSnapshot::parse_and_validate(&json).unwrap();
        assert_eq!(parsed, snap);
        assert_eq!(parsed.to_settings_writes().unwrap().len(), 20);
    }

    #[test]
    fn rejects_secret_keys_and_bad_schema() {
        let err = AppearanceSnapshot::parse_and_validate(r#"{"apiKey":"x"}"#).unwrap_err();
        assert!(err.contains("secret"));
        let mut snap = AppearanceSnapshot::from_settings_map(&sample_map()).unwrap();
        snap.schema_version = 99;
        assert!(snap.validate().is_err());
    }

    #[test]
    fn fingerprint_is_stable_for_unchanged_snapshot() {
        let snap = AppearanceSnapshot::from_settings_map(&sample_map()).unwrap();
        let fp1 = snap.fingerprint().unwrap();
        let fp2 = snap.fingerprint().unwrap();
        assert_eq!(fp1, fp2);
        assert_eq!(fp1.len(), 64);
    }
}
