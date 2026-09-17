//! Local web research: discovery ranking + hybrid Exa / Crawl4AI provider.

#![allow(unused_imports)]

mod discovery;
mod hybrid;
pub mod orchestrator;
mod provider;
mod ranking;
pub mod retrieval;

pub use discovery::{
    classify_discovery_seed, DiscoverySeed, NEEDS_EXA_OR_SEED_MESSAGE, NEEDS_SEED_MESSAGE,
};
pub use hybrid::{research_capability_notice, HybridSearchProvider};
pub use orchestrator::{
    canonicalize_url, classify_research_intent, deduplicate_results, sanitize_prompt_injection,
};
pub use provider::Crawl4aiSearchProvider;
pub use ranking::rank_web_candidates;
pub use retrieval::fetch_web_page_orchestrated;
