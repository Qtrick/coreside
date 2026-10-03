//! Authoritative application/surface lineage resolution.
//!
//! Database rows are authority. Surface-id string formatting (`surf-*`) is never
//! a security boundary — it is at most a naming convenience for new creates.

use serde::{Deserialize, Serialize};

use crate::db::{Database, DbError, DbResult};
use crate::runtime_v2::surfaces::{get_surface, SurfaceRecord};

use super::errors::KernelError;
use super::manifest::{application_accepts_mutations, authoritative_application_id};

/// Resolved lineage for one surface as stored in SQLite.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceLineage {
    pub surface_id: String,
    pub tool_id: Option<String>,
    /// Manifest-backed application id when one exists; otherwise tool_id when bound.
    pub application_id: Option<String>,
    pub conversation_id: Option<String>,
    pub project_id: Option<String>,
    pub definition_revision: i64,
    pub lifecycle_state: String,
    pub archived: bool,
    pub has_manifest: bool,
}

impl SurfaceLineage {
    pub fn from_surface(db: &Database, surface: &SurfaceRecord) -> Self {
        let tool_id = surface.tool_id.clone();
        // Manifest presence is recorded separately; application identity is the
        // bound tool_id when present (create-before-manifest and full apps alike).
        let has_manifest = tool_id
            .as_deref()
            .and_then(|tid| authoritative_application_id(db, Some(tid)))
            .is_some();
        let application_id = tool_id.clone();
        Self {
            surface_id: surface.id.clone(),
            tool_id,
            application_id,
            conversation_id: surface.conversation_id.clone(),
            project_id: surface.project_id.clone(),
            definition_revision: surface.current_revision,
            lifecycle_state: surface.lifecycle_state.clone(),
            archived: surface.archived,
            has_manifest,
        }
    }

    pub fn accepts_mutations(&self, db: &Database) -> bool {
        if self.archived {
            return false;
        }
        if matches!(
            self.lifecycle_state.as_str(),
            "archived" | "disabled" | "suspended" | "failed"
        ) {
            return false;
        }
        match self.application_id.as_deref() {
            Some(app) if self.has_manifest => application_accepts_mutations(db, app),
            Some(_) => true,
            None => false,
        }
    }
}

/// Resolve a surface id through SQLite. Fails closed when the surface is missing.
pub fn resolve_surface_lineage(db: &Database, surface_id: &str) -> DbResult<SurfaceLineage> {
    let surface = get_surface(db, surface_id)?;
    Ok(SurfaceLineage::from_surface(db, &surface))
}

/// Resolve lineage and require it to belong to `expected_application_id`.
pub fn resolve_surface_for_application(
    db: &Database,
    surface_id: &str,
    expected_application_id: &str,
) -> Result<SurfaceLineage, KernelError> {
    let lineage = resolve_surface_lineage(db, surface_id).map_err(|e| match e {
        DbError::NotFound(_) => KernelError::Validation(format!(
            "surface '{surface_id}' does not exist for application '{expected_application_id}'"
        )),
        other => KernelError::Db(other),
    })?;

    let bound = lineage.application_id.as_deref().ok_or_else(|| {
        KernelError::Validation(format!(
            "surface '{surface_id}' has no bound application lineage"
        ))
    })?;

    if bound != expected_application_id {
        return Err(KernelError::Validation(format!(
            "surface '{surface_id}' belongs to application '{bound}', not '{expected_application_id}'"
        )));
    }

    if lineage.archived || lineage.lifecycle_state == "archived" {
        return Err(KernelError::Validation(format!(
            "surface '{surface_id}' is archived and cannot receive mutations"
        )));
    }

    if !lineage.accepts_mutations(db) {
        return Err(KernelError::Validation(format!(
            "application '{expected_application_id}' is disabled, suspended, or otherwise not mutable"
        )));
    }

    crate::security::assert_not_protected(expected_application_id)
        .map_err(KernelError::Protected)?;
    crate::security::assert_not_protected(surface_id).map_err(KernelError::Protected)?;

    Ok(lineage)
}

