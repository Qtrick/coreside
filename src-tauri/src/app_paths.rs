//! Authoritative application-data paths for Coreside.

use std::path::{Path, PathBuf};

use crate::db::{product_data_dir, DbError, DbResult};

/// Canonical managed roots under the product data directory.
#[derive(Debug, Clone)]
pub struct AppPaths {
    pub product_root: PathBuf,
    pub database: PathBuf,
    pub media: PathBuf,
    pub attachments: PathBuf,
    pub backups: PathBuf,
    pub restore_staging: PathBuf,
    pub quarantine: PathBuf,
    pub diagnostics: PathBuf,
    pub crawler: PathBuf,
    pub recovery: PathBuf,
}

impl AppPaths {
    pub fn resolve() -> DbResult<Self> {
        let base = dirs::data_dir().ok_or_else(|| {
            DbError::Invalid("Could not resolve application data directory".into())
        })?;
        Ok(Self::from_base(&base))
    }

    pub fn from_base(base: &Path) -> Self {
        let product_root = product_data_dir(base);
        Self {
            database: product_root.join("coreside.db"),
            media: product_root.join("media"),
            attachments: product_root.join("chat-attachments"),
            backups: product_root.join("backups"),
            restore_staging: product_root.join("restore-staging"),
            quarantine: product_root.join("quarantine"),
            diagnostics: product_root.join("diagnostics"),
            crawler: product_root.join("crawler"),
            recovery: product_root.join("recovery"),
            product_root,
        }
    }

    pub fn ensure_dirs(&self) -> DbResult<()> {
        for dir in [
            &self.product_root,
            &self.media,
            &self.attachments,
            &self.backups,
            &self.restore_staging,
            &self.quarantine,
            &self.diagnostics,
            &self.crawler,
            &self.recovery,
        ] {
            std::fs::create_dir_all(dir).map_err(|e| {
                DbError::Invalid(format!("Failed to create {}: {e}", dir.display()))
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn all_roots_share_canonical_product_dir() {
        let dir = tempdir().unwrap();
        let paths = AppPaths::from_base(dir.path());
        assert!(paths.database.starts_with(&paths.product_root));
        assert!(paths.media.starts_with(&paths.product_root));
        assert!(paths.backups.starts_with(&paths.product_root));
        assert_eq!(paths.product_root, dir.path().join("coreside"));
    }
}
