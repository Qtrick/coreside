//! Single Crawl4AI sidecar process supervisor (stdio NDJSON).

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{oneshot, Mutex};
use tokio::time::{sleep, timeout};
use uuid::Uuid;

use super::errors::CrawlerError;
use super::health::assert_runtime_ready;
use super::installation::{
    resolve_crawl4ai_base_directory, resolve_crawler_data_root, resolve_service_root,
    resolve_sidecar_python,
};
use super::models::{CrawlerStatusView, ProtocolRequest};
use super::protocol::{
    encode_request, is_terminal_event, matches_request_id, parse_event_line,
    terminal_payload_result, validate_version,
};
use super::resources::ResourceProfile;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_RESTART_ATTEMPTS: u32 = 3;

struct PendingWaiter {
    tx: oneshot::Sender<Result<Value, CrawlerError>>,
}

struct LiveChild {
    child: Child,
    stdin: ChildStdin,
}

struct SupervisorInner {
    live: Option<LiveChild>,
    pending: HashMap<String, PendingWaiter>,
    restart_attempts: u32,
    resource_profile: ResourceProfile,
    shutting_down: bool,
}

/// Owns at most one Crawl4AI sidecar child; restarts with bounded backoff.
pub struct CrawlerSupervisor {
    inner: Arc<Mutex<SupervisorInner>>,
    running: Arc<AtomicBool>,
}

impl Default for CrawlerSupervisor {
    fn default() -> Self {
        Self::new()
    }
}