/// Prefer bound tool_canvas / latest live surface, else canonical `surf-*` when present.
///
/// Read-only: no mutation or lifecycle gates. Mutating callers must use
/// [`resolve_application_surface`].
pub fn lookup_bound_application_surface_id(db: &Database, application_id: &str) -> Option<String> {
    let bound_id: Option<String> = db
        .conn()
        .query_row(
            "SELECT id FROM surfaces WHERE tool_id = ?1 AND archived = 0
             ORDER BY CASE WHEN placement = 'tool_canvas' THEN 0 ELSE 1 END, updated_at DESC
             LIMIT 1",
            [application_id],
            |row| row.get(0),
        )
        .ok();
    if bound_id.is_some() {
        return bound_id;
    }
    let canonical = crate::runtime_v2::surfaces::surface_id_for_tool(application_id);
    match get_surface(db, &canonical) {
        Ok(_) => Some(canonical),
        Err(_) => None,
    }
}

/// Resolve the live surface that belongs to an application for evolve OCC.
/// Prefers an explicit surface id, then conversation-scoped binding, then canonical id.
pub fn resolve_application_surface(
    db: &Database,
    application_id: &str,
    preferred_surface_id: Option<&str>,
    conversation_id: Option<&str>,
) -> Result<SurfaceLineage, KernelError> {
    if let Some(sid) = preferred_surface_id
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return resolve_surface_for_application(db, sid, application_id);
    }

    if let Some(cid) = conversation_id {
        if let Ok(Some(surf)) =
            crate::runtime_v2::surfaces::resolve_prompt_surface_for_tool(db, cid, application_id)
        {
            return resolve_surface_for_application(db, &surf.id, application_id);
        }
    }

    let sid = lookup_bound_application_surface_id(db, application_id).ok_or_else(|| {
        KernelError::Validation(format!(
            "surface for application '{application_id}' does not exist"
        ))
    })?;
    resolve_surface_for_application(db, &sid, application_id)
}

/// Optional conversation/project scope checks for evolve/apply.
#[derive(Debug, Clone, Default)]
pub struct LineageScope {
    pub conversation_id: Option<String>,
    pub project_id: Option<String>,
}

impl LineageScope {
    /// Fail closed: when the caller supplies an expected conversation/project,
    /// the resolved surface must positively prove that binding.
    /// Missing actual ownership is a reject, not a pass.
    pub fn enforce(&self, lineage: &SurfaceLineage) -> Result<(), KernelError> {
        if let Some(ref expected) = self.conversation_id {
            match lineage.conversation_id.as_deref() {
                Some(actual) if actual == expected => {}
                Some(actual) => {
                    return Err(KernelError::Validation(format!(
                        "surface '{}' belongs to conversation '{actual}', not '{expected}'",
                        lineage.surface_id
                    )));
                }
                None => {
                    return Err(KernelError::Validation(format!(
                        "surface '{}' has no conversation binding; expected '{expected}'",
                        lineage.surface_id
                    )));
                }
            }
        }
        if let Some(ref expected) = self.project_id {
            match lineage.project_id.as_deref() {
                Some(actual) if actual == expected => {}
                Some(actual) => {
                    return Err(KernelError::Validation(format!(
                        "surface '{}' belongs to project '{actual}', not '{expected}'",
                        lineage.surface_id
                    )));
                }
                None => {
                    return Err(KernelError::Validation(format!(
                        "surface '{}' has no project binding; expected '{expected}'",
                        lineage.surface_id
                    )));
                }
            }
        }
        Ok(())
    }
}

