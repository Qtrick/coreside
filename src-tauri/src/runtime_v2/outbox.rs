//! Post-commit event outbox — never mutate live EventBus before SQLite commit.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::db::{now_rfc3339, Database, DbResult};
use crate::runtime_v2::events::{EventBus, EventRef, Subscription, SurfaceEvent};
use rusqlite::params;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum DeferredBusEffect {
    AddSubscription(Subscription),
    RemoveSubscription { id: String },
    RemoveSubscriptionsForSurface { surface_id: String },
    SetSubscriptionEnabled { id: String, enabled: bool },
    Dispatch(SurfaceEvent),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitOutcome {
    Committed,
    RejectedValidation,
    Conflicted,
    FailedBeforeCommit,
    RecoveredPriorCommitted,
    InterruptedRecoverable,
}

impl CommitOutcome {
    pub fn is_success(self) -> bool {
        matches!(
            self,
            CommitOutcome::Committed | CommitOutcome::RecoveredPriorCommitted
        )
    }
}

pub fn enqueue_outbox(
    db: &Database,
    transaction_id: Option<&str>,
    conversation_id: Option<&str>,
    turn_id: Option<&str>,
    attempt_id: Option<&str>,
    sequence: i64,
    effect: &DeferredBusEffect,
) -> DbResult<String> {
    let id = format!("outbox-{}", Uuid::new_v4());
    let now = now_rfc3339();
    let effect_type = match effect {
        DeferredBusEffect::AddSubscription(_) => "add_subscription",
        DeferredBusEffect::RemoveSubscription { .. } => "remove_subscription",
        DeferredBusEffect::RemoveSubscriptionsForSurface { .. } => "remove_subscriptions_surface",
        DeferredBusEffect::SetSubscriptionEnabled { .. } => "set_subscription_enabled",
        DeferredBusEffect::Dispatch(_) => "dispatch",
    };
    db.conn().execute(
        "INSERT INTO commit_event_outbox (
            id, transaction_id, conversation_id, turn_id, attempt_id, sequence,
            effect_type, payload_json, status, created_at, delivery_attempts
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,'pending',?9,0)",
        params![
            id,
            transaction_id,
            conversation_id,
            turn_id,
            attempt_id,
            sequence,
            effect_type,
            serde_json::to_string(effect)?,
            now
        ],
    )?;
    Ok(id)
}

pub fn apply_deferred_effect(bus: &mut EventBus, effect: &DeferredBusEffect) {
    match effect {
        DeferredBusEffect::AddSubscription(sub) => {
            let _ = bus.add_subscription(sub.clone());
        }
        DeferredBusEffect::RemoveSubscription { id } => {
            bus.remove_subscription(id);
        }
        DeferredBusEffect::RemoveSubscriptionsForSurface { surface_id } => {
            bus.remove_subscriptions_for_surface(surface_id);
        }
        DeferredBusEffect::SetSubscriptionEnabled { id, enabled } => {
            bus.set_subscription_enabled(id, *enabled);
        }
        DeferredBusEffect::Dispatch(ev) => {
            let _ = bus.dispatch(ev, 0);
        }
    }
}

/// Deliver pending outbox rows after a successful commit. Restart-safe and idempotent.
///
/// Requires a live EventBus — never mark rows delivered without applying them.
pub fn flush_pending_outbox(db: &Database, bus: Option<&mut EventBus>) -> DbResult<usize> {
    let Some(bus) = bus else {
        // ponytail: without a bus, leave rows pending for a later flush (startup / retry)
        return Ok(0);
    };
    let mut stmt = db.conn().prepare(
        "SELECT id, payload_json FROM commit_event_outbox
         WHERE status = 'pending'
         ORDER BY created_at ASC, sequence ASC
         LIMIT 200",
    )?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);

    let mut delivered = 0usize;
    let now = now_rfc3339();
    for (id, payload) in rows {
        let effect: DeferredBusEffect = match serde_json::from_str(&payload) {
            Ok(e) => e,
            Err(e) => {
                db.conn().execute(
                    "UPDATE commit_event_outbox SET status = 'failed', last_error = ?1,
                     delivery_attempts = delivery_attempts + 1 WHERE id = ?2",
                    params![e.to_string(), id],
                )?;
                continue;
            }
        };
        apply_deferred_effect(bus, &effect);
        let updated = db.conn().execute(
            "UPDATE commit_event_outbox SET status = 'delivered', delivered_at = ?1,
             delivery_attempts = delivery_attempts + 1 WHERE id = ?2 AND status = 'pending'",
            params![now, id],
        )?;
        if updated > 0 {
            delivered += 1;
        }
    }
    Ok(delivered)
}

