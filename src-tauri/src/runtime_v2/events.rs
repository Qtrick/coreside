//! Typed event bus with loop protection and idempotency.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use super::limits::{
    EVENT_COOLDOWN_MS, MAX_EVENT_DEPTH, MAX_EVENTS_PER_SURFACE_PER_MINUTE,
    MAX_IDENTICAL_EVENTS_PER_INTERVAL, MAX_SUBSCRIPTIONS_PER_SURFACE,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EventRef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceEvent {
    pub id: String,
    pub event_type: String,
    pub scope: String,
    pub source: EventRef,
    pub target: EventRef,
    #[serde(default)]
    pub payload: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    pub id: String,
    pub owner_surface_id: String,
    pub event_types: Vec<String>,
    pub source_filter: EventRef,
    pub target: EventRef,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventBusError {
    LoopDetected,
    RateLimited,
    Duplicate,
    DepthExceeded,
    SubscriptionLimit,
    Suspended { surface_id: String },
    CrossProjectDenied,
}

impl std::fmt::Display for EventBusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoopDetected => write!(f, "event loop detected"),
            Self::RateLimited => write!(f, "event rate limited"),
            Self::Duplicate => write!(f, "duplicate event"),
            Self::DepthExceeded => write!(f, "event depth exceeded"),
            Self::SubscriptionLimit => write!(f, "subscription limit exceeded"),
            Self::Suspended { surface_id } => write!(f, "surface suspended: {surface_id}"),
            Self::CrossProjectDenied => write!(f, "cross-project event denied"),
        }
    }
}

#[derive(Debug)]
struct SurfaceRateState {
    timestamps: VecDeque<Instant>,
    identical: HashMap<String, VecDeque<Instant>>,
    last_dispatch: Option<Instant>,
    loop_strikes: u32,
    suspended: bool,
}

impl Default for SurfaceRateState {
    fn default() -> Self {
        Self {
            timestamps: VecDeque::new(),
            identical: HashMap::new(),
            last_dispatch: None,
            loop_strikes: 0,
            suspended: false,
        }
    }
}

#[derive(Debug, Default)]
pub struct EventBus {
    subscriptions: HashMap<String, Subscription>,
    by_owner: HashMap<String, Vec<String>>,
    seen_idempotency: HashSet<String>,
    rates: HashMap<String, SurfaceRateState>,
}

