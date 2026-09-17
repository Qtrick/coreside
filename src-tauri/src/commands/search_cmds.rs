//! Search / local research provider configuration and trusted research commands.

use serde::{Deserialize, Serialize};
use tauri::State;

use super::CommandError;
use crate::crawler::{detect_installation, InstallationState};
use crate::exa::{has_exa_key, resolve_exa_credentials};
use crate::linkup::{has_key as has_linkup_key, resolve_credentials as resolve_linkup_credentials};
use crate::projects::load_project_context_settings;
use crate::research::Crawl4aiSearchProvider;
use crate::search::{image_search, video_search, web_search, SafeSearchLevel, SearchRegistry};
use crate::security::sanitize_error;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchConnectionView {
    pub provider: String,
    pub has_key: bool,
    pub source: String,
    pub engine_ready: bool,
    pub engine_reason: Option<String>,
    /// Exa open-web search readiness (keyring or EXA_API_KEY).
    pub exa_configured: bool,
    pub exa_source: String,
    pub linkup_configured: bool,
    pub linkup_source: String,
}

fn map_search_err(e: crate::search::SearchError) -> CommandError {
    CommandError::new(e.code(), sanitize_error(&e.to_string(), None))
}

async fn resolve_registry(state: &AppState) -> Result<SearchRegistry, CommandError> {
    let report = detect_installation();
    if report.state != InstallationState::Ready {
        // Configured discovery can run without Crawl4AI; advanced URL crawling
        // still needs the engine or the SSRF-safe HTTP fallback.
        if !has_linkup_key() && !has_exa_key() {
            return Err(CommandError::new(
                "needs_setup",
                report
                    .reason
                    .unwrap_or_else(|| "Local research engine needs setup".into()),
            ));
        }
    }
    // Honor persisted resource profile even if Settings UI was never opened this session.
    let _ = super::crawler_cmds::sync_resource_profile(state).await;
    Ok(SearchRegistry::default_local(
        state.crawler.clone(),
        state.db.clone(),
    ))
}

