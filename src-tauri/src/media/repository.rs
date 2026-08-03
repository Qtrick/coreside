use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use crate::db::{Database, DbError, DbResult};

use super::errors::MediaError;
use super::models::{ImportMediaInput, MediaAsset};
use super::storage::{delete_asset_file, write_asset_bytes};

fn row_to_asset(row: &rusqlite::Row<'_>) -> rusqlite::Result<MediaAsset> {
    Ok(MediaAsset {
        id: row.get(0)?,
        project_id: row.get(1)?,
        category: row.get(2)?,
        title: row.get(3)?,
        local_filename: row.get(4)?,
        mime_type: row.get(5)?,
        byte_size: row.get(6)?,
        width: row.get(7)?,
        height: row.get(8)?,
        duration_ms: row.get(9)?,
        content_hash: row.get(10)?,
        source_url: row.get(11)?,
        source_page_url: row.get(12)?,
        creator: row.get(13)?,
        license: row.get(14)?,
        attribution: row.get(15)?,
        validation_status: row.get(16)?,
        created_at: row.get(17)?,
        last_used_at: row.get(18)?,
        thumbnail_filename: row.get(19)?,
    })
}

const SELECT_COLS: &str = "id, project_id, category, title, local_filename, mime_type, byte_size,
    width, height, duration_ms, content_hash, source_url, source_page_url, creator, license,
    attribution, validation_status, created_at, last_used_at, thumbnail_filename";

pub fn find_by_hash(db: &Database, hash: &str) -> DbResult<Option<MediaAsset>> {
    let mut stmt = db.conn().prepare(&format!(
        "SELECT {SELECT_COLS} FROM media_assets WHERE content_hash = ?1 LIMIT 1"
    ))?;
    let asset = stmt.query_row(params![hash], row_to_asset).optional()?;
    Ok(asset)
}

pub fn list_media_assets(
    db: &Database,
    project_id: Option<&str>,
    limit: usize,
) -> DbResult<Vec<MediaAsset>> {
    let limit = limit.clamp(1, 200) as i64;
    let rows: Vec<MediaAsset> = if let Some(pid) = project_id.filter(|s| !s.is_empty()) {
        let mut stmt = db.conn().prepare(&format!(
            "SELECT {SELECT_COLS} FROM media_assets WHERE project_id = ?1
             ORDER BY created_at DESC LIMIT ?2"
        ))?;
        let mapped = stmt.query_map(params![pid, limit], row_to_asset)?;
        mapped.collect::<Result<Vec<_>, _>>()?
    } else {
        let mut stmt = db.conn().prepare(&format!(
            "SELECT {SELECT_COLS} FROM media_assets ORDER BY created_at DESC LIMIT ?1"
        ))?;
        let mapped = stmt.query_map(params![limit], row_to_asset)?;
        mapped.collect::<Result<Vec<_>, _>>()?
    };
    Ok(rows)
}

pub fn get_media_asset(db: &Database, id: &str) -> DbResult<MediaAsset> {
    let mut stmt = db.conn().prepare(&format!(
        "SELECT {SELECT_COLS} FROM media_assets WHERE id = ?1"
    ))?;
    stmt.query_row(params![id], row_to_asset)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(id.to_string()),
            other => DbError::Sqlite(other),
        })
}

/// Whether an asset is visible to a tool/agent turn scoped to `project_id`.
/// Unscoped library assets (`project_id` NULL) are readable from any turn.
/// Project-scoped assets are only readable when the turn's project matches.
pub fn media_readable_in_scope(asset: &MediaAsset, project_id: Option<&str>) -> bool {
    match asset.project_id.as_deref() {
        None => true,
        Some(asset_project) => project_id == Some(asset_project),
    }
}

