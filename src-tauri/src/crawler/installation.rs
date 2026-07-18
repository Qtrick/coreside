use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use super::errors::CrawlerError;
use super::models::InstallationState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallationReport {
    pub state: InstallationState,
    pub python_path: Option<PathBuf>,
    pub service_root: PathBuf,
    pub crawl4ai_import_ok: bool,
    pub reason: Option<String>,
}

/// Resolve the preferred sidecar Python interpreter relative to the repo / bundle.
pub fn resolve_sidecar_python() -> PathBuf {
    let service_root = resolve_service_root();
    #[cfg(windows)]
    {
        service_root.join(".venv").join("Scripts").join("python.exe")
    }
    #[cfg(not(windows))]
    {
        service_root.join(".venv").join("bin").join("python")
    }
}

pub fn resolve_service_root() -> PathBuf {
    // Prefer explicit override for packaging / tests.
    if let Ok(override_path) = std::env::var("CORESIDE_CRAWLER_SERVICE_ROOT") {
        let trimmed = override_path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(repo_root) = manifest_dir.parent() {
        return repo_root.join("services").join("crawl4ai");
    }
    PathBuf::from("services").join("crawl4ai")
}

pub fn resolve_crawler_data_root() -> PathBuf {
    if let Ok(override_path) = std::env::var("CORESIDE_CRAWLER_DATA_ROOT") {
        let trimmed = override_path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    if let Some(base) = dirs::data_dir() {
        return base.join("coreside").join("crawler");
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(repo_root) = manifest_dir.parent() {
        return repo_root.join(".coreside").join("crawler");
    }
    PathBuf::from(".coreside").join("crawler")
}

/// Crawl4AI cache parent directory (`CRAWL4_AI_BASE_DIRECTORY`).
pub fn resolve_crawl4ai_base_directory() -> PathBuf {
    resolve_crawler_data_root()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(".coreside"))
}

pub fn detect_installation() -> InstallationReport {
    let service_root = resolve_service_root();
    let python_path = resolve_sidecar_python();

    if !python_path.exists() {
        return InstallationReport {
            state: InstallationState::NeedsSetup,
            python_path: Some(python_path.clone()),
            service_root,
            crawl4ai_import_ok: false,
            reason: Some(format!(
                "Crawl4AI Python venv not found at {}. Run the crawler setup script.",
                python_path.display()
            )),
        };
    }

    match probe_crawl4ai_import(&python_path) {
        Ok(()) => InstallationReport {
            state: InstallationState::Ready,
            python_path: Some(python_path),
            service_root,
            crawl4ai_import_ok: true,
            reason: None,
        },
        Err(reason) => InstallationReport {
            state: InstallationState::NeedsSetup,
            python_path: Some(python_path),
            service_root,
            crawl4ai_import_ok: false,
            reason: Some(reason),
        },
    }
}

pub fn require_ready() -> Result<InstallationReport, CrawlerError> {
    let report = detect_installation();
    match report.state {
        InstallationState::Ready => Ok(report),
        InstallationState::NeedsSetup | InstallationState::Error => Err(CrawlerError::NeedsSetup(
            report
                .reason
                .clone()
                .unwrap_or_else(|| "Local research engine needs setup".into()),
        )),
    }
}

fn probe_crawl4ai_import(python: &Path) -> Result<(), String> {
    let output = Command::new(python)
        .args([
            "-c",
            "import crawl4ai; import coreside_crawler; print('ok')",
        ])
        .current_dir(resolve_service_root())
        .output()
        .map_err(|e| format!("failed to execute {}: {e}", python.display()))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "crawl4ai/sidecar import failed: {}",
            stderr.trim().chars().take(240).collect::<String>()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_root_points_at_crawl4ai() {
        let root = resolve_service_root();
        assert!(root.ends_with("crawl4ai") || root.to_string_lossy().contains("crawl4ai"));
    }

    #[test]
    fn python_path_is_under_venv() {
        let py = resolve_sidecar_python();
        let s = py.to_string_lossy();
        assert!(s.contains(".venv"));
        assert!(s.contains("python"));
    }
}
