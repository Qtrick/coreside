//! Shared application state.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;

use crate::config::{self, AppConfig};
use crate::crawler::CrawlerSupervisor;
use crate::db::Database;

pub struct AppState {
    pub config: Mutex<AppConfig>,
    pub db: Arc<Mutex<Database>>,
    pub active_requests: Mutex<HashMap<String, CancellationToken>>,
    pub crawler: Arc<CrawlerSupervisor>,
}

impl AppState {
    pub fn new(config: AppConfig, db: Database) -> Self {
        Self {
            config: Mutex::new(config),
            db: Arc::new(Mutex::new(db)),
            active_requests: Mutex::new(HashMap::new()),
            crawler: Arc::new(CrawlerSupervisor::new()),
        }
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
