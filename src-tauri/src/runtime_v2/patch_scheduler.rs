//! Patch scheduler — dependency graph, priority, backpressure, supersession.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;

use super::limits::{MAX_PATCH_QUEUE, MAX_PATCH_QUEUE_BYTES, MAX_PREVIEW_HZ};
use super::operations::{validate_operations, AppOperation};
use crate::application_kernel::{apply_change, ChangeRequest, ChangeResult};
use crate::db::{now_rfc3339, Database, DbError, DbResult};
use crate::runtime_v2::EventBus;
use rusqlite::params;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum PatchPriority {
    CriticalRecovery,
    DirectUserInteraction,
    ActiveTurnPreview,
    ApprovedPersistentChange,
    NavigationUpdate,
    BackgroundRefresh,
    AutomationUpdate,
    LowPriorityEnrichment,
}

impl PatchPriority {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CriticalRecovery => "critical_recovery",
            Self::DirectUserInteraction => "direct_user_interaction",
            Self::ActiveTurnPreview => "active_turn_preview",
            Self::ApprovedPersistentChange => "approved_persistent_change",
            Self::NavigationUpdate => "navigation_update",
            Self::BackgroundRefresh => "background_refresh",
            Self::AutomationUpdate => "automation_update",
            Self::LowPriorityEnrichment => "low_priority_enrichment",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "critical_recovery" => Some(Self::CriticalRecovery),
            "direct_user_interaction" => Some(Self::DirectUserInteraction),
            "active_turn_preview" => Some(Self::ActiveTurnPreview),
            "approved_persistent_change" => Some(Self::ApprovedPersistentChange),
            "navigation_update" => Some(Self::NavigationUpdate),
            "background_refresh" => Some(Self::BackgroundRefresh),
            "automation_update" => Some(Self::AutomationUpdate),
            "low_priority_enrichment" => Some(Self::LowPriorityEnrichment),
            _ => None,
        }
    }

    pub fn rank(self) -> u8 {
        match self {
            Self::CriticalRecovery => 0,
            Self::DirectUserInteraction => 1,
            Self::ActiveTurnPreview => 2,
            Self::ApprovedPersistentChange => 3,
            Self::NavigationUpdate => 4,
            Self::BackgroundRefresh => 5,
            Self::AutomationUpdate => 6,
            Self::LowPriorityEnrichment => 7,
        }
    }
}

