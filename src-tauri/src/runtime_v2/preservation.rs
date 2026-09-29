//! Component preservation policies — trusted Partial Update adaptation.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::db::{now_rfc3339, Database, DbError, DbResult};
use rusqlite::params;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreservationPolicy {
    Replace,
    PreserveInstance,
    PreserveState,
    PreserveUserInput,
    PreserveMediaState,
    PreserveScroll,
    PreserveFocus,
    PreserveSelection,
    PreserveIfCompatible,
    ResetExplicitly,
}

impl PreservationPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Replace => "replace",
            Self::PreserveInstance => "preserve_instance",
            Self::PreserveState => "preserve_state",
            Self::PreserveUserInput => "preserve_user_input",
            Self::PreserveMediaState => "preserve_media_state",
            Self::PreserveScroll => "preserve_scroll",
            Self::PreserveFocus => "preserve_focus",
            Self::PreserveSelection => "preserve_selection",
            Self::PreserveIfCompatible => "preserve_if_compatible",
            Self::ResetExplicitly => "reset_explicitly",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "replace" => Some(Self::Replace),
            "preserve_instance" => Some(Self::PreserveInstance),
            "preserve_state" => Some(Self::PreserveState),
            "preserve_user_input" => Some(Self::PreserveUserInput),
            "preserve_media_state" => Some(Self::PreserveMediaState),
            "preserve_scroll" => Some(Self::PreserveScroll),
            "preserve_focus" => Some(Self::PreserveFocus),
            "preserve_selection" => Some(Self::PreserveSelection),
            "preserve_if_compatible" => Some(Self::PreserveIfCompatible),
            "reset_explicitly" => Some(Self::ResetExplicitly),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreservationRecord {
    pub id: String,
    pub surface_id: String,
    pub component_id: String,
    pub preservation_key: Option<String>,
    pub policy: PreservationPolicy,
    pub state_schema_version: String,
    pub capability_pack: Option<String>,
    pub component_type: String,
    pub last_preserved_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Whether an incoming component should preserve live state given stored policy.
pub fn should_preserve(
    policy: PreservationPolicy,
    incoming_key: Option<&str>,
    stored_key: Option<&str>,
    compatible: bool,
) -> bool {
    match policy {
        PreservationPolicy::Replace | PreservationPolicy::ResetExplicitly => false,
        PreservationPolicy::PreserveInstance
        | PreservationPolicy::PreserveState
        | PreservationPolicy::PreserveUserInput
        | PreservationPolicy::PreserveMediaState
        | PreservationPolicy::PreserveScroll
        | PreservationPolicy::PreserveFocus
        | PreservationPolicy::PreserveSelection => true,
        PreservationPolicy::PreserveIfCompatible => {
            if let (Some(a), Some(b)) = (incoming_key, stored_key) {
                a == b && compatible
            } else {
                compatible
            }
        }
    }
}

/// User-input prop keys (typed values / drafts — not focus, scroll, or media).
const USER_INPUT_PROP_KEYS: &[&str] = &[
    "value",
    "checked",
    "selected",
    "selectedIndex",
    "selectedIndices",
    "text",
    "draft",
];

const SELECTION_PROP_KEYS: &[&str] = &["selectionStart", "selectionEnd"];

const MEDIA_PROP_KEYS: &[&str] = &["currentTime", "paused", "muted", "volume"];

const SCROLL_PROP_KEYS: &[&str] = &["scrollTop", "scrollLeft"];

/// Focus is continuity-owned (DOM restore), not copied via component props.
const FOCUS_PROP_KEYS: &[&str] = &[];

/// Prop keys allowed for a given preservation dimension.
/// `preserve_focus` must NOT drag along value/scroll/media.
pub fn prop_keys_for_policy(policy: PreservationPolicy) -> &'static [&'static str] {
    match policy {
        PreservationPolicy::Replace | PreservationPolicy::ResetExplicitly => &[],
        PreservationPolicy::PreserveUserInput => USER_INPUT_PROP_KEYS,
        PreservationPolicy::PreserveMediaState => MEDIA_PROP_KEYS,
        PreservationPolicy::PreserveScroll => SCROLL_PROP_KEYS,
        PreservationPolicy::PreserveFocus => FOCUS_PROP_KEYS,
        PreservationPolicy::PreserveSelection => SELECTION_PROP_KEYS,
        // Instance/state/compatible: preserve every live dimension (PU-style continuity).
        PreservationPolicy::PreserveInstance
        | PreservationPolicy::PreserveState
        | PreservationPolicy::PreserveIfCompatible => {
            // Concatenate statically via a fixed table to avoid allocation.
            const ALL: &[&str] = &[
                "value",
                "checked",
                "selected",
                "selectedIndex",
                "selectedIndices",
                "text",
                "draft",
                "selectionStart",
                "selectionEnd",
                "currentTime",
                "scrollTop",
                "scrollLeft",
                "paused",
                "muted",
                "volume",
            ];
            ALL
        }
    }
}

