use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};
use uuid::Uuid;

use super::limits::{
    EVENT_COOLDOWN_MS, EVENT_SUSPENSION_SECS, MAX_EVENTS_PER_SURFACE_PER_MINUTE, MAX_EVENT_DEPTH,
    MAX_IDEMPOTENCY_KEYS, MAX_IDENTICAL_EVENTS_PER_INTERVAL, MAX_SUBSCRIPTIONS_PER_SURFACE,
};
use crate::db::{now_rfc3339, Database};
use rusqlite::{params, OptionalExtension};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_id: Option<String>,
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
#[serde(tag = "action", content = "params", rename_all = "camelCase")]
pub enum EventHandler {
    SetState {
        key: String,
        value: Value,
    },
    PatchState {
        key: String,
        patch: Value,
    },
    SetComponentProp {
        component_id: String,
        prop: String,
        value: Value,
    },
    InvokeRegisteredAction {
        action_name: String,
        #[serde(default)]
        parameters: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        application_id: Option<String>,
    },
    EmitClientNotification {
        title: String,
        message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        level: Option<String>,
    },
    SubmitEvent {
        event_type: String,
        #[serde(default)]
        payload: Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    pub id: String,
    pub owner_surface_id: String,
    pub event_types: Vec<String>,
    pub source_filter: EventRef,
    pub target: EventRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handler: Option<EventHandler>,
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
    CrossAppDenied,
    InvalidHandler { reason: String },
    ExecutionFailed { reason: String },
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
            Self::CrossAppDenied => write!(f, "cross-application event denied"),
            Self::InvalidHandler { reason } => write!(f, "invalid event handler: {reason}"),
            Self::ExecutionFailed { reason } => {
                write!(f, "event delivery execution failed: {reason}")
            }
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

    pub fn get_subscription(&self, id: &str) -> Option<&Subscription> {
        self.subscriptions.get(id)
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
    /// Fails closed: malformed subscription JSON is quarantined and rejected.
    pub fn load_from_db(db: &Database) -> Self {
        let mut bus = Self::new();
        let Ok(mut stmt) = db.conn().prepare(
            "SELECT id, owner_surface_id, event_types_json, source_filter_json, target_json, handler_json, enabled
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
                row.get::<_, Option<String>>(5)?,
                row.get::<_, i64>(6)?,
            ))
        });
        let Ok(rows) = rows else {
            return bus;
        };
        for row_result in rows {
            let row = match row_result {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("Database row read error in load_from_db: {e}");
                    continue;
                }
            };
            let (id, owner, types_s, source_s, target_s, handler_s, enabled) = row;
            let event_types: Vec<String> = match serde_json::from_str(&types_s) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("quarantining subscription {id}: corrupt event_types_json: {e}");
                    let _ = db.conn().execute(
                        "UPDATE surface_subscriptions SET enabled = 0 WHERE id = ?1",
                        [&id],
                    );
                    continue;
                }
            };
            let source_filter: EventRef = match serde_json::from_str(&source_s) {
                Ok(sf) => sf,
                Err(e) => {
                    eprintln!("quarantining subscription {id}: corrupt source_filter_json: {e}");
                    let _ = db.conn().execute(
                        "UPDATE surface_subscriptions SET enabled = 0 WHERE id = ?1",
                        [&id],
                    );
                    continue;
                }
            };
            let target: EventRef = match serde_json::from_str(&target_s) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("quarantining subscription {id}: corrupt target_json: {e}");
                    let _ = db.conn().execute(
                        "UPDATE surface_subscriptions SET enabled = 0 WHERE id = ?1",
                        [&id],
                    );
                    continue;
                }
            };
            let handler: Option<EventHandler> = match handler_s {
                Some(hs) if !hs.trim().is_empty() && hs != "null" => {
                    match serde_json::from_str(&hs) {
                        Ok(h) => Some(h),
                        Err(e) => {
                            eprintln!("quarantining subscription {id}: corrupt handler_json: {e}");
                            let _ = db.conn().execute(
                                "UPDATE surface_subscriptions SET enabled = 0 WHERE id = ?1",
                                [&id],
                            );
                            continue;
                        }
                    }
                }
                _ => None,
            };
            let _ = bus.add_subscription(Subscription {
                id,
                owner_surface_id: owner,
                event_types,
                source_filter,
                target,
                handler,
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

        // Cross-application guard
        if let (Some(sa), Some(ta)) = (
            event.source.application_id.as_ref(),
            event.target.application_id.as_ref(),
        ) {
            if sa != ta {
                return Err(EventBusError::CrossAppDenied);
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
            // Event type filtering
            if !sub.event_types.is_empty()
                && !sub.event_types.iter().any(|t| t == &event.event_type)
            {
                continue;
            }

            // Source filter matching (Section 15)
            let sf = &sub.source_filter;
            if sf.surface_id.is_some() && sf.surface_id != event.source.surface_id {
                continue;
            }
            if sf.tool_id.is_some() && sf.tool_id != event.source.tool_id {
                continue;
            }
            if sf.conversation_id.is_some() && sf.conversation_id != event.source.conversation_id {
                continue;
            }
            if sf.project_id.is_some() && sf.project_id != event.source.project_id {
                continue;
            }
            if sf.component_id.is_some() && sf.component_id != event.source.component_id {
                continue;
            }
            if sf.application_id.is_some() && sf.application_id != event.source.application_id {
                continue;
            }

            // Target matching (Section 15)
            // If the event targets a specific surface, only subscriptions owned by that surface match!
            if let Some(target_surface) = &event.target.surface_id {
                if &sub.owner_surface_id != target_surface {
                    continue;
                }
            }
            // If the event targets a specific component, and the subscription specifies a target component:
            if event.target.component_id.is_some()
                && sub.target.component_id.is_some()
                && event.target.component_id != sub.target.component_id
            {
                continue;
            }
            // If the subscription targets a specific surface:
            if sub.target.surface_id.is_some()
                && event.target.surface_id.is_some()
                && sub.target.surface_id != event.target.surface_id
            {
                continue;
            }
            // If the event targets a specific application:
            if event.target.application_id.is_some()
                && sub.target.application_id.is_some()
                && event.target.application_id != sub.target.application_id
            {
                continue;
            }

            matched.push(sub.id.clone());
        }
        Ok(matched)
    }
}

/// Resolves declarative `$event.payload.<field>` references into actual values from event payload.
fn resolve_template_value(template: &Value, event_payload: &Value) -> Value {
    match template {
        Value::String(s) => {
            if s == "$event.payload" {
                event_payload.clone()
            } else if let Some(path) = s.strip_prefix("$event.payload.") {
                let parts: Vec<&str> = path.split('.').collect();
                let mut current = event_payload;
                for part in parts {
                    if let Some(next) = current.get(part) {
                        current = next;
                    } else {
                        return Value::Null;
                    }
                }
                current.clone()
            } else {
                Value::String(s.clone())
            }
        }
        Value::Object(map) => {
            let mut resolved = serde_json::Map::new();
            for (k, v) in map {
                resolved.insert(k.clone(), resolve_template_value(v, event_payload));
            }
            Value::Object(resolved)
        }
        Value::Array(arr) => Value::Array(
            arr.iter()
                .map(|item| resolve_template_value(item, event_payload))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Helper to execute a single AppOperation within a standard transaction for an event handler.
fn apply_handler_op(
    db: &mut Database,
    event: &SurfaceEvent,
    summary: &str,
    op: super::operations::AppOperation,
) -> Result<Value, String> {
    let txn = super::transactions::create_transaction(
        db,
        event.source.conversation_id.as_deref(),
        event.source.project_id.as_deref(),
        None,
        summary,
        &[op],
        true,
    )
    .map_err(|e| e.to_string())?;

    let mut deferred = Vec::new();
    let res = super::transactions::apply_transaction_deferred(db, &txn.id, &mut deferred)
        .map_err(|e| e.to_string())?;
    if res.transaction.status != "applied" || !res.conflicts.is_empty() {
        return Err(format!(
            "handler transaction '{}' was not applied (status='{}', conflicts={:?})",
            txn.id, res.transaction.status, res.conflicts
        ));
    }
    Ok(serde_json::to_value(&res).unwrap_or(Value::Null))
}

/// Execute a durable event delivery: runs declarative handler as an AppOperation
/// inside a standard transaction, and records status in surface_event_deliveries.
pub fn execute_durable_event_delivery(
    db: &mut Database,
    bus: &mut EventBus,
    event: &SurfaceEvent,
    subscription_id: &str,
    depth: usize,
) -> Result<String, EventBusError> {
    if depth > MAX_EVENT_DEPTH {
        return Err(EventBusError::DepthExceeded);
    }
    let sub = bus
        .get_subscription(subscription_id)
        .cloned()
        .ok_or_else(|| EventBusError::InvalidHandler {
            reason: format!("subscription {subscription_id} not found in bus"),
        })?;

    // Idempotency: check if already delivered
    let already_delivered: bool = db
        .conn()
        .query_row(
            "SELECT 1 FROM surface_event_deliveries WHERE event_id = ?1 AND subscription_id = ?2 AND status = 'delivered'",
            params![event.id, subscription_id],
            |_| Ok(true),
        )
        .optional()
        .map_err(|e| EventBusError::ExecutionFailed {
            reason: format!("Failed to query delivery status for event '{}': {e}", event.id),
        })?
        .unwrap_or(false);
    if already_delivered {
        return Ok("already_delivered".to_string());
    }

    let delivery_id = format!("deliv-{}", Uuid::new_v4());
    let now = now_rfc3339();

    // Ensure surface_events record exists to satisfy foreign key constraint
    db.conn()
        .execute(
            "INSERT OR IGNORE INTO surface_events (
            id, source_json, target_json, scope, event_type, payload_json,
            idempotency_key, status, created_at, processed_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', ?8, NULL)",
            params![
                event.id,
                serde_json::to_string(&event.source).unwrap_or_else(|_| "{}".into()),
                serde_json::to_string(&event.target).unwrap_or_else(|_| "{}".into()),
                event.scope,
                event.event_type,
                event.payload.to_string(),
                event.idempotency_key,
                now
            ],
        )
        .map_err(|e| EventBusError::ExecutionFailed {
            reason: format!(
                "Database error inserting surface_events '{}': {e}",
                event.id
            ),
        })?;

    // Ensure surface_subscriptions record exists to satisfy foreign key constraint
    db.conn().execute(
        "INSERT OR IGNORE INTO surface_subscriptions (
            id, owner_surface_id, source_filter_json, target_json, event_types_json, handler_json, enabled, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?7)",
        params![
            sub.id,
            sub.owner_surface_id,
            serde_json::to_string(&sub.source_filter).unwrap_or_else(|_| "{}".into()),
            serde_json::to_string(&sub.target).unwrap_or_else(|_| "{}".into()),
            serde_json::to_string(&sub.event_types).unwrap_or_else(|_| "[]".into()),
            serde_json::to_string(&sub.handler).unwrap_or_else(|_| "{}".into()),
            now
        ],
    ).map_err(|e| EventBusError::ExecutionFailed {
        reason: format!("Database error inserting surface_subscriptions '{}': {e}", sub.id),
    })?;

    // Insert pending delivery record matching migration 031 schema
    db.conn()
        .execute(
            "INSERT INTO surface_event_deliveries (
            id, event_id, subscription_id, surface_id, status, attempt_count,
            last_error, created_at, processed_at
         ) VALUES (?1, ?2, ?3, ?4, 'pending', 1, NULL, ?5, NULL)",
            params![
                delivery_id,
                event.id,
                subscription_id,
                sub.owner_surface_id,
                now,
            ],
        )
        .map_err(|e| EventBusError::ExecutionFailed {
            reason: format!(
                "Database error inserting surface_event_deliveries '{}': {e}",
                delivery_id
            ),
        })?;

    let execution_result: Result<Value, String> = if let Some(handler) = &sub.handler {
        match handler {
            EventHandler::SetState { key, value } => {
                let resolved_val = resolve_template_value(value, &event.payload);
                let op = super::operations::AppOperation {
                    id: format!("op-evt-{}", Uuid::new_v4()),
                    op_type: "state.set".to_string(),
                    target: super::operations::OperationTarget {
                        surface_id: Some(sub.owner_surface_id.clone()),
                        ..Default::default()
                    },
                    payload: serde_json::json!({
                        "key": key,
                        "value": resolved_val,
                    }),
                    ..Default::default()
                };
                apply_handler_op(db, event, &format!("Event handler: set state {key}"), op)
            }
            EventHandler::PatchState { key, patch } => {
                let resolved_patch = resolve_template_value(patch, &event.payload);
                let op = super::operations::AppOperation {
                    id: format!("op-evt-{}", Uuid::new_v4()),
                    op_type: "state.patch".to_string(),
                    target: super::operations::OperationTarget {
                        surface_id: Some(sub.owner_surface_id.clone()),
                        ..Default::default()
                    },
                    payload: serde_json::json!({
                        "key": key,
                        "patch": resolved_patch,
                    }),
                    ..Default::default()
                };
                apply_handler_op(db, event, &format!("Event handler: patch state {key}"), op)
            }
            EventHandler::SetComponentProp {
                component_id,
                prop,
                value,
            } => {
                let resolved_val = resolve_template_value(value, &event.payload);
                let op = super::operations::AppOperation {
                    id: format!("op-evt-{}", Uuid::new_v4()),
                    op_type: "component.update_props".to_string(),
                    target: super::operations::OperationTarget {
                        surface_id: Some(sub.owner_surface_id.clone()),
                        component_id: Some(component_id.clone()),
                        ..Default::default()
                    },
                    payload: serde_json::json!({
                        "componentId": component_id,
                        "props": {
                            prop: resolved_val,
                        }
                    }),
                    ..Default::default()
                };
                apply_handler_op(
                    db,
                    event,
                    &format!("Event handler: set prop {prop} on {component_id}"),
                    op,
                )
            }
            EventHandler::InvokeRegisteredAction {
                action_name,
                parameters,
                application_id,
            } => {
                match super::get_surface(db, &sub.owner_surface_id) {
                    Ok(surface) => {
                        if let Some(expected_app) = application_id {
                            if surface.tool_id.as_deref() != Some(expected_app) {
                                Err(format!(
                                "cross-application action invocation denied: surface owned by {:?}, requested {}",
                                surface.tool_id, expected_app
                            ))
                            } else {
                                let resolved_params =
                                    resolve_template_value(parameters, &event.payload);
                                let ctx = crate::application_kernel::registered_actions::ActionRunContext {
                                actor: "event_handler".into(),
                                venue: crate::application_kernel::registered_actions::Venue::Application,
                                presence: crate::application_kernel::registered_actions::Presence::Present,
                                application_id: surface.tool_id.clone().or(application_id.clone()),
                                project_id: surface.project_id.clone(),
                                conversation_id: surface.conversation_id.clone(),
                                session_id: crate::application_kernel::registered_actions::context::session_id().to_string(),
                                run_id: format!("run-{}", Uuid::new_v4()),
                                trigger: Some(format!("event:{}", event.event_type)),
                                surface_id: Some(sub.owner_surface_id.clone()),
                                component_id: None,
                                depth: depth as u32,
                            };
                                let outcome = crate::application_kernel::registered_actions::execute_registered_action(
                                db,
                                &ctx,
                                action_name,
                                &resolved_params,
                                None,
                            );
                                match outcome {
                                crate::application_kernel::registered_actions::ActionOutcome::Ok { data, .. } => {
                                    Ok(serde_json::json!({
                                        "status": "success",
                                        "action": action_name,
                                        "surfaceId": sub.owner_surface_id,
                                        "output": data,
                                    }))
                                }
                                crate::application_kernel::registered_actions::ActionOutcome::PendingApproval { approval_id, .. } => {
                                    Ok(serde_json::json!({
                                        "status": "pending_approval",
                                        "approvalId": approval_id,
                                        "action": action_name,
                                        "surfaceId": sub.owner_surface_id,
                                    }))
                                }
                                crate::application_kernel::registered_actions::ActionOutcome::Error { message, code } => {
                                    Err(format!("action '{action_name}' failed with {code}: {message}"))
                                }
                                crate::application_kernel::registered_actions::ActionOutcome::Blocked { reason, .. } => {
                                    Err(format!("action '{action_name}' was blocked: {reason}"))
                                }
                            }
                            }
                        } else {
                            let resolved_params =
                                resolve_template_value(parameters, &event.payload);
                            let ctx = crate::application_kernel::registered_actions::ActionRunContext {
                            actor: "event_handler".into(),
                            venue: crate::application_kernel::registered_actions::Venue::Application,
                            presence: crate::application_kernel::registered_actions::Presence::Present,
                            application_id: surface.tool_id.clone(),
                            project_id: surface.project_id.clone(),
                            conversation_id: surface.conversation_id.clone(),
                            session_id: crate::application_kernel::registered_actions::context::session_id().to_string(),
                            run_id: format!("run-{}", Uuid::new_v4()),
                            trigger: Some(format!("event:{}", event.event_type)),
                            surface_id: Some(sub.owner_surface_id.clone()),
                            component_id: None,
                            depth: depth as u32,
                        };
                            let outcome = crate::application_kernel::registered_actions::execute_registered_action(
                            db,
                            &ctx,
                            action_name,
                            &resolved_params,
                            None,
                        );
                            match outcome {
                            crate::application_kernel::registered_actions::ActionOutcome::Ok { data, .. } => {
                                Ok(serde_json::json!({
                                    "status": "success",
                                    "action": action_name,
                                    "surfaceId": sub.owner_surface_id,
                                    "output": data,
                                }))
                            }
                            crate::application_kernel::registered_actions::ActionOutcome::PendingApproval { approval_id, .. } => {
                                Ok(serde_json::json!({
                                    "status": "pending_approval",
                                    "approvalId": approval_id,
                                    "action": action_name,
                                    "surfaceId": sub.owner_surface_id,
                                }))
                            }
                            crate::application_kernel::registered_actions::ActionOutcome::Error { message, code } => {
                                Err(format!("action '{action_name}' failed with {code}: {message}"))
                            }
                            crate::application_kernel::registered_actions::ActionOutcome::Blocked { reason, .. } => {
                                Err(format!("action '{action_name}' was blocked: {reason}"))
                            }
                        }
                        }
                    }
                    Err(e) => Err(format!("surface not found: {e}")),
                }
            }
            EventHandler::EmitClientNotification {
                title,
                message,
                level,
            } => {
                let op = super::operations::AppOperation {
                    id: format!("op-evt-{}", Uuid::new_v4()),
                    op_type: "chat.notification".to_string(),
                    target: super::operations::OperationTarget {
                        surface_id: Some(sub.owner_surface_id.clone()),
                        conversation_id: event.source.conversation_id.clone(),
                        ..Default::default()
                    },
                    payload: serde_json::json!({
                        "title": title,
                        "message": message,
                        "level": level.as_deref().unwrap_or("info"),
                    }),
                    ..Default::default()
                };
                apply_handler_op(db, event, "Event handler: notification", op)
            }
            EventHandler::SubmitEvent {
                event_type,
                payload,
            } => {
                let child_event = SurfaceEvent {
                    id: format!("evt-{}", Uuid::new_v4()),
                    event_type: event_type.clone(),
                    scope: "surface".into(),
                    source: EventRef {
                        surface_id: Some(sub.owner_surface_id.clone()),
                        conversation_id: event.source.conversation_id.clone(),
                        project_id: event.source.project_id.clone(),
                        ..Default::default()
                    },
                    target: EventRef::default(),
                    payload: resolve_template_value(payload, &event.payload),
                    idempotency_key: None,
                };
                match bus.dispatch(&child_event, depth + 1) {
                    Ok(child_matches) => {
                        let mut child_err = None;
                        for child_sub_id in child_matches {
                            if let Err(e) = execute_durable_event_delivery(
                                db,
                                bus,
                                &child_event,
                                &child_sub_id,
                                depth + 1,
                            ) {
                                child_err = Some(format!(
                                    "Child event delivery failed for child sub '{child_sub_id}': {e}"
                                ));
                                break;
                            }
                        }
                        if let Some(err_msg) = child_err {
                            Err(err_msg)
                        } else {
                            Ok(serde_json::json!({
                                "childEventId": child_event.id,
                            }))
                        }
                    }
                    Err(e) => Err(format!("Child event dispatch failed: {e}")),
                }
            }
        }
    } else {
        Ok(serde_json::json!({"status": "notified"}))
    };

    let processed_now = now_rfc3339();
    match execution_result {
        Ok(_val) => {
            db.conn()
                .execute(
                    "UPDATE surface_event_deliveries SET status = 'delivered',
                 processed_at = ?1 WHERE id = ?2",
                    params![processed_now, delivery_id],
                )
                .map_err(|e| EventBusError::ExecutionFailed {
                    reason: format!(
                        "Database error updating delivery status for '{}': {e}",
                        delivery_id
                    ),
                })?;
            db.conn().execute(
                "UPDATE surface_events SET status = 'processed', processed_at = ?1 WHERE id = ?2",
                params![processed_now, event.id],
            ).map_err(|e| EventBusError::ExecutionFailed {
                reason: format!("Database error updating surface_event status for '{}': {e}", event.id),
            })?;
            Ok(delivery_id)
        }
        Err(err) => {
            if let Err(e) = db.conn().execute(
                "UPDATE surface_event_deliveries SET status = 'failed',
                 last_error = ?1, processed_at = ?2 WHERE id = ?3",
                params![err, processed_now, delivery_id],
            ) {
                tracing::error!("Failed to record failed status for delivery '{delivery_id}': {e}");
            }
            Err(EventBusError::ExecutionFailed { reason: err })
        }
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

    #[test]
    fn source_filter_matches_properly() {
        let mut bus = EventBus::new();
        let sub = Subscription {
            id: "sub-1".into(),
            owner_surface_id: "s1".into(),
            event_types: vec!["task_created".into()],
            source_filter: EventRef {
                surface_id: Some("source-surf".into()),
                application_id: Some("app-todo".into()),
                ..Default::default()
            },
            target: EventRef::default(),
            handler: None,
            enabled: true,
        };
        bus.add_subscription(sub).unwrap();

        // Event from matching surface and app
        let ev_match = SurfaceEvent {
            id: "e1".into(),
            event_type: "task_created".into(),
            scope: "surface".into(),
            source: EventRef {
                surface_id: Some("source-surf".into()),
                application_id: Some("app-todo".into()),
                ..Default::default()
            },
            target: EventRef::default(),
            payload: json!({ "title": "Buy groceries" }),
            idempotency_key: None,
        };
        let matched = bus.dispatch(&ev_match, 0).unwrap();
        assert_eq!(matched, vec!["sub-1"]);

        // Event from different surface
        bus.rates.clear();
        let ev_diff_surf = SurfaceEvent {
            source: EventRef {
                surface_id: Some("other-surf".into()),
                application_id: Some("app-todo".into()),
                ..Default::default()
            },
            ..ev_match.clone()
        };
        assert!(bus.dispatch(&ev_diff_surf, 0).unwrap().is_empty());

        // Event from different app
        bus.rates.clear();
        let ev_diff_app = SurfaceEvent {
            source: EventRef {
                surface_id: Some("source-surf".into()),
                application_id: Some("other-app".into()),
                ..Default::default()
            },
            ..ev_match.clone()
        };
        assert!(bus.dispatch(&ev_diff_app, 0).unwrap().is_empty());
    }

    #[test]
    fn target_matching_isolates_surfaces() {
        let mut bus = EventBus::new();
        // Subscription on surface-A
        let sub_a = Subscription {
            id: "sub-a".into(),
            owner_surface_id: "surface-A".into(),
            event_types: vec!["ping".into()],
            source_filter: EventRef::default(),
            target: EventRef::default(),
            handler: None,
            enabled: true,
        };
        // Subscription on surface-B
        let sub_b = Subscription {
            id: "sub-b".into(),
            owner_surface_id: "surface-B".into(),
            event_types: vec!["ping".into()],
            source_filter: EventRef::default(),
            target: EventRef::default(),
            handler: None,
            enabled: true,
        };
        bus.add_subscription(sub_a).unwrap();
        bus.add_subscription(sub_b).unwrap();

        // Event targeted specifically to surface-B
        let ev_targeted = SurfaceEvent {
            id: "e-target".into(),
            event_type: "ping".into(),
            scope: "surface".into(),
            source: EventRef::default(),
            target: EventRef {
                surface_id: Some("surface-B".into()),
                ..Default::default()
            },
            payload: json!({}),
            idempotency_key: None,
        };
        let matched = bus.dispatch(&ev_targeted, 0).unwrap();
        // Only surface-B's subscription should match, NOT surface-A's!
        assert_eq!(matched, vec!["sub-b"]);
    }

    #[test]
    fn cross_app_events_are_denied() {
        let mut bus = EventBus::new();
        let ev_cross = SurfaceEvent {
            id: "e-cross".into(),
            event_type: "data_sync".into(),
            scope: "surface".into(),
            source: EventRef {
                application_id: Some("app-finance".into()),
                ..Default::default()
            },
            target: EventRef {
                application_id: Some("app-notes".into()),
                ..Default::default()
            },
            payload: json!({}),
            idempotency_key: None,
        };
        let err = bus.dispatch(&ev_cross, 0).unwrap_err();
        assert_eq!(err, EventBusError::CrossAppDenied);
    }

    #[test]
    fn load_from_db_quarantines_corrupt_json() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("corrupt.db")).unwrap();
        let conv = crate::db::create_conversation(
            &mut db,
            crate::db::DEFAULT_WORKSPACE_ID,
            "Quarantine Test",
            None,
        )
        .unwrap();
        let surf = super::super::surfaces::create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Quarantine Surface",
            &json!({"type":"container","id":"root"}),
            &[],
        )
        .unwrap();

        // Insert a corrupt row
        db.conn()
            .execute(
                "INSERT INTO surface_subscriptions (
                    id, owner_surface_id, source_filter_json, target_json,
                    event_types_json, handler_json, enabled, created_at, updated_at
                 ) VALUES ('sub-bad', ?1, '{corrupt json', '{}', '[]', '{}', 1, datetime('now'), datetime('now'))",
                [&surf.id],
            )
            .unwrap();

        // Hydrate from DB
        let bus = EventBus::load_from_db(&db);
        // Bad subscription must NOT be in memory
        assert!(bus.get_subscription("sub-bad").is_none());

        // And in SQLite it must have been quarantined to enabled = 0
        let enabled: i64 = db
            .conn()
            .query_row(
                "SELECT enabled FROM surface_subscriptions WHERE id = 'sub-bad'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            enabled, 0,
            "corrupted subscription must be quarantined to disabled"
        );
    }

    #[test]
    fn durable_event_delivery_executes_handler_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("deliv.db")).unwrap();
        let conv = crate::db::create_conversation(
            &mut db,
            crate::db::DEFAULT_WORKSPACE_ID,
            "Event Test",
            None,
        )
        .unwrap();
        let surf = super::super::surfaces::create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Task Manager",
            &json!({"type":"container","id":"root"}),
            &[],
        )
        .unwrap();
        let surface_id = surf.id.as_str();

        let mut bus = EventBus::new();
        let sub = Subscription {
            id: "sub-counter".into(),
            owner_surface_id: surface_id.into(),
            event_types: vec!["task.added".into()],
            source_filter: EventRef::default(),
            target: EventRef::default(),
            handler: Some(EventHandler::SetState {
                key: "count".into(),
                value: json!(10),
            }),
            enabled: true,
        };
        bus.add_subscription(sub).unwrap();

        let ev = SurfaceEvent {
            id: "evt-add-1".into(),
            event_type: "task.added".into(),
            scope: "surface".into(),
            source: EventRef {
                surface_id: Some(surface_id.into()),
                conversation_id: Some(conv.id.clone()),
                ..Default::default()
            },
            target: EventRef::default(),
            payload: json!({ "taskName": "Write tests" }),
            idempotency_key: None,
        };

        // First delivery executes and succeeds
        let res = execute_durable_event_delivery(&mut db, &mut bus, &ev, "sub-counter", 0);
        assert!(res.is_ok(), "res was: {:?}", res);

        // Check delivery status in surface_event_deliveries
        let status: String = db
            .conn()
            .query_row(
                "SELECT status FROM surface_event_deliveries WHERE event_id = ?1 AND subscription_id = ?2",
                rusqlite::params![ev.id, "sub-counter"],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "delivered");

        // Check surface state was actually updated by the handler!
        let state = super::super::surfaces::get_surface_state(&db, surface_id).unwrap();
        assert_eq!(state.get("count").unwrap().as_i64(), Some(10));

        // Re-delivery of the same event and subscription is idempotent and does not fail
        let re_res = execute_durable_event_delivery(&mut db, &mut bus, &ev, "sub-counter", 0);
        assert_eq!(re_res.unwrap(), "already_delivered");
    }

    #[test]
    fn test_child_event_failure_fails_closed_and_does_not_mark_parent_delivered() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("evt_fail.db")).unwrap();
        let conv = crate::db::create_conversation(
            &mut db,
            crate::db::DEFAULT_WORKSPACE_ID,
            "Test Conv",
            None,
        )
        .unwrap();
        let surf = super::super::surfaces::create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Parent Surface",
            &json!({"type":"container","id":"root"}),
            &[],
        )
        .unwrap();

        let mut bus = EventBus::new();
        // Parent subscription emits a child event that will fail child delivery due to DepthExceeded
        let parent_sub = Subscription {
            id: "sub-parent".into(),
            owner_surface_id: surf.id.clone(),
            event_types: vec!["parent.trigger".into()],
            source_filter: EventRef::default(),
            target: EventRef::default(),
            handler: Some(EventHandler::SubmitEvent {
                event_type: "child.trigger".into(),
                payload: json!({}),
            }),
            enabled: true,
        };
        bus.add_subscription(parent_sub).unwrap();

        let child_sub = Subscription {
            id: "sub-child".into(),
            owner_surface_id: surf.id.clone(),
            event_types: vec!["child.trigger".into()],
            source_filter: EventRef::default(),
            target: EventRef::default(),
            handler: Some(EventHandler::SetState {
                key: "child_ran".into(),
                value: json!(true),
            }),
            enabled: true,
        };
        bus.add_subscription(child_sub).unwrap();

        let parent_event = SurfaceEvent {
            id: "evt-parent-fail".into(),
            event_type: "parent.trigger".into(),
            scope: "surface".into(),
            source: EventRef {
                surface_id: Some(surf.id.clone()),
                conversation_id: Some(conv.id.clone()),
                ..Default::default()
            },
            target: EventRef::default(),
            payload: json!({}),
            idempotency_key: None,
        };

        // Delivering at MAX_EVENT_DEPTH means child event dispatch will exceed MAX_EVENT_DEPTH
        let res = execute_durable_event_delivery(
            &mut db,
            &mut bus,
            &parent_event,
            "sub-parent",
            MAX_EVENT_DEPTH,
        );
        assert!(
            res.is_err(),
            "Delivery must fail when child event delivery fails"
        );

        // Verify parent delivery status is 'failed', NOT 'delivered'
        let status: String = db
            .conn()
            .query_row(
                "SELECT status FROM surface_event_deliveries WHERE event_id = ?1 AND subscription_id = ?2",
                rusqlite::params![parent_event.id, "sub-parent"],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "failed");

        // Verify surface_events status is NOT 'processed'
        let evt_status: String = db
            .conn()
            .query_row(
                "SELECT status FROM surface_events WHERE id = ?1",
                rusqlite::params![parent_event.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_ne!(evt_status, "processed");
    }
}
