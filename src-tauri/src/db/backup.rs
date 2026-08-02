//! Consistent SQLite profile snapshots via the Online Backup API.

use std::path::Path;

use rusqlite::{backup::Backup, Connection};

use super::{Database, DbError, DbResult};

impl Database {
    /// Create a consistent on-disk snapshot of this database at `dest`.
    /// Uses SQLite's Online Backup API (not a raw filesystem copy of a live WAL DB).
    pub fn snapshot_to_path(&self, dest: &Path) -> DbResult<()> {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| DbError::Invalid(format!("Failed to create backup directory: {e}")))?;
        }
        if dest.exists() {
            std::fs::remove_file(dest)
                .map_err(|e| DbError::Invalid(format!("Failed to clear backup target: {e}")))?;
        }

        let mut dest_conn = Connection::open(dest)?;
        {
            let backup = Backup::new(self.conn(), &mut dest_conn)
                .map_err(|e| DbError::Invalid(format!("Backup init failed: {e}")))?;
            backup
                .run_to_completion(100, std::time::Duration::from_millis(10), None)
                .map_err(|e| DbError::Invalid(format!("Backup copy failed: {e}")))?;
        }
        // Validate snapshot independently.
        dest_conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        let quick: String =
            dest_conn.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
        if quick != "ok" {
            let _ = std::fs::remove_file(dest);
            return Err(DbError::Invalid(format!(
                "Backup failed quick_check: {quick}"
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn snapshot_passes_quick_check() {
        let dir = tempdir().unwrap();
        let src = Database::open_path(&dir.path().join("src.db")).unwrap();
        let dest = dir.path().join("snap.db");
        src.snapshot_to_path(&dest).unwrap();
        assert!(dest.exists());
        let opened = Database::open_path(&dest).unwrap();
        assert_eq!(opened.quick_check().unwrap(), "ok");
    }
}
