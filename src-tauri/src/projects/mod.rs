//! Projects domain: CRUD, indexing, summaries, retrieval, export.

mod export;
mod indexing;
mod models;
mod repository;
mod retrieval;
mod summaries;
mod validation;

pub use export::export_project_json;
pub use indexing::{
    index_message_for_conversation, rebuild_project_index, remove_conversation_from_index,
};
pub use models::{
    CreateProjectInput, DeleteProjectMode, Project, ProjectContextHit, ProjectContextSettings,
    UpdateProjectInput,
};
pub use repository::{
    archive_project, assign_conversation_to_project, create_project, delete_project,
    duplicate_conversation, get_project, list_project_conversations, list_projects,
    list_unassigned_conversations, move_conversations_to_project, remove_conversation_from_project,
    rename_conversation, restore_project, set_project_wallpaper, touch_last_opened, update_project,
};
pub use retrieval::search_project_context;
pub use summaries::{deterministic_fallback_summary, set_project_summary};

use crate::db::{get_setting, Database, DbResult};

/// Load project-context related settings from the settings table.
pub fn load_project_context_settings(db: &Database) -> DbResult<ProjectContextSettings> {
    let include = get_setting(db, "includeProjectContext")?
        .map(|v| v != "false")
        .unwrap_or(true);
    let safe_search = get_setting(db, "safeSearch")?.unwrap_or_else(|| "standard".into());
    Ok(ProjectContextSettings {
        include_project_context: include,
        safe_search,
    })
}
