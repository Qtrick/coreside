//! Application configuration loaded from environment / `.env`.

mod env;

pub use env::{load_config, AppConfig, PublicAiStatus};
