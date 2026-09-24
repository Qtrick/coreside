//! Compile-gated E2E helpers. Only linked when Cargo feature `e2e` is enabled.
//!
//! Seed via env (not a public Tauri command):
//! - `CORESIDE_E2E_SEED=existing` — insert a stable fixture profile when the DB is empty

use crate::ai::ToolDefinition;
use crate::application_kernel::manifest::ensure_manifest_for_tool;
use crate::application_kernel::registered_actions::descriptor::find_action;
use crate::db::{self, apply_tool_change, save_tool_state, Database, DEFAULT_WORKSPACE_ID};
use crate::runtime_v2::surfaces::{surface_id_for_tool, upsert_surface_from_tool};
use serde_json::json;

const EXISTING_CONV_ID: &str = "conv-e2e-existing";
const EXISTING_CONV_TITLE: &str = "Biology notes";
const EXISTING_TOOL_ID: &str = "tool-e2e-notes";
const EXISTING_TOOL_NAME: &str = "E2E Notes";
const EXISTING_APPROVAL_ID: &str = "approval-e2e-1";
const EXISTING_GRANT_ID: &str = "grant-e2e-1";
const E2E_INPUT_COMPONENT_ID: &str = "e2e-note-input";
const E2E_STATE_KEY: &str = "note";

/// Optionally seed a narrow fixture profile after migrations + workspace ensure.
pub fn maybe_seed(db: &mut Database) {
    let Ok(raw) = std::env::var("CORESIDE_E2E_SEED") else {
        return;
    };
    let profile = raw.trim();
    if profile.is_empty() {
        return;
    }
    match profile {
        "existing" => {
            if let Err(err) = seed_existing_profile(db) {
                tracing::error!(error = %err, "e2e seed failed");
            }
        }
        "local" => {
            if let Err(err) = seed_local_profile(db) {
                tracing::error!(error = %err, "e2e local seed failed");
            }
        }
        other => {
            tracing::warn!(profile = other, "unknown CORESIDE_E2E_SEED value; ignoring");
        }
    }
}

fn seed_local_profile(db: &mut Database) -> db::DbResult<()> {
    let now = db::now_rfc3339();
    // Ensure only one active connection (idx_provider_connections_one_active)
    db.conn().execute(
        "UPDATE provider_connections SET is_active = 0 WHERE is_active = 1",
        [],
    )?;
    db.conn().execute(
        "INSERT INTO provider_connections (
            id, provider, label, base_url, model_default, keyring_account, is_active, last_status, last_tested_at, created_at, updated_at
        ) VALUES (
            'conn-e2e-local', 'ollama', 'Local Ollama', 'http://127.0.0.1:11434', 'llama3', 'coreside-e2e-local', 1, 'ready', ?1, ?1, ?1
        ) ON CONFLICT(id) DO UPDATE SET is_active = 1, last_status = 'ready'",
        rusqlite::params![now],
    )?;

    let count: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM conversations WHERE workspace_id = ?1",
        [DEFAULT_WORKSPACE_ID],
        |r| r.get(0),
    )?;
    if count == 0 {
        db.conn().execute(
            "INSERT INTO conversations (id, workspace_id, title, project_id, pinned, archived, created_at, updated_at)
             VALUES ('conv-e2e-local', ?1, 'Local AI Chat', NULL, 0, 0, ?2, ?2)",
            rusqlite::params![DEFAULT_WORKSPACE_ID, now],
        )?;
        db.conn().execute(
            "INSERT INTO messages (id, conversation_id, role, content)
             VALUES ('msg-e2e-local-1', 'conv-e2e-local', 'user', 'Hello local AI')",
            [],
        )?;
    }
    tracing::info!("e2e local profile seeded");
    Ok(())
}

