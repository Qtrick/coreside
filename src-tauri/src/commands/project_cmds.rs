//! Project IPC commands.

use serde::Deserialize;
use tauri::State;

use super::CommandError;
use crate::db::{self, create_conversation, Conversation, DEFAULT_WORKSPACE_ID};
use crate::projects::{
    archive_project, assign_conversation_to_project, create_project, delete_project,
    deterministic_fallback_summary, duplicate_conversation, export_project_json,
    get_project, list_project_conversations, list_projects, list_unassigned_conversations,
    move_conversations_to_project, rebuild_project_index, remove_conversation_from_project,
    rename_conversation, restore_project, search_project_context, set_project_summary,
    set_project_wallpaper, touch_last_opened, update_project, CreateProjectInput,
    DeleteProjectMode, Project, ProjectContextHit, UpdateProjectInput,
};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteProjectInput {
    pub project_id: String,
    /// Defaults to KeepChats when omitted or null (never silent cascade).
    #[serde(default)]
    pub mode: Option<DeleteProjectMode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignChatsInput {
    pub project_id: String,
    pub conversation_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchProjectContextInput {
    pub project_id: String,
    pub query: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProjectInput {
    pub project_id: String,
    pub destination_path: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProjectResult {
    pub ok: bool,
    pub path: String,
    pub message: String,
}

#[tauri::command]
pub fn list_projects_cmd(
    state: State<'_, AppState>,
    include_archived: Option<bool>,
) -> Result<Vec<Project>, CommandError> {
    let db = state.db.lock();
    Ok(list_projects(&db, include_archived.unwrap_or(false))?)
}

#[tauri::command]
pub fn get_project_cmd(state: State<'_, AppState>, project_id: String) -> Result<Project, CommandError> {
    let db = state.db.lock();
    Ok(get_project(&db, project_id.trim())?)
}

#[tauri::command]
pub fn create_project_cmd(
    state: State<'_, AppState>,
    input: CreateProjectInput,
) -> Result<Project, CommandError> {
    let mut db = state.db.lock();
    Ok(create_project(&mut db, &input)?)
}

#[tauri::command]
pub fn update_project_cmd(
    state: State<'_, AppState>,
    project_id: String,
    input: UpdateProjectInput,
) -> Result<Project, CommandError> {
    let mut db = state.db.lock();
    Ok(update_project(&mut db, project_id.trim(), &input)?)
}

#[tauri::command]
pub fn archive_project_cmd(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Project, CommandError> {
    let mut db = state.db.lock();
    Ok(archive_project(&mut db, project_id.trim())?)
}

#[tauri::command]
pub fn restore_project_cmd(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Project, CommandError> {
    let mut db = state.db.lock();
    Ok(restore_project(&mut db, project_id.trim())?)
}

#[tauri::command]
pub fn delete_project_cmd(
    state: State<'_, AppState>,
    input: DeleteProjectInput,
) -> Result<(), CommandError> {
    let mut db = state.db.lock();
    let mode = input.mode.unwrap_or(DeleteProjectMode::KeepChats);
    Ok(delete_project(&mut db, input.project_id.trim(), mode)?)
}

#[tauri::command]
pub fn create_conversation_in_project(
    state: State<'_, AppState>,
    project_id: String,
    title: Option<String>,
    workspace_id: Option<String>,
) -> Result<Conversation, CommandError> {
    let mut db = state.db.lock();
    let ws = workspace_id.unwrap_or_else(|| DEFAULT_WORKSPACE_ID.to_string());
    let _ = db::ensure_default_workspace(&db)?;
    let _ = get_project(&db, project_id.trim())?;
    let title = title
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| "New chat".to_string());
    Ok(create_conversation(
        &mut db,
        &ws,
        &title,
        Some(project_id.trim()),
    )?)
}

#[tauri::command]
pub fn assign_chats_to_project(
    state: State<'_, AppState>,
    input: AssignChatsInput,
) -> Result<Vec<Conversation>, CommandError> {
    let mut db = state.db.lock();
    Ok(move_conversations_to_project(
        &mut db,
        &input.conversation_ids,
        input.project_id.trim(),
    )?)
}

#[tauri::command]
pub fn remove_chat_from_project(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Conversation, CommandError> {
    let mut db = state.db.lock();
    Ok(remove_conversation_from_project(&mut db, conversation_id.trim())?)
}

#[tauri::command]
pub fn list_project_conversations_cmd(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<Conversation>, CommandError> {
    let db = state.db.lock();
    Ok(list_project_conversations(&db, project_id.trim())?)
}

#[tauri::command]
pub fn list_unassigned_conversations_cmd(
    state: State<'_, AppState>,
    workspace_id: Option<String>,
) -> Result<Vec<Conversation>, CommandError> {
    let db = state.db.lock();
    Ok(list_unassigned_conversations(
        &db,
        workspace_id.as_deref(),
    )?)
}

#[tauri::command]
pub fn search_project_context_cmd(
    state: State<'_, AppState>,
    input: SearchProjectContextInput,
) -> Result<Vec<ProjectContextHit>, CommandError> {
    let db = state.db.lock();
    Ok(search_project_context(
        &db,
        input.project_id.trim(),
        input.query.trim(),
        input.limit.unwrap_or(10),
    )?)
}

#[tauri::command]
pub fn refresh_project_summary(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<String, CommandError> {
    let mut db = state.db.lock();
    let summary = deterministic_fallback_summary(&db, project_id.trim())?;
    Ok(set_project_summary(&mut db, project_id.trim(), &summary)?)
}

#[tauri::command]
pub fn rebuild_project_index_cmd(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<u64, CommandError> {
    let db = state.db.lock();
    Ok(rebuild_project_index(&db, project_id.trim())?)
}

#[tauri::command]
pub fn rename_conversation_cmd(
    state: State<'_, AppState>,
    conversation_id: String,
    title: String,
) -> Result<Conversation, CommandError> {
    let mut db = state.db.lock();
    Ok(rename_conversation(&mut db, conversation_id.trim(), &title)?)
}

#[tauri::command]
pub fn duplicate_conversation_cmd(
    state: State<'_, AppState>,
    conversation_id: String,
    title: Option<String>,
) -> Result<Conversation, CommandError> {
    let mut db = state.db.lock();
    Ok(duplicate_conversation(
        &mut db,
        conversation_id.trim(),
        title.as_deref(),
    )?)
}

#[tauri::command]
pub fn export_project(
    state: State<'_, AppState>,
    input: ExportProjectInput,
) -> Result<ExportProjectResult, CommandError> {
    let db = state.db.lock();
    let payload = export_project_json(&db, input.project_id.trim())?;
    let path = std::path::PathBuf::from(input.destination_path.trim());
    if path.as_os_str().is_empty() {
        return Err(CommandError::new("invalid", "Choose a save location"));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| CommandError::new("io", format!("Could not create folder: {e}")))?;
    }
    let text = serde_json::to_string_pretty(&payload)
        .map_err(|e| CommandError::new("export", e.to_string()))?;
    std::fs::write(&path, text)
        .map_err(|e| CommandError::new("io", format!("Could not write export: {e}")))?;
    Ok(ExportProjectResult {
        ok: true,
        path: path.to_string_lossy().into_owned(),
        message: "Project exported".into(),
    })
}

#[tauri::command]
pub fn set_project_wallpaper_cmd(
    state: State<'_, AppState>,
    project_id: String,
    wallpaper_json: Option<String>,
) -> Result<Project, CommandError> {
    let mut db = state.db.lock();
    Ok(set_project_wallpaper(
        &mut db,
        project_id.trim(),
        wallpaper_json.as_deref(),
    )?)
}

#[tauri::command]
pub fn touch_project_opened(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Project, CommandError> {
    let mut db = state.db.lock();
    Ok(touch_last_opened(&mut db, project_id.trim())?)
}

#[tauri::command]
pub fn assign_conversation_to_project_cmd(
    state: State<'_, AppState>,
    conversation_id: String,
    project_id: String,
) -> Result<Conversation, CommandError> {
    let mut db = state.db.lock();
    Ok(assign_conversation_to_project(
        &mut db,
        conversation_id.trim(),
        project_id.trim(),
    )?)
}
