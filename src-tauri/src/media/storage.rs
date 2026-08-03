use std::path::{Path, PathBuf};

use super::errors::MediaError;
use crate::app_paths::AppPaths;

pub fn media_root() -> Result<PathBuf, MediaError> {
    let paths =
        AppPaths::resolve().map_err(|e| MediaError::Storage(format!("app paths: {e}")))?;
    Ok(paths.media)
}

pub fn ensure_media_root() -> Result<PathBuf, MediaError> {
    let root = media_root()?;
    std::fs::create_dir_all(&root).map_err(|e| MediaError::Storage(e.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700));
    }
    Ok(root)
}

pub fn asset_path(local_filename: &str) -> Result<PathBuf, MediaError> {
    let name = Path::new(local_filename)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| MediaError::Invalid("invalid local filename".into()))?;
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains("..")
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
    {
        return Err(MediaError::Invalid("path traversal rejected".into()));
    }
    Ok(ensure_media_root()?.join(name))
}

pub fn write_asset_bytes(local_filename: &str, bytes: &[u8]) -> Result<PathBuf, MediaError> {
    let path = asset_path(local_filename)?;
    let parent = path
        .parent()
        .ok_or_else(|| MediaError::Storage("invalid media destination".into()))?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|e| MediaError::Storage(e.to_string()))?;
    use std::io::Write;
    tmp.write_all(bytes)
        .map_err(|e| MediaError::Storage(e.to_string()))?;
    tmp.flush()
        .map_err(|e| MediaError::Storage(e.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o600));
    }
    tmp.persist(&path)
        .map_err(|e| MediaError::Storage(e.error.to_string()))?;
    Ok(path)
}

pub fn delete_asset_file(local_filename: &str) -> Result<(), MediaError> {
    let path = asset_path(local_filename)?;
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| MediaError::Storage(e.to_string()))?;
    }
    Ok(())
}

/// Resolve a validated absolute path for an asset file under the managed media directory.
pub fn resolve_asset_file_path(local_filename: &str) -> Result<PathBuf, MediaError> {
    let path = asset_path(local_filename)?;
    if !path.exists() {
        return Err(MediaError::NotFound(local_filename.into()));
    }
    let root = ensure_media_root()?;
    let canonical = path
        .canonicalize()
        .map_err(|e| MediaError::Storage(e.to_string()))?;
    let root_canonical = root
        .canonicalize()
        .map_err(|e| MediaError::Storage(e.to_string()))?;
    if !canonical.starts_with(&root_canonical) {
        return Err(MediaError::Invalid("path outside media directory".into()));
    }
    Ok(canonical)
}
