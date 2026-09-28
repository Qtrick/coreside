//! Authoritative persistence for rules-engine interactive surfaces.
//!
//! A surface whose SoftwareDocument carries an `interactive` definition owns a
//! subset of its surface state (declared, derived and engine-reserved keys).
//! Only [`dispatch`] may change those keys: it validates identity, revision,
//! event idempotency, actor and parameters, executes the transition in Rust, and
//! commits state + revision + action log atomically. Renderer saves and agent
//! `state.*` operations cannot modify owned keys.
//!
//! The action log is append-only. Rows carrying `snapshot_json` are checkpoints
//! (initialization, adoption after a definition change, undo). Any historical
//! state is reconstructed by re-executing rows after the nearest checkpoint and
//! verifying each post-state hash.

use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::rules_engine::{InteractiveAppDefinition, LegalActionSummary};
use super::surfaces::{get_surface, get_surface_state_with_revision};
use crate::db::{now_rfc3339, Database, DbError, DbResult};

const MAX_EVENT_ID_LEN: usize = 128;
const CHECKPOINT_INTERVAL: i64 = 500;
/// Bound the history UI/API payload; older rows remain reconstructible via seq.
const MAX_HISTORY_ENTRIES: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    User,
    Ai,
}

impl Origin {
    fn as_str(self) -> &'static str {
        match self {
            Origin::User => "user",
            Origin::Ai => "ai",
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InteractiveView {
    pub surface_id: String,
    pub application_id: String,
    pub state_revision: i64,
    /// Surface state with engine internals and read-restricted keys removed.
    pub state: Value,
    /// Actor a local user gesture acts as; `None` while an AI actor must move.
    pub actor: Option<String>,
    pub waiting_for: Option<String>,
    pub status: String,
    pub winner: Option<String>,
    pub message: Option<String>,
    pub legal_actions: Vec<LegalActionSummary>,
    pub seq: i64,
    pub duplicate: bool,
    pub reinitialized: bool,
}

/// Parse the interactive definition embedded in a surface definition, if any.
/// A present-but-invalid definition is an error, never silently ignored.
pub fn interactive_definition_of(
    definition: &Value,
) -> Result<Option<InteractiveAppDefinition>, String> {
    match definition.get("interactive") {
        None | Some(Value::Null) => Ok(None),
        Some(v) => InteractiveAppDefinition::parse_and_admit(v).map(Some),
    }
}

fn require_definition(definition: &Value) -> DbResult<InteractiveAppDefinition> {
    interactive_definition_of(definition)
        .map_err(DbError::Invalid)?
        .ok_or_else(|| DbError::Invalid("surface is not an interactive application".into()))
}

/// Reject any change to engine-owned keys outside [`dispatch`]. Writing an
/// owned key with its current value is allowed so whole-state saves from the
/// renderer keep working.
pub fn guard_owned_key_writes(
    definition: &Value,
    current: &Value,
    patch: &Value,
) -> Result<(), String> {
    let Some(def) = interactive_definition_of(definition)? else {
        return Ok(());
    };
    let Some(patch) = patch.as_object() else {
        return Ok(());
    };
    let owned = def.owned_keys();
    for (k, v) in patch {
        if owned.contains(k) && current.get(k) != Some(v) {
            return Err(format!(
                "state key '{k}' is owned by the interactive rules engine; use an interactive action"
            ));
        }
    }
    Ok(())
}

fn split_owned(def: &InteractiveAppDefinition, state: &Value) -> (Value, Map<String, Value>) {
    let owned = def.owned_keys();
    let mut mine = Map::new();
    let mut rest = Map::new();
    if let Some(obj) = state.as_object() {
        for (k, v) in obj {
            if owned.contains(k) {
                mine.insert(k.clone(), v.clone());
            } else {
                rest.insert(k.clone(), v.clone());
            }
        }
    }
    (Value::Object(mine), rest)
}

fn merge(owned: &Value, mut rest: Map<String, Value>) -> Value {
    if let Some(obj) = owned.as_object() {
        for (k, v) in obj {
            rest.insert(k.clone(), v.clone());
        }
    }
    Value::Object(rest)
}

fn canonical(v: &Value) -> String {
    match v {
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            let inner: Vec<String> = keys
                .iter()
                .map(|k| format!("{}:{}", Value::String((*k).clone()), canonical(&m[*k])))
                .collect();
            format!("{{{}}}", inner.join(","))
        }
        Value::Array(a) => format!(
            "[{}]",
            a.iter().map(canonical).collect::<Vec<_>>().join(",")
        ),
        other => other.to_string(),
    }
}

pub fn state_hash(v: &Value) -> String {
    hex::encode(Sha256::digest(canonical(v).as_bytes()))
}

#[derive(Debug, Clone)]
struct LogRow {
    seq: i64,
    event_id: String,
    action_id: String,
    actor: String,
    params: Value,
    definition_revision: i64,
    post_state_hash: String,
    snapshot: Option<Value>,
}

fn corrupted(what: &str, e: impl std::fmt::Display) -> DbError {
    DbError::Invalid(format!("corrupted interactive {what}: {e}"))
}

fn is_constraint_violation(err: &DbError) -> bool {
    matches!(
        err,
        DbError::Sqlite(rusqlite::Error::SqliteFailure(e, _))
            if e.code == rusqlite::ErrorCode::ConstraintViolation
    )
}

fn event_exists(db: &Database, surface_id: &str, event_id: &str) -> DbResult<bool> {
    Ok(db
        .conn()
        .query_row(
            "SELECT 1 FROM interactive_action_log WHERE surface_id = ?1 AND event_id = ?2",
            params![surface_id, event_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

/// Soft-idempotent recovery when a concurrent retry lost the UNIQUE(event_id) race.
fn soft_duplicate_view(
    db: &mut Database,
    surface_id: &str,
    event_id: &str,
    err: DbError,
) -> DbResult<InteractiveView> {
    if is_constraint_violation(&err) {
        if event_exists(db, surface_id, event_id)? {
            let loaded = ensure_initialized(db, surface_id)?;
            return Ok(view_of(surface_id, &loaded, None, true));
        }
        return Err(DbError::Conflict(format!(
            "interactive commit conflict on surface '{surface_id}'"
        )));
    }
    Err(err)
}

fn read_rows(db: &Database, surface_id: &str, upto: Option<i64>) -> DbResult<Vec<LogRow>> {
    let mut stmt = db.conn().prepare(
        "SELECT seq, event_id, action_id, actor, params_json, definition_revision, post_state_hash, snapshot_json
         FROM interactive_action_log WHERE surface_id = ?1 AND seq <= ?2 ORDER BY seq",
    )?;
    let rows = stmt.query_map(params![surface_id, upto.unwrap_or(i64::MAX)], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, i64>(5)?,
            r.get::<_, String>(6)?,
            r.get::<_, Option<String>>(7)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (seq, event_id, action_id, actor, params_json, def_rev, hash, snap) = row?;
        out.push(LogRow {
            seq,
            event_id,
            action_id,
            actor,
            params: serde_json::from_str(&params_json)
                .map_err(|e| corrupted("action params", e))?,
            definition_revision: def_rev,
            post_state_hash: hash,
            snapshot: snap
                .map(|s| serde_json::from_str(&s).map_err(|e| corrupted("checkpoint", e)))
                .transpose()?,
        });
    }
    Ok(out)
}

fn last_row(db: &Database, surface_id: &str) -> DbResult<Option<(i64, i64)>> {
    Ok(db
        .conn()
        .query_row(
            "SELECT seq, definition_revision FROM interactive_action_log
             WHERE surface_id = ?1 ORDER BY seq DESC LIMIT 1",
            [surface_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?)
}

fn last_checkpoint_seq(db: &Database, surface_id: &str) -> DbResult<i64> {
    Ok(db.conn().query_row(
        "SELECT COALESCE(MAX(seq), 0) FROM interactive_action_log
             WHERE surface_id = ?1 AND snapshot_json IS NOT NULL",
        [surface_id],
        |r| r.get(0),
    )?)
}

#[allow(clippy::too_many_arguments)]
fn commit(
    db: &mut Database,
    surface_id: &str,
    new_state: &Value,
    expected_revision: i64,
    seq: i64,
    event_id: &str,
    action_id: &str,
    actor: &str,
    origin: &str,
    params_v: &Value,
    definition_revision: i64,
    owned_after: &Value,
    snapshot: Option<&Value>,
) -> DbResult<i64> {
    let state_json = new_state.to_string();
    if state_json.len() > super::limits::MAX_STATE_JSON_BYTES {
        return Err(DbError::Invalid("surface state too large".into()));
    }
    let now = now_rfc3339();
    let hash = state_hash(owned_after);
    let params_json = params_v.to_string();
    let snapshot_json = snapshot.map(Value::to_string);
    db.with_transaction(|conn| {
        let updated = conn.execute(
            "UPDATE surface_state SET state_json = ?1, state_revision = state_revision + 1, updated_at = ?2
             WHERE surface_id = ?3 AND state_revision = ?4",
            params![state_json, now, surface_id, expected_revision],
        )?;
        if updated == 0 {
            let exists: bool = conn
                .query_row(
                    "SELECT 1 FROM surface_state WHERE surface_id = ?1",
                    [surface_id],
                    |_| Ok(true),
                )
                .optional()?
                .unwrap_or(false);
            if exists || expected_revision > 1 {
                return Err(DbError::Conflict(format!(
                    "state revision conflict on surface '{surface_id}'"
                )));
            }
            conn.execute(
                "INSERT INTO surface_state (surface_id, state_json, state_revision, updated_at) VALUES (?1, ?2, 2, ?3)",
                params![surface_id, state_json, now],
            )?;
        }
        conn.execute(
            "INSERT INTO interactive_action_log (id, surface_id, seq, event_id, action_id, actor, origin,
                params_json, definition_revision, pre_state_revision, post_state_revision, post_state_hash,
                snapshot_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                format!("ial-{}", Uuid::new_v4()),
                surface_id,
                seq,
                event_id,
                action_id,
                actor,
                origin,
                params_json,
                definition_revision,
                expected_revision,
                expected_revision + 1,
                hash,
                snapshot_json,
                now
            ],
        )?;
        Ok(expected_revision + 1)
    })
}

struct Loaded {
    def: InteractiveAppDefinition,
    /// Full surface definition (for projecting non-owned rest through top-level contracts).
    surface_definition: Value,
    owned: Value,
    rest: Map<String, Value>,
    revision: i64,
    definition_revision: i64,
    seq: i64,
    reinitialized: bool,
}

/// Load authoritative state, creating a checkpoint on first use or after the
/// definition changed. Corrupt state under an unchanged definition fails closed.
fn ensure_initialized(db: &mut Database, surface_id: &str) -> DbResult<Loaded> {
    let surface = get_surface(db, surface_id)?;
    let def = require_definition(&surface.definition)?;
    let (state, revision) = get_surface_state_with_revision(db, surface_id)?;
    let (owned, rest) = split_owned(&def, &state);
    let last = last_row(db, surface_id)?;
    if let Some((seq, def_rev)) = last {
        if def_rev == surface.current_revision {
            def.validate_state(&owned)
                .map_err(|e| corrupted("state", e))?;
            return Ok(Loaded {
                def,
                surface_definition: surface.definition,
                owned,
                rest,
                revision,
                definition_revision: def_rev,
                seq,
                reinitialized: false,
            });
        }
    }
    let (next_owned, action, reinit) = match def.validate_state(&owned) {
        Ok(()) => (owned, "__adopt", false),
        Err(_) => (
            def.initial_instance_state().map_err(DbError::Invalid)?,
            "__init",
            true,
        ),
    };
    let seq = last.map(|(s, _)| s).unwrap_or(0) + 1;
    let merged = merge(&next_owned, rest.clone());
    let new_rev = commit(
        db,
        surface_id,
        &merged,
        revision,
        seq,
        &format!("{action}-{seq}"),
        action,
        "system",
        "system",
        &Value::Null,
        surface.current_revision,
        &next_owned,
        Some(&next_owned),
    )?;
    Ok(Loaded {
        def,
        surface_definition: surface.definition,
        owned: next_owned,
        rest,
        revision: new_rev,
        definition_revision: surface.current_revision,
        seq,
        reinitialized: reinit,
    })
}

fn view_of(
    surface_id: &str,
    l: &Loaded,
    message: Option<String>,
    duplicate: bool,
) -> InteractiveView {
    let (actor, waiting_for) = match l.def.resolve_user_actor(&l.owned) {
        Ok(a) => (Some(a), None),
        Err(_) => (
            None,
            l.owned
                .get("currentPlayer")
                .and_then(Value::as_str)
                .map(str::to_string),
        ),
    };
    let legal = actor
        .as_deref()
        .or(waiting_for.as_deref())
        .map(|a| l.def.get_legal_actions(&l.owned, a))
        .unwrap_or_default();
    // Public interactive state + non-owned rest filtered by top-level contracts.
    // Never merge raw `rest` — restricted/sensitive keys must not leak via incidental state.
    let public = super::visibility::project_surface_state(
        &l.surface_definition,
        &merge(&l.owned, l.rest.clone()),
        super::visibility::Audience::Renderer,
    );
    InteractiveView {
        surface_id: surface_id.to_string(),
        application_id: l.def.id.clone(),
        state_revision: l.revision,
        state: public,
        actor,
        waiting_for,
        status: l
            .owned
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("active")
            .to_string(),
        winner: l
            .owned
            .get("winner")
            .and_then(Value::as_str)
            .map(str::to_string),
        message,
        legal_actions: legal,
        seq: l.seq,
        duplicate,
        reinitialized: l.reinitialized,
    }
}

pub fn get_view(db: &mut Database, surface_id: &str) -> DbResult<InteractiveView> {
    let loaded = ensure_initialized(db, surface_id)?;
    Ok(view_of(surface_id, &loaded, None, false))
}

/// Authoritative model context for an AI turn. Frontend trigger fields are ignored;
/// Rust regenerates public state, legal actions, actor, and revision.
pub fn ai_turn_context(db: &mut Database, surface_id: &str) -> DbResult<Value> {
    let loaded = ensure_initialized(db, surface_id)?;
    let view = view_of(surface_id, &loaded, None, false);
    let catalog = super::visibility::model_capability_catalog(&loaded.def, &view.state);
    Ok(json!({
        "surfaceId": view.surface_id,
        "applicationId": view.application_id,
        "stateRevision": view.state_revision,
        "seq": view.seq,
        "status": view.status,
        "winner": view.winner,
        "waitingFor": view.waiting_for,
        "actor": view.actor,
        "legalActions": view.legal_actions,
        "publicState": view.state,
        "modelDefinition": loaded.def.model_view(),
        "capabilityCatalog": catalog,
        "authority": "rust",
        "instruction": "Propose one interactive.action using legalActions and stateRevision. Hidden state is unavailable.",
    }))
}

fn validate_event_id(event_id: &str) -> DbResult<()> {
    let ok = !event_id.is_empty()
        && event_id.len() <= MAX_EVENT_ID_LEN
        && !event_id.starts_with("__")
        && event_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '.'));
    if ok {
        Ok(())
    } else {
        Err(DbError::Invalid("invalid interactive event id".into()))
    }
}

/// Execute one interactive action authoritatively.
pub fn dispatch(
    db: &mut Database,
    surface_id: &str,
    expected_state_revision: i64,
    event_id: &str,
    action_id: &str,
    params_v: &Value,
    origin: Origin,
) -> DbResult<InteractiveView> {
    validate_event_id(event_id)?;
    let duplicate: Option<i64> = db
        .conn()
        .query_row(
            "SELECT seq FROM interactive_action_log WHERE surface_id = ?1 AND event_id = ?2",
            params![surface_id, event_id],
            |r| r.get(0),
        )
        .optional()?;
    let mut loaded = ensure_initialized(db, surface_id)?;
    if duplicate.is_some() {
        return Ok(view_of(surface_id, &loaded, None, true));
    }
    if loaded.reinitialized || expected_state_revision != loaded.revision {
        return Err(DbError::Conflict(format!(
            "stale interactive state: expected revision {expected_state_revision}, current {}",
            loaded.revision
        )));
    }

    let actor = match origin {
        Origin::User => loaded
            .def
            .resolve_user_actor(&loaded.owned)
            .map_err(DbError::Invalid)?,
        Origin::Ai => {
            let cp = loaded
                .owned
                .get("currentPlayer")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            if !loaded.def.ai_actors.contains(&cp) {
                return Err(DbError::Invalid("it is not an AI actor's turn".into()));
            }
            cp
        }
    };

    let mut next_owned = loaded.owned.clone();
    let outcome = loaded
        .def
        .execute_action(&mut next_owned, action_id, params_v, &actor)
        .map_err(DbError::Invalid)?;
    let merged = merge(&next_owned, loaded.rest.clone());
    let seq = loaded.seq + 1;
    let checkpoint =
        (seq - last_checkpoint_seq(db, surface_id)? >= CHECKPOINT_INTERVAL).then_some(&next_owned);
    loaded.revision = match commit(
        db,
        surface_id,
        &merged,
        loaded.revision,
        seq,
        event_id,
        action_id,
        &actor,
        origin.as_str(),
        params_v,
        loaded.definition_revision,
        &next_owned,
        checkpoint,
    ) {
        Ok(rev) => rev,
        Err(err) => return soft_duplicate_view(db, surface_id, event_id, err),
    };
    loaded.owned = next_owned;
    loaded.seq = seq;
    Ok(view_of(surface_id, &loaded, outcome.message, false))
}

fn definition_at(
    db: &Database,
    surface_id: &str,
    revision: i64,
) -> DbResult<InteractiveAppDefinition> {
    let json: Option<String> = db
        .conn()
        .query_row(
            "SELECT definition_json FROM surface_versions WHERE surface_id = ?1 AND revision = ?2
             ORDER BY created_at DESC LIMIT 1",
            params![surface_id, revision],
            |r| r.get(0),
        )
        .optional()?;
    let def_value: Value = match json {
        Some(j) => serde_json::from_str(&j).map_err(|e| corrupted("definition version", e))?,
        None => {
            let s = get_surface(db, surface_id)?;
            if s.current_revision != revision {
                return Err(DbError::NotFound(format!(
                    "definition revision {revision} for surface {surface_id}"
                )));
            }
            s.definition
        }
    };
    require_definition(&def_value)
}

/// Read-only, deterministic reconstruction of the owned state after `seq`.
/// Performs no writes, model calls, event dispatch or network access. Every
/// re-executed transition must reproduce its recorded hash.
pub fn reconstruct(db: &Database, surface_id: &str, seq: i64) -> DbResult<Value> {
    let rows = read_rows(db, surface_id, Some(seq))?;
    let start = rows
        .iter()
        .rposition(|r| r.snapshot.is_some())
        .ok_or_else(|| {
            DbError::NotFound(format!("no interactive checkpoint at or before {seq}"))
        })?;
    let mut state = rows[start].snapshot.clone().unwrap_or(Value::Null);
    if state_hash(&state) != rows[start].post_state_hash {
        return Err(corrupted("checkpoint", "hash mismatch"));
    }
    let mut cached: Option<(i64, InteractiveAppDefinition)> = None;
    for row in &rows[start + 1..] {
        // Checkpoints (__init/__adopt/__undo) carry snapshots and are normally
        // selected as `start`. If one appears mid-stream, apply the snapshot
        // instead of calling execute_action on a synthetic action id.
        if matches!(row.action_id.as_str(), "__init" | "__adopt" | "__undo") {
            let snap = row.snapshot.as_ref().ok_or_else(|| {
                corrupted(
                    "replay",
                    format!("seq {} missing checkpoint snapshot", row.seq),
                )
            })?;
            if state_hash(snap) != row.post_state_hash {
                return Err(corrupted(
                    "replay",
                    format!("seq {} checkpoint hash mismatch", row.seq),
                ));
            }
            state = snap.clone();
            continue;
        }
        let def = match &cached {
            Some((rev, d)) if *rev == row.definition_revision => d,
            _ => {
                cached = Some((
                    row.definition_revision,
                    definition_at(db, surface_id, row.definition_revision)?,
                ));
                &cached.as_ref().unwrap().1
            }
        };
        def.execute_action(&mut state, &row.action_id, &row.params, &row.actor)
            .map_err(|e| corrupted("replay", format!("seq {}: {e}", row.seq)))?;
        if state_hash(&state) != row.post_state_hash {
            return Err(corrupted("replay", format!("seq {} diverged", row.seq)));
        }
    }
    if rows.last().map(|r| r.seq) != Some(seq) {
        return Err(DbError::NotFound(format!("interactive seq {seq}")));
    }
    Ok(state)
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub seq: i64,
    pub event_id: String,
    pub action_id: String,
    pub actor: String,
    /// Redacted parameter projection — never raw secret action params.
    pub params: Value,
    pub checkpoint: bool,
}

pub fn history(db: &Database, surface_id: &str) -> DbResult<Vec<HistoryEntry>> {
    let mut entries: Vec<HistoryEntry> = read_rows(db, surface_id, None)?
        .into_iter()
        .map(|r| HistoryEntry {
            seq: r.seq,
            event_id: r.event_id,
            action_id: r.action_id,
            actor: r.actor,
            params: super::visibility::redact_history_params(&r.params),
            checkpoint: r.snapshot.is_some(),
        })
        .collect();
    if entries.len() > MAX_HISTORY_ENTRIES {
        entries = entries.split_off(entries.len() - MAX_HISTORY_ENTRIES);
    }
    Ok(entries)
}

/// Public view of the reconstructed state at `seq` (for the replay UI/model).
/// Merges historical engine-owned keys with the live non-owned rest so the
/// renderer keeps incidental UI state while inspecting a past board/position.
/// Both sides are projected through the central visibility policy.
pub fn replay_view(db: &Database, surface_id: &str, seq: i64) -> DbResult<Value> {
    let surface = get_surface(db, surface_id)?;
    let def = require_definition(&surface.definition)?;
    let (current, _) = get_surface_state_with_revision(db, surface_id)?;
    let reconstructed = reconstruct(db, surface_id, seq)?;
    // Build a temporary merged state then project through the surface definition.
    let (_, rest) = split_owned(&def, &current);
    let merged = merge(&reconstructed, rest);
    Ok(super::visibility::project_surface_state(
        &surface.definition,
        &merged,
        super::visibility::Audience::Renderer,
    ))
}

/// Restore the state from before the most recent non-checkpoint action.
/// Recorded as a checkpoint so replay stays exact.
pub fn undo(
    db: &mut Database,
    surface_id: &str,
    expected_state_revision: i64,
    event_id: &str,
) -> DbResult<InteractiveView> {
    validate_event_id(event_id)?;
    let duplicate: Option<i64> = db
        .conn()
        .query_row(
            "SELECT seq FROM interactive_action_log WHERE surface_id = ?1 AND event_id = ?2",
            params![surface_id, event_id],
            |r| r.get(0),
        )
        .optional()?;
    let loaded = ensure_initialized(db, surface_id)?;
    // Soft-idempotent: retries with the same event_id must not UNIQUE-fail.
    if duplicate.is_some() {
        return Ok(view_of(surface_id, &loaded, None, true));
    }
    if expected_state_revision != loaded.revision {
        return Err(DbError::Conflict("stale interactive state".into()));
    }
    let rows = read_rows(db, surface_id, None)?;
    // Walk back over prior undos so repeated undo keeps stepping backwards.
    // `__init`/`__adopt` checkpoints are barriers: history before them used
    // another definition or baseline.
    let mut target = None;
    let mut skip = 0usize;
    for r in rows.iter().rev() {
        match r.action_id.as_str() {
            "__undo" => skip += 1,
            "__init" | "__adopt" => break,
            _ if skip == 0 => {
                target = Some(r.seq - 1);
                break;
            }
            _ => skip -= 1,
        }
    }
    let target = target.ok_or_else(|| DbError::Invalid("nothing to undo".into()))?;
    let prior = reconstruct(db, surface_id, target)?;
    let merged = merge(&prior, loaded.rest.clone());
    let seq = loaded.seq + 1;
    let revision = match commit(
        db,
        surface_id,
        &merged,
        loaded.revision,
        seq,
        event_id,
        "__undo",
        "system",
        "user",
        &Value::Null,
        loaded.definition_revision,
        &prior,
        Some(&prior),
    ) {
        Ok(rev) => rev,
        Err(err) => return soft_duplicate_view(db, surface_id, event_id, err),
    };
    let l = Loaded {
        owned: prior,
        revision,
        seq,
        ..loaded
    };
    Ok(view_of(surface_id, &l, None, false))
}
