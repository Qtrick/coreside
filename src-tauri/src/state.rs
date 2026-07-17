//! Shared application state.

use std::collections::HashMap;

use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;

use crate::config::AppConfig;
use crate::db::Database;

pub struct AppState {
    pub config: AppConfig,
    pub db: Mutex<Database>,
    pub active_requests: Mutex<HashMap<String, CancellationToken>>,
}

impl AppState {
    pub fn new(config: AppConfig, db: Database) -> Self {
        Self {
            config,
            db: Mutex::new(db),
            active_requests: Mutex::new(HashMap::new()),
        }
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