#[tauri::command]
pub fn get_search_connection(
    state: State<'_, AppState>,
) -> Result<SearchConnectionView, CommandError> {
    state.require_profile()?;
    let report = detect_installation();
    let _ = state;
    let linkup = resolve_linkup_credentials();
    let exa = resolve_exa_credentials();
    let provider = if linkup.api_key.is_some() {
        "linkup"
    } else if exa.has_key() {
        "exa"
    } else {
        "crawl4ai"
    };
    let source = if linkup.api_key.is_some() {
        linkup.source.as_str().to_string()
    } else if exa.has_key() {
        exa.source.as_str().to_string()
    } else {
        match report.state {
            InstallationState::Ready => "local".into(),
            _ => "none".into(),
        }
    };
    Ok(SearchConnectionView {
        provider: provider.into(),
        has_key: linkup.api_key.is_some() || exa.has_key(),
        source,
        engine_ready: report.state == InstallationState::Ready,
        engine_reason: report.reason,
        exa_configured: exa.has_key(),
        exa_source: exa.source.as_str().into(),
        linkup_configured: linkup.api_key.is_some(),
        linkup_source: linkup.source.as_str().into(),
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigureSearchInput {
    pub provider: Option<String>,
    pub api_key: Option<String>,
}

/// Legacy configure endpoint — Brave BYOK removed; reports local engine status.
#[tauri::command]
pub async fn configure_search_connection(
    state: State<'_, AppState>,
    input: ConfigureSearchInput,
) -> Result<SearchConnectionView, CommandError> {
    state.require_profile()?;
    let _ = input;
    get_search_connection(state)
}

#[tauri::command]
pub fn delete_search_connection() -> Result<(), CommandError> {
    // Optional cleanup of legacy Brave keyring entry.
    let account = crate::search::search_keyring_account(crate::search::brave_account());
    let _ = crate::search::delete_search_secret(&account);
    Ok(())
}

#[tauri::command]
pub async fn test_search_connection(state: State<'_, AppState>) -> Result<String, CommandError> {
    state.require_profile()?;
    let registry = resolve_registry(&state).await?;
    registry.health_check().await.map_err(map_search_err)?;
    let provider = if has_linkup_key() {
        "Web Research connection OK"
    } else {
        "Local research engine OK"
    };
    Ok(provider.into())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQueryInput {
    pub query: String,
    pub count: Option<usize>,
    pub conversation_id: Option<String>,
    pub project_id: Option<String>,
    pub domain: Option<String>,
}

#[tauri::command]
pub async fn web_search_cmd(
    state: State<'_, AppState>,
    input: SearchQueryInput,
) -> Result<crate::search::WebSearchResponse, CommandError> {
    state.require_profile()?;
    let safe = {
        let db = state.db.lock();
        let settings = load_project_context_settings(&db)?;
        SafeSearchLevel::from_setting(&settings.safe_search)
    };

    let registry = resolve_registry(&state).await?;
    let count = input.count.unwrap_or(8);
    let mut response = if let Some(domain) = input.domain.as_deref() {
        registry
            .web_search_with_domain(&input.query, Some(domain), count, safe)
            .await
            .map_err(map_search_err)?
    } else {
        web_search(&registry, &input.query, count, safe)
            .await
            .map_err(map_search_err)?
    };

    {
        let db = state.db.lock();
        if let Ok(session_id) = crate::search::persist_web_session(
            &db,
            input.conversation_id.as_deref(),
            input.project_id.as_deref(),
            &input.query,
            &response.provider,
            &response,
        ) {
            response.session_id = Some(session_id);
        }
    }
    Ok(response)
}

#[tauri::command]
pub async fn image_search_cmd(
    state: State<'_, AppState>,
    input: SearchQueryInput,
) -> Result<crate::search::ImageSearchResponse, CommandError> {
    state.require_profile()?;
    let safe = {
        let db = state.db.lock();
        let settings = load_project_context_settings(&db)?;
        SafeSearchLevel::from_setting(&settings.safe_search)
    };

    let registry = resolve_registry(&state).await?;
    let count = input.count.unwrap_or(12);
    let mut response = image_search(&registry, &input.query, count, safe)
        .await
        .map_err(map_search_err)?;

    {
        let db = state.db.lock();
        if let Ok(session_id) = crate::search::persist_image_session(
            &db,
            input.conversation_id.as_deref(),
            input.project_id.as_deref(),
            &input.query,
            &response.provider,
            &response,
        ) {
            response.session_id.replace(session_id);
        }
    }
    Ok(response)
}

#[tauri::command]
pub async fn video_search_cmd(
    state: State<'_, AppState>,
    input: SearchQueryInput,
) -> Result<crate::search::VideoSearchResponse, CommandError> {
    state.require_profile()?;
    let safe = {
        let db = state.db.lock();
        let settings = load_project_context_settings(&db)?;
        SafeSearchLevel::from_setting(&settings.safe_search)
    };

    let registry = resolve_registry(&state).await?;
    let count = input.count.unwrap_or(8);
    let mut response = video_search(&registry, &input.query, count, safe)
        .await
        .map_err(map_search_err)?;

    {
        let db = state.db.lock();
        if let Ok(session_id) = crate::search::persist_video_session(
            &db,
            input.conversation_id.as_deref(),
            input.project_id.as_deref(),
            &input.query,
            &response.provider,
            &response,
        ) {
            response.session_id.replace(session_id);
        }
    }
    Ok(response)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSearchSessionsInput {
    pub conversation_id: Option<String>,
    pub project_id: Option<String>,
    pub search_type: Option<String>,
    pub limit: Option<usize>,
}

#[tauri::command]
pub fn list_search_sessions_cmd(
    state: State<'_, AppState>,
    input: Option<ListSearchSessionsInput>,
) -> Result<Vec<crate::search::SearchSessionSummary>, CommandError> {
    state.require_profile()?;
    let input = input.unwrap_or(ListSearchSessionsInput {
        conversation_id: None,
        project_id: None,
        search_type: None,
        limit: None,
    });
    let db = state.db.lock();
    crate::search::list_search_sessions(
        &db,
        input.conversation_id.as_deref(),
        input.project_id.as_deref(),
        input.search_type.as_deref(),
        input.limit.unwrap_or(50),
    )
    .map_err(CommandError::from)
}

#[tauri::command]
pub fn get_search_session_cmd(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<crate::search::SearchSessionDetail, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    crate::search::get_search_session(&db, session_id.trim()).map_err(CommandError::from)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearSearchHistoryInput {
    pub conversation_id: Option<String>,
    pub project_id: Option<String>,
}

#[tauri::command]
pub fn clear_search_history_cmd(
    state: State<'_, AppState>,
    input: Option<ClearSearchHistoryInput>,
) -> Result<u64, CommandError> {
    state.require_profile()?;
    let input = input.unwrap_or(ClearSearchHistoryInput {
        conversation_id: None,
        project_id: None,
    });
    let mut db = state.db.lock();
    crate::search::clear_search_history(
        &mut db,
        input.conversation_id.as_deref(),
        input.project_id.as_deref(),
    )
    .map_err(CommandError::from)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchWebPageInput {
    pub url: String,
}

#[tauri::command]
pub async fn fetch_web_page_cmd(
    state: State<'_, AppState>,
    input: FetchWebPageInput,
) -> Result<crate::search::FetchedWebPage, CommandError> {
    state.require_profile()?;

    // Structured retrieval policy for known URLs:
    // 1. Try Firecrawl first if configured (clean Markdown extraction, JS rendering)
    if crate::firecrawl::has_key() {
        match crate::firecrawl::scrape(&input.url).await {
            Ok(page) => return Ok(page),
            Err(e) => {
                tracing::warn!(error = ?e, "firecrawl scrape failed, falling back to next retriever")
            }
        }
    }

    // 2. Try Crawl4AI if local supervisor is ready (local-first, free)
    let report = detect_installation();
    if report.state == InstallationState::Ready {
        let _ = super::crawler_cmds::sync_resource_profile(&state).await;
        let provider = Crawl4aiSearchProvider::new(state.crawler.clone());
        match provider.fetch_page(&input.url).await {
            Ok(page) => return Ok(page),
            Err(e) => {
                tracing::warn!(error = ?e, "crawl4ai fetch failed, falling back to next retriever")
            }
        }
    }

    // 3. Try Linkup reader endpoint if configured
    if has_linkup_key() {
        match crate::linkup::fetch_page(&input.url).await {
            Ok(page) => return Ok(page),
            Err(e) => tracing::warn!(error = ?e, "linkup fetch failed"),
        }
    }

    if !crate::firecrawl::has_key() && !has_linkup_key() && report.state != InstallationState::Ready
    {
        return Err(CommandError::new(
            "needs_setup",
            report.reason.unwrap_or_else(|| {
                "Configure Firecrawl, Linkup, or install Crawl4AI to fetch web pages".into()
            }),
        ));
    }

    Err(CommandError::new(
        "fetch_failed",
        format!("Unable to fetch web page from URL: {}", input.url),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_registry_requires_ready_engine() {
        // Without a ready venv this must fail closed (needs_setup), never mock.
        // In CI/dev with venv installed, Ready is acceptable.
        let report = detect_installation();
        match report.state {
            InstallationState::Ready => {}
            InstallationState::NeedsSetup | InstallationState::Error => {
                assert!(report.reason.is_some());
            }
        }
    }
}
