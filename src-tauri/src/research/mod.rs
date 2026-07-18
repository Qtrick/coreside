//! Local web research: discovery ranking + hybrid Exa / Crawl4AI provider.

#![allow(unused_imports)]

mod discovery;
mod hybrid;
mod provider;
mod ranking;

pub use discovery::{
    classify_discovery_seed, DiscoverySeed, NEEDS_EXA_OR_SEED_MESSAGE, NEEDS_SEED_MESSAGE,
};
pub use hybrid::{research_capability_notice, HybridSearchProvider};
pub use provider::Crawl4aiSearchProvider;
pub use ranking::rank_web_candidates;