fn prop_key<'a>(props: Option<&'a Value>, key: &str) -> Option<&'a str> {
    props
        .and_then(|p| p.get(key))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
}

/// Resolve policy from op payload, then DB, then default PreserveIfCompatible.
pub fn resolve_policy_for_apply(
    db: &Database,
    surface_id: &str,
    component_id: Option<&str>,
    payload: &Value,
) -> PreservationPolicy {
    if let Some(s) = payload
        .get("preservationPolicy")
        .or_else(|| payload.get("policy"))
        .and_then(|v| v.as_str())
    {
        if let Some(p) = PreservationPolicy::parse(s) {
            return p;
        }
    }
    if let Some(cid) = component_id {
        if let Ok(rec) = get_preservation(db, surface_id, cid) {
            return rec.policy;
        }
    }
    PreservationPolicy::PreserveIfCompatible
}

/// On replace: merge live props from old → new for the policy's dimensions only.
/// Returns whether preservation was applied.
pub fn apply_preservation_on_replace(
    old: &crate::ai::ToolComponent,
    new: &mut crate::ai::ToolComponent,
    policy: PreservationPolicy,
) -> bool {
    let incoming_key = prop_key(new.props.as_ref(), "preservationKey")
        .or_else(|| prop_key(new.props.as_ref(), "preservation_key"));
    let stored_key = prop_key(old.props.as_ref(), "preservationKey")
        .or_else(|| prop_key(old.props.as_ref(), "preservation_key"));
    let compatible = old.component_type == new.component_type;
    if !should_preserve(policy, incoming_key, stored_key, compatible) {
        return false;
    }
    let keys = prop_keys_for_policy(policy);
    if keys.is_empty() {
        // Policy allows continuity (e.g. preserve_focus) but has no prop keys —
        // instance identity still survives; DOM continuity handles focus.
        return true;
    }
    let Some(old_props) = old.props.as_ref().and_then(|v| v.as_object()) else {
        return true; // keep instance id / type continuity without props
    };
    let mut merged = match new.props.take() {
        Some(Value::Object(m)) => m,
        Some(_) | None => serde_json::Map::new(),
    };
    for key in keys {
        if let Some(v) = old_props.get(*key) {
            // Prefer live values for explicit preserve_* dimensions.
            // For PreserveIfCompatible, do not overwrite keys the agent explicitly set.
            let agent_set = merged.contains_key(*key);
            let live_wins = !matches!(policy, PreservationPolicy::PreserveIfCompatible);
            if live_wins || !agent_set {
                merged.insert((*key).to_string(), v.clone());
            }
        }
    }
    new.props = Some(Value::Object(merged));
    true
}

/// Resolve valueKey from a component (field or props).
pub fn component_value_key(comp: &crate::ai::ToolComponent) -> Option<&str> {
    comp.value_key
        .as_deref()
        .filter(|s| !s.is_empty())
        .or_else(|| prop_key(comp.props.as_ref(), "valueKey"))
        .or_else(|| prop_key(comp.props.as_ref(), "value_key"))
        .or_else(|| prop_key(comp.props.as_ref(), "stateKey"))
}

