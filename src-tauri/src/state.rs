//! Shared application state.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use parking_lot::Mutex;
use serde::Serialize;
use tauri::ipc::Channel;
use tokio_util::sync::CancellationToken;

use crate::config::{self, AppConfig};
use crate::crawler::CrawlerSupervisor;
use crate::db::{BootstrapStatus, Database};
use crate::maintenance::{MaintenanceMode, MaintenanceStage, MaintenanceStatus};
use crate::quiescence::{PauseToken, QuiescedSubsystem, QuiescenceCoordinator};
use crate::runtime_v2::EventBus;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum QueueChangeKind {
    ItemAdded,
    ItemActivated,
    ItemCancelled,
    ItemCompleted,
    QueueSnapshot,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueChangedEvent {
    pub kind: QueueChangeKind,
    pub conversation_id: String,
    pub item_id: Option<String>,
}

/// Conversation-scoped Sync / Conflict payloads (same wire shape as AgentTurnEvent).
/// Lives in state so AppState can own the subscriber map without depending on commands.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SyncScopedEvent {
    #[serde(rename_all = "camelCase")]
    Sync {
        conversation_id: Option<String>,
        surface_ids: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tool_ids: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        application_id: Option<String>,
        revision: Option<i64>,
        #[serde(rename = "syncKind")]
        sync_kind: String,
    },
    #[serde(rename_all = "camelCase")]
    Conflict {
        conversation_id: Option<String>,
        message: String,
        conflicts: Vec<String>,
    },
}

pub struct AppState {
    pub config: Mutex<AppConfig>,
    pub db: Arc<Mutex<Database>>,
    /// Profile readiness. When not Ready, `db` holds a recovery shell database.
    pub bootstrap: Mutex<BootstrapStatus>,
    pub active_requests: Mutex<HashMap<String, CancellationToken>>,
    /// Conversations with an in-flight queue drain task (prevents concurrent drainers).
    pub queue_drain_inflight: Mutex<HashSet<String>>,
    /// Conversation-scoped queue UI Channels (no process-wide queue bus).
    pub queue_subscribers: Mutex<HashMap<String, Vec<Channel<QueueChangedEvent>>>>,
    /// Conversation-scoped Sync/Conflict Channels keyed by window label.
    /// Remounts replace the same window's Channel (no zombie absorbers); distinct
    /// windows (main + tool-*) still fan out together.
    pub sync_subscribers: Mutex<HashMap<String, HashMap<String, Channel<SyncScopedEvent>>>>,
    pub crawler: Arc<CrawlerSupervisor>,
    pub event_bus: Mutex<EventBus>,
    pub maintenance: Mutex<MaintenanceMode>,
    /// Pause/stop gate for profile-dependent subsystems during maintenance/restore.
    pub quiescence: QuiescenceCoordinator,
}

impl AppState {
    pub fn new(config: AppConfig, db: Database) -> Self {
        Self::new_with_bootstrap(config, db, BootstrapStatus::Ready)
    }

    pub fn new_with_bootstrap(config: AppConfig, db: Database, bootstrap: BootstrapStatus) -> Self {
        let mut event_bus = EventBus::load_from_db(&db);
        // Drain post-commit effects left pending after a crash between COMMIT and flush.
        let _ = crate::runtime_v2::outbox::flush_pending_outbox(&db, Some(&mut event_bus));
        Self {
            config: Mutex::new(config),
            db: Arc::new(Mutex::new(db)),
            bootstrap: Mutex::new(bootstrap),
            active_requests: Mutex::new(HashMap::new()),
            queue_drain_inflight: Mutex::new(HashSet::new()),
            queue_subscribers: Mutex::new(HashMap::new()),
            sync_subscribers: Mutex::new(HashMap::new()),
            crawler: Arc::new(CrawlerSupervisor::new()),
            event_bus: Mutex::new(event_bus),
            maintenance: Mutex::new(MaintenanceMode::default()),
            quiescence: QuiescenceCoordinator::default(),
        }
    }