/// Agent cannot assign priorities above `approved_persistent_change`.
pub fn validate_agent_priority(priority: PatchPriority) -> Result<(), String> {
    if priority.rank() < PatchPriority::ApprovedPersistentChange.rank() {
        Err(format!(
            "agent cannot assign {} priority",
            priority.as_str()
        ))
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledPatch {
    pub id: String,
    pub operation_id: String,
    pub transaction_id: Option<String>,
    pub turn_id: Option<String>,
    pub conversation_id: Option<String>,
    pub surface_id: Option<String>,
    pub priority: PatchPriority,
    pub status: String,
    pub sequence_number: i64,
    pub depends_on: Vec<String>,
    pub payload: Value,
    pub created_at: String,
    pub applied_at: Option<String>,
    pub failed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleRequest {
    pub conversation_id: Option<String>,
    pub turn_id: Option<String>,
    pub surface_id: Option<String>,
    pub priority: PatchPriority,
    pub operations: Vec<AppOperation>,
    pub source_type: String,
    pub from_agent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleAndApplyResult {
    pub scheduled: Vec<ScheduledPatch>,
    pub applied: Vec<ChangeResult>,
    pub superseded: Vec<String>,
}

pub fn record_manual_edit_provenance(
    db: &mut Database,
    transaction_id: Option<&str>,
    surface_id: Option<&str>,
    source_type: &str,
    user_action_type: &str,
    affected: &[String],
) -> DbResult<String> {
    if source_type != "user" && source_type != "direct_manipulation" {
        return Err(DbError::Invalid(
            "manual_edit_provenance requires user or direct_manipulation source".into(),
        ));
    }
    let id = format!("mprov-{}", Uuid::new_v4());
    let now = now_rfc3339();
    let affected_json = serde_json::to_string(affected)?;
    db.conn().execute(
        "INSERT INTO manual_edit_provenance (
            id, transaction_id, surface_id, source_type, user_action_type, affected_json, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id,
            transaction_id,
            surface_id,
            source_type,
            user_action_type,
            affected_json,
            now
        ],
    )?;
    Ok(id)
}

fn queued_count(db: &Database) -> DbResult<usize> {
    let count: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM patch_scheduler_items WHERE status = 'queued'",
        [],
        |r| r.get(0),
    )?;
    Ok(count as usize)
}

fn queued_bytes(db: &Database) -> DbResult<usize> {
    let bytes: i64 = db.conn().query_row(
        "SELECT COALESCE(SUM(LENGTH(payload_json)), 0) FROM patch_scheduler_items WHERE status = 'queued'",
        [],
        |r| r.get(0),
    )?;
    Ok(bytes as usize)
}

pub fn enforce_queue_limits(db: &Database, additional_bytes: usize) -> DbResult<()> {
    let count = queued_count(db)?;
    if count >= MAX_PATCH_QUEUE {
        return Err(DbError::Invalid(format!(
            "patch queue full: max {MAX_PATCH_QUEUE} items"
        )));
    }
    let bytes = queued_bytes(db)?;
    if bytes + additional_bytes > MAX_PATCH_QUEUE_BYTES {
        return Err(DbError::Invalid(format!(
            "patch queue bytes exceeded: max {MAX_PATCH_QUEUE_BYTES}"
        )));
    }
    Ok(())
}

fn supersede_preview_items(
    db: &mut Database,
    surface_id: Option<&str>,
    new_id: &str,
) -> DbResult<Vec<String>> {
    let Some(sid) = surface_id else {
        return Ok(Vec::new());
    };
    let now = now_rfc3339();
    let mut stmt = db.conn().prepare(
        "SELECT id FROM patch_scheduler_items
         WHERE status = 'queued' AND priority = 'active_turn_preview' AND surface_id = ?1",
    )?;
    let ids: Vec<String> = stmt
        .query_map([sid], |r| r.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    for old_id in &ids {
        db.conn().execute(
            "UPDATE patch_scheduler_items SET status = 'superseded', superseded_by = ?1,
             supersession_reason = 'newer_preview', failed_at = ?2 WHERE id = ?3",
            params![new_id, now, old_id],
        )?;
    }
    Ok(ids)
}

pub fn schedule_patches(db: &mut Database, req: &ScheduleRequest) -> DbResult<Vec<ScheduledPatch>> {
    if req.from_agent {
        validate_agent_priority(req.priority).map_err(DbError::Invalid)?;
    }
    validate_operations(&req.operations).map_err(DbError::Invalid)?;

    let payload_bytes: usize = serde_json::to_string(&req.operations)
        .map(|s| s.len())
        .unwrap_or(0);
    enforce_queue_limits(db, payload_bytes)?;

    let mut scheduled = Vec::new();
    let now = now_rfc3339();
    let base_seq: i64 = db.conn().query_row(
        "SELECT COALESCE(MAX(sequence_number), 0) FROM patch_scheduler_items",
        [],
        |r| r.get(0),
    )?;

    for (i, op) in req.operations.iter().enumerate() {
        let id = format!("patch-{}", Uuid::new_v4());
        let depends_on = op.depends_on.clone().unwrap_or_default();
        let depends_json = serde_json::to_string(&depends_on)?;
        let payload_json = serde_json::to_string(op)?;
        let surface_id = req
            .surface_id
            .as_deref()
            .or(op.target.surface_id.as_deref());

        if req.priority == PatchPriority::ActiveTurnPreview {
            let _ = supersede_preview_items(db, surface_id, &id)?;
        }

        db.conn().execute(
            "INSERT INTO patch_scheduler_items (
                id, operation_id, turn_id, conversation_id, surface_id, priority, status,
                sequence_number, depends_on_json, payload_json, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'queued', ?7, ?8, ?9, ?10)",
            params![
                id,
                op.id,
                req.turn_id,
                req.conversation_id,
                surface_id,
                req.priority.as_str(),
                base_seq + i as i64 + 1,
                depends_json,
                payload_json,
                now
            ],
        )?;
        scheduled.push(get_scheduled_patch(db, &id)?);
    }
    Ok(scheduled)
}

pub fn get_scheduled_patch(db: &Database, id: &str) -> DbResult<ScheduledPatch> {
    db.conn()
        .query_row(
            "SELECT id, operation_id, transaction_id, turn_id, conversation_id, surface_id,
                    priority, status, sequence_number, depends_on_json, payload_json,
                    created_at, applied_at, failed_at
             FROM patch_scheduler_items WHERE id = ?1",
            [id],
            |row| parse_patch_row(row),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                DbError::NotFound(format!("scheduled patch {id}"))
            }
            other => DbError::Sqlite(other),
        })
}