/// Drop drafts + surface_state keys for a component when policy does not preserve.
/// Uses OCC so concurrent user state writes fail closed rather than clobber.
pub fn invalidate_component_live_state(
    db: &mut Database,
    surface_id: &str,
    component_id: &str,
    value_key: Option<&str>,
) -> DbResult<()> {
    let _ = super::drafts::delete_draft(db, surface_id, component_id, "main");
    let (mut state, rev) = super::surfaces::get_surface_state_with_revision(db, surface_id)?;
    let Some(obj) = state.as_object_mut() else {
        return Ok(());
    };
    let mut removed = false;
    let prefix = format!("{component_id}:");
    let keys: Vec<String> = obj
        .keys()
        .filter(|k| {
            *k == component_id
                || k.starts_with(&prefix)
                || value_key.is_some_and(|vk| *k == vk)
        })
        .cloned()
        .collect();
    for k in keys {
        obj.remove(&k);
        removed = true;
    }
    if removed {
        // Fail closed on concurrent user writes during definition mutation.
        super::surfaces::save_surface_state_occ(db, surface_id, &state, rev)?;
    }
    Ok(())
}

/// Drop specific surface_state keys via OCC without clearing component_id bindings.
/// Used when valueKey is renamed under an otherwise preserving policy.
pub fn clear_surface_state_keys(
    db: &mut Database,
    surface_id: &str,
    keys: &[&str],
) -> DbResult<()> {
    if keys.is_empty() {
        return Ok(());
    }
    let (mut state, rev) = super::surfaces::get_surface_state_with_revision(db, surface_id)?;
    let Some(obj) = state.as_object_mut() else {
        return Ok(());
    };
    let mut removed = false;
    for key in keys {
        if key.is_empty() {
            continue;
        }
        if obj.remove(*key).is_some() {
            removed = true;
        }
    }
    if removed {
        super::surfaces::save_surface_state_occ(db, surface_id, &state, rev)?;
    }
    Ok(())
}

