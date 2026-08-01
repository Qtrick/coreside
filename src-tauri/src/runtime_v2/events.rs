//! Typed event bus with loop protection and idempotency.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use super::limits::{
    EVENT_COOLDOWN_MS, EVENT_SUSPENSION_SECS, MAX_EVENTS_PER_SURFACE_PER_MINUTE, MAX_EVENT_DEPTH,
    MAX_IDEMPOTENCY_KEYS, MAX_IDENTICAL_EVENTS_PER_INTERVAL, MAX_SUBSCRIPTIONS_PER_SURFACE,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
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

#[derive(Debug, Default)]
struct SurfaceRateState {
    timestamps: VecDeque<Instant>,
    identical: HashMap<String, VecDeque<Instant>>,
    last_dispatch: Option<Instant>,
    loop_strikes: u32,
    suspended_until: Option<Instant>,
}

impl SurfaceRateState {
    fn suspend(&mut self, now: Instant) {
        self.suspended_until = Some(now + Duration::from_secs(EVENT_SUSPENSION_SECS));
    }

    /// Clears an expired suspension and reports whether the surface is still held.
    fn is_suspended(&mut self, now: Instant) -> bool {
        match self.suspended_until {
            Some(until) if now < until => true,
            Some(_) => {
                self.suspended_until = None;
                self.loop_strikes = 0;
                self.identical.clear();
                false
            }
            None => false,
        }
    }
}

#[derive(Debug, Default)]
pub struct EventBus {
    subscriptions: HashMap<String, Subscription>,
    by_owner: HashMap<String, Vec<String>>,
    seen_idempotency: HashSet<String>,
    idempotency_order: VecDeque<String>,
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
            let event_types: Vec<String> = serde_json::from_str(&types_s).unwrap_or_default();
            let source_filter: EventRef = serde_json::from_str(&source_s).unwrap_or_default();
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

    /// Clear a loop suspension early, e.g. after the user repairs the surface.
    pub fn unsuspend(&mut self, surface_id: &str) {
        if let Some(st) = self.rates.get_mut(surface_id) {
            st.suspended_until = None;
            st.loop_strikes = 0;
            st.identical.clear();
        }
    }

    fn remember_idempotency(&mut self, key: &str) -> bool {
        if !self.seen_idempotency.insert(key.to_string()) {
            return false;
        }
        self.idempotency_order.push_back(key.to_string());
        while self.idempotency_order.len() > MAX_IDEMPOTENCY_KEYS {
            if let Some(oldest) = self.idempotency_order.pop_front() {
                self.seen_idempotency.remove(&oldest);
            }
        }
        true
    }

    /// Release a key claimed by a dispatch that then failed, so the caller can
    /// retry the same event once the rate limit or suspension clears.
    fn forget_idempotency(&mut self, key: &str) {
        if self.seen_idempotency.remove(key) {
            if let Some(pos) = self.idempotency_order.iter().rposition(|k| k == key) {
                self.idempotency_order.remove(pos);
            }
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
            if !self.remember_idempotency(key) {
                return Err(EventBusError::Duplicate);
            }
        }
        let result = self.dispatch_checked(event);
        // A claimed key must not outlive a dispatch that never delivered the
        // event, or the retry after a rate limit would look like a duplicate.
        if result.is_err() {
            if let Some(key) = &event.idempotency_key {
                self.forget_idempotency(key);
            }
        }
        result
    }

    fn dispatch_checked(&mut self, event: &SurfaceEvent) -> Result<Vec<String>, EventBusError> {
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
        let now = Instant::now();
        let st = self.rates.entry(surface_key.clone()).or_default();
        if st.is_suspended(now) {
            return Err(EventBusError::Suspended {
                surface_id: surface_key,
            });
        }
        if let Some(last) = st.last_dispatch {
            if now.duration_since(last) < Duration::from_millis(EVENT_COOLDOWN_MS) {
                st.loop_strikes += 1;
                if st.loop_strikes >= 5 {
                    st.suspend(now);
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
                st.suspend(now);
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
            if !sub.event_types.is_empty()
                && !sub.event_types.iter().any(|t| t == &event.event_type)
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

    #[test]
    fn a_failed_dispatch_releases_its_idempotency_key() {
        let mut bus = EventBus::new();
        let ev = SurfaceEvent {
            id: "e1".into(),
            event_type: "form_submitted".into(),
            scope: "surface".into(),
            source: EventRef {
                surface_id: Some("s1".into()),
                project_id: Some("p1".into()),
                ..Default::default()
            },
            target: EventRef {
                project_id: Some("p2".into()),
                ..Default::default()
            },
            payload: json!({ "a": 1 }),
            idempotency_key: Some("k1".into()),
        };
        assert_eq!(
            bus.dispatch(&ev, 0).unwrap_err(),
            EventBusError::CrossProjectDenied
        );
        assert!(
            bus.seen_idempotency.is_empty(),
            "a rejected event must not burn its key"
        );

        // The same key now succeeds once the event is addressed correctly.
        let mut fixed = ev.clone();
        fixed.target.project_id = Some("p1".into());
        assert!(bus.dispatch(&fixed, 0).is_ok());
        assert_eq!(
            bus.dispatch(&fixed, 0).unwrap_err(),
            EventBusError::Duplicate
        );
    }

    #[test]
    fn loop_suspension_expires_so_the_surface_can_recover() {
        let mut st = SurfaceRateState::default();
        let now = Instant::now();
        st.loop_strikes = 5;
        st.suspend(now);
        assert!(st.is_suspended(now), "suspension holds while it is fresh");

        let after = now + Duration::from_secs(EVENT_SUSPENSION_SECS + 1);
        assert!(
            !st.is_suspended(after),
            "suspension must release when quiet"
        );
        assert_eq!(st.loop_strikes, 0, "strikes reset on recovery");
    }

    #[test]
    fn explicit_unsuspend_clears_a_live_suspension() {
        let mut bus = EventBus::new();
        let now = Instant::now();
        bus.rates.entry("s1".to_string()).or_default().suspend(now);
        assert!(bus.rates.get_mut("s1").unwrap().is_suspended(now));

        bus.unsuspend("s1");
        assert!(!bus.rates.get_mut("s1").unwrap().is_suspended(now));
    }

    #[test]
    fn idempotency_memory_stays_bounded() {
        let mut bus = EventBus::new();
        for i in 0..(MAX_IDEMPOTENCY_KEYS + 500) {
            assert!(bus.remember_idempotency(&format!("k{i}")));
        }
        assert_eq!(bus.seen_idempotency.len(), MAX_IDEMPOTENCY_KEYS);
        assert_eq!(bus.idempotency_order.len(), MAX_IDEMPOTENCY_KEYS);
        // The oldest keys were evicted, so they no longer register as duplicates.
        assert!(bus.remember_idempotency("k0"));
        // Recent keys are still deduplicated.
        assert!(!bus.remember_idempotency(&format!("k{}", MAX_IDEMPOTENCY_KEYS + 499)));
    }
}