fn seed_existing_profile(db: &mut Database) -> db::DbResult<()> {
    let count: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM conversations WHERE workspace_id = ?1",
        [DEFAULT_WORKSPACE_ID],
        |r| r.get(0),
    )?;
    if count > 0 {
        tracing::info!("e2e existing profile already seeded; skipping");
        return Ok(());
    }

    let now = db::now_rfc3339();
    db.conn().execute(
        "INSERT INTO conversations (id, workspace_id, title, project_id, pinned, archived, created_at, updated_at)
         VALUES (?1, ?2, ?3, NULL, 0, 0, ?4, ?5)",
        rusqlite::params![
            EXISTING_CONV_ID,
            DEFAULT_WORKSPACE_ID,
            EXISTING_CONV_TITLE,
            now,
            now
        ],
    )?;
    db.conn().execute(
        "INSERT INTO messages (id, conversation_id, role, content)
         VALUES ('msg-e2e-1', ?1, 'user', 'Remind me what mitosis is')",
        [EXISTING_CONV_ID],
    )?;

    let tool_def: ToolDefinition = serde_json::from_value(json!({
        "id": EXISTING_TOOL_ID,
        "name": EXISTING_TOOL_NAME,
        "description": "Seeded personal tool for desktop E2E",
        "layout": "single-column",
        "components": [{
            "id": E2E_INPUT_COMPONENT_ID,
            "type": "textInput",
            "valueKey": E2E_STATE_KEY,
            "props": {
                "label": "Note",
                "placeholder": "Type a note"
            }
        }]
    }))?;

    let applied = apply_tool_change(
        db,
        DEFAULT_WORKSPACE_ID,
        &tool_def,
        "create",
        None,
        "e2e seed",
    )?;
    let surface_id = surface_id_for_tool(EXISTING_TOOL_ID);
    upsert_surface_from_tool(db, &applied.definition, DEFAULT_WORKSPACE_ID, 1)?;
    ensure_manifest_for_tool(db, EXISTING_TOOL_ID, EXISTING_TOOL_NAME, &surface_id)?;
    save_tool_state(db, EXISTING_TOOL_ID, &json!({ E2E_STATE_KEY: "" }))?;

    let write_action = find_action("local_data.write")
        .ok_or_else(|| db::DbError::Invalid("bundled action local_data.write missing".into()))?;
    let query_action = find_action("local_data.query")
        .ok_or_else(|| db::DbError::Invalid("bundled action local_data.query missing".into()))?;

    db.conn().execute(
        "INSERT INTO runtime_approvals (
            id, application_id, action_name, action_title, risk, critical,
            input_preview, input_json, call_hash, descriptor_hash,
            venue, presence, status, created_at, expires_at, surface_id
         ) VALUES (
            ?1, ?2, ?3, ?4, 'write', 0,
            'Save note draft', '{\"modelId\":\"notes\",\"data\":{\"text\":\"hello\"}}',
            'e2e-call-hash-1', ?5,
            'application', 'present', 'pending', ?6, '2099-01-01T00:00:00Z', ?7
         )",
        rusqlite::params![
            EXISTING_APPROVAL_ID,
            EXISTING_TOOL_ID,
            write_action.name,
            write_action.title,
            write_action.descriptor_hash(),
            now,
            surface_id,
        ],
    )?;

    // Match mint_grant vocabulary: scope_kind=application_action, duration=standing.
    db.conn().execute(
        "INSERT INTO runtime_action_grants (
            id, subject, application_id, action_name, descriptor_hash, scope_kind,
            scope_json, duration, source, status, granted_at
         ) VALUES (
            ?1, 'local-user', ?2, ?3, ?4, 'application_action', '{}', 'standing',
            'e2e_seed', 'active', ?5
         )",
        rusqlite::params![
            EXISTING_GRANT_ID,
            EXISTING_TOOL_ID,
            query_action.name,
            query_action.descriptor_hash(),
            now,
        ],
    )?;

    tracing::info!(
        conversation_id = EXISTING_CONV_ID,
        tool_id = EXISTING_TOOL_ID,
        approval_id = EXISTING_APPROVAL_ID,
        grant_id = EXISTING_GRANT_ID,
        "e2e existing profile seeded"
    );
    Ok(())
}