impl CrawlerSupervisor {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(SupervisorInner {
                live: None,
                pending: HashMap::new(),
                restart_attempts: 0,
                resource_profile: ResourceProfile::DEFAULT,
                shutting_down: false,
            })),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub async fn set_resource_profile(&self, profile: ResourceProfile) {
        self.inner.lock().await.resource_profile = profile;
    }

    pub async fn resource_profile(&self) -> ResourceProfile {
        self.inner.lock().await.resource_profile
    }

    pub async fn status_view(&self) -> CrawlerStatusView {
        let report = super::installation::detect_installation();
        let profile = self.resource_profile().await;
        CrawlerStatusView {
            installation: report.state,
            running: self.is_running(),
            python_path: report.python_path.as_ref().map(|p| p.display().to_string()),
            reason: report.reason,
            resource_profile: profile.as_str().to_string(),
            sidecar_version: None,
            crawl4ai_version: None,
        }
    }

    /// Lazy-start sidecar after Ready; no-op if already running.
    pub async fn ensure_started(&self) -> Result<(), CrawlerError> {
        if self.is_running() {
            let guard = self.inner.lock().await;
            if guard.live.is_some() {
                return Ok(());
            }
        }
        assert_runtime_ready()?;
        self.spawn_child().await
    }

    async fn spawn_child(&self) -> Result<(), CrawlerError> {
        {
            let guard = self.inner.lock().await;
            if guard.shutting_down {
                return Err(CrawlerError::NotReady("supervisor is shutting down".into()));
            }
            if guard.live.is_some() {
                return Ok(());
            }
        }

        let python = resolve_sidecar_python();
        let service_root = resolve_service_root();
        let data_root = resolve_crawler_data_root();
        let crawl4_base = resolve_crawl4ai_base_directory();
        std::fs::create_dir_all(&data_root)?;
        std::fs::create_dir_all(&crawl4_base)?;

        tracing::info!(
            python = %python.display(),
            data_root = %data_root.display(),
            "starting Crawl4AI sidecar"
        );

        let mut command = Command::new(&python);
        let path_sep = if cfg!(windows) { ';' } else { ':' };
        let pythonpath = match std::env::var("PYTHONPATH") {
            Ok(existing) if !existing.is_empty() => {
                format!("{}{}{}", service_root.display(), path_sep, existing)
            }
            _ => service_root.display().to_string(),
        };
        command
            .arg("-m")
            .arg("coreside_crawler")
            .current_dir(&service_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("CRAWL4_AI_BASE_DIRECTORY", &crawl4_base)
            .env("CORESIDE_CRAWLER_DATA_ROOT", &data_root)
            .env("PYTHONPATH", &pythonpath)
            .kill_on_drop(true);

        let mut child = command
            .spawn()
            .map_err(|e| CrawlerError::Sidecar(format!("failed to spawn sidecar: {e}")))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| CrawlerError::Sidecar("missing stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| CrawlerError::Sidecar("missing stdout".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| CrawlerError::Sidecar("missing stderr".into()))?;

        {
            let mut guard = self.inner.lock().await;
            if guard.live.is_some() {
                let _ = child.kill().await;
                return Ok(());
            }
            guard.live = Some(LiveChild { child, stdin });
        }

        self.running.store(true, Ordering::SeqCst);
        self.spawn_stdout_reader(stdout);
        self.spawn_stderr_reader(stderr);
        self.spawn_exit_watcher();

        match timeout(
            HANDSHAKE_TIMEOUT,
            self.send_command_inner("health", json!({})),
        )
        .await
        {
            Ok(Ok(_)) => {
                self.inner.lock().await.restart_attempts = 0;
                Ok(())
            }
            Ok(Err(e)) => {
                self.force_kill().await;
                Err(e)
            }
            Err(_) => {
                self.force_kill().await;
                Err(CrawlerError::Timeout)
            }
        }
    }

    fn spawn_stdout_reader(&self, stdout: tokio::process::ChildStdout) {
        let inner = self.inner.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                match parse_event_line(&line) {
                    Ok(Some(env)) => {
                        if let Err(e) = validate_version(&env) {
                            tracing::warn!(error = %e, "sidecar protocol version mismatch");
                            continue;
                        }
                        if !is_terminal_event(&env.event_type) {
                            tracing::debug!(
                                request_id = ?env.request_id,
                                event = %env.event_type,
                                "crawler progress"
                            );
                            continue;
                        }
                        let Some(rid) = env.request_id.clone() else {
                            continue;
                        };
                        let result = terminal_payload_result(&env);
                        let mut guard = inner.lock().await;
                        if let Some(waiter) = guard.pending.remove(&rid) {
                            if matches_request_id(&env, &rid) {
                                let _ = waiter.tx.send(result);
                            }
                        }
                    }
                    Ok(None) => {
                        tracing::warn!("ignored malformed crawler protocol line");
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "crawler protocol parse error");
                    }
                }
            }
        });
    }

    fn spawn_stderr_reader(&self, stderr: tokio::process::ChildStderr) {
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if !line.trim().is_empty() {
                    tracing::warn!(target: "coreside_crawler", "{line}");
                }
            }
        });
    }

    fn spawn_exit_watcher(&self) {
        let inner = self.inner.clone();
        let running = self.running.clone();
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_millis(400)).await;
                let mut guard = inner.lock().await;
                if guard.shutting_down {
                    break;
                }
                let Some(live) = guard.live.as_mut() else {
                    break;
                };
                match live.child.try_wait() {
                    Ok(Some(status)) => {
                        tracing::error!(?status, "Crawl4AI sidecar exited unexpectedly");
                        guard.live = None;
                        fail_all_pending(
                            &mut guard,
                            CrawlerError::Sidecar("sidecar exited".into()),
                        );
                        running.store(false, Ordering::SeqCst);
                        break;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        tracing::warn!(error = %e, "sidecar try_wait failed");
                        break;
                    }
                }
            }
        });
    }

    async fn force_kill(&self) {
        let mut guard = self.inner.lock().await;
        if let Some(mut live) = guard.live.take() {
            let _ = live.child.kill().await;
            let _ = live.child.wait().await;
        }
        fail_all_pending(&mut guard, CrawlerError::Sidecar("sidecar killed".into()));
        self.running.store(false, Ordering::SeqCst);
    }

    pub async fn send_command(&self, command: &str, payload: Value) -> Result<Value, CrawlerError> {
        self.ensure_started().await?;
        if !self.is_running() {
            self.restart_with_backoff().await?;
        }
        match timeout(REQUEST_TIMEOUT, self.send_command_inner(command, payload)).await {
            Ok(r) => r,
            Err(_) => Err(CrawlerError::Timeout),
        }
    }

    async fn send_command_inner(
        &self,
        command: &str,
        payload: Value,
    ) -> Result<Value, CrawlerError> {
        let request_id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();

        {
            let mut guard = self.inner.lock().await;
            if guard.live.is_none() {
                return Err(CrawlerError::NotReady("sidecar is not running".into()));
            }
            guard
                .pending
                .insert(request_id.clone(), PendingWaiter { tx });

            let mut payload = payload;
            if let Some(obj) = payload.as_object_mut() {
                obj.entry("resourceProfile".to_string())
                    .or_insert_with(|| json!(guard.resource_profile.as_str()));
            }

            let req = ProtocolRequest::new(&request_id, command, payload);
            let line = encode_request(&req)?;
            let live = guard
                .live
                .as_mut()
                .ok_or_else(|| CrawlerError::NotReady("sidecar is not running".into()))?;
            live.stdin
                .write_all(format!("{line}\n").as_bytes())
                .await
                .map_err(|e| CrawlerError::Io(e.to_string()))?;
            live.stdin
                .flush()
                .await
                .map_err(|e| CrawlerError::Io(e.to_string()))?;
        }

        rx.await
            .map_err(|_| CrawlerError::Sidecar("request waiter dropped".into()))?
    }

    /// Cancel every in-flight sidecar request (Stop button / agent cancel).
    ///
    /// Uses [`super::cancellation::cancel_request`] per id so the cancel
    /// protocol path stays shared with any future per-request UI cancel.
    pub async fn cancel_active(&self) -> usize {
        let ids: Vec<String> = {
            let guard = self.inner.lock().await;
            guard.pending.keys().cloned().collect()
        };
        if ids.is_empty() {
            return 0;
        }

        let mut cancelled = 0usize;
        for id in &ids {
            match super::cancellation::cancel_request(self, id).await {
                Ok(true) => cancelled += 1,
                Ok(false) => {
                    tracing::debug!(request_id = %id, "sidecar reported cancel=false");
                }
                Err(e) => tracing::debug!(error = %e, request_id = %id, "sidecar cancel failed"),
            }
        }

        // Unblock any waiters still hanging after the cancel command.
        {
            let mut guard = self.inner.lock().await;
            let still: Vec<String> = guard.pending.keys().cloned().collect();
            for id in still {
                if let Some(waiter) = guard.pending.remove(&id) {
                    let _ = waiter.tx.send(Err(CrawlerError::Cancelled));
                }
            }
        }
        cancelled
    }

    pub async fn shutdown(&self) -> Result<(), CrawlerError> {
        {
            let mut guard = self.inner.lock().await;
            guard.shutting_down = true;
        }

        if self.is_running() {
            let shutdown_result = timeout(
                SHUTDOWN_TIMEOUT,
                self.send_command_inner("shutdown", json!({})),
            )
            .await;
            match shutdown_result {
                Ok(Ok(_)) => {}
                Ok(Err(e)) => tracing::warn!(error = %e, "sidecar shutdown command failed"),
                Err(_) => tracing::warn!("sidecar shutdown timed out"),
            }
        }

        self.force_kill().await;
        Ok(())
    }

    /// After unexpected exit, attempt bounded restart with exponential backoff.
    pub async fn restart_with_backoff(&self) -> Result<(), CrawlerError> {
        let attempt = {
            let mut guard = self.inner.lock().await;
            if guard.shutting_down {
                return Err(CrawlerError::NotReady("supervisor is shutting down".into()));
            }
            if guard.restart_attempts >= MAX_RESTART_ATTEMPTS {
                return Err(CrawlerError::Sidecar(format!(
                    "sidecar restart limit reached ({MAX_RESTART_ATTEMPTS})"
                )));
            }
            guard.restart_attempts += 1;
            guard.restart_attempts
        };
        let backoff = Duration::from_millis(250 * 2u64.pow(attempt.saturating_sub(1)));
        sleep(backoff).await;

        self.running.store(false, Ordering::SeqCst);
        {
            let mut guard = self.inner.lock().await;
            if let Some(mut live) = guard.live.take() {
                let _ = live.child.kill().await;
            }
        }
        self.spawn_child().await
    }
}

