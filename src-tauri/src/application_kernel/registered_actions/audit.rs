//! Bounded, redacted audit trail for registered action execution.
//!
//! This is an operational record of *what ran*, not model reasoning. It never
//! stores prompts, provider payloads, credentials, or raw inputs.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{now_rfc3339, Database, DbResult};
use rusqlite::params;

use super::context::ActionRunContext;

pub const MAX_AUDIT_EVENTS: i64 = 2_000;
const MAX_DETAIL_CHARS: usize = 300;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEvent {
    pub id: String,
    pub kind: String,
    pub actor: String,
    pub venue: String,
    pub presence: String,
    pub application_id: Option<String>,
    pub project_id: Option<String>,
    pub conversation_id: Option<String>,
    pub run_id: Option<String>,
    pub action_name: Option<String>,
    pub input_preview: Option<String>,
    pub outcome: String,
    pub risk: Option<String>,
    pub decision_source: Option<String>,
    pub approval_id: Option<String>,
    pub grant_id: Option<String>,
    pub detail: Option<String>,
    pub duration_ms: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Default)]
pub struct AuditInput<'a> {
    pub kind: &'a str,
    pub action_name: Option<&'a str>,
    pub input_preview: Option<&'a str>,
    pub outcome: &'a str,
    pub risk: Option<&'a str>,
    pub decision_source: Option<&'a str>,
    pub approval_id: Option<&'a str>,
    pub grant_id: Option<&'a str>,
    pub detail: Option<&'a str>,
    pub duration_ms: Option<i64>,
}

pub fn append_event(
    db: &mut Database,
    ctx: &ActionRunContext,
    event: AuditInput<'_>,
) -> DbResult<()> {
    let detail = event.detail.map(sanitize);
    let preview = event.input_preview.map(sanitize);
    db.conn().execute(
        "INSERT INTO runtime_audit_events (
            id, kind, actor, venue, presence, application_id, project_id, conversation_id,
            run_id, action_name, input_preview, outcome, risk, decision_source,
            approval_id, grant_id, detail, duration_ms, created_at
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)",
        params![
            format!("audit-{}", Uuid::new_v4()),
            event.kind,
            ctx.actor,
            ctx.venue.as_str(),
            ctx.presence.as_str(),
            ctx.application_id,
            ctx.project_id,
            ctx.conversation_id,
            ctx.run_id,
            event.action_name,
            preview,
            event.outcome,
            event.risk,
            event.decision_source,
            event.approval_id,
            event.grant_id,
            detail,
            event.duration_ms,
            now_rfc3339()
        ],
    )?;
    trim(db)?;
    Ok(())
}

fn sanitize(text: &str) -> String {
    crate::security::redact_secrets(text, None)
        .chars()
        .take(MAX_DETAIL_CHARS)
        .collect()
}

fn trim(db: &Database) -> DbResult<()> {
    db.conn().execute(
        "DELETE FROM runtime_audit_events WHERE id NOT IN (
            SELECT id FROM runtime_audit_events ORDER BY created_at DESC, rowid DESC LIMIT ?1
         )",
        [MAX_AUDIT_EVENTS],
    )?;
    Ok(())
}

pub fn list_events(
    db: &Database,
    application_id: Option<&str>,
    limit: usize,
) -> DbResult<Vec<AuditEvent>> {
    let lim = limit.clamp(1, 500) as i64;
    let cols = "id, kind, actor, venue, presence, application_id, project_id, conversation_id,
        run_id, action_name, input_preview, outcome, risk, decision_source, approval_id,
        grant_id, detail, duration_ms, created_at";
    let mut out = Vec::new();
    if let Some(app) = application_id {
        let mut stmt = db.conn().prepare(&format!(
            "SELECT {cols} FROM runtime_audit_events WHERE application_id = ?1
             ORDER BY created_at DESC, rowid DESC LIMIT ?2"
        ))?;
        for row in stmt.query_map(params![app, lim], map_event)? {
            out.push(row?);
        }
    } else {
        let mut stmt = db.conn().prepare(&format!(
            "SELECT {cols} FROM runtime_audit_events ORDER BY created_at DESC, rowid DESC LIMIT ?1"
        ))?;
        for row in stmt.query_map([lim], map_event)? {
            out.push(row?);
        }
    }
    Ok(out)
}

