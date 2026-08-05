//! Exa Search integration: credentials, client, usage, budget, profiles.

#![allow(unused_imports)]

mod budget;
mod cache;
mod client;
mod credentials;
mod errors;
mod models;
mod profiles;
mod usage;

pub use budget::{
    assert_can_spend, budget_status, classify_threshold, load_budget_config, save_budget_config,
    BudgetConfig, BudgetStatus, BudgetThreshold,
};
pub use cache::{fingerprint, normalize_query, ExaSearchCache};
pub use client::{
    peek_cached_search, search, test_connection, test_connection_with_key, ExaSearchOutcome,
};
pub use credentials::{
    delete_exa_api_key, exa_keyring_account, has_exa_key, require_exa_api_key,
    resolve_exa_credentials, store_exa_api_key, ExaCredentialSource, ResolvedExaCredentials,
    EXA_ENV_KEY, EXA_PROVIDER,
};
pub use errors::ExaError;
pub use models::{
    estimate_search_cost, ExaContentsRequest, ExaCostDollars, ExaResult, ExaSearchRequest,
    ExaSearchResponse,
};
pub use profiles::SearchProfile;
pub use usage::{
    list_recent_usage, month_key_now, record_usage, sum_month_actual_cost, usage_summary,
    UsageEntry, UsageSummary,
};

use crate::db::{get_setting, set_setting, Database, DbResult};

pub const SETTING_SEARCH_PROFILE: &str = "searchProfile";

pub fn load_search_profile(db: &Database) -> SearchProfile {
    match get_setting(db, SETTING_SEARCH_PROFILE) {
        Ok(Some(raw)) => SearchProfile::parse(&raw),
        _ => SearchProfile::Saver,
    }
}

pub fn save_search_profile(db: &mut Database, profile: SearchProfile) -> DbResult<()> {
    set_setting(db, SETTING_SEARCH_PROFILE, profile.as_str())
}
