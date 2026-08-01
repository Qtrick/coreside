//! Runtime action grants — remembered authority for registered actions.
//!
//! A grant is *not* a permission. Permissions (`application_permissions`) say
//! which capability categories an application may ever touch, and only the user
//! grants those. A runtime grant only remembers that the user already approved
//! a particular call shape so Coreside stops asking every time.

use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::db::{now_rfc3339, Database, DbError, DbResult};
use rusqlite::params;

use super::context::{ActionRunContext, Presence};
use super::descriptor::{ActionDescriptor, ActionRisk};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantScope {
    /// Only this exact input, for this application.
    Exact,
    /// Any input for this action, for this application.
    Action,
    /// Any input for this action, bound to one application (used for away runs).
    ApplicationAction,
}

impl GrantScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Action => "action",
            Self::ApplicationAction => "application_action",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "exact" => Some(Self::Exact),
            "action" => Some(Self::Action),
            "application_action" => Some(Self::ApplicationAction),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantDuration {
    Once,
    Session,
    Standing,
}

impl GrantDuration {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Once => "once",
            Self::Session => "session",
            Self::Standing => "standing",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "once" => Some(Self::Once),
            "session" => Some(Self::Session),
            "standing" => Some(Self::Standing),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGrant {
    pub id: String,
    pub subject: String,
    pub application_id: Option<String>,
    pub action_name: String,
    pub descriptor_hash: String,
    pub scope_kind: String,
    pub scope: serde_json::Value,
    pub duration: String,
    pub session_id: Option<String>,
    pub source: String,
    pub status: String,
    pub granted_at: String,
    pub revoked_at: Option<String>,
    pub expires_at: Option<String>,
}

pub const DEFAULT_SUBJECT: &str = "local-user";

/// Mint a grant. Trusted callers only — reached through an approved approval.
pub fn mint_grant(
    db: &mut Database,
    ctx: &ActionRunContext,
    descriptor: &ActionDescriptor,
    scope: GrantScope,
    duration: GrantDuration,
    input_hash: Option<&str>,
    source: &str,
) -> DbResult<RuntimeGrant> {
    // Destructive and critical always require a fresh present-user confirmation;
    // remembering them (session or standing) would create dead or dangerous authority.
    if descriptor.risk == ActionRisk::Destructive {
        return Err(DbError::Invalid(
            "destructive actions cannot be remembered".into(),
        ));
    }
    if descriptor.critical {
        return Err(DbError::Invalid(
            "critical actions cannot be remembered".into(),
        ));
    }
    if scope == GrantScope::Exact && input_hash.is_none() {
        return Err(DbError::Invalid(
            "exact-scope grants require an input hash".into(),
        ));
    }
    if scope == GrantScope::ApplicationAction && ctx.application_id.is_none() {
        return Err(DbError::Invalid(
            "application-bound grants require an application".into(),
        ));
    }

    let id = format!("grant-{}", Uuid::new_v4());
    let scope_json = match input_hash {
        Some(hash) => json!({ "inputHash": hash }),
        None => json!({}),
    };
    let session = match duration {
        GrantDuration::Standing => None,
        _ => Some(ctx.session_id.clone()),
    };
    db.conn().execute(
        "INSERT INTO runtime_action_grants (
            id, subject, application_id, action_name, descriptor_hash, scope_kind,
            scope_json, duration, session_id, source, status, granted_at
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'active',?11)",
        params![
            id,
            DEFAULT_SUBJECT,
            ctx.application_id,
            descriptor.name,
            descriptor.descriptor_hash(),
            scope.as_str(),
            scope_json.to_string(),
            duration.as_str(),
            session,
            source,
            now_rfc3339()
        ],
    )?;
    get_grant(db, &id)
}

pub fn get_grant(db: &Database, id: &str) -> DbResult<RuntimeGrant> {
    db.conn()
        .query_row(
            &format!("SELECT {GRANT_COLS} FROM runtime_action_grants WHERE id = ?1"),
            [id],
            map_grant,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(format!("grant {id}")),
            other => DbError::Sqlite(other),
        })
}

const GRANT_COLS: &str = "id, subject, application_id, action_name, descriptor_hash, scope_kind,
    scope_json, duration, session_id, source, status, granted_at, revoked_at, expires_at";

fn map_grant(row: &rusqlite::Row<'_>) -> rusqlite::Result<RuntimeGrant> {
    let scope_s: String = row.get(6)?;
    Ok(RuntimeGrant {
        id: row.get(0)?,
        subject: row.get(1)?,
        application_id: row.get(2)?,
        action_name: row.get(3)?,
        descriptor_hash: row.get(4)?,
        scope_kind: row.get(5)?,
        scope: serde_json::from_str(&scope_s).unwrap_or_else(|_| json!({})),
        duration: row.get(7)?,
        session_id: row.get(8)?,
        source: row.get(9)?,
        status: row.get(10)?,
        granted_at: row.get(11)?,
        revoked_at: row.get(12)?,
        expires_at: row.get(13)?,
    })
}