fn parse_patch_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScheduledPatch> {
    let priority_str: String = row.get(6)?;
    let depends_json: String = row.get(9)?;
    let payload_json: String = row.get(10)?;
    Ok(ScheduledPatch {
        id: row.get(0)?,
        operation_id: row.get(1)?,
        transaction_id: row.get(2)?,
        turn_id: row.get(3)?,
        conversation_id: row.get(4)?,
        surface_id: row.get(5)?,
        priority: PatchPriority::parse(&priority_str)
            .unwrap_or(PatchPriority::ApprovedPersistentChange),
        status: row.get(7)?,
        sequence_number: row.get(8)?,
        depends_on: serde_json::from_str(&depends_json).unwrap_or_default(),
        payload: serde_json::from_str(&payload_json).unwrap_or(Value::Null),
        created_at: row.get(11)?,
        applied_at: row.get(12)?,
        failed_at: row.get(13)?,
    })
}

/// Detect cycles in operation dependency graph.
pub fn detect_dependency_cycle(ops: &[AppOperation]) -> Option<Vec<String>> {
    let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();
    for op in ops {
        let deps: Vec<&str> = op
            .depends_on
            .as_ref()
            .map(|d| d.iter().map(String::as_str).collect())
            .unwrap_or_default();
        graph.insert(op.id.as_str(), deps);
    }
    let mut visiting: HashSet<&str> = HashSet::new();
    let mut visited: HashSet<&str> = HashSet::new();
    let mut path: Vec<&str> = Vec::new();

    fn dfs<'a>(
        node: &'a str,
        graph: &HashMap<&'a str, Vec<&'a str>>,
        visiting: &mut HashSet<&'a str>,
        visited: &mut HashSet<&'a str>,
        path: &mut Vec<&'a str>,
    ) -> Option<Vec<String>> {
        if visiting.contains(node) {
            let start = path.iter().position(|&n| n == node).unwrap_or(0);
            return Some(path[start..].iter().map(|s| (*s).to_string()).collect());
        }
        if visited.contains(node) {
            return None;
        }
        visiting.insert(node);
        path.push(node);
        if let Some(deps) = graph.get(node) {
            for dep in deps {
                if let Some(cycle) = dfs(dep, graph, visiting, visited, path) {
                    return Some(cycle);
                }
            }
        }
        path.pop();
        visiting.remove(node);
        visited.insert(node);
        None
    }

    for op in ops {
        if let Some(cycle) = dfs(op.id.as_str(), &graph, &mut visiting, &mut visited, &mut path) {
            return Some(cycle);
        }
    }
    None
}

/// Topological order for operations respecting depends_on.
pub fn topological_order(ops: &[AppOperation]) -> Result<Vec<usize>, String> {
    if let Some(cycle) = detect_dependency_cycle(ops) {
        return Err(format!("dependency cycle: {}", cycle.join(" -> ")));
    }
    let id_to_idx: HashMap<&str, usize> = ops
        .iter()
        .enumerate()
        .map(|(i, op)| (op.id.as_str(), i))
        .collect();
    let mut in_degree = vec![0usize; ops.len()];
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); ops.len()];
    for (i, op) in ops.iter().enumerate() {
        if let Some(deps) = &op.depends_on {
            for dep in deps {
                if let Some(&j) = id_to_idx.get(dep.as_str()) {
                    adj[j].push(i);
                    in_degree[i] += 1;
                }
            }
        }
    }
    let mut queue: VecDeque<usize> = in_degree
        .iter()
        .enumerate()
        .filter(|(_, d)| **d == 0)
        .map(|(i, _)| i)
        .collect();
    let mut order = Vec::new();
    while let Some(i) = queue.pop_front() {
        order.push(i);
        for &next in &adj[i] {
            in_degree[next] -= 1;
            if in_degree[next] == 0 {
                queue.push_back(next);
            }
        }
    }
    if order.len() != ops.len() {
        return Err("dependency cycle detected".into());
    }
    Ok(order)
}

