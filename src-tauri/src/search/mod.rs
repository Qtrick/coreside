//! Trusted web research: provider-neutral layer with SSRF-safe fetch.

mod cache;
mod citations;
mod errors;
mod fetch;
mod history;
mod images;
mod mock;
mod models;
mod normalization;
mod provider;
mod registry;
mod safety;
mod videos;
mod web;

pub use errors::SearchError;
pub use fetch::{
    brave_account, build_http_client, delete_search_secret, fetch_web_page, get_search_secret,
    search_keyring_account, send_public_get, set_search_secret, MAX_FETCH_BYTES,
};
pub use history::{
    clear_search_history, get_search_session, list_search_sessions, persist_image_session,
    persist_video_session, persist_web_session, SearchSessionDetail, SearchSessionSummary,
};
pub use images::image_search;
pub use models::{
    FetchedWebPage, ImageSearchResponse, ImageSearchResult, ResearchIntent, SafeSearchLevel,
    SearchResultProvenance, VideoSearchResponse, VideoSearchResult, WebSearchResponse,
    WebSearchResult,
};
pub use normalization::{
    bound_text, normalize_image_results, normalize_video_results, normalize_web_results,
};
pub use provider::{SearchProvider, SearchRequest};
pub use registry::SearchRegistry;
pub use safety::validate_public_http_url;
pub use videos::video_search;
pub use web::web_search;
