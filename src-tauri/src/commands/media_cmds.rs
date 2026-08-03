//! Media library CRUD commands.

use serde::Serialize;
use tauri::State;

use super::CommandError;
use crate::media::{
    delete_media_asset, find_media_asset_usages, get_media_asset, list_media_assets,
    resolve_asset_file_path, touch_media_used, ImportMediaInput, MediaAsset, MediaAssetUsage,
};
use crate::security::sanitize_error;
use crate::state::AppState;

fn map_media_err(e: crate::media::MediaError) -> CommandError {
    CommandError::new(e.code(), sanitize_error(&e.to_string(), None))
}

#[tauri::command]
pub fn list_media_assets_cmd(
    state: State<'_, AppState>,
    project_id: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<MediaAsset>, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    list_media_assets(&db, project_id.as_deref(), limit.unwrap_or(50)).map_err(CommandError::from)
}

#[tauri::command]
pub fn get_media_asset_cmd(
    state: State<'_, AppState>,
    asset_id: String,
) -> Result<MediaAsset, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    get_media_asset(&db, &asset_id).map_err(|e| e.into())
}

#[tauri::command]
pub fn delete_media_asset_cmd(
    state: State<'_, AppState>,
    asset_id: String,
) -> Result<(), CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    delete_media_asset(&mut db, &asset_id).map_err(|e| e.into())
}

#[tauri::command]
pub async fn import_media_asset_cmd(
    state: State<'_, AppState>,
    input: ImportMediaInput,
) -> Result<MediaAsset, CommandError> {
    state.require_profile()?;
    let (bytes, hash, meta) =
        crate::media::download_and_validate(input.url.trim(), input.category.as_deref())
            .await
            .map_err(map_media_err)?;

    let mut db = state.db.lock();
    if let Some(existing) = crate::media::find_by_hash(&db, &hash).map_err(CommandError::from)? {
        return Ok(existing);
    }

    let meta = crate::media::enrich_metadata(meta, &bytes).map_err(map_media_err)?;
    crate::media::insert_media_asset(&mut db, &input, &bytes, &meta, &hash).map_err(map_media_err)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaAssetSrc {
    /// Asset-protocol URL for rendering (no bare absolute filesystem path).
    pub url: String,
    pub mime_type: String,
}

fn asset_protocol_url(path: &std::path::Path) -> String {
    let raw = path.to_string_lossy();
    let encoded: String = raw
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{:02X}", b),
        })
        .collect();
    #[cfg(any(windows, target_os = "android"))]
    {
        format!("http://asset.localhost/{encoded}")
    }
    #[cfg(not(any(windows, target_os = "android")))]
    {
        format!("asset://localhost/{encoded}")
    }
}

#[tauri::command]
pub fn get_media_asset_src_cmd(
    state: State<'_, AppState>,
    asset_id: String,
) -> Result<MediaAssetSrc, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    let asset = get_media_asset(&db, &asset_id).map_err(CommandError::from)?;
    let path = resolve_asset_file_path(&asset.local_filename).map_err(map_media_err)?;
    Ok(MediaAssetSrc {
        url: asset_protocol_url(&path),
        mime_type: asset.mime_type,
    })
}

#[tauri::command]
pub fn get_media_asset_thumb_src_cmd(
    state: State<'_, AppState>,
    asset_id: String,
) -> Result<Option<MediaAssetSrc>, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    let asset = get_media_asset(&db, &asset_id).map_err(CommandError::from)?;
    let Some(thumb) = asset.thumbnail_filename.filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let path = resolve_asset_file_path(&thumb).map_err(map_media_err)?;
    Ok(Some(MediaAssetSrc {
        url: asset_protocol_url(&path),
        mime_type: "image/jpeg".into(),
    }))
}

#[tauri::command]
pub fn media_asset_usage_cmd(
    state: State<'_, AppState>,
    asset_id: String,
) -> Result<Vec<MediaAssetUsage>, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    find_media_asset_usages(&db, &asset_id).map_err(CommandError::from)
}

#[tauri::command]
pub fn touch_media_asset_cmd(
    state: State<'_, AppState>,
    asset_id: String,
) -> Result<(), CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    touch_media_used(&db, &asset_id).map_err(|e| e.into())
}