pub fn list_grants(db: &Database, application_id: Option<&str>) -> DbResult<Vec<RuntimeGrant>> {
    let mut out = Vec::new();
    if let Some(app) = application_id {
        let mut stmt = db.conn().prepare(&format!(
            "SELECT {GRANT_COLS} FROM runtime_action_grants
             WHERE application_id = ?1 ORDER BY granted_at DESC LIMIT 500"
        ))?;
        for row in stmt.query_map([app], map_grant)? {
            out.push(row?);
        }
    } else {
        let mut stmt = db.conn().prepare(&format!(
            "SELECT {GRANT_COLS} FROM runtime_action_grants ORDER BY granted_at DESC LIMIT 500"
        ))?;
        for row in stmt.query_map([], map_grant)? {
            out.push(row?);
        }
    }
    Ok(out)
}

pub fn revoke_grant(db: &mut Database, id: &str) -> DbResult<()> {
    let n = db.conn().execute(
        "UPDATE runtime_action_grants SET status = 'revoked', revoked_at = ?2
         WHERE id = ?1 AND status = 'active'",
        params![id, now_rfc3339()],
    )?;
    if n == 0 {
        return Err(DbError::NotFound(format!("active grant {id}")));
    }
    Ok(())
}

/// Revoke every active grant for an application (used when permissions are
/// revoked or an application is disabled).
pub fn revoke_grants_for_application(db: &mut Database, application_id: &str) -> DbResult<u64> {
    let n = db.conn().execute(
        "UPDATE runtime_action_grants SET status = 'revoked', revoked_at = ?2
         WHERE application_id = ?1 AND status = 'active'",
        params![application_id, now_rfc3339()],
    )?;
    Ok(n as u64)
}

/// Find a grant that authorizes this call, or `None`.
pub fn match_grant(
    db: &Database,
    ctx: &ActionRunContext,
    descriptor: &ActionDescriptor,
    input_hash: &str,
) -> DbResult<Option<RuntimeGrant>> {
    let hash = descriptor.descriptor_hash();
    let mut stmt = db.conn().prepare(&format!(
        "SELECT {GRANT_COLS} FROM runtime_action_grants
         WHERE status = 'active' AND action_name = ?1 AND descriptor_hash = ?2
         ORDER BY granted_at DESC LIMIT 200"
    ))?;
    let rows = stmt.query_map(params![descriptor.name, hash], map_grant)?;
    for grant in rows.flatten() {
        if grant_matches(&grant, ctx, descriptor, input_hash) {
            return Ok(Some(grant));
        }
    }
    Ok(None)
}

fn grant_matches(
    grant: &RuntimeGrant,
    ctx: &ActionRunContext,
    descriptor: &ActionDescriptor,
    input_hash: &str,
) -> bool {
    if grant.status != "active" {
        return false;
    }
    if let Some(expires) = grant.expires_at.as_deref() {
        if expires <= now_rfc3339().as_str() {
            return false;
        }
    }
    // Application binding: a grant minted for an application never authorizes a
    // different application, and an unbound grant never authorizes an app run.
    if grant.application_id.as_deref() != ctx.application_id.as_deref() {
        return false;
    }
    let duration = match GrantDuration::parse(&grant.duration) {
        Some(d) => d,
        None => return false,
    };
    if duration != GrantDuration::Standing
        && grant.session_id.as_deref() != Some(ctx.session_id.as_str())
    {
        return false;
    }
    let scope = match GrantScope::parse(&grant.scope_kind) {
        Some(s) => s,
        None => return false,
    };
    if scope == GrantScope::Exact
        && grant.scope.get("inputHash").and_then(|v| v.as_str()) != Some(input_hash)
    {
        return false;
    }

    if ctx.presence == Presence::Away {
        // Away runs only honor standing, application-bound authority minted by
        // the user or an automation. Destructive and critical never qualify.
        if descriptor.risk == ActionRisk::Destructive || descriptor.critical {
            return false;
        }
        if duration != GrantDuration::Standing {
            return false;
        }
        if scope != GrantScope::ApplicationAction || grant.application_id.is_none() {
            return false;
        }
        if !matches!(grant.source.as_str(), "automation" | "user") {
            return false;
        }
    }
    true
}

