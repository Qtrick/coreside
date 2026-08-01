//! Change-impact analysis — consumer-friendly summaries.

use crate::db::Database;
use crate::runtime_v2::operations::AppOperation;

pub fn summarize_operations(db: &Database, operations: &[AppOperation]) -> String {
    let mut surfaces = 0usize;
    let mut components = 0usize;
    let mut data = 0usize;
    let mut destructive = 0usize;
    let mut manifests = 0usize;
    let mut warnings: Vec<String> = Vec::new();

    for op in operations {
        let t = op.op_type.as_str();
        if t.starts_with("surface.") || t.starts_with("chat.inline") {
            surfaces += 1;
        }
        if t.starts_with("component.") {
            components += 1;
        }
        if t.starts_with("data.") {
            data += 1;
        }
        if t.starts_with("manifest.") {
            manifests += 1;
        }
        if t.contains("delete") || t == "data.migrate" {
            destructive += 1;
            if t == "data.migrate"
                && op
                    .payload
                    .get("migration")
                    .and_then(|m| m.get("migrationType"))
                    .and_then(|v| v.as_str())
                    == Some("remove_field")
            {
                warnings.push(
                    "Removing a field may permanently remove values from existing records.".into(),
                );
            }
        }
        if let Some(app) = op.target.application_id.as_deref() {
            if let Ok(count) = count_records(db, app) {
                if count > 0 && t.starts_with("data.migrate") {
                    warnings.push(format!("This will affect about {count} stored records."));
                }
            }
        }
    }

    let mut parts = Vec::new();
    if manifests > 0 {
        parts.push(format!("update {manifests} application manifest(s)"));
    }
    if surfaces > 0 {
        parts.push(format!("update {surfaces} surface(s)"));
    }
    if components > 0 {
        parts.push(format!("change {components} component(s)"));
    }
    if data > 0 {
        parts.push(format!("touch {data} data operation(s)"));
    }
    if destructive > 0 {
        parts.push(format!("{destructive} potentially destructive step(s)"));
    }
    if parts.is_empty() {
        parts.push("apply a low-impact update".into());
    }
    let mut summary = format!("This will {}.", parts.join(", "));
    for w in warnings {
        summary.push(' ');
        summary.push_str(&w);
    }
    summary
}

fn count_records(db: &Database, application_id: &str) -> Result<i64, ()> {
    db.conn()
        .query_row(
            "SELECT COUNT(*) FROM generated_data_records WHERE application_id = ?1",
            [application_id],
            |row| row.get(0),
        )
        .map_err(|_| ())
}

pub fn dependency_impact(db: &Database, target_type: &str, target_id: &str) -> Vec<String> {
    let mut stmt = match db.conn().prepare(
        "SELECT source_type, source_id, relationship_type FROM application_dependencies
         WHERE target_type = ?1 AND target_id = ?2",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let rows = stmt.query_map([target_type, target_id], |row| {
        Ok(format!(
            "{} {} ({})",
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?
        ))
    });
    match rows {
        Ok(r) => r.filter_map(|x| x.ok()).collect(),
        Err(_) => vec![],
    }
}
