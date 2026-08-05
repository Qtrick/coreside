//! Exa connection, usage, budget, and search profile commands.

use serde::{Deserialize, Serialize};
use tauri::State;

use super::CommandError;
use crate::exa::{
    budget_status, delete_exa_api_key, list_recent_usage, load_budget_config, load_search_profile,
    resolve_exa_credentials, save_budget_config, save_search_profile, store_exa_api_key,
    test_connection, test_connection_with_key, usage_summary, BudgetStatus, SearchProfile,
    UsageEntry, UsageSummary,
};
use crate::security::sanitize_error;
use crate::state::AppState;

fn map_exa_err(e: crate::exa::ExaError) -> CommandError {
    let key = resolve_exa_credentials().api_key;
    CommandError::new(e.code(), sanitize_error(&e.to_string(), key.as_deref()))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExaConnectionView {
    pub configured: bool,
    pub source: String,
    pub account: String,
}

#[tauri::command]
pub fn get_exa_connection() -> Result<ExaConnectionView, CommandError> {
    let resolved = resolve_exa_credentials();
    Ok(ExaConnectionView {
        configured: resolved.has_key(),
        source: resolved.source.as_str().into(),
        account: crate::exa::exa_keyring_account(),
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigureExaInput {
    pub api_key: String,
}

#[tauri::command]
pub async fn configure_exa_connection(
    input: ConfigureExaInput,
) -> Result<ExaConnectionView, CommandError> {
    let new_key = input.api_key.trim().to_string();
    if new_key.is_empty() {
        return Err(CommandError::new("invalid", "Exa API key is required"));
    }

    // Test-before-swap: probe the candidate key without replacing the prior secret first.
    test_connection_with_key(&new_key)
        .await
        .map_err(map_exa_err)?;

    let previous = resolve_exa_credentials().api_key;
    store_exa_api_key(&new_key).map_err(map_exa_err)?;

    // Compensating restore if a follow-up probe against the stored key fails.
    if let Err(e) = test_connection().await {
        if let Some(prev) = previous.as_deref() {
            let _ = store_exa_api_key(prev);
        } else {
            let _ = delete_exa_api_key();
        }
        return Err(map_exa_err(e));
    }

    get_exa_connection()
}

#[tauri::command]
pub fn delete_exa_connection() -> Result<(), CommandError> {
    delete_exa_api_key().map_err(map_exa_err)
}

#[tauri::command]
pub async fn test_exa_connection() -> Result<String, CommandError> {
    // Reload .env so EXA_API_KEY edits are picked up without restart.
    let _ = crate::config::load_config();
    test_connection().await.map_err(map_exa_err)
}

#[tauri::command]
pub fn get_exa_usage(
    state: State<'_, AppState>,
    month_key: Option<String>,
) -> Result<UsageSummary, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    usage_summary(&db, month_key.as_deref()).map_err(CommandError::from)
}

#[tauri::command]
pub fn list_exa_usage(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<UsageEntry>, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    list_recent_usage(&db, limit.unwrap_or(50)).map_err(CommandError::from)
}

#[tauri::command]
pub fn get_exa_budget(state: State<'_, AppState>) -> Result<BudgetStatus, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    Ok(budget_status(&db))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetExaBudgetInput {
    pub monthly_budget_usd: Option<f64>,
    pub soft_percent: Option<f64>,
    pub critical_percent: Option<f64>,
    pub hard_percent: Option<f64>,
}

#[tauri::command]
pub fn set_exa_budget(
    state: State<'_, AppState>,
    input: SetExaBudgetInput,
) -> Result<BudgetStatus, CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    let mut cfg = load_budget_config(&db);
    // Always apply monthly budget from this command: null / missing = unlimited.
    cfg.monthly_budget_usd = input.monthly_budget_usd.filter(|v| *v > 0.0);
    if let Some(v) = input.soft_percent {
        cfg.soft_percent = v.clamp(1.0, 100.0);
    }
    if let Some(v) = input.critical_percent {
        cfg.critical_percent = v.clamp(1.0, 100.0);
    }
    if let Some(v) = input.hard_percent {
        cfg.hard_percent = v.clamp(1.0, 100.0);
    }
    // Keep soft ≤ critical ≤ hard.
    if cfg.soft_percent > cfg.critical_percent {
        cfg.soft_percent = cfg.critical_percent;
    }
    if cfg.critical_percent > cfg.hard_percent {
        cfg.critical_percent = cfg.hard_percent;
    }
    save_budget_config(&mut db, &cfg).map_err(CommandError::from)?;
    Ok(budget_status(&db))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchProfileView {
    pub profile: String,
    pub search_type: String,
    pub num_results: usize,
    pub max_crawl_pages: usize,
    pub max_refinements: usize,
}

impl From<SearchProfile> for SearchProfileView {
    fn from(profile: SearchProfile) -> Self {
        Self {
            profile: profile.as_str().into(),
            search_type: profile.search_type().into(),
            num_results: profile.num_results(),
            max_crawl_pages: profile.max_crawl_pages(),
            max_refinements: profile.max_refinements(),
        }
    }
}

#[tauri::command]
pub fn get_search_profile(state: State<'_, AppState>) -> Result<SearchProfileView, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    Ok(SearchProfileView::from(load_search_profile(&db)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetSearchProfileInput {
    pub profile: String,
}

#[tauri::command]
pub fn set_search_profile(
    state: State<'_, AppState>,
    input: SetSearchProfileInput,
) -> Result<SearchProfileView, CommandError> {
    state.require_profile()?;
    let profile = SearchProfile::parse(&input.profile);
    let mut db = state.db.lock();
    save_search_profile(&mut db, profile).map_err(CommandError::from)?;
    Ok(SearchProfileView::from(profile))
}
