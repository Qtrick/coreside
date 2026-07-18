//! Crawl4AI sidecar supervisor and protocol.

#![allow(unused_imports)]

mod cancellation;
mod errors;
mod health;
mod installation;
mod models;
mod protocol;
mod resources;
mod supervisor;

pub use cancellation::cancel_request;
pub use errors::CrawlerError;
pub use health::{assert_runtime_ready, detect_runtime_health, RuntimeHealth};
pub use installation::{
    detect_installation, require_ready, resolve_crawler_data_root, resolve_sidecar_python,
    InstallationReport,
};
pub use models::{
    CacheStatsView, CrawlerStatusView, InstallationState, ProtocolEnvelope, ProtocolRequest,
    PROTOCOL_VERSION,
};
pub use resources::ResourceProfile;
pub use supervisor::{to_search_error, CrawlerSupervisor};