fn fail_all_pending(guard: &mut SupervisorInner, err: CrawlerError) {
    let pending = std::mem::take(&mut guard.pending);
    for (_, waiter) in pending {
        let _ = waiter.tx.send(Err(clone_error(&err)));
    }
}

fn clone_error(err: &CrawlerError) -> CrawlerError {
    match err {
        CrawlerError::NeedsSetup(m) => CrawlerError::NeedsSetup(m.clone()),
        CrawlerError::NotReady(m) => CrawlerError::NotReady(m.clone()),
        CrawlerError::Invalid(m) => CrawlerError::Invalid(m.clone()),
        CrawlerError::Protocol(m) => CrawlerError::Protocol(m.clone()),
        CrawlerError::Sidecar(m) => CrawlerError::Sidecar(m.clone()),
        CrawlerError::Timeout => CrawlerError::Timeout,
        CrawlerError::Cancelled => CrawlerError::Cancelled,
        CrawlerError::Io(m) => CrawlerError::Io(m.clone()),
    }
}

/// Map crawler errors into search-layer errors for providers.
pub fn to_search_error(err: CrawlerError) -> crate::search::SearchError {
    use crate::search::SearchError;
    match err {
        CrawlerError::NeedsSetup(m) | CrawlerError::NotReady(m) => SearchError::NotConfigured(m),
        CrawlerError::Invalid(m) => SearchError::Invalid(m),
        CrawlerError::Timeout => SearchError::Timeout,
        CrawlerError::Cancelled => SearchError::Invalid("cancelled".into()),
        other => SearchError::Provider(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crawler::InstallationState;

    #[test]
    fn status_defaults_to_not_running() {
        let s = CrawlerSupervisor::new();
        assert!(!s.is_running());
    }

    #[tokio::test]
    async fn status_view_reports_installation() {
        let s = CrawlerSupervisor::new();
        let view = s.status_view().await;
        assert!(matches!(
            view.installation,
            InstallationState::Ready | InstallationState::NeedsSetup | InstallationState::Error
        ));
        assert_eq!(view.resource_profile, "balanced");
    }
}
