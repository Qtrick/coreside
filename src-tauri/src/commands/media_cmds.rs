//! Media library CRUD commands.

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
    let db = state.db.lock();
    list_media_assets(&db, project_id.as_deref(), limit.unwrap_or(50))
        .map_err(|e| CommandError::new("db", e.to_string()))
}

#[tauri::command]
pub fn get_media_asset_cmd(
    state: State<'_, AppState>,
    asset_id: String,
) -> Result<MediaAsset, CommandError> {
    let db = state.db.lock();
    get_media_asset(&db, &asset_id).map_err(|e| e.into())
}

#[tauri::command]
pub fn delete_media_asset_cmd(
    state: State<'_, AppState>,
    asset_id: String,
) -> Result<(), CommandError> {
    let mut db = state.db.lock();
    delete_media_asset(&mut db, &asset_id).map_err(|e| e.into())
}

#[tauri::command]
pub async fn import_media_asset_cmd(
    state: State<'_, AppState>,
    input: ImportMediaInput,
) -> Result<MediaAsset, CommandError> {
    let (bytes, hash, meta) = crate::media::download_and_validate(
        input.url.trim(),
        input.category.as_deref(),
    )
    .await
    .map_err(map_media_err)?;

    let mut db = state.db.lock();
    if let Some(existing) =
        crate::media::find_by_hash(&db, &hash).map_err(|e| CommandError::new("db", e.to_string()))?
    {
        return Ok(existing);
    }

    let meta = crate::media::enrich_metadata(meta, &bytes).map_err(map_media_err)?;
    crate::media::insert_media_asset(&mut db, &input, &bytes, &meta, &hash).map_err(map_media_err)
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaAssetSrc {
    pub absolute_path: String,
    pub mime_type: String,
}

#[tauri::command]
pub fn get_media_asset_src_cmd(
    state: State<'_, AppState>,
    asset_id: String,
) -> Result<MediaAssetSrc, CommandError> {
    let db = state.db.lock();
    let asset = get_media_asset(&db, &asset_id).map_err(CommandError::from)?;
    let path = resolve_asset_file_path(&asset.local_filename).map_err(map_media_err)?;
    Ok(MediaAssetSrc {
        absolute_path: path.to_string_lossy().into_owned(),
        mime_type: asset.mime_type,
    })
}

#[tauri::command]
pub fn get_media_asset_thumb_src_cmd(
    state: State<'_, AppState>,
    asset_id: String,
) -> Result<Option<MediaAssetSrc>, CommandError> {
    let db = state.db.lock();
    let asset = get_media_asset(&db, &asset_id).map_err(CommandError::from)?;
    let Some(thumb) = asset.thumbnail_filename.filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let path = resolve_asset_file_path(&thumb).map_err(map_media_err)?;
    Ok(Some(MediaAssetSrc {
        absolute_path: path.to_string_lossy().into_owned(),
        mime_type: "image/jpeg".into(),
    }))
}

#[tauri::command]
pub fn media_asset_usage_cmd(
    state: State<'_, AppState>,
    asset_id: String,
) -> Result<Vec<MediaAssetUsage>, CommandError> {
    let db = state.db.lock();
    find_media_asset_usages(&db, &asset_id).map_err(|e| CommandError::new("db", e.to_string()))
}

#[tauri::command]
pub fn touch_media_asset_cmd(
    state: State<'_, AppState>,
    asset_id: String,
) -> Result<(), CommandError> {
    let db = state.db.lock();
    touch_media_used(&db, &asset_id).map_err(|e| e.into())
}
