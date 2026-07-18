//! Application configuration loaded from environment / `.env`.

mod env;

pub use env::{
    load_config, AppConfig, PublicAiStatus, DEFAULT_ANTHROPIC_BASE_URL, DEFAULT_ANTHROPIC_MODEL,
    DEFAULT_GEMINI_BASE_URL, DEFAULT_GEMINI_MODEL, DEFAULT_OPENAI_BASE_URL, DEFAULT_OPENAI_MODEL,
    DEFAULT_OPENROUTER_BASE_URL, DEFAULT_OPENROUTER_MODEL,
};
