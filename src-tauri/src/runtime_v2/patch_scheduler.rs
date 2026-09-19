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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_type: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
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

/// Enforce backpressure limits before inserting `batch_count` new items with `additional_bytes`.
///
/// P0.3 fix: both count and byte checks account for the ENTIRE incoming batch atomically,
/// not just each item individually. This prevents a batch of N items from collectively
/// overflowing a queue that individually passes per-item checks.
pub fn enforce_queue_limits(
    db: &Database,
    batch_count: usize,
    additional_bytes: usize,
) -> DbResult<()> {
    let count = queued_count(db)?;
    if count + batch_count > MAX_PATCH_QUEUE {
        return Err(DbError::Invalid(format!(
            "patch queue full: max {MAX_PATCH_QUEUE} items (current {count}, adding {batch_count})"
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
    // P0.3: pass the full batch count so the limit is checked atomically.
    enforce_queue_limits(db, req.operations.len(), payload_bytes)?;

    let base_seq: i64 = db.conn().query_row(
        "SELECT COALESCE(MAX(sequence_number), 0) FROM patch_scheduler_items",
        [],
        |r| r.get(0),
    )?;

    // P0.4: supersede old previews ONCE for the entire batch before any insertion.
    // This prevents sibling operations in the same batch from mutually superseding each other.
    let preview_batch_id = if req.priority == PatchPriority::ActiveTurnPreview {
        let batch_id = format!("preview-batch-{}", Uuid::new_v4());
        let surface_id = req
            .surface_id
            .as_deref()
            .or_else(|| req.operations.first().and_then(|o| o.target.surface_id.as_deref()));
        let _ = supersede_preview_items(db, surface_id, &batch_id)?;
        Some(batch_id)
    } else {
        None
    };
    let _ = preview_batch_id; // recorded in supersession; not currently persisted per-op    // First pass: generate patch IDs and build operation_id -> patch_id mapping for the batch
    let mut op_to_patch: HashMap<String, String> = HashMap::new();
    let mut batch_plan: Vec<(String, &AppOperation)> = Vec::new();
    for op in &req.operations {
        let patch_id = format!("patch-{}", Uuid::new_v4());
        op_to_patch.insert(op.id.clone(), patch_id.clone());
        batch_plan.push((patch_id, op));
    }

    let now = now_rfc3339();

    // Atomic batch insertion
    db.conn().execute_batch("SAVEPOINT sched_batch")?;
    let mut scheduled = Vec::new();

    let insert_result = (|| -> DbResult<Vec<ScheduledPatch>> {
        for (i, (id, op)) in batch_plan.iter().enumerate() {
            let mut resolved_deps = Vec::new();
            if let Some(deps) = &op.depends_on {
                for dep in deps {
                    if let Some(mapped_patch_id) = op_to_patch.get(dep) {
                        resolved_deps.push(mapped_patch_id.clone());
                    } else {
                        // Check if dep is an existing operation_id or patch_id
                        let existing: Option<String> = db
                            .conn()
                            .query_row(
                                "SELECT id FROM patch_scheduler_items WHERE operation_id = ?1 OR id = ?1 LIMIT 1",
                                [dep],
                                |r| r.get(0),
                            )
                            .ok();
                        if let Some(exist_id) = existing {
                            resolved_deps.push(exist_id);
                        } else {
                            // Keep dep so flush detects missing dependency and fails closed
                            resolved_deps.push(dep.clone());
                        }
                    }
                }
            }

            let depends_json = serde_json::to_string(&resolved_deps)?;
            let payload_json = serde_json::to_string(op)?;
            let surface_id = req
                .surface_id
                .as_deref()
                .or(op.target.surface_id.as_deref());

            db.conn().execute(
                "INSERT INTO patch_scheduler_items (
                    id, operation_id, turn_id, conversation_id, surface_id, priority, status,
                    sequence_number, depends_on_json, payload_json, created_at, model, provider, source_type
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'queued', ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
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
                    now,
                    req.model,
                    req.provider,
                    req.source_type,
                ],
            )?;
            scheduled.push(get_scheduled_patch(db, id)?);
        }
        Ok(scheduled)
    })();

    match insert_result {
        Ok(res) => {
            db.conn().execute_batch("RELEASE sched_batch")?;
            Ok(res)
        }
        Err(e) => {
            let _ = db.conn().execute_batch("ROLLBACK TO sched_batch");
            Err(e)
        }
    }
}

pub fn get_scheduled_patch(db: &Database, id: &str) -> DbResult<ScheduledPatch> {
    db.conn()
        .query_row(
            "SELECT id, operation_id, transaction_id, turn_id, conversation_id, surface_id,
                    priority, status, sequence_number, depends_on_json, payload_json,
                    created_at, applied_at, failed_at, model, provider, source_type
             FROM patch_scheduler_items WHERE id = ?1",
            [id],
            parse_patch_row,
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
        model: row.get(14)?,
        provider: row.get(15)?,
        source_type: row.get(16).ok(),
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
        if let Some(cycle) = dfs(
            op.id.as_str(),
            &graph,
            &mut visiting,
            &mut visited,
            &mut path,
        ) {
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
                let Some(&j) = id_to_idx.get(dep.as_str()) else {
                    return Err(format!(
                        "missing depends_on reference '{dep}' from operation '{}'",
                        op.id
                    ));
                };
                adj[j].push(i);
                in_degree[i] += 1;
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
    let mut query = String::from("SELECT id FROM patch_scheduler_items WHERE status = 'queued'");
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

    // Build dependency graph and topologically sort patches
    let mut patches: HashMap<String, ScheduledPatch> = HashMap::new();
    for patch_id in &ids {
        let patch = get_scheduled_patch(db, patch_id)?;
        patches.insert(patch_id.clone(), patch);
    }

    // Kahn's algorithm for dependency-aware ordering
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();
    let mut missing_dep_patches: HashSet<String> = HashSet::new();

    // Map both patch id and operation_id to patch id in current queued set
    let mut op_to_patch_id: HashMap<String, String> = HashMap::new();
    for (id, patch) in &patches {
        op_to_patch_id.insert(id.clone(), id.clone());
        op_to_patch_id.insert(patch.operation_id.clone(), id.clone());
    }

    for (id, patch) in &patches {
        in_degree.entry(id.clone()).or_insert(0);
        for dep_id in &patch.depends_on {
            let resolved_dep_id = op_to_patch_id
                .get(dep_id)
                .cloned()
                .unwrap_or_else(|| dep_id.clone());
            if patches.contains_key(&resolved_dep_id) {
                dependents
                    .entry(resolved_dep_id.clone())
                    .or_default()
                    .push(id.clone());
                *in_degree.entry(id.clone()).or_insert(0) += 1;
            } else {
                let is_applied: bool = db
                    .conn()
                    .query_row(
                        "SELECT 1 FROM patch_scheduler_items WHERE (id = ?1 OR operation_id = ?1) AND status = 'applied'",
                        [dep_id],
                        |_| Ok(true),
                    )
                    .unwrap_or(false);
                if !is_applied {
                    missing_dep_patches.insert(id.clone());
                }
            }
        }
    }

    let now = now_rfc3339();

    // Transitive failure propagation: any patch depending on a missing or failed patch is dependency_failed
    let mut failed_dep_patches: HashSet<String> = HashSet::new();
    let mut fail_queue: VecDeque<String> = missing_dep_patches.iter().cloned().collect();

    while let Some(failed_id) = fail_queue.pop_front() {
        if let Some(deps) = dependents.get(&failed_id) {
            for dep_id in deps {
                if !missing_dep_patches.contains(dep_id) && failed_dep_patches.insert(dep_id.clone()) {
                    fail_queue.push_back(dep_id.clone());
                }
            }
        }
    }

    // Patches with missing dependencies fail closed! Never executed!
    for id in &missing_dep_patches {
        db.conn().execute(
            "UPDATE patch_scheduler_items SET status = 'failed', failed_at = ?1,
             error_category = 'dependency_missing' WHERE id = ?2 AND status = 'queued'",
            params![now, id],
        )?;
        in_degree.remove(id);
    }

    // Patches whose dependencies failed or were missing fail closed as dependency_failed!
    for id in &failed_dep_patches {
        db.conn().execute(
            "UPDATE patch_scheduler_items SET status = 'failed', failed_at = ?1,
             error_category = 'dependency_failed' WHERE id = ?2 AND status = 'queued'",
            params![now, id],
        )?;
        in_degree.remove(id);
    }

    // Ready queue with deterministic sorting: (sequence_number, id)
    let mut ready: Vec<String> = in_degree
        .iter()
        .filter(|(id, d)| **d == 0 && !missing_dep_patches.contains(*id) && !failed_dep_patches.contains(*id))
        .map(|(id, _)| id.clone())
        .collect();

    ready.sort_by(|a, b| {
        let pa = patches.get(a);
        let pb = patches.get(b);
        let seq_a = pa.map(|p| p.sequence_number).unwrap_or(0);
        let seq_b = pb.map(|p| p.sequence_number).unwrap_or(0);
        seq_a.cmp(&seq_b).then_with(|| a.cmp(b))
    });

    let mut queue: VecDeque<String> = ready.into_iter().collect();
    let mut ordered: Vec<String> = Vec::new();
    while let Some(id) = queue.pop_front() {
        ordered.push(id.clone());
        if let Some(deps) = dependents.get(&id) {
            let mut newly_ready = Vec::new();
            for dep_id in deps {
                if let Some(deg) = in_degree.get_mut(dep_id) {
                    *deg -= 1;
                    if *deg == 0 && !missing_dep_patches.contains(dep_id) && !failed_dep_patches.contains(dep_id) {
                        newly_ready.push(dep_id.clone());
                    }
                }
            }
            newly_ready.sort_by(|a, b| {
                let pa = patches.get(a);
                let pb = patches.get(b);
                let seq_a = pa.map(|p| p.sequence_number).unwrap_or(0);
                let seq_b = pb.map(|p| p.sequence_number).unwrap_or(0);
                seq_a.cmp(&seq_b).then_with(|| a.cmp(b))
            });
            for nr in newly_ready {
                queue.push_back(nr);
            }
        }
    }

    // Any remaining items in in_degree with deg > 0 are true dependency cycles! Fail closed!
    for (id, deg) in &in_degree {
        if *deg > 0 && !missing_dep_patches.contains(id) && !failed_dep_patches.contains(id) {
            db.conn().execute(
                "UPDATE patch_scheduler_items SET status = 'failed', failed_at = ?1,
                 error_category = 'dependency_cycle' WHERE id = ?2 AND status = 'queued'",
                params![now, id],
            )?;
        }
    }

    let mut applied = Vec::new();
    for patch_id in ordered {
        let Some(patch) = patches.get(&patch_id) else {
            continue;
        };
        let op: AppOperation = serde_json::from_value(patch.payload.clone())
            .map_err(|e| DbError::Invalid(e.to_string()))?;

        let effective_source_type = patch.source_type.as_deref().unwrap_or("agent");
        let effective_approval = if effective_source_type == "agent" {
            false
        } else {
            approval_granted
        };

        let result = apply_change(
            db,
            bus.as_deref_mut(),
            ChangeRequest {
                proposal_id: None,
                conversation_id: patch.conversation_id.clone(),
                project_id: None,
                turn_id: patch.turn_id.clone(),
                summary: format!("Patch {}", patch.operation_id),
                operations: vec![op],
                silent: patch.priority == PatchPriority::ActiveTurnPreview,
                source_type: effective_source_type.into(),
                provider: patch.provider.clone(),
                model: patch.model.clone(),
                require_approval: false,
                approval_granted: effective_approval,
            },
        )
        .map_err(|e| DbError::Invalid(e.user_message()))?;

        if result.is_committed() {
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
        } else if result.proposal_id.is_some() {
            db.conn().execute(
                "UPDATE patch_scheduler_items SET status = 'pending_approval', failed_at = ?1 WHERE id = ?2",
                params![now, patch_id],
            )?;
            applied.push(result);
        } else {
            db.conn().execute(
                "UPDATE patch_scheduler_items SET status = 'failed', failed_at = ?1,
                 error_category = 'apply' WHERE id = ?2",
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
        let surface_id = req.surface_id.as_deref().or_else(|| {
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
    let order = topological_order(&req.operations).map_err(DbError::Invalid)?;

    // One kernel transaction for the whole ordered batch — never leave earlier
    // ops committed when a later op fails (Partial Update atomicity).
    let ordered_ops: Vec<_> = order
        .iter()
        .map(|&idx| req.operations[idx].clone())
        .collect();
    let now = now_rfc3339();
    let source_type = req.source_type.clone();
    let op_ids: Vec<String> = ordered_ops.iter().map(|o| o.id.clone()).collect();

    let result = apply_change(
        db,
        bus.as_deref_mut(),
        ChangeRequest {
            proposal_id: None,
            conversation_id: req.conversation_id.clone(),
            project_id: None,
            turn_id: req.turn_id.clone(),
            summary: format!("Patch batch ({})", ordered_ops.len()),
            operations: ordered_ops,
            silent: req.priority == PatchPriority::ActiveTurnPreview,
            source_type: source_type.clone(),
            provider: req.provider.clone(),
            model: req.model.clone(),
            require_approval: false,
            approval_granted,
        },
    );

    let change_result = match result {
        Ok(r) => r,
        Err(e) => {
            for patch in &scheduled {
                let _ = db.conn().execute(
                    "UPDATE patch_scheduler_items SET status = 'failed', failed_at = ?1,
                     error_category = 'apply' WHERE id = ?2",
                    params![now, patch.id],
                );
            }
            return Err(DbError::Invalid(e.user_message()));
        }
    };

    if change_result.is_committed() {
        let txn_id = change_result
            .apply
            .as_ref()
            .map(|a| a.transaction.id.as_str());
        for patch in &scheduled {
            db.conn().execute(
                "UPDATE patch_scheduler_items SET status = 'applied', applied_at = ?1,
                 transaction_id = ?2 WHERE id = ?3",
                params![now, txn_id, patch.id],
            )?;
        }
        if source_type == "user" || source_type == "direct_manipulation" {
            let surface_id = scheduled.first().and_then(|p| p.surface_id.as_deref());
            let _ = record_manual_edit_provenance(
                db,
                txn_id,
                surface_id,
                &source_type,
                "patch_apply",
                &op_ids,
            );
        }
    } else if change_result.proposal_id.is_some() {
        for patch in &scheduled {
            db.conn().execute(
                "UPDATE patch_scheduler_items SET status = 'pending_approval' WHERE id = ?1",
                params![patch.id],
            )?;
        }
    } else {
        for patch in &scheduled {
            let _ = db.conn().execute(
                "UPDATE patch_scheduler_items SET status = 'failed', failed_at = ?1,
                 error_category = 'apply' WHERE id = ?2",
                params![now, patch.id],
            );
        }
    }

    Ok(ScheduleAndApplyResult {
        scheduled,
        applied: vec![change_result],
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
            enforce_queue_limits(&db, 1, 1).unwrap();
            db.conn()
                .execute(
                    "INSERT INTO patch_scheduler_items (id, operation_id, priority, status, payload_json)
                     VALUES (?1, ?2, 'approved_persistent_change', 'queued', '{}')",
                    params![format!("p-{}", Uuid::new_v4()), format!("op-{}", Uuid::new_v4())],
                )
                .unwrap();
        }
        // Queue is now full. Adding 1 more must fail.
        assert!(enforce_queue_limits(&db, 1, 1).is_err());
    }

    #[test]
    fn backpressure_batch_count_overflow() {
        // P0.3: batch of N items must be rejected atomically even if queue has MAX-1 items.
        let db = test_db();
        // Fill queue to MAX - 1.
        for _ in 0..MAX_PATCH_QUEUE - 1 {
            db.conn()
                .execute(
                    "INSERT INTO patch_scheduler_items (id, operation_id, priority, status, payload_json)
                     VALUES (?1, ?2, 'approved_persistent_change', 'queued', '{}')",
                    params![format!("p-{}", Uuid::new_v4()), format!("op-{}", Uuid::new_v4())],
                )
                .unwrap();
        }
        // One item individually passes.
        assert!(enforce_queue_limits(&db, 1, 0).is_ok());
        // But a batch of 3 must fail since MAX-1 + 3 > MAX.
        assert!(
            enforce_queue_limits(&db, 3, 0).is_err(),
            "batch exceeding MAX_PATCH_QUEUE must be rejected atomically"
        );
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
        let ops = vec![simple_op("a", vec![]), simple_op("b", vec!["a".into()])];
        let order = topological_order(&ops).unwrap();
        assert_eq!(order, vec![0, 1]);
    }

    #[test]
    fn transitive_dependency_failure_propagation() {
        let mut db = test_db();
        let ops = vec![
            simple_op("op_a", vec!["op_nonexistent".into()]),
            simple_op("op_b", vec!["op_a".into()]),
            simple_op("op_c", vec!["op_b".into()]),
        ];
        let req = ScheduleRequest {
            conversation_id: Some("conv-sched-1".into()),
            turn_id: Some("turn-1".into()),
            surface_id: None,
            priority: PatchPriority::ApprovedPersistentChange,
            operations: ops,
            source_type: "user".into(),
            from_agent: false,
            model: None,
            provider: None,
        };
        let scheduled = schedule_patches(&mut db, &req).unwrap();
        assert_eq!(scheduled.len(), 3);

        let patch_a_id = &scheduled[0].id;
        let patch_b_id = &scheduled[1].id;
        let patch_c_id = &scheduled[2].id;

        // Flush the scheduler
        let _ = flush_scheduler(&mut db, &mut None, Some("conv-sched-1"), "user", true).unwrap();

        // Check statuses and error_categories
        let patch_a = get_scheduled_patch(&db, patch_a_id).unwrap();
        assert_eq!(patch_a.status, "failed");
        let cat_a: String = db
            .conn()
            .query_row(
                "SELECT error_category FROM patch_scheduler_items WHERE id = ?1",
                [patch_a_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cat_a, "dependency_missing");

        let patch_b = get_scheduled_patch(&db, patch_b_id).unwrap();
        assert_eq!(patch_b.status, "failed");
        let cat_b: String = db
            .conn()
            .query_row(
                "SELECT error_category FROM patch_scheduler_items WHERE id = ?1",
                [patch_b_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            cat_b, "dependency_failed",
            "Dependent of missing patch must be dependency_failed, not dependency_cycle"
        );

        let patch_c = get_scheduled_patch(&db, patch_c_id).unwrap();
        assert_eq!(patch_c.status, "failed");
        let cat_c: String = db
            .conn()
            .query_row(
                "SELECT error_category FROM patch_scheduler_items WHERE id = ?1",
                [patch_c_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            cat_c, "dependency_failed",
            "Transitive dependent must be dependency_failed, not dependency_cycle"
        );
    }
}
