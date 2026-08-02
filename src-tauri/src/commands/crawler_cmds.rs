//! Local Crawl4AI research engine commands.

use serde::Deserialize;
use tauri::State;

use super::CommandError;
use crate::crawler::{detect_installation, CacheStatsView, CrawlerStatusView, ResourceProfile};
use crate::security::sanitize_error;
use crate::state::AppState;

fn map_crawler_err(e: crate::crawler::CrawlerError) -> CommandError {
    CommandError::new(e.code(), sanitize_error(&e.to_string(), None))
}

fn load_resource_profile(state: &AppState) -> ResourceProfile {
    let db = state.db.lock();
    let value: Option<String> = db
        .conn()
        .query_row(
            "SELECT value FROM crawler_settings WHERE key = 'webResearchResourceProfile'",
            [],
            |r| r.get(0),
        )
        .ok();
    value
        .map(|v| ResourceProfile::parse(&v))
        .unwrap_or(ResourceProfile::DEFAULT)
}

/// Apply persisted resource profile to the live supervisor (call before research work).
pub async fn sync_resource_profile(state: &AppState) -> ResourceProfile {
    let profile = load_resource_profile(state);
    state.crawler.set_resource_profile(profile).await;
    profile
}

#[tauri::command]
pub async fn get_crawler_status(
    state: State<'_, AppState>,
) -> Result<CrawlerStatusView, CommandError> {
    state.require_profile()?;
    let profile = sync_resource_profile(&state).await;
    let mut view = state.crawler.status_view().await;
    view.resource_profile = profile.as_str().to_string();
    Ok(view)
}

#[tauri::command]
pub fn get_crawler_installation() -> Result<crate::crawler::InstallationReport, CommandError> {
    Ok(detect_installation())
}

#[tauri::command]
pub async fn cleanup_crawler_cache(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, CommandError> {
    state.require_profile()?;
    state
        .crawler
        .send_command("cleanup_cache", serde_json::json!({}))
        .await
        .map_err(map_crawler_err)
}

#[tauri::command]
pub async fn get_crawler_cache_stats(
    state: State<'_, AppState>,
) -> Result<CacheStatsView, CommandError> {
    state.require_profile()?;
    let payload = state
        .crawler
        .send_command("cache_stats", serde_json::json!({}))
        .await
        .map_err(map_crawler_err)?;
    Ok(CacheStatsView::from_payload(&payload))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetResourceProfileInput {
    pub profile: String,
}

#[tauri::command]
pub async fn set_web_research_resource_profile(
    state: State<'_, AppState>,
    input: SetResourceProfileInput,
) -> Result<String, CommandError> {
    state.require_profile()?;
    let profile = ResourceProfile::parse(&input.profile);
    state.crawler.set_resource_profile(profile).await;
    {
        let db = state.db.lock();
        db.conn()
            .execute(
                "INSERT INTO crawler_settings (key, value, updated_at)
                 VALUES ('webResearchResourceProfile', ?1, datetime('now'))
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = excluded.updated_at",
                [profile.as_str()],
            )
            .map_err(|e| CommandError::from(crate::db::DbError::from(e)))?;
    }
    Ok(profile.as_str().to_string())
}
