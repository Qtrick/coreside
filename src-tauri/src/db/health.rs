//! Database health checks (quick / integrity / foreign keys).

use rusqlite::OptionalExtension;
use serde::Serialize;

use super::{Database, DbResult};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseHealthReport {
    pub status: String,
    pub quick_check: String,
    pub foreign_key_check: String,
    pub schema_version: Option<String>,
    pub byte_size: Option<u64>,
}

impl Database {
    pub fn quick_check(&self) -> DbResult<String> {
        let result: String = self
            .conn()
            .query_row("PRAGMA quick_check", [], |row| row.get(0))?;
        Ok(result)
    }

    pub fn integrity_check(&self) -> DbResult<String> {
        let result: String = self
            .conn()
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        Ok(result)
    }

    pub fn foreign_key_check_summary(&self) -> DbResult<String> {
        let mut stmt = self.conn().prepare("PRAGMA foreign_key_check")?;
        let mut rows = stmt.query([])?;
        let mut count = 0usize;
        while rows.next()?.is_some() {
            count += 1;
            if count > 50 {
                break;
            }
        }
        if count == 0 {
            Ok("ok".into())
        } else {
            Ok(format!("{count}_violations"))
        }
    }

    pub fn latest_migration_name(&self) -> DbResult<Option<String>> {
        let name: Option<String> = self
            .conn()
            .query_row(
                "SELECT name FROM _migrations ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(name)
    }

    pub fn health_report(&self) -> DbResult<DatabaseHealthReport> {
        let quick = self.quick_check()?;
        let fk = self.foreign_key_check_summary()?;
        let schema = self.latest_migration_name()?;
        let byte_size = std::fs::metadata(self.path()).ok().map(|m| m.len());
        let status = if quick == "ok" && fk == "ok" {
            "healthy"
        } else {
            "needs_attention"
        };
        Ok(DatabaseHealthReport {
            status: status.into(),
            quick_check: quick,
            foreign_key_check: fk,
            schema_version: schema,
            byte_size,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn fresh_db_is_healthy() {
        let dir = tempdir().unwrap();
        let db = Database::open_path(&dir.path().join("h.db")).unwrap();
        let report = db.health_report().unwrap();
        assert_eq!(report.quick_check, "ok");
        assert_eq!(report.foreign_key_check, "ok");
        assert_eq!(report.status, "healthy");
    }
}