pub fn lookup_idempotency_outcome(
    db: &Database,
    scope_key: &str,
) -> DbResult<Option<(String, Value)>> {
    let row = db.conn().query_row(
        "SELECT outcome, result_json FROM apply_idempotency_outcomes WHERE scope_key = ?1",
        [scope_key],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
    );
    match row {
        Ok((outcome, json)) => {
            let value: Value = serde_json::from_str(&json).unwrap_or(Value::Null);
            Ok(Some((outcome, value)))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn store_idempotency_outcome(
    db: &Database,
    scope_key: &str,
    profile_id: Option<&str>,
    conversation_id: Option<&str>,
    turn_id: Option<&str>,
    transaction_id: Option<&str>,
    outcome: &str,
    result: &Value,
) -> DbResult<()> {
    let now = now_rfc3339();
    db.conn().execute(
        "INSERT OR IGNORE INTO apply_idempotency_outcomes (
            scope_key, profile_id, conversation_id, turn_id, transaction_id,
            outcome, result_json, created_at
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            scope_key,
            profile_id,
            conversation_id,
            turn_id,
            transaction_id,
            outcome,
            result.to_string(),
            now
        ],
    )?;
    Ok(())
}

pub fn idempotency_scope_key(
    conversation_id: Option<&str>,
    turn_id: Option<&str>,
    keys: &[String],
) -> Option<String> {
    if keys.is_empty() {
        return None;
    }
    let mut parts = vec![
        conversation_id.unwrap_or("_").to_string(),
        turn_id.unwrap_or("_").to_string(),
    ];
    let mut sorted = keys.to_vec();
    sorted.sort();
    parts.extend(sorted);
    Some(parts.join("|"))
}

/// Build an idempotency scope only when every operation carries a key.
/// Partial key sets would ignore unkeyed ops on retry (false recover).
pub fn idempotency_keys_for_batch(
    ops: &[crate::runtime_v2::operations::AppOperation],
) -> Vec<String> {
    if ops.is_empty() || !ops.iter().all(|o| o.idempotency_key.is_some()) {
        return Vec::new();
    }
    ops.iter()
        .filter_map(|o| o.idempotency_key.clone())
        .collect()
}

/// Validate every depends_on reference exists in the batch (fail closed).
pub fn validate_dependency_refs(
    ops: &[crate::runtime_v2::operations::AppOperation],
) -> Result<(), String> {
    let ids: std::collections::HashSet<&str> = ops.iter().map(|o| o.id.as_str()).collect();
    for op in ops {
        if let Some(deps) = &op.depends_on {
            for dep in deps {
                if !ids.contains(dep.as_str()) {
                    return Err(format!(
                        "missing depends_on reference '{dep}' from operation '{}'",
                        op.id
                    ));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn missing_dependency_fails_closed() {
        let ops = vec![crate::runtime_v2::operations::AppOperation {
            id: "a".into(),
            op_type: "state.set".into(),
            target: Default::default(),
            base_revision: None,
            transaction_group: None,
            idempotency_key: None,
            depends_on: Some(vec!["missing-op".into()]),
            payload: serde_json::json!({}),
            requires_approval: None,
            destructive: None,
            audience: None,
        }];
        assert!(validate_dependency_refs(&ops).is_err());
    }

    #[test]
    fn outbox_survives_until_flush() {
        let dir = tempdir().unwrap();
        let db = crate::db::Database::open_path(&dir.path().join("o.db")).unwrap();
        let effect = DeferredBusEffect::Dispatch(SurfaceEvent {
            id: "evt-1".into(),
            event_type: "test".into(),
            scope: "surface".into(),
            source: EventRef::default(),
            target: EventRef::default(),
            payload: serde_json::json!({}),
            idempotency_key: Some("k1".into()),
        });
        enqueue_outbox(&db, Some("txn-1"), None, None, None, 0, &effect).unwrap();
        // Without a bus, rows must stay pending (never mark delivered blindly).
        assert_eq!(flush_pending_outbox(&db, None).unwrap(), 0);
        let pending: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM commit_event_outbox WHERE status = 'pending'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(pending, 1);
        let mut bus = EventBus::new();
        let n = flush_pending_outbox(&db, Some(&mut bus)).unwrap();
        assert_eq!(n, 1);
        let n2 = flush_pending_outbox(&db, Some(&mut bus)).unwrap();
        assert_eq!(n2, 0);
    }

    #[test]
    fn partial_idempotency_keys_do_not_form_scope() {
        let ops = vec![
            crate::runtime_v2::operations::AppOperation {
                id: "a".into(),
                op_type: "state.set".into(),
                target: Default::default(),
                base_revision: None,
                transaction_group: None,
                idempotency_key: Some("k1".into()),
                depends_on: None,
                payload: serde_json::json!({}),
                requires_approval: None,
                destructive: None,
                audience: None,
            },
            crate::runtime_v2::operations::AppOperation {
                id: "b".into(),
                op_type: "state.set".into(),
                target: Default::default(),
                base_revision: None,
                transaction_group: None,
                idempotency_key: None,
                depends_on: None,
                payload: serde_json::json!({}),
                requires_approval: None,
                destructive: None,
                audience: None,
            },
        ];
        assert!(idempotency_keys_for_batch(&ops).is_empty());
        assert!(
            idempotency_scope_key(Some("c"), Some("t"), &idempotency_keys_for_batch(&ops))
                .is_none()
        );
    }
}
