//! JSON export for projects (no credentials, paths, or secrets).

use serde_json::{json, Value};

use crate::db::{get_messages, Database, DbResult};
use crate::exports::{
    assert_no_credential_tokens, redact_secret_patterns, strip_secrets_from_value,
};

use super::models::Project;
use super::repository::{get_project, list_project_conversations};
use super::summaries::{deterministic_fallback_summary, get_chat_summary};

pub fn export_project_json(db: &Database, project_id: &str) -> DbResult<Value> {
    let project = get_project(db, project_id)?;
    let conversations = list_project_conversations(db, project_id)?;
    let summary = project
        .summary
        .clone()
        .or_else(|| deterministic_fallback_summary(db, project_id).ok())
        .map(|s| redact_secret_patterns(&s));

    let mut chats = Vec::new();
    for conv in conversations {
        let messages = get_messages(db, &conv.id)?;
        let chat_summary = get_chat_summary(db, &conv.id)
            .ok()
            .flatten()
            .map(|s| redact_secret_patterns(&s.summary));
        let message_values: Vec<Value> = messages
            .iter()
            .map(|m| {
                json!({
                    "id": m.id,
                    "role": m.role,
                    "content": redact_secret_patterns(&m.content),
                    "createdAt": m.created_at,
                    "metadata": m.metadata.as_ref().map(strip_secrets_from_value),
                })
            })
            .collect();
        chats.push(json!({
            "id": conv.id,
            "title": conv.title,
            "pinned": conv.pinned,
            "archived": conv.archived,
            "createdAt": conv.created_at,
            "updatedAt": conv.updated_at,
            "summary": chat_summary,
            "messages": message_values,
        }));
    }

    let payload = json!({
        "format": "coreside-project",
        "version": 1,
        "exportedAt": chrono::Utc::now().to_rfc3339(),
        "project": project_payload(&project, summary),
        "conversations": chats,
    });
    // High-confidence token prefixes only — chat text may mention "api key" innocently.
    assert_no_credential_tokens(&payload.to_string()).map_err(crate::db::DbError::Invalid)?;
    Ok(payload)
}

fn project_payload(project: &Project, summary: Option<String>) -> Value {
    json!({
        "id": project.id,
        "name": project.name,
        "description": project.description.as_deref().map(redact_secret_patterns),
        "iconKey": project.icon_key,
        "instructions": project.instructions.as_deref().map(redact_secret_patterns),
        "summary": summary,
        "summaryUpdatedAt": project.summary_updated_at,
        "archived": project.archived,
        "pinned": project.pinned,
        "createdAt": project.created_at,
        "updatedAt": project.updated_at,
        "lastOpenedAt": project.last_opened_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{create_conversation, insert_message, Database, DEFAULT_WORKSPACE_ID};
    use crate::projects::{create_project, CreateProjectInput};
    use tempfile::tempdir;

    #[test]
    fn export_omits_wallpaper_and_secrets() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("exp.db")).unwrap();
        let project = create_project(
            &mut db,
            &CreateProjectInput {
                name: "Export".into(),
                description: None,
                icon_key: None,
                instructions: None,
                pinned: None,
            },
        )
        .unwrap();
        super::super::repository::set_project_wallpaper(
            &mut db,
            &project.id,
            Some(r#"{"schemaVersion":"1","type":"image-cover","assetId":"local-asset-1"}"#),
        )
        .unwrap();
        let conv =
            create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Chat", Some(&project.id)).unwrap();
        insert_message(
            &mut db,
            &conv.id,
            "user",
            "hello sk-ant-abcdefghijklmnopqrstuvwxyz",
            Some(&json!({"apiKey": "secret"})),
        )
        .unwrap();

        let exported = export_project_json(&db, &project.id).unwrap();
        let text = exported.to_string();
        assert!(!text.contains("local-asset-1"));
        assert!(!text.contains("apiKey"));
        assert!(!text.contains("sk-ant-"));
        assert!(text.contains("hello"));
        assert!(text.contains("[REDACTED]"));
    }
}
