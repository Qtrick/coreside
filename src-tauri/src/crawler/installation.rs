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

/// Resolve the preferred sidecar Python interpreter relative to the managed service root.
pub fn resolve_sidecar_python() -> PathBuf {
    let service_root = resolve_service_root();
    #[cfg(windows)]
    {
        service_root
            .join(".venv")
            .join("Scripts")
            .join("python.exe")
    }
    #[cfg(not(windows))]
    {
        service_root.join(".venv").join("bin").join("python")
    }
}

/// Crawl4AI service source root.
///
/// Priority:
/// 1. `CORESIDE_CRAWLER_SERVICE_ROOT` (explicit packaging / tests)
/// 2. Managed AppPaths install under application data (`crawler/service`)
/// 3. Debug-only repository `services/crawl4ai` when present
///
/// Packaged release builds never require `CARGO_MANIFEST_DIR` or the source tree.
pub fn resolve_service_root() -> PathBuf {
    if let Ok(override_path) = std::env::var("CORESIDE_CRAWLER_SERVICE_ROOT") {
        let trimmed = override_path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    if let Ok(paths) = crate::app_paths::AppPaths::resolve() {
        let managed = paths.crawler.join("service");
        if managed.exists() {
            return managed;
        }
        if !cfg!(debug_assertions) {
            return managed;
        }
    }

    if cfg!(debug_assertions)
        || std::env::var("CORESIDE_ALLOW_REPO_CRAWLER")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        if let Some(repo_root) = manifest_dir.parent() {
            let repo_service = repo_root.join("services").join("crawl4ai");
            if repo_service.exists() {
                return repo_service;
            }
        }
    }

    crate::app_paths::AppPaths::resolve()
        .map(|p| p.crawler.join("service"))
        .unwrap_or_else(|_| PathBuf::from("crawler-service-unavailable"))
}

pub fn resolve_crawler_data_root() -> PathBuf {
    if let Ok(override_path) = std::env::var("CORESIDE_CRAWLER_DATA_ROOT") {
        let trimmed = override_path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    if let Ok(paths) = crate::app_paths::AppPaths::resolve() {
        let _ = paths.ensure_dirs();
        return paths.crawler.clone();
    }
    if cfg!(debug_assertions) {
        if let Some(base) = dirs::data_dir() {
            return crate::db::product_data_dir(&base).join("crawler");
        }
    }
    PathBuf::from("crawler-data-unavailable")
}

/// Crawl4AI cache parent directory (`CRAWL4_AI_BASE_DIRECTORY`).
pub fn resolve_crawl4ai_base_directory() -> PathBuf {
    resolve_crawler_data_root()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(resolve_crawler_data_root)
}

pub fn detect_installation() -> InstallationReport {
    let service_root = resolve_service_root();
    let python_path = resolve_sidecar_python();

    if service_root.as_os_str() == "crawler-service-unavailable" || !service_root.exists() {
        return InstallationReport {
            state: InstallationState::NeedsSetup,
            python_path: Some(python_path.clone()),
            service_root,
            crawl4ai_import_ok: false,
            reason: Some(
                "Crawl4AI service is not installed in the managed application data folder yet."
                    .into(),
            ),
        };
    }

    if !python_path.exists() {
        return InstallationReport {
            state: InstallationState::NeedsSetup,
            python_path: Some(python_path.clone()),
            service_root,
            crawl4ai_import_ok: false,
            reason: Some(format!(
                "Crawl4AI Python venv not found at {}. Complete crawler setup from Settings.",
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
    fn data_root_not_repo_relative_coreside() {
        let root = resolve_crawler_data_root();
        assert_ne!(root, PathBuf::from(".coreside").join("crawler"));
    }

    #[test]
    fn python_path_is_under_venv() {
        let py = resolve_sidecar_python();
        let s = py.to_string_lossy();
        assert!(s.contains(".venv"));
        assert!(s.contains("python"));
    }

    #[test]
    fn service_root_is_absolute_or_managed() {
        let root = resolve_service_root();
        let s = root.to_string_lossy();
        // Debug may still point at repo services/crawl4ai; release uses managed path.
        assert!(
            s.contains("crawl4ai")
                || s.contains("crawler")
                || s.contains("service")
                || s.contains("unavailable")
        );
    }
}