/// Resolve the authoritative application for an evolve/create binding using DB.
///
/// Prefer manifest proof. Fall back to an existing tool row / bound surface when
/// the application was created without a full manifest yet.
pub fn resolve_application_identity(
    db: &Database,
    application_id: &str,
    allow_missing_manifest: bool,
) -> Result<(), KernelError> {
    crate::security::assert_not_protected(application_id).map_err(KernelError::Protected)?;
    if authoritative_application_id(db, Some(application_id)).is_some() {
        if !application_accepts_mutations(db, application_id) {
            return Err(KernelError::Validation(format!(
                "application '{application_id}' is disabled or suspended"
            )));
        }
        return Ok(());
    }
    if allow_missing_manifest {
        // Create path: tool/manifest may not exist yet.
        return Ok(());
    }
    // Evolve without a manifest still requires durable tool or surface lineage.
    let tool_exists: bool = db
        .conn()
        .query_row(
            "SELECT 1 FROM tools WHERE id = ?1 LIMIT 1",
            [application_id],
            |_| Ok(true),
        )
        .unwrap_or(false);
    let bound_surface_exists: bool = db
        .conn()
        .query_row(
            "SELECT 1 FROM surfaces WHERE tool_id = ?1 AND archived = 0 LIMIT 1",
            [application_id],
            |_| Ok(true),
        )
        .unwrap_or(false);
    if tool_exists || bound_surface_exists {
        return Ok(());
    }
    Err(KernelError::Validation(format!(
        "application '{application_id}' has no durable lineage"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{create_conversation, Database, DEFAULT_WORKSPACE_ID};
    use crate::runtime_v2::surfaces::bind_surface_tool_id;
    use serde_json::json;
    use tempfile::tempdir;

    fn insert_tool(db: &Database, id: &str) {
        db.conn()
            .execute(
                "INSERT INTO tools (id, workspace_id, name, description, layout, definition_json, current_version, created_at, updated_at)
                 VALUES (?1, ?2, ?1, '', 'stack', '{}', 1, datetime('now'), datetime('now'))",
                rusqlite::params![id, DEFAULT_WORKSPACE_ID],
            )
            .unwrap();
    }

    #[test]
    fn resolves_bound_surface_not_spoofed_prefix() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("lineage.db")).unwrap();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "L", None).unwrap();
        insert_tool(&db, "app-real");
        let surf = crate::runtime_v2::surfaces::create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Inline",
            &json!({
                "id": "doc-inline",
                "name": "Inline",
                "layout": "stack",
                "components": [{"id": "t", "type": "text", "props": {"text": "hi"}}]
            }),
            &[],
        )
        .unwrap();
        bind_surface_tool_id(&mut db, &surf.id, "app-real").unwrap();

        let lineage = resolve_surface_for_application(&db, &surf.id, "app-real").unwrap();
        assert_eq!(lineage.application_id.as_deref(), Some("app-real"));
        // Authority is the DB binding, not whether the id looks like surf-{app}.
        assert_eq!(lineage.tool_id.as_deref(), Some("app-real"));

        // Spoofed surf-* id that is not this surface must fail closed.
        let spoof = format!("surf-app-real-spoof-{}", surf.id);
        let err = resolve_surface_for_application(&db, &spoof, "app-real")
            .expect_err("spoofed surf id must fail");
        assert!(matches!(err, KernelError::Validation(_)));

        // Wrong application claim against a real surface fails.
        insert_tool(&db, "app-other");
        let err = resolve_surface_for_application(&db, &surf.id, "app-other")
            .expect_err("cross-app claim must fail");
        assert!(matches!(err, KernelError::Validation(_)));
    }

    #[test]
    fn resolve_application_surface_prefers_bound_uuid_when_canonical_missing() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("lineage-resolve.db")).unwrap();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Resolve", None).unwrap();
        insert_tool(&db, "app-real");
        let surf = crate::runtime_v2::surfaces::create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Inline",
            &json!({
                "id": "doc-inline",
                "name": "Inline",
                "layout": "stack",
                "components": [{"id": "t", "type": "text", "props": {"text": "hi"}}]
            }),
            &[],
        )
        .unwrap();
        bind_surface_tool_id(&mut db, &surf.id, "app-real").unwrap();

        let canonical = "surf-app-real";
        assert_ne!(surf.id.as_str(), canonical);
        assert!(crate::runtime_v2::surfaces::get_surface(&db, canonical).is_err());

        let lineage =
            resolve_application_surface(&db, "app-real", None, None).expect("bound surface");
        assert_eq!(lineage.surface_id, surf.id);

        let hostile = format!("surf-app-real-hostile");
        let err = resolve_application_surface(&db, "app-real", Some(&hostile), None)
            .expect_err("preferred hostile surf-* must fail");
        assert!(matches!(err, KernelError::Validation(_)));
    }

    #[test]
    fn rejects_cross_application_surface() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("lineage-x.db")).unwrap();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "L", None).unwrap();
        insert_tool(&db, "app-a");
        insert_tool(&db, "app-b");
        let surf = crate::runtime_v2::surfaces::create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "A",
            &json!({
                "id": "doc-a",
                "name": "A",
                "layout": "stack",
                "components": [{"id": "t", "type": "text", "props": {"text": "hi"}}]
            }),
            &[],
        )
        .unwrap();
        bind_surface_tool_id(&mut db, &surf.id, "app-a").unwrap();

        let err = resolve_surface_for_application(&db, &surf.id, "app-b")
            .expect_err("cross-app must fail");
        assert!(
            matches!(err, KernelError::Validation(ref m) if m.contains("belongs to")),
            "got {err:?}"
        );
    }

    fn lineage_with(conversation_id: Option<&str>, project_id: Option<&str>) -> SurfaceLineage {
        SurfaceLineage {
            surface_id: "surf-test".into(),
            tool_id: Some("app-test".into()),
            application_id: Some("app-test".into()),
            conversation_id: conversation_id.map(str::to_string),
            project_id: project_id.map(str::to_string),
            definition_revision: 1,
            lifecycle_state: "active".into(),
            archived: false,
            has_manifest: false,
        }
    }

    #[test]
    fn lineage_scope_conversation_match_passes() {
        let scope = LineageScope {
            conversation_id: Some("conv-a".into()),
            project_id: None,
        };
        scope
            .enforce(&lineage_with(Some("conv-a"), None))
            .expect("matching conversation must pass");
    }

    #[test]
    fn lineage_scope_conversation_mismatch_rejects() {
        let scope = LineageScope {
            conversation_id: Some("conv-a".into()),
            project_id: None,
        };
        let err = scope
            .enforce(&lineage_with(Some("conv-b"), None))
            .expect_err("foreign conversation must reject");
        assert!(matches!(err, KernelError::Validation(ref m) if m.contains("belongs to")));
    }

    #[test]
    fn lineage_scope_conversation_missing_rejects() {
        let scope = LineageScope {
            conversation_id: Some("conv-a".into()),
            project_id: None,
        };
        let err = scope
            .enforce(&lineage_with(None, None))
            .expect_err("missing conversation binding must reject");
        assert!(matches!(
            err,
            KernelError::Validation(ref m) if m.contains("no conversation binding")
        ));
    }

    #[test]
    fn lineage_scope_project_match_passes() {
        let scope = LineageScope {
            conversation_id: None,
            project_id: Some("proj-a".into()),
        };
        scope
            .enforce(&lineage_with(None, Some("proj-a")))
            .expect("matching project must pass");
    }

    #[test]
    fn lineage_scope_project_mismatch_rejects() {
        let scope = LineageScope {
            conversation_id: None,
            project_id: Some("proj-a".into()),
        };
        let err = scope
            .enforce(&lineage_with(None, Some("proj-b")))
            .expect_err("foreign project must reject");
        assert!(matches!(err, KernelError::Validation(ref m) if m.contains("belongs to")));
    }

    #[test]
    fn lineage_scope_project_missing_rejects() {
        let scope = LineageScope {
            conversation_id: None,
            project_id: Some("proj-a".into()),
        };
        let err = scope
            .enforce(&lineage_with(None, None))
            .expect_err("missing project binding must reject");
        assert!(matches!(
            err,
            KernelError::Validation(ref m) if m.contains("no project binding")
        ));
    }

    #[test]
    fn lineage_scope_both_supplied_one_missing_rejects() {
        let scope = LineageScope {
            conversation_id: Some("conv-a".into()),
            project_id: Some("proj-a".into()),
        };
        let err = scope
            .enforce(&lineage_with(Some("conv-a"), None))
            .expect_err("missing project when both expected must reject");
        assert!(matches!(
            err,
            KernelError::Validation(ref m) if m.contains("no project binding")
        ));
    }

    #[test]
    fn lineage_scope_none_supplied_preserves_pass() {
        let scope = LineageScope::default();
        scope
            .enforce(&lineage_with(None, None))
            .expect("no expected scope must pass");
        scope
            .enforce(&lineage_with(Some("conv-a"), Some("proj-a")))
            .expect("no expected scope must pass even with bindings");
    }
}