    /// Register a conversation-scoped Channel for queue mutation events.
    /// Replaces any prior Channels for this conversation so UI remounts cannot
    /// accumulate dead subscribers (ponytail: one live Channel per conversation).
    pub fn subscribe_queue(&self, conversation_id: String, channel: Channel<QueueChangedEvent>) {
        self.queue_subscribers
            .lock()
            .insert(conversation_id, vec![channel]);
    }

    /// Deliver a queue event only to Channels registered for that conversation.
    /// Dead Channels (failed send) are dropped from the registry.
    pub fn emit_queue_changed_to_subscribers(&self, event: &QueueChangedEvent) {
        let mut registry = self.queue_subscribers.lock();
        let Some(subs) = registry.get_mut(&event.conversation_id) else {
            return;
        };
        subs.retain(|ch| ch.send(event.clone()).is_ok());
        if subs.is_empty() {
            registry.remove(&event.conversation_id);
        }
    }

    /// Register a Sync/Conflict Channel for one conversation + window.
    /// Replaces any prior Channel for the same window label so remounts cannot
    /// leave zombies that absorb Sync (scoped_ok) while dropping JS handlers.
    pub fn subscribe_sync(
        &self,
        conversation_id: String,
        window_label: impl Into<String>,
        channel: Channel<SyncScopedEvent>,
    ) {
        let label = {
            let raw = window_label.into();
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                "main".to_string()
            } else {
                trimmed.to_string()
            }
        };
        self.sync_subscribers
            .lock()
            .entry(conversation_id)
            .or_default()
            .insert(label, channel);
    }

    /// Fan-out Sync/Conflict to Channels for `conversation_id`. Returns successful send count.
    /// Dead Channels are dropped. Does not use the process-wide event bus.
    pub fn emit_sync_to_subscribers(
        &self,
        conversation_id: &str,
        event: &SyncScopedEvent,
    ) -> usize {
        let mut registry = self.sync_subscribers.lock();
        let Some(subs) = registry.get_mut(conversation_id) else {
            return 0;
        };
        let mut ok = 0usize;
        subs.retain(|_label, ch| {
            if ch.send(event.clone()).is_ok() {
                ok += 1;
                true
            } else {
                false
            }
        });
        if subs.is_empty() {
            registry.remove(conversation_id);
        }
        ok
    }

    pub fn sync_subscriber_count(&self, conversation_id: &str) -> usize {
        self.sync_subscribers
            .lock()
            .get(conversation_id)
            .map(|v| v.len())
            .unwrap_or(0)
    }

    /// Claim exclusive queue-drain ownership for a conversation. Returns false if
    /// another drain task already holds it, or if quiescence has paused drains.
    pub fn try_begin_queue_drain(&self, conversation_id: &str) -> bool {
        if !self.quiescence.allows(QuiescedSubsystem::QueueDrain) {
            return false;
        }
        self.queue_drain_inflight
            .lock()
            .insert(conversation_id.to_string())
    }

    pub fn end_queue_drain(&self, conversation_id: &str) {
        self.queue_drain_inflight.lock().remove(conversation_id);
    }

    pub fn bootstrap_status(&self) -> BootstrapStatus {
        self.bootstrap.lock().clone()
    }

    pub fn profile_ready(&self) -> bool {
        self.bootstrap.lock().is_ready()
    }

    /// Require a healthy profile database (not the recovery shell) and no maintenance.
    pub fn require_profile(&self) -> Result<(), crate::commands::CommandError> {
        self.maintenance.lock().require_inactive()?;
        if self.profile_ready() {
            Ok(())
        } else {
            Err(crate::commands::CommandError::new(
                "database_unavailable",
                "Coreside needs Recovery before this action can run.",
            ))
        }
    }

    pub fn require_not_maintenance(&self) -> Result<(), crate::commands::CommandError> {
        self.maintenance.lock().require_inactive()
    }

    pub fn maintenance_status(&self) -> MaintenanceStatus {
        self.maintenance.lock().status()
    }

    pub fn begin_maintenance(
        &self,
        operation_type: &str,
    ) -> Result<String, crate::commands::CommandError> {
        let id = self.maintenance.lock().begin(operation_type)?;
        let _pause_token: PauseToken = self.quiescence.pause();
        if let Ok(paths) = crate::app_paths::AppPaths::resolve() {
            let mut journal = crate::maintenance_journal::MaintenanceJournal::new(operation_type);
            journal.operation_id = id.clone();
            journal.profile_generation = Some(self.quiescence.generation());
            if let Err(err) = crate::maintenance_journal::persist_journal(&paths, &journal) {
                let _ = self.maintenance.lock().clear(&id);
                self.quiescence.force_resume();
                return Err(err);
            }
        }
        Ok(id)
    }

    pub fn set_maintenance_stage(
        &self,
        operation_id: &str,
        stage: MaintenanceStage,
    ) -> Result<(), crate::commands::CommandError> {
        self.maintenance.lock().set_stage(operation_id, stage)?;
        if let Ok(paths) = crate::app_paths::AppPaths::resolve() {
            if let Ok(Some(mut journal)) = crate::maintenance_journal::load_journal(&paths) {
                if journal.operation_id == operation_id {
                    journal.touch_stage(stage);
                    journal.profile_generation = Some(self.quiescence.generation());
                    let _ = crate::maintenance_journal::persist_journal(&paths, &journal);
                }
            }
        }
        Ok(())
    }

    pub fn clear_maintenance(
        &self,
        operation_id: &str,
    ) -> Result<(), crate::commands::CommandError> {
        self.maintenance.lock().clear(operation_id)?;
        // Force resume: generation may have bumped during restore; token binding
        // only protects against stale async resumes, not the owning clear path.
        self.quiescence.force_resume();
        if let Ok(paths) = crate::app_paths::AppPaths::resolve() {
            if let Ok(Some(journal)) = crate::maintenance_journal::load_journal(&paths) {
                if journal.operation_id == operation_id {
                    let _ = crate::maintenance_journal::clear_journal(&paths);
                }
            }
        }
        Ok(())
    }

    /// Replace the active database after a successful retry or restore.
    pub fn replace_profile_database(&self, db: Database) {
        let mut event_bus = EventBus::load_from_db(&db);
        let _ = crate::runtime_v2::outbox::flush_pending_outbox(&db, Some(&mut event_bus));
        *self.event_bus.lock() = event_bus;
        *self.db.lock() = db;
        *self.bootstrap.lock() = BootstrapStatus::Ready;
        // Invalidate pause tokens issued against the previous profile generation.
        let _ = self.quiescence.bump_generation();
    }

    /// Re-read `.env` from disk so Refresh / Test pick up keys after save.
    pub fn reload_config(&self) -> AppConfig {
        let next = config::load_config();
        *self.config.lock() = next.clone();
        next
    }

    pub fn snapshot_config(&self) -> AppConfig {
        self.config.lock().clone()
    }

    pub fn register_request(&self, key: &str, token: CancellationToken) {
        self.active_requests.lock().insert(key.to_string(), token);
    }

    pub fn take_request(&self, key: &str) -> Option<CancellationToken> {
        self.active_requests.lock().remove(key)
    }

    pub fn cancel_request(&self, key: &str) -> bool {
        if let Some(token) = self.active_requests.lock().get(key) {
            token.cancel();
            true
        } else {
            false
        }
    }

    pub fn cancel_all(&self) {
        let map = self.active_requests.lock();
        for token in map.values() {
            token.cancel();
        }
    }
}

#[cfg(test)]
impl AppState {
    pub fn new_for_test(db: Database) -> Self {
        Self::new(config::load_config(), db)
    }
}