fn map_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<AuditEvent> {
    Ok(AuditEvent {
        id: row.get(0)?,
        kind: row.get(1)?,
        actor: row.get(2)?,
        venue: row.get(3)?,
        presence: row.get(4)?,
        application_id: row.get(5)?,
        project_id: row.get(6)?,
        conversation_id: row.get(7)?,
        run_id: row.get(8)?,
        action_name: row.get(9)?,
        input_preview: row.get(10)?,
        outcome: row.get(11)?,
        risk: row.get(12)?,
        decision_source: row.get(13)?,
        approval_id: row.get(14)?,
        grant_id: row.get(15)?,
        detail: row.get(16)?,
        duration_ms: row.get(17)?,
        created_at: row.get(18)?,
    })
}

/// Clear history. Refused while an approval is live or a grant is active, so
/// clearing the log can never be used to hide authority that still applies.
pub fn clear_events(db: &mut Database) -> DbResult<u64> {
    if super::approvals::has_live_approvals(db)? {
        return Err(crate::db::DbError::Invalid(
            "resolve pending approvals before clearing history".into(),
        ));
    }
    let active_grants: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM runtime_action_grants WHERE status = 'active'",
        [],
        |r| r.get(0),
    )?;
    if active_grants > 0 {
        return Err(crate::db::DbError::Invalid(
            "revoke remembered permissions before clearing history".into(),
        ));
    }
    let n = db.conn().execute("DELETE FROM runtime_audit_events", [])?;
    Ok(n as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application_kernel::registered_actions::context::{Presence, Venue};
    use crate::application_kernel::registered_actions::descriptor::find_action;
    use crate::application_kernel::registered_actions::grants::{
        mint_grant, GrantDuration, GrantScope,
    };
    use crate::application_kernel::registered_actions::testing::test_db;

    fn ctx() -> ActionRunContext {
        ActionRunContext {
            actor: "user".into(),
            venue: Venue::Application,
            presence: Presence::Present,
            application_id: Some("app-1".into()),
            project_id: None,
            conversation_id: None,
            session_id: "session-test".into(),
            run_id: "run-test".into(),
            trigger: None,
            surface_id: None,
            component_id: None,
            depth: 0,
        }
    }

    fn event<'a>(outcome: &'a str) -> AuditInput<'a> {
        AuditInput {
            kind: "action",
            action_name: Some("local_data.query"),
            outcome,
            risk: Some("read"),
            ..Default::default()
        }
    }

    #[test]
    fn appends_and_lists() {
        let (mut db, _dir) = test_db();
        append_event(&mut db, &ctx(), event("ok")).unwrap();
        let events = list_events(&db, Some("app-1"), 10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].outcome, "ok");
        assert!(list_events(&db, Some("app-2"), 10).unwrap().is_empty());
    }

    #[test]
    fn redacts_secret_shaped_detail() {
        let (mut db, _dir) = test_db();
        let mut e = event("error");
        e.detail = Some("failed with AIzaSyA1234567890abcdefghijklmno");
        append_event(&mut db, &ctx(), e).unwrap();
        let stored = list_events(&db, None, 1).unwrap();
        let detail = stored[0].detail.clone().unwrap();
        assert!(!detail.contains("AIzaSyA1234567890abcdefghijklmno"));
        assert!(detail.contains("[REDACTED]"));
    }

    #[test]
    fn clear_is_blocked_while_authority_is_live() {
        let (mut db, _dir) = test_db();
        append_event(&mut db, &ctx(), event("ok")).unwrap();
        let d = find_action("local_data.write").unwrap();
        let g = mint_grant(
            &mut db,
            &ctx(),
            d,
            GrantScope::Action,
            GrantDuration::Session,
            None,
            "user",
        )
        .unwrap();
        assert!(clear_events(&mut db).is_err());
        crate::application_kernel::registered_actions::grants::revoke_grant(&mut db, &g.id)
            .unwrap();
        assert_eq!(clear_events(&mut db).unwrap(), 1);
    }
}
