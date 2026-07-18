//! Local media library: validated downloads, dedup by content hash.

mod download;
mod errors;
mod limits;
mod metadata;
mod models;
mod repository;
mod storage;
mod thumbnails;
mod validation;

pub use download::download_and_validate;
pub use errors::MediaError;
pub use models::{ImportMediaInput, MediaAsset};
pub use repository::{
    delete_media_asset, find_by_hash, find_media_asset_usages, get_media_asset,
    insert_media_asset, list_media_assets, media_readable_in_scope, touch_media_used,
    MediaAssetUsage,
};
pub use storage::resolve_asset_file_path;

pub use metadata::enrich_metadata;

pub async fn import_media_asset(
    db: &mut crate::db::Database,
    input: ImportMediaInput,
) -> Result<MediaAsset, MediaError> {
    let url = input.url.trim();
    if url.is_empty() {
        return Err(MediaError::Invalid("url is required".into()));
    }

    let (bytes, hash, meta) =
        download_and_validate(url, input.category.as_deref()).await?;
    let meta = enrich_metadata(meta, &bytes)?;

    if let Some(existing) = find_by_hash(db, &hash).map_err(|e| MediaError::Database(e.to_string()))? {
        return Ok(existing);
    }

    insert_media_asset(db, &input, &bytes, &meta, &hash)
}
