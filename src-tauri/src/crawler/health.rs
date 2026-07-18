use serde::{Deserialize, Serialize};

use super::errors::CrawlerError;
use super::installation::{detect_installation, resolve_sidecar_python};
use super::models::InstallationState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHealth {
    pub ready: bool,
    pub venv_python_exists: bool,
    pub crawl4ai_import_ok: bool,
    pub python_path: Option<String>,
    pub detail: Option<String>,
}

/// Detect whether the local Crawl4AI runtime can start (venv + import).
pub fn detect_runtime_health() -> RuntimeHealth {
    let report = detect_installation();
    let python = resolve_sidecar_python();
    RuntimeHealth {
        ready: report.state == InstallationState::Ready,
        venv_python_exists: python.exists(),
        crawl4ai_import_ok: report.crawl4ai_import_ok,
        python_path: report.python_path.map(|p| p.display().to_string()),
        detail: report.reason,
    }
}

pub fn assert_runtime_ready() -> Result<RuntimeHealth, CrawlerError> {
    let health = detect_runtime_health();
    if health.ready {
        Ok(health)
    } else {
        Err(CrawlerError::NeedsSetup(
            health
                .detail
                .unwrap_or_else(|| "Local research engine is not installed".into()),
        ))
    }
}