pub fn delete_media_asset(db: &mut Database, id: &str) -> DbResult<()> {
    let asset = get_media_asset(db, id)?;
    crate::media::thumbnails::delete_thumbnail(asset.thumbnail_filename.as_deref());
    delete_asset_file(&asset.local_filename).map_err(|e| DbError::Invalid(e.to_string()))?;
    db.conn()
        .execute("DELETE FROM media_assets WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn insert_media_asset(
    db: &mut Database,
    input: &ImportMediaInput,
    bytes: &[u8],
    meta: &super::models::MediaMetadata,
    hash: &str,
) -> Result<MediaAsset, MediaError> {
    if let Some(existing) =
        find_by_hash(db, hash).map_err(|e| MediaError::Database(e.to_string()))?
    {
        return Ok(existing);
    }

    let id = Uuid::new_v4().to_string();
    let ext = extension_for_mime(&meta.mime_type);
    let local_filename = format!("{id}.{ext}");
    write_asset_bytes(&local_filename, bytes).map_err(|e| MediaError::Storage(e.to_string()))?;

    let thumbnail_filename =
        match crate::media::thumbnails::generate_thumbnail(&id, &meta.mime_type, bytes) {
            Ok(name) => name,
            Err(e) => {
                tracing::warn!(error = %e, "thumbnail generation failed");
                None
            }
        };

    let title = input
        .title
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("Imported asset")
        .to_string();

    let category = input
        .category
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or(meta.category.as_str())
        .to_string();

    if let Err(e) = db.conn().execute(
        "INSERT INTO media_assets
             (id, project_id, category, title, local_filename, mime_type, byte_size,
              width, height, duration_ms, content_hash, source_url, source_page_url,
              creator, license, attribution, validation_status, thumbnail_filename)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,'ok',?17)",
        params![
            id,
            input.project_id,
            category,
            title,
            local_filename,
            meta.mime_type,
            bytes.len() as i64,
            meta.width.map(|v| v as i64),
            meta.height.map(|v| v as i64),
            meta.duration_ms,
            hash,
            input.url,
            input.source_page_url,
            input.creator,
            input.license,
            input.attribution,
            thumbnail_filename.clone(),
        ],
    ) {
        let _ = delete_asset_file(&local_filename);
        if let Some(thumb) = thumbnail_filename.as_deref() {
            let _ = delete_asset_file(thumb);
        }
        return Err(MediaError::Database(e.to_string()));
    }

    match get_media_asset(db, &id) {
        Ok(asset) => Ok(asset),
        Err(e) => {
            let _ = db
                .conn()
                .execute("DELETE FROM media_assets WHERE id = ?1", params![id]);
            let _ = delete_asset_file(&local_filename);
            if let Some(thumb) = thumbnail_filename.as_deref() {
                let _ = delete_asset_file(thumb);
            }
            Err(MediaError::Database(e.to_string()))
        }
    }
}

fn extension_for_mime(mime: &str) -> &'static str {
    match mime {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "video/quicktime" => "mov",
        _ => "bin",
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaAssetUsage {
    pub project_id: String,
    pub project_name: String,
}

/// Projects whose validated wallpaper JSON references a media asset id.
pub fn find_media_asset_usages(db: &Database, asset_id: &str) -> DbResult<Vec<MediaAssetUsage>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, name, wallpaper_json FROM projects
         WHERE wallpaper_json IS NOT NULL AND wallpaper_json != ''",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let mut usages = Vec::new();
    for row in rows {
        let (project_id, project_name, wallpaper_json) = row?;
        if wallpaper_references_asset(&wallpaper_json, asset_id) {
            usages.push(MediaAssetUsage {
                project_id,
                project_name,
            });
        }
    }
    Ok(usages)
}

fn wallpaper_references_asset(wallpaper_json: &str, asset_id: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(wallpaper_json) else {
        return false;
    };
    if value
        .get("assetId")
        .and_then(|v| v.as_str())
        .is_some_and(|id| id == asset_id)
    {
        return true;
    }
    if let Some(ids) = value.get("slideAssetIds").and_then(|v| v.as_array()) {
        for id in ids {
            if id.as_str() == Some(asset_id) {
                return true;
            }
        }
    }
    false
}

pub fn touch_media_used(db: &Database, id: &str) -> DbResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let n = db.conn().execute(
        "UPDATE media_assets SET last_used_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    if n == 0 {
        return Err(DbError::NotFound(id.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::models::MediaAsset;

    fn sample(project_id: Option<&str>) -> MediaAsset {
        MediaAsset {
            id: "a1".into(),
            project_id: project_id.map(str::to_string),
            category: "image".into(),
            title: "t".into(),
            local_filename: "a1.png".into(),
            mime_type: "image/png".into(),
            byte_size: 1,
            width: None,
            height: None,
            duration_ms: None,
            content_hash: "h".into(),
            source_url: None,
            source_page_url: None,
            creator: None,
            license: None,
            attribution: None,
            validation_status: "ok".into(),
            created_at: "now".into(),
            last_used_at: None,
            thumbnail_filename: None,
        }
    }

    #[test]
    fn scope_allows_unscoped_everywhere() {
        let asset = sample(None);
        assert!(media_readable_in_scope(&asset, None));
        assert!(media_readable_in_scope(&asset, Some("p1")));
    }

    #[test]
    fn scope_blocks_other_project() {
        let asset = sample(Some("p2"));
        assert!(!media_readable_in_scope(&asset, Some("p1")));
        assert!(!media_readable_in_scope(&asset, None));
        assert!(media_readable_in_scope(&asset, Some("p2")));
    }
}
