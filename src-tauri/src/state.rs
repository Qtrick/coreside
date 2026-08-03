//! Shared application state.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;

use crate::config::{self, AppConfig};
use crate::crawler::CrawlerSupervisor;
use crate::db::{BootstrapStatus, Database};
use crate::maintenance::{MaintenanceMode, MaintenanceStage, MaintenanceStatus};
use crate::runtime_v2::EventBus;

pub struct AppState {
    pub config: Mutex<AppConfig>,
    pub db: Arc<Mutex<Database>>,
    /// Profile readiness. When not Ready, `db` holds a recovery shell database.
    pub bootstrap: Mutex<BootstrapStatus>,
    pub active_requests: Mutex<HashMap<String, CancellationToken>>,
    /// Conversations with an in-flight queue drain task (prevents concurrent drainers).
    pub queue_drain_inflight: Mutex<HashSet<String>>,
    pub crawler: Arc<CrawlerSupervisor>,
    pub event_bus: Mutex<EventBus>,
    pub maintenance: Mutex<MaintenanceMode>,
}

impl AppState {
    pub fn new(config: AppConfig, db: Database) -> Self {
        Self::new_with_bootstrap(config, db, BootstrapStatus::Ready)
    }

    pub fn new_with_bootstrap(
        config: AppConfig,
        db: Database,
        bootstrap: BootstrapStatus,
    ) -> Self {
        let event_bus = EventBus::load_from_db(&db);
        Self {
            config: Mutex::new(config),
            db: Arc::new(Mutex::new(db)),
            bootstrap: Mutex::new(bootstrap),
            active_requests: Mutex::new(HashMap::new()),
            queue_drain_inflight: Mutex::new(HashSet::new()),
            crawler: Arc::new(CrawlerSupervisor::new()),
            event_bus: Mutex::new(event_bus),
            maintenance: Mutex::new(MaintenanceMode::default()),
        }
    }

    /// Claim exclusive queue-drain ownership for a conversation. Returns false if
    /// another drain task already holds it.
    pub fn try_begin_queue_drain(&self, conversation_id: &str) -> bool {
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
        self.maintenance.lock().begin(operation_type)
    }

    pub fn set_maintenance_stage(
        &self,
        operation_id: &str,
        stage: MaintenanceStage,
    ) -> Result<(), crate::commands::CommandError> {
        self.maintenance.lock().set_stage(operation_id, stage)
    }

    pub fn clear_maintenance(
        &self,
        operation_id: &str,
    ) -> Result<(), crate::commands::CommandError> {
        self.maintenance.lock().clear(operation_id)
    }

    /// Replace the active database after a successful retry or restore.
    pub fn replace_profile_database(&self, db: Database) {
        *self.event_bus.lock() = EventBus::load_from_db(&db);
        *self.db.lock() = db;
        *self.bootstrap.lock() = BootstrapStatus::Ready;
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