pub fn upsert_preservation(
    db: &mut Database,
    surface_id: &str,
    component_id: &str,
    policy: PreservationPolicy,
    preservation_key: Option<&str>,
    component_type: &str,
    capability_pack: Option<&str>,
) -> DbResult<PreservationRecord> {
    let now = now_rfc3339();
    let existing: Option<String> = db
        .conn()
        .query_row(
            "SELECT id FROM component_preservation WHERE surface_id = ?1 AND component_id = ?2",
            params![surface_id, component_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(DbError::Sqlite)?;

    let id = existing.unwrap_or_else(|| format!("pres-{}", Uuid::new_v4()));
    db.conn().execute(
        "INSERT INTO component_preservation (
            id, surface_id, component_id, preservation_key, policy, component_type,
            capability_pack, last_preserved_at, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(surface_id, component_id) DO UPDATE SET
            preservation_key = excluded.preservation_key,
            policy = excluded.policy,
            component_type = excluded.component_type,
            capability_pack = excluded.capability_pack,
            last_preserved_at = excluded.last_preserved_at,
            updated_at = excluded.updated_at",
        params![
            id,
            surface_id,
            component_id,
            preservation_key,
            policy.as_str(),
            component_type,
            capability_pack,
            now,
            now,
            now
        ],
    )?;
    get_preservation(db, surface_id, component_id)
}

pub fn get_preservation(
    db: &Database,
    surface_id: &str,
    component_id: &str,
) -> DbResult<PreservationRecord> {
    db.conn()
        .query_row(
            "SELECT id, surface_id, component_id, preservation_key, policy,
                    state_schema_version, capability_pack, component_type,
                    last_preserved_at, created_at, updated_at
             FROM component_preservation WHERE surface_id = ?1 AND component_id = ?2",
            params![surface_id, component_id],
            |row| {
                let policy_str: String = row.get(4)?;
                Ok(PreservationRecord {
                    id: row.get(0)?,
                    surface_id: row.get(1)?,
                    component_id: row.get(2)?,
                    preservation_key: row.get(3)?,
                    policy: PreservationPolicy::parse(&policy_str)
                        .unwrap_or(PreservationPolicy::PreserveIfCompatible),
                    state_schema_version: row.get(5)?,
                    capability_pack: row.get(6)?,
                    component_type: row.get(7)?,
                    last_preserved_at: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                DbError::NotFound(format!("preservation {surface_id}/{component_id}"))
            }
            other => DbError::Sqlite(other),
        })
}

pub fn list_preservation_for_surface(
    db: &Database,
    surface_id: &str,
) -> DbResult<Vec<PreservationRecord>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, surface_id, component_id, preservation_key, policy,
                state_schema_version, capability_pack, component_type,
                last_preserved_at, created_at, updated_at
         FROM component_preservation WHERE surface_id = ?1",
    )?;
    let rows = stmt.query_map([surface_id], |row| {
        let policy_str: String = row.get(4)?;
        Ok(PreservationRecord {
            id: row.get(0)?,
            surface_id: row.get(1)?,
            component_id: row.get(2)?,
            preservation_key: row.get(3)?,
            policy: PreservationPolicy::parse(&policy_str)
                .unwrap_or(PreservationPolicy::PreserveIfCompatible),
            state_schema_version: row.get(5)?,
            capability_pack: row.get(6)?,
            component_type: row.get(7)?,
            last_preserved_at: row.get(8)?,
            created_at: row.get(9)?,
            updated_at: row.get(10)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::Sqlite)
}

trait OptionalRow<T> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalRow<T> for Result<T, rusqlite::Error> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn test_db() -> crate::db::Database {
        let dir = tempdir().unwrap();
        crate::db::Database::open_path(&dir.path().join("t.db")).unwrap()
    }

    #[test]
    fn preservation_match_by_key() {
        assert!(should_preserve(
            PreservationPolicy::PreserveIfCompatible,
            Some("timer-1"),
            Some("timer-1"),
            true
        ));
        assert!(!should_preserve(
            PreservationPolicy::PreserveIfCompatible,
            Some("timer-2"),
            Some("timer-1"),
            true
        ));
        assert!(!should_preserve(
            PreservationPolicy::Replace,
            Some("x"),
            Some("x"),
            true
        ));
        assert!(should_preserve(
            PreservationPolicy::PreserveFocus,
            None,
            None,
            false
        ));
    }

    #[test]
    fn replace_merges_user_input_when_compatible() {
        use crate::ai::ToolComponent;
        use serde_json::json;
        let old = ToolComponent {
            id: "field".into(),
            component_type: "text_input".into(),
            value_key: None,
            props: Some(json!({"value": "typed", "label": "Name", "maximum": 8})),
            children: None,
            ..Default::default()
        };
        let mut new = ToolComponent {
            id: "field".into(),
            component_type: "text_input".into(),
            value_key: None,
            props: Some(json!({"label": "Full name", "maximum": 12})),
            children: None,
            ..Default::default()
        };
        assert!(apply_preservation_on_replace(
            &old,
            &mut new,
            PreservationPolicy::PreserveUserInput
        ));
        assert_eq!(new.props.as_ref().unwrap()["value"], "typed");
        assert_eq!(new.props.as_ref().unwrap()["label"], "Full name");
        assert_eq!(new.props.as_ref().unwrap()["maximum"], 12);
    }

    #[test]
    fn preserve_focus_does_not_copy_value_or_scroll() {
        use crate::ai::ToolComponent;
        use serde_json::json;
        let old = ToolComponent {
            id: "field".into(),
            component_type: "text_input".into(),
            value_key: None,
            props: Some(json!({
                "value": "typed",
                "scrollTop": 40,
                "selectionStart": 2,
                "selectionEnd": 4
            })),
            children: None,
            ..Default::default()
        };
        let mut new = ToolComponent {
            id: "field".into(),
            component_type: "text_input".into(),
            value_key: None,
            props: Some(json!({"label": "Renamed"})),
            children: None,
            ..Default::default()
        };
        assert!(apply_preservation_on_replace(
            &old,
            &mut new,
            PreservationPolicy::PreserveFocus
        ));
        let props = new.props.as_ref().unwrap();
        assert!(props.get("value").is_none());
        assert!(props.get("scrollTop").is_none());
        assert!(props.get("selectionStart").is_none());
        assert_eq!(props["label"], "Renamed");
    }

    #[test]
    fn preserve_scroll_only_copies_scroll_keys() {
        use crate::ai::ToolComponent;
        use serde_json::json;
        let old = ToolComponent {
            id: "list".into(),
            component_type: "scroll".into(),
            value_key: None,
            props: Some(json!({
                "value": "secret",
                "scrollTop": 120,
                "scrollLeft": 8,
                "currentTime": 9.5
            })),
            children: None,
            ..Default::default()
        };
        let mut new = ToolComponent {
            id: "list".into(),
            component_type: "scroll".into(),
            value_key: None,
            props: Some(json!({})),
            children: None,
            ..Default::default()
        };
        assert!(apply_preservation_on_replace(
            &old,
            &mut new,
            PreservationPolicy::PreserveScroll
        ));
        let props = new.props.as_ref().unwrap();
        assert_eq!(props["scrollTop"], 120);
        assert_eq!(props["scrollLeft"], 8);
        assert!(props.get("value").is_none());
        assert!(props.get("currentTime").is_none());
    }

    #[test]
    fn preserve_media_state_only_copies_media_keys() {
        use crate::ai::ToolComponent;
        use serde_json::json;
        let old = ToolComponent {
            id: "player".into(),
            component_type: "video".into(),
            value_key: None,
            props: Some(json!({
                "value": "caption",
                "currentTime": 42.5,
                "paused": true,
                "muted": false,
                "volume": 0.6,
                "scrollTop": 99
            })),
            children: None,
            ..Default::default()
        };
        let mut new = ToolComponent {
            id: "player".into(),
            component_type: "video".into(),
            value_key: None,
            props: Some(json!({"label": "Clip"})),
            children: None,
            ..Default::default()
        };
        assert!(apply_preservation_on_replace(
            &old,
            &mut new,
            PreservationPolicy::PreserveMediaState
        ));
        let props = new.props.as_ref().unwrap();
        assert_eq!(props["currentTime"], 42.5);
        assert_eq!(props["paused"], true);
        assert_eq!(props["muted"], false);
        assert_eq!(props["volume"], 0.6);
        assert!(props.get("value").is_none());
        assert!(props.get("scrollTop").is_none());
        assert_eq!(props["label"], "Clip");
    }

    #[test]
    fn preserve_selection_only_copies_selection_keys() {
        use crate::ai::ToolComponent;
        use serde_json::json;
        let old = ToolComponent {
            id: "field".into(),
            component_type: "text_input".into(),
            value_key: None,
            props: Some(json!({
                "value": "hello",
                "selectionStart": 1,
                "selectionEnd": 4,
                "scrollTop": 20
            })),
            children: None,
            ..Default::default()
        };
        let mut new = ToolComponent {
            id: "field".into(),
            component_type: "text_input".into(),
            value_key: None,
            props: Some(json!({"label": "New"})),
            children: None,
            ..Default::default()
        };
        assert!(apply_preservation_on_replace(
            &old,
            &mut new,
            PreservationPolicy::PreserveSelection
        ));
        let props = new.props.as_ref().unwrap();
        assert_eq!(props["selectionStart"], 1);
        assert_eq!(props["selectionEnd"], 4);
        assert!(props.get("value").is_none());
        assert!(props.get("scrollTop").is_none());
    }

    #[test]
    fn preserve_user_input_does_not_copy_scroll_or_media() {
        use crate::ai::ToolComponent;
        use serde_json::json;
        let old = ToolComponent {
            id: "field".into(),
            component_type: "text_input".into(),
            value_key: None,
            props: Some(json!({
                "value": "typed",
                "draft": "d",
                "scrollTop": 50,
                "currentTime": 3.0,
                "selectionStart": 0
            })),
            children: None,
            ..Default::default()
        };
        let mut new = ToolComponent {
            id: "field".into(),
            component_type: "text_input".into(),
            value_key: None,
            props: Some(json!({"label": "L"})),
            children: None,
            ..Default::default()
        };
        assert!(apply_preservation_on_replace(
            &old,
            &mut new,
            PreservationPolicy::PreserveUserInput
        ));
        let props = new.props.as_ref().unwrap();
        assert_eq!(props["value"], "typed");
        assert_eq!(props["draft"], "d");
        assert!(props.get("scrollTop").is_none());
        assert!(props.get("currentTime").is_none());
        assert!(props.get("selectionStart").is_none());
    }

    #[test]
    fn replace_does_not_merge_when_reset() {
        use crate::ai::ToolComponent;
        use serde_json::json;
        let old = ToolComponent {
            id: "field".into(),
            component_type: "text_input".into(),
            value_key: None,
            props: Some(json!({"value": "typed"})),
            children: None,
            ..Default::default()
        };
        let mut new = ToolComponent {
            id: "field".into(),
            component_type: "text_input".into(),
            value_key: None,
            props: Some(json!({"value": ""})),
            children: None,
            ..Default::default()
        };
        assert!(!apply_preservation_on_replace(
            &old,
            &mut new,
            PreservationPolicy::ResetExplicitly
        ));
        assert_eq!(new.props.as_ref().unwrap()["value"], "");
    }

    #[test]
    fn invalidate_uses_occ_and_value_key() {
        use crate::db::{create_conversation, DEFAULT_WORKSPACE_ID};
        use crate::runtime_v2::surfaces::{create_inline_surface, save_surface_state_occ};
        use serde_json::json;

        let mut db = test_db();
        let conv =
            create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "PresInv", None).unwrap();
        let def = json!({
            "id": "d",
            "name": "Inline",
            "layout": "stack",
            "components": [{"id": "cmp-1", "type": "textInput", "valueKey": "taskTitle", "props": {"label": "Title"}}]
        });
        let surface =
            create_inline_surface(&mut db, &conv.id, None, None, "Inline", &def, &[]).unwrap();
        let st = json!({"cmp-1": "x", "taskTitle": "hello", "other": 1});
        save_surface_state_occ(&mut db, &surface.id, &st, 1).unwrap();

        invalidate_component_live_state(&mut db, &surface.id, "cmp-1", Some("taskTitle")).unwrap();
        let (state, rev) =
            super::super::surfaces::get_surface_state_with_revision(&db, &surface.id).unwrap();
        assert!(rev >= 2);
        assert!(state.get("cmp-1").is_none());
        assert!(state.get("taskTitle").is_none());
        assert_eq!(state["other"], 1);
    }

    #[test]
    fn clear_surface_state_keys_drops_orphan_without_cid() {
        use crate::db::{create_conversation, DEFAULT_WORKSPACE_ID};
        use crate::runtime_v2::surfaces::{create_inline_surface, save_surface_state_occ};
        use serde_json::json;

        let mut db = test_db();
        let conv =
            create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "PresOrphan", None).unwrap();
        let def = json!({
            "id": "d",
            "name": "Inline",
            "layout": "stack",
            "components": [{"id": "cmp-1", "type": "textInput", "valueKey": "newTitle", "props": {"label": "Title"}}]
        });
        let surface =
            create_inline_surface(&mut db, &conv.id, None, None, "Inline", &def, &[]).unwrap();
        let st = json!({"cmp-1": "keep", "oldTitle": "orphan", "other": 1});
        save_surface_state_occ(&mut db, &surface.id, &st, 1).unwrap();

        clear_surface_state_keys(&mut db, &surface.id, &["oldTitle"]).unwrap();
        let (state, _) =
            super::super::surfaces::get_surface_state_with_revision(&db, &surface.id).unwrap();
        assert!(state.get("oldTitle").is_none());
        assert_eq!(state["cmp-1"], "keep");
        assert_eq!(state["other"], 1);
    }

    #[test]
    fn preservation_roundtrip() {
        let mut db = test_db();
        let rec = upsert_preservation(
            &mut db,
            "surf-1",
            "cmp-1",
            PreservationPolicy::PreserveMediaState,
            Some("media-key"),
            "video",
            Some("media"),
        )
        .unwrap();
        assert_eq!(rec.policy, PreservationPolicy::PreserveMediaState);
        let loaded = get_preservation(&db, "surf-1", "cmp-1").unwrap();
        assert_eq!(loaded.preservation_key.as_deref(), Some("media-key"));
    }
}