impl EventBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_subscription(&mut self, sub: Subscription) -> Result<(), EventBusError> {
        let owner = sub.owner_surface_id.clone();
        let list = self.by_owner.entry(owner.clone()).or_default();
        if list.len() >= MAX_SUBSCRIPTIONS_PER_SURFACE {
            return Err(EventBusError::SubscriptionLimit);
        }
        list.push(sub.id.clone());
        self.subscriptions.insert(sub.id.clone(), sub);
        Ok(())
    }

    pub fn remove_subscription(&mut self, id: &str) {
        if let Some(sub) = self.subscriptions.remove(id) {
            if let Some(list) = self.by_owner.get_mut(&sub.owner_surface_id) {
                list.retain(|x| x != id);
            }
        }
    }

    pub fn set_subscription_enabled(&mut self, id: &str, enabled: bool) {
        if let Some(sub) = self.subscriptions.get_mut(id) {
            sub.enabled = enabled;
        }
    }

    pub fn remove_subscriptions_for_surface(&mut self, surface_id: &str) {
        if let Some(ids) = self.by_owner.remove(surface_id) {
            for id in ids {
                self.subscriptions.remove(&id);
            }
        }
        self.rates.remove(surface_id);
    }

    /// Hydrate in-memory subscriptions from SQLite after restart.
    pub fn load_from_db(db: &crate::db::Database) -> Self {
        let mut bus = Self::new();
        let Ok(mut stmt) = db.conn().prepare(
            "SELECT id, owner_surface_id, event_types_json, source_filter_json, target_json, enabled
             FROM surface_subscriptions",
        ) else {
            return bus;
        };
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
            ))
        });
        let Ok(rows) = rows else {
            return bus;
        };
        for row in rows.flatten() {
            let (id, owner, types_s, source_s, target_s, enabled) = row;
            let event_types: Vec<String> =
                serde_json::from_str(&types_s).unwrap_or_default();
            let source_filter: EventRef =
                serde_json::from_str(&source_s).unwrap_or_default();
            let target: EventRef = serde_json::from_str(&target_s).unwrap_or_default();
            let _ = bus.add_subscription(Subscription {
                id,
                owner_surface_id: owner,
                event_types,
                source_filter,
                target,
                enabled: enabled != 0,
            });
        }
        bus
    }

    pub fn unsuspend(&mut self, surface_id: &str) {
        if let Some(st) = self.rates.get_mut(surface_id) {
            st.suspended = false;
            st.loop_strikes = 0;
        }
    }

    pub fn dispatch(
        &mut self,
        event: &SurfaceEvent,
        depth: usize,
    ) -> Result<Vec<String>, EventBusError> {
        if depth > MAX_EVENT_DEPTH {
            return Err(EventBusError::DepthExceeded);
        }
        if let Some(key) = &event.idempotency_key {
            if !self.seen_idempotency.insert(key.clone()) {
                return Err(EventBusError::Duplicate);
            }
        }
        // Cross-project guard
        if let (Some(sp), Some(tp)) = (
            event.source.project_id.as_ref(),
            event.target.project_id.as_ref(),
        ) {
            if sp != tp {
                return Err(EventBusError::CrossProjectDenied);
            }
        }

        let surface_key = event
            .source
            .surface_id
            .clone()
            .unwrap_or_else(|| "global".into());
        let st = self.rates.entry(surface_key.clone()).or_default();
        if st.suspended {
            return Err(EventBusError::Suspended {
                surface_id: surface_key,
            });
        }
        let now = Instant::now();
        if let Some(last) = st.last_dispatch {
            if now.duration_since(last) < Duration::from_millis(EVENT_COOLDOWN_MS) {
                st.loop_strikes += 1;
                if st.loop_strikes >= 5 {
                    st.suspended = true;
                    return Err(EventBusError::LoopDetected);
                }
                return Err(EventBusError::RateLimited);
            }
        }
        st.last_dispatch = Some(now);
        while st
            .timestamps
            .front()
            .is_some_and(|t| now.duration_since(*t) > Duration::from_secs(60))
        {
            st.timestamps.pop_front();
        }
        if st.timestamps.len() >= MAX_EVENTS_PER_SURFACE_PER_MINUTE {
            return Err(EventBusError::RateLimited);
        }
        st.timestamps.push_back(now);

        let fingerprint = format!("{}:{}", event.event_type, event.payload);
        let bucket = st.identical.entry(fingerprint).or_default();
        while bucket
            .front()
            .is_some_and(|t| now.duration_since(*t) > Duration::from_secs(10))
        {
            bucket.pop_front();
        }
        if bucket.len() >= MAX_IDENTICAL_EVENTS_PER_INTERVAL {
            st.loop_strikes += 1;
            if st.loop_strikes >= 3 {
                st.suspended = true;
                return Err(EventBusError::LoopDetected);
            }
            return Err(EventBusError::RateLimited);
        }
        bucket.push_back(now);

        let mut matched = Vec::new();
        for sub in self.subscriptions.values() {
            if !sub.enabled {
                continue;
            }
            if !sub.event_types.is_empty() && !sub.event_types.iter().any(|t| t == &event.event_type)
            {
                continue;
            }
            matched.push(sub.id.clone());
        }
        Ok(matched)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detects_duplicate_idempotency() {
        let mut bus = EventBus::new();
        let ev = SurfaceEvent {
            id: "e1".into(),
            event_type: "form_submitted".into(),
            scope: "surface".into(),
            source: EventRef {
                surface_id: Some("s1".into()),
                ..Default::default()
            },
            target: EventRef::default(),
            payload: json!({"a":1}),
            idempotency_key: Some("k1".into()),
        };
        assert!(bus.dispatch(&ev, 0).is_ok());
        assert_eq!(bus.dispatch(&ev, 0).unwrap_err(), EventBusError::Duplicate);
    }
}

impl Default for EventRef {
    fn default() -> Self {
        Self {
            surface_id: None,
            tool_id: None,
            conversation_id: None,
            project_id: None,
            component_id: None,
        }
    }
}