/// A once-grant is spent as soon as it authorizes a call.
pub fn consume_once_grant(db: &mut Database, grant: &RuntimeGrant) -> DbResult<()> {
    if grant.duration != GrantDuration::Once.as_str() {
        return Ok(());
    }
    db.conn().execute(
        "UPDATE runtime_action_grants SET status = 'consumed', revoked_at = ?2 WHERE id = ?1",
        params![grant.id, now_rfc3339()],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application_kernel::registered_actions::context::Venue;
    use crate::application_kernel::registered_actions::descriptor::find_action;
    use crate::application_kernel::registered_actions::testing::test_db;

    fn ctx(app: Option<&str>) -> ActionRunContext {
        ActionRunContext {
            actor: "user".into(),
            venue: Venue::Application,
            presence: Presence::Present,
            application_id: app.map(|s| s.to_string()),
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

    #[test]
    fn action_scope_grant_matches_any_input() {
        let (mut db, _dir) = test_db();
        let d = find_action("local_data.write").unwrap();
        let c = ctx(Some("app-1"));
        mint_grant(&mut db, &c, d, GrantScope::Action, GrantDuration::Session, None, "user").unwrap();
        assert!(match_grant(&db, &c, d, "hash-a").unwrap().is_some());
        assert!(match_grant(&db, &c, d, "hash-b").unwrap().is_some());
    }

    #[test]
    fn exact_scope_grant_only_matches_same_input() {
        let (mut db, _dir) = test_db();
        let d = find_action("local_data.write").unwrap();
        let c = ctx(Some("app-1"));
        mint_grant(
            &mut db,
            &c,
            d,
            GrantScope::Exact,
            GrantDuration::Session,
            Some("hash-a"),
            "user",
        )
        .unwrap();
        assert!(match_grant(&db, &c, d, "hash-a").unwrap().is_some());
        assert!(match_grant(&db, &c, d, "hash-b").unwrap().is_none());
    }

    #[test]
    fn grant_does_not_cross_applications() {
        let (mut db, _dir) = test_db();
        let d = find_action("local_data.write").unwrap();
        mint_grant(
            &mut db,
            &ctx(Some("app-1")),
            d,
            GrantScope::Action,
            GrantDuration::Session,
            None,
            "user",
        )
        .unwrap();
        assert!(match_grant(&db, &ctx(Some("app-2")), d, "h").unwrap().is_none());
        assert!(match_grant(&db, &ctx(None), d, "h").unwrap().is_none());
    }

    #[test]
    fn session_grant_does_not_match_other_session() {
        let (mut db, _dir) = test_db();
        let d = find_action("local_data.write").unwrap();
        mint_grant(
            &mut db,
            &ctx(Some("app-1")),
            d,
            GrantScope::Action,
            GrantDuration::Session,
            None,
            "user",
        )
        .unwrap();
        let mut other = ctx(Some("app-1"));
        other.session_id = "session-other".into();
        assert!(match_grant(&db, &other, d, "h").unwrap().is_none());
    }

    #[test]
    fn destructive_cannot_be_remembered() {
        let (mut db, _dir) = test_db();
        let d = find_action("local_data.delete").unwrap();
        for duration in [GrantDuration::Session, GrantDuration::Standing, GrantDuration::Once] {
            let err = mint_grant(
                &mut db,
                &ctx(Some("app-1")),
                d,
                GrantScope::Exact,
                duration,
                Some("h"),
                "user",
            );
            assert!(err.is_err(), "{duration:?} must be refused");
        }
    }

    #[test]
    fn critical_cannot_be_remembered() {
        let (mut db, _dir) = test_db();
        let d = find_action("export.prepare").unwrap();
        for duration in [GrantDuration::Session, GrantDuration::Standing] {
            assert!(
                mint_grant(
                    &mut db,
                    &ctx(Some("app-1")),
                    d,
                    GrantScope::Action,
                    duration,
                    None,
                    "user"
                )
                .is_err(),
                "{duration:?} must be refused"
            );
        }
    }

    #[test]
    fn away_requires_standing_application_bound_grant() {
        let (mut db, _dir) = test_db();
        let d = find_action("local_data.write").unwrap();
        let present = ctx(Some("app-1"));
        // A session action-scope grant is enough while present, not while away.
        mint_grant(&mut db, &present, d, GrantScope::Action, GrantDuration::Session, None, "user")
            .unwrap();
        let mut away = ctx(Some("app-1"));
        away.presence = Presence::Away;
        assert!(match_grant(&db, &present, d, "h").unwrap().is_some());
        assert!(match_grant(&db, &away, d, "h").unwrap().is_none());

        mint_grant(
            &mut db,
            &present,
            d,
            GrantScope::ApplicationAction,
            GrantDuration::Standing,
            None,
            "automation",
        )
        .unwrap();
        assert!(match_grant(&db, &away, d, "h").unwrap().is_some());
    }

    #[test]
    fn revoked_grant_stops_matching() {
        let (mut db, _dir) = test_db();
        let d = find_action("local_data.write").unwrap();
        let c = ctx(Some("app-1"));
        let g = mint_grant(&mut db, &c, d, GrantScope::Action, GrantDuration::Session, None, "user")
            .unwrap();
        revoke_grant(&mut db, &g.id).unwrap();
        assert!(match_grant(&db, &c, d, "h").unwrap().is_none());
    }

    #[test]
    fn once_grant_is_spent_after_use() {
        let (mut db, _dir) = test_db();
        let d = find_action("local_data.write").unwrap();
        let c = ctx(Some("app-1"));
        let g = mint_grant(
            &mut db,
            &c,
            d,
            GrantScope::Exact,
            GrantDuration::Once,
            Some("hash-a"),
            "user",
        )
        .unwrap();
        assert!(match_grant(&db, &c, d, "hash-a").unwrap().is_some());
        consume_once_grant(&mut db, &g).unwrap();
        assert!(match_grant(&db, &c, d, "hash-a").unwrap().is_none());
    }
}