pub fn flush_scheduler(
    db: &mut Database,
    bus: &mut Option<&mut EventBus>,
    conversation_id: Option<&str>,
    source_type: &str,
    approval_granted: bool,
) -> DbResult<Vec<ChangeResult>> {
    let mut query = String::from(
        "SELECT id FROM patch_scheduler_items WHERE status = 'queued'",
    );
    if conversation_id.is_some() {
        query.push_str(" AND conversation_id = ?1");
    }
    query.push_str(" ORDER BY sequence_number ASC");

    let ids: Vec<String> = if let Some(cid) = conversation_id {
        let mut stmt = db.conn().prepare(&query)?;
        let rows = stmt.query_map([cid], |r| r.get(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    } else {
        let mut stmt = db.conn().prepare(&query)?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let mut applied = Vec::new();
    let now = now_rfc3339();
    for patch_id in ids {
        let patch = get_scheduled_patch(db, &patch_id)?;
        let op: AppOperation = serde_json::from_value(patch.payload.clone())
            .map_err(|e| DbError::Invalid(e.to_string()))?;
        let result = apply_change(
            db,
            bus.as_deref_mut(),
            ChangeRequest {
                conversation_id: patch.conversation_id.clone(),
                project_id: None,
                turn_id: patch.turn_id.clone(),
                summary: format!("Patch {}", patch.operation_id),
                operations: vec![op],
                silent: patch.priority == PatchPriority::ActiveTurnPreview,
                source_type: source_type.into(),
                provider: None,
                model: None,
                require_approval: false,
                approval_granted,
            },
        )
        .map_err(|e| DbError::Invalid(e.user_message()))?;

        if result.apply.is_some() {
            db.conn().execute(
                "UPDATE patch_scheduler_items SET status = 'applied', applied_at = ?1,
                 transaction_id = ?2 WHERE id = ?3",
                params![
                    now,
                    result.apply.as_ref().map(|a| a.transaction.id.as_str()),
                    patch_id
                ],
            )?;
            if source_type == "user" || source_type == "direct_manipulation" {
                let _ = record_manual_edit_provenance(
                    db,
                    result.apply.as_ref().map(|a| a.transaction.id.as_str()),
                    patch.surface_id.as_deref(),
                    source_type,
                    "patch_apply",
                    &[patch.operation_id.clone()],
                );
            }
            applied.push(result);
        } else {
            db.conn().execute(
                "UPDATE patch_scheduler_items SET status = 'pending_approval', failed_at = ?1 WHERE id = ?2",
                params![now, patch_id],
            )?;
            applied.push(result);
        }
    }
    Ok(applied)
}

/// Validate, schedule, and apply patches in dependency order.
pub fn schedule_and_apply(
    db: &mut Database,
    bus: &mut Option<&mut EventBus>,
    req: ScheduleRequest,
    approval_granted: bool,
) -> DbResult<ScheduleAndApplyResult> {
    if let Some(cycle) = detect_dependency_cycle(&req.operations) {
        return Err(DbError::Invalid(format!(
            "dependency cycle: {}",
            cycle.join(" -> ")
        )));
    }

    let superseded = if req.priority == PatchPriority::ActiveTurnPreview {
        let surface_id = req
            .surface_id
            .as_deref()
            .or_else(|| {
                req.operations
                    .first()
                    .and_then(|o| o.target.surface_id.as_deref())
            });
        // ponytail: supersession happens during schedule_patches per op
        let _ = surface_id;
        Vec::new()
    } else {
        Vec::new()
    };

    let scheduled = schedule_patches(db, &req)?;
    let order = topological_order(&req.operations)
        .map_err(DbError::Invalid)?;

    let mut applied = Vec::new();
    let now = now_rfc3339();
    let source_type = req.source_type.clone();

    for &idx in &order {
        let op = &req.operations[idx];
        let patch = scheduled
            .iter()
            .find(|p| p.operation_id == op.id)
            .ok_or_else(|| DbError::Invalid("scheduled patch missing".into()))?;

        let result = apply_change(
            db,
            bus.as_deref_mut(),
            ChangeRequest {
                conversation_id: req.conversation_id.clone(),
                project_id: None,
                turn_id: req.turn_id.clone(),
                summary: format!("Patch {}", op.id),
                operations: vec![op.clone()],
                silent: req.priority == PatchPriority::ActiveTurnPreview,
                source_type: source_type.clone(),
                provider: None,
                model: None,
                require_approval: false,
                approval_granted,
            },
        );

        let change_result = match result {
            Ok(r) => r,
            Err(e) => {
                let _ = db.conn().execute(
                    "UPDATE patch_scheduler_items SET status = 'failed', failed_at = ?1,
                     error_category = 'apply' WHERE id = ?2",
                    params![now, patch.id],
                );
                return Err(DbError::Invalid(e.user_message()));
            }
        };

        if change_result.apply.is_some() {
            db.conn().execute(
                "UPDATE patch_scheduler_items SET status = 'applied', applied_at = ?1,
                 transaction_id = ?2 WHERE id = ?3",
                params![
                    now,
                    change_result.apply.as_ref().map(|a| a.transaction.id.as_str()),
                    patch.id
                ],
            )?;
            if source_type == "user" || source_type == "direct_manipulation" {
                let _ = record_manual_edit_provenance(
                    db,
                    change_result.apply.as_ref().map(|a| a.transaction.id.as_str()),
                    patch.surface_id.as_deref(),
                    &source_type,
                    "patch_apply",
                    &[op.id.clone()],
                );
            }
        } else {
            db.conn().execute(
                "UPDATE patch_scheduler_items SET status = 'pending_approval' WHERE id = ?1",
                params![patch.id],
            )?;
        }
        applied.push(change_result);
    }

    Ok(ScheduleAndApplyResult {
        scheduled,
        applied,
        superseded,
    })
}

/// ponytail: MAX_PREVIEW_HZ is enforced by the frontend; constant exported for introspection.
pub fn preview_rate_limit_hz() -> u32 {
    MAX_PREVIEW_HZ
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_v2::operations::{AppOperation, OperationTarget};
    use tempfile::tempdir;

    fn test_db() -> crate::db::Database {
        let dir = tempdir().unwrap();
        crate::db::Database::open_path(&dir.path().join("t.db")).unwrap()
    }

    fn simple_op(id: &str, deps: Vec<String>) -> AppOperation {
        AppOperation {
            id: id.into(),
            op_type: "state.set".into(),
            target: OperationTarget::default(),
            base_revision: None,
            payload: serde_json::json!({}),
            transaction_group: None,
            idempotency_key: None,
            depends_on: if deps.is_empty() { None } else { Some(deps) },
            requires_approval: None,
            destructive: None,
            audience: None,
        }
    }

    #[test]
    fn dependency_cycle_reject() {
        let ops = vec![
            simple_op("a", vec!["b".into()]),
            simple_op("b", vec!["a".into()]),
        ];
        assert!(detect_dependency_cycle(&ops).is_some());
        assert!(topological_order(&ops).is_err());
    }

    #[test]
    fn backpressure_limit() {
        let db = test_db();
        for _ in 0..MAX_PATCH_QUEUE {
            enforce_queue_limits(&db, 1).unwrap();
            db.conn()
                .execute(
                    "INSERT INTO patch_scheduler_items (id, operation_id, priority, status, payload_json)
                     VALUES (?1, ?2, 'approved_persistent_change', 'queued', '{}')",
                    params![format!("p-{}", Uuid::new_v4()), format!("op-{}", Uuid::new_v4())],
                )
                .unwrap();
        }
        assert!(enforce_queue_limits(&db, 1).is_err());
    }

    #[test]
    fn agent_cannot_assign_critical_recovery() {
        assert!(validate_agent_priority(PatchPriority::CriticalRecovery).is_err());
        assert!(validate_agent_priority(PatchPriority::DirectUserInteraction).is_err());
        assert!(validate_agent_priority(PatchPriority::ActiveTurnPreview).is_err());
        assert!(validate_agent_priority(PatchPriority::ApprovedPersistentChange).is_ok());
        assert!(validate_agent_priority(PatchPriority::NavigationUpdate).is_ok());
    }

    #[test]
    fn topological_order_respects_deps() {
        let ops = vec![
            simple_op("a", vec![]),
            simple_op("b", vec!["a".into()]),
        ];
        let order = topological_order(&ops).unwrap();
        assert_eq!(order, vec![0, 1]);
    }
}
