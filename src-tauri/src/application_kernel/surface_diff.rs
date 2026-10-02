//! Authoritative ApplicationSpec snapshot + stable-identity surface diff.
//!
//! Model-facing UpdateSurface still carries a proposed ToolDefinition.
//! At DB-backed compile time we compare against the trusted current surface
//! and emit the smallest safe component/state operation set.
//!
//! Identity rules (intentional):
//! - existing component IDs are preserved
//! - new components receive their proposed IDs (or generated ones upstream)
//! - prop/action changes never mint a new identity
//! - type change ⇒ component.replace (new identity semantics)
//! - unrelated siblings are never remounted by this compiler

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::ai::response_schema::{ToolComponent, ToolDefinition};
use crate::db::Database;
use crate::runtime_v2::operations::{AppOperation, OperationTarget};
use crate::runtime_v2::software_document::SoftwareDocument;
use crate::runtime_v2::surfaces::get_surface;

use super::compiler::CompiledChange;
use super::errors::KernelError;
use super::lineage::resolve_application_surface;
use super::COMPILER_VERSION;

/// Compiler-visible snapshot of one application surface (no secrets / approvals).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationSpec {
    pub application_id: String,
    pub revision: i64,
    pub surface_id: String,
    pub tool: ToolDefinition,
}

/// Load the trusted current ApplicationSpec for a tool/application id from SQLite.
///
/// Surface identity comes from DB lineage (`resolve_application_surface`), not
/// from inventing `surf-{application_id}`.
pub fn load_application_spec(
    db: &Database,
    application_id: &str,
) -> Result<ApplicationSpec, KernelError> {
    // Preserve Protected / Db categories — do not collapse into Validation.
    let lineage = match resolve_application_surface(db, application_id, None, None) {
        Ok(lineage) => lineage,
        Err(KernelError::Validation(msg)) => {
            return Err(KernelError::Validation(format!(
                "cannot load ApplicationSpec for '{application_id}': {msg}"
            )));
        }
        Err(other) => return Err(other),
    };
    let surface_id = lineage.surface_id;
    let surface = get_surface(db, &surface_id).map_err(|e| match e {
        crate::db::DbError::NotFound(_) => KernelError::Validation(format!(
            "cannot load ApplicationSpec for '{application_id}': surface '{surface_id}' not found"
        )),
        other => KernelError::Db(other),
    })?;
    let doc = SoftwareDocument::from_value(&surface.definition).map_err(|e| {
        KernelError::Validation(format!(
            "surface '{surface_id}' definition is not a valid ApplicationSpec: {e}"
        ))
    })?;
    let mut tool = flatten_synthetic_section_containers(doc.to_tool_definition());
    // Authority identity comes from the DB surface/tool binding, not model-proposed ids.
    if tool.id.trim().is_empty() {
        tool.id = application_id.to_string();
    }
    Ok(ApplicationSpec {
        application_id: application_id.to_string(),
        revision: surface.current_revision,
        surface_id,
        tool,
    })
}

#[derive(Debug, Clone)]
struct IndexedComponent {
    component: ToolComponent,
    parent_id: Option<String>,
    index: usize,
}

fn index_tree(
    components: &[ToolComponent],
    parent_id: Option<&str>,
    out: &mut BTreeMap<String, IndexedComponent>,
) {
    for (index, c) in components.iter().enumerate() {
        out.insert(
            c.id.clone(),
            IndexedComponent {
                component: c.clone(),
                parent_id: parent_id.map(|s| s.to_string()),
                index,
            },
        );
        if let Some(children) = c.children.as_ref() {
            index_tree(children, Some(&c.id), out);
        }
    }
}

fn new_op(op_type: &str, target: OperationTarget, payload: Value) -> AppOperation {
    AppOperation {
        id: format!("op-{}", Uuid::new_v4()),
        op_type: op_type.into(),
        target,
        base_revision: None,
        transaction_group: Some("compiled".into()),
        idempotency_key: Some(format!("ik-{}", Uuid::new_v4())),
        depends_on: None,
        payload,
        requires_approval: None,
        destructive: None,
        audience: None,
    }
}

fn component_payload(c: &ToolComponent) -> Value {
    serde_json::to_value(c).unwrap_or_else(|_| json!({ "id": c.id }))
}

fn props_equal(a: &Option<Value>, b: &Option<Value>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => x == y,
        (None, Some(Value::Object(m))) if m.is_empty() => true,
        (Some(Value::Object(m)), None) if m.is_empty() => true,
        _ => false,
    }
}

fn top_level_metadata_equal(current: &ToolDefinition, proposed: &ToolDefinition) -> bool {
    if current.name != proposed.name
        || current.description != proposed.description
        || current.layout != proposed.layout
    {
        return false;
    }
    // Empty proposed contracts mean "leave inferred/current contracts alone".
    // Component inserts with valueKey still infer model-writable contracts at apply.
    if !proposed.state_contracts.is_empty() && current.state_contracts != proposed.state_contracts {
        return false;
    }
    if !proposed.action_contracts.is_empty()
        && current.action_contracts != proposed.action_contracts
    {
        return false;
    }
    if proposed.interactive.is_some() && current.interactive != proposed.interactive {
        return false;
    }
    true
}

/// Diff current trusted tool vs proposed UpdateSurface tool into minimal ops.
///
/// Falls back to `tool.full_replace` when top-level metadata/contracts diverge
/// in ways component ops cannot express, or when the diff would remove every
/// existing component (treat as redesign).
pub fn diff_surface_update(
    current: &ApplicationSpec,
    proposed: &ToolDefinition,
    tool_id: &str,
    change_summary: Option<&str>,
    base_revision: Option<i64>,
) -> Result<CompiledChange, KernelError> {
    crate::security::assert_not_protected(tool_id).map_err(KernelError::Protected)?;
    crate::security::assert_not_protected(&proposed.id).map_err(KernelError::Protected)?;
    if proposed.id != tool_id && !proposed.id.is_empty() {
        return Err(KernelError::Validation(
            "UpdateSurface tool.id must match toolId".into(),
        ));
    }

    let surface_id = current.surface_id.clone();
    let summary = change_summary
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("Update {}", proposed.name));

    if !top_level_metadata_equal(&current.tool, proposed) {
        return Ok(full_replace_change(
            tool_id,
            &surface_id,
            proposed,
            &summary,
            base_revision,
        ));
    }

    let mut cur_idx = BTreeMap::new();
    let mut prop_idx = BTreeMap::new();
    index_tree(&current.tool.components, None, &mut cur_idx);
    index_tree(&proposed.components, None, &mut prop_idx);

    let cur_ids: BTreeSet<_> = cur_idx.keys().cloned().collect();
    let prop_ids: BTreeSet<_> = prop_idx.keys().cloned().collect();

    // Redesign guard: removing all existing components → full replace.
    if !cur_ids.is_empty() && cur_ids.is_disjoint(&prop_ids) {
        return Ok(full_replace_change(
            tool_id,
            &surface_id,
            proposed,
            &summary,
            base_revision,
        ));
    }

    let mut moves = Vec::new();
    let mut updates = Vec::new();
    let mut inserts = Vec::new();
    let mut removals = Vec::new();

    // Shared ids: move/replace/update before inserts; removals last so children can
    // be reparented off wrapper nodes persisted by surface.create (e.g. section-main).
    for id in cur_ids.intersection(&prop_ids) {
        let before = cur_idx.get(id).expect("indexed");
        let after = prop_idx.get(id).expect("indexed");

        if before.component.component_type != after.component.component_type {
            let mut op = new_op(
                "component.replace",
                OperationTarget {
                    surface_id: Some(surface_id.clone()),
                    tool_id: Some(tool_id.to_string()),
                    component_id: Some(id.clone()),
                    ..Default::default()
                },
                json!({ "component": component_payload(&after.component) }),
            );
            op.base_revision = base_revision;
            updates.push(op);
            continue;
        }

        if before.parent_id != after.parent_id || before.index != after.index {
            let mut op = new_op(
                "component.move",
                OperationTarget {
                    surface_id: Some(surface_id.clone()),
                    tool_id: Some(tool_id.to_string()),
                    component_id: Some(id.clone()),
                    ..Default::default()
                },
                json!({
                    "toParentId": after.parent_id,
                    "index": after.index,
                }),
            );
            op.base_revision = base_revision;
            moves.push(op);
        }

        if !props_equal(&before.component.props, &after.component.props)
            || before.component.value_key != after.component.value_key
            || before.component.layout_role != after.component.layout_role
            || before.component.col_span != after.component.col_span
            || before.component.row_span != after.component.row_span
        {
            let mut payload = json!({
                "props": after.component.props.clone().unwrap_or_else(|| json!({})),
            });
            if let Some(ref vk) = after.component.value_key {
                payload["valueKey"] = json!(vk);
            }
            let mut op = new_op(
                "component.update_props",
                OperationTarget {
                    surface_id: Some(surface_id.clone()),
                    tool_id: Some(tool_id.to_string()),
                    component_id: Some(id.clone()),
                    ..Default::default()
                },
                payload,
            );
            op.base_revision = base_revision;
            updates.push(op);
        }

        if before.component.actions != after.component.actions {
            let mut op = new_op(
                "component.update_actions",
                OperationTarget {
                    surface_id: Some(surface_id.clone()),
                    tool_id: Some(tool_id.to_string()),
                    component_id: Some(id.clone()),
                    ..Default::default()
                },
                json!({
                    "actions": after.component.actions.clone().unwrap_or_default(),
                }),
            );
            op.base_revision = base_revision;
            updates.push(op);
        }
    }

    let mut insert_order: Vec<&str> = prop_ids.difference(&cur_ids).map(|s| s.as_str()).collect();
    insert_order.sort_by_key(|id| {
        prop_idx
            .get(*id)
            .map(|n| (n.parent_id.clone().unwrap_or_default(), n.index))
            .unwrap_or_default()
    });
    for id in insert_order {
        let node = prop_idx.get(id).expect("indexed");
        if let Some(ref pid) = node.parent_id {
            if !cur_ids.contains(pid) {
                continue;
            }
        }
        let mut op = new_op(
            "component.insert",
            OperationTarget {
                surface_id: Some(surface_id.clone()),
                tool_id: Some(tool_id.to_string()),
                parent_id: node.parent_id.clone(),
                component_id: Some(node.component.id.clone()),
                ..Default::default()
            },
            json!({
                "component": component_payload(&node.component),
                "index": node.index,
            }),
        );
        op.base_revision = base_revision;
        inserts.push(op);
    }

    for id in cur_ids.difference(&prop_ids) {
        let mut op = new_op(
            "component.remove",
            OperationTarget {
                surface_id: Some(surface_id.clone()),
                tool_id: Some(tool_id.to_string()),
                component_id: Some(id.clone()),
                ..Default::default()
            },
            json!({}),
        );
        op.base_revision = base_revision;
        removals.push(op);
    }

    let mut operations = Vec::new();
    operations.extend(moves);
    // Inserts before action/props updates so new valueKey components exist when
    // validators infer state contracts referenced by update_actions.
    operations.extend(inserts);
    operations.extend(updates);
    operations.extend(removals);

    if operations.is_empty() {
        // No structural delta — still emit a no-op-safe metadata touch via full replace
        // only when the proposed tool JSON differs in unknown ways; otherwise empty is ok
        // for callers that already migrated data models separately.
        return Ok(CompiledChange {
            compiler_version: COMPILER_VERSION.into(),
            operations: vec![],
            summary: format!("{summary} (no component changes)"),
            rollback_hint: "none".into(),
        });
    }

    Ok(CompiledChange {
        compiler_version: COMPILER_VERSION.into(),
        operations,
        summary,
        rollback_hint: "component.* reverse ops / surface.restore previous revision".into(),
    })
}

/// `SoftwareDocument::to_tool_definition` may inject synthetic `section-*` containers
/// for rich layouts. Those ids are not persisted as components — flatten before diff.
fn flatten_synthetic_section_containers(mut tool: ToolDefinition) -> ToolDefinition {
    let mut flat = Vec::new();
    for component in tool.components.drain(..) {
        let is_synthetic = component.component_type == "container"
            && component
                .props
                .as_ref()
                .and_then(|p| p.get("syntheticSection"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
        if is_synthetic {
            if let Some(children) = component.children {
                flat.extend(children);
            }
        } else {
            flat.push(component);
        }
    }
    tool.components = flat;
    tool
}

fn full_replace_change(
    tool_id: &str,
    surface_id: &str,
    tool: &ToolDefinition,
    summary: &str,
    base_revision: Option<i64>,
) -> CompiledChange {
    let mut op = new_op(
        "tool.full_replace",
        OperationTarget {
            surface_id: Some(surface_id.to_string()),
            tool_id: Some(tool_id.to_string()),
            surface_type: Some("tool".into()),
            placement: Some("tool_canvas".into()),
            ..Default::default()
        },
        json!({
            "action": "update",
            "tool": tool,
            "targetToolId": tool_id,
            "changeSummary": summary,
        }),
    );
    op.base_revision = base_revision;
    CompiledChange {
        compiler_version: COMPILER_VERSION.into(),
        operations: vec![op],
        summary: summary.to_string(),
        rollback_hint: "surface.restore previous revision".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::plan_fixtures::{task_tracker_add_due_dates_plan, task_tracker_create_plan};
    use crate::application_kernel::compiler::{compile, ChangeIntent};
    use crate::db::Database;

    fn tool_from_create() -> ToolDefinition {
        match task_tracker_create_plan().intents.into_iter().next() {
            Some(ChangeIntent::CreateSurface { tool, .. }) => tool,
            _ => panic!("create plan must start with CreateSurface"),
        }
    }

    fn tool_from_evolve() -> ToolDefinition {
        for intent in task_tracker_add_due_dates_plan(Some(1)).intents {
            if let ChangeIntent::UpdateSurface { tool, .. } = intent {
                return tool;
            }
        }
        panic!("evolve plan missing UpdateSurface");
    }

    #[test]
    fn due_date_evolution_emits_granular_ops_not_full_replace() {
        let current_tool = tool_from_create();
        let proposed = tool_from_evolve();
        let spec = ApplicationSpec {
            application_id: "tool-task-tracker".into(),
            revision: 1,
            surface_id: "surf-tool-task-tracker".into(),
            tool: current_tool,
        };
        let compiled = diff_surface_update(
            &spec,
            &proposed,
            "tool-task-tracker",
            Some("Add due dates"),
            Some(1),
        )
        .unwrap();
        assert!(
            compiled
                .operations
                .iter()
                .all(|o| o.op_type != "tool.full_replace"),
            "expected granular ops, got {:?}",
            compiled
                .operations
                .iter()
                .map(|o| o.op_type.as_str())
                .collect::<Vec<_>>()
        );
        assert!(compiled
            .operations
            .iter()
            .any(|o| o.op_type == "component.insert"
                && o.target.component_id.as_deref() == Some("tm-new-due")));
        assert!(compiled
            .operations
            .iter()
            .any(|o| o.op_type == "component.update_props"
                && o.target.component_id.as_deref() == Some("tm-list")));
        assert!(compiled
            .operations
            .iter()
            .any(|o| o.op_type == "component.update_actions"
                && o.target.component_id.as_deref() == Some("tm-add-btn")));
        // Stable siblings must not be removed/reinserted.
        assert!(!compiled.operations.iter().any(|o| {
            o.op_type == "component.remove"
                && o.target.component_id.as_deref() == Some("tm-heading")
        }));
        assert!(!compiled.operations.iter().any(|o| {
            o.op_type == "component.insert"
                && o.target.component_id.as_deref() == Some("tm-heading")
        }));
    }

    #[test]
    fn inspection_update_surface_still_full_replaces_without_spec() {
        let proposed = tool_from_evolve();
        let c = compile(ChangeIntent::UpdateSurface {
            tool_id: "tool-task-tracker".into(),
            tool: proposed,
            change_summary: Some("Add due dates".into()),
            base_revision: Some(1),
        })
        .unwrap();
        assert_eq!(c.operations[0].op_type, "tool.full_replace");
    }

    #[test]
    fn load_application_spec_rejects_unknown_application() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open_path(&dir.path().join("spec.db")).unwrap();
        let err = load_application_spec(&db, "tool-does-not-exist").unwrap_err();
        assert!(matches!(err, KernelError::Validation(_)));
    }

    #[test]
    fn load_application_spec_preserves_protected_category() {
        use crate::db::{create_conversation, DEFAULT_WORKSPACE_ID};

        const PROTECTED_APP: &str = "core.settings.ai";

        let dir = tempfile::tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("spec-protected.db")).unwrap();
        let conv =
            create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Protected spec", None).unwrap();
        // Bypass bind_surface_tool_id (it rejects protected ids) so we can prove
        // load_application_spec preserves KernelError::Protected rather than collapsing
        // it into Validation.
        db.conn()
            .execute(
                "INSERT INTO tools (id, workspace_id, name, description, layout, definition_json, current_version, created_at, updated_at)
                 VALUES (?1, ?2, ?1, '', 'stack', '{}', 1, datetime('now'), datetime('now'))",
                rusqlite::params![PROTECTED_APP, DEFAULT_WORKSPACE_ID],
            )
            .unwrap();
        let surf = crate::runtime_v2::surfaces::create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Protected",
            &json!({
                "id": "doc-p",
                "name": "Protected",
                "layout": "stack",
                "components": [{"id": "t", "type": "text", "props": {"text": "x"}}]
            }),
            &[],
        )
        .unwrap();
        db.conn()
            .execute(
                "UPDATE surfaces SET tool_id = ?1 WHERE id = ?2",
                rusqlite::params![PROTECTED_APP, surf.id],
            )
            .unwrap();

        let err = load_application_spec(&db, PROTECTED_APP).unwrap_err();
        assert!(
            matches!(err, KernelError::Protected(_)),
            "protected ids must not collapse to Validation; got {err:?}"
        );
    }

    #[test]
    fn load_application_spec_resolves_bound_uuid_not_canonical_surf_prefix() {
        use crate::db::{create_conversation, DEFAULT_WORKSPACE_ID};
        use crate::runtime_v2::surfaces::{bind_surface_tool_id, get_surface};

        const APP_ID: &str = "tool-task-tracker";

        let dir = tempfile::tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("spec-lineage.db")).unwrap();
        let conv =
            create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Spec lineage", None).unwrap();
        db.conn()
            .execute(
                "INSERT INTO tools (id, workspace_id, name, description, layout, definition_json, current_version, created_at, updated_at)
                 VALUES (?1, ?2, ?1, '', 'stack', '{}', 1, datetime('now'), datetime('now'))",
                rusqlite::params![APP_ID, DEFAULT_WORKSPACE_ID],
            )
            .unwrap();
        let surf = crate::runtime_v2::surfaces::create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Task Tracker",
            &json!({
                "id": "doc-tt",
                "name": "Task Tracker",
                "layout": "stack",
                "components": [{"id": "t", "type": "text", "props": {"text": "hi"}}]
            }),
            &[],
        )
        .unwrap();
        bind_surface_tool_id(&mut db, &surf.id, APP_ID).unwrap();

        let canonical = format!("surf-{APP_ID}");
        assert_ne!(surf.id, canonical);
        assert!(get_surface(&db, &canonical).is_err());

        let spec = load_application_spec(&db, APP_ID).expect("bound inline surface");
        assert_eq!(spec.surface_id, surf.id);
        assert_eq!(spec.application_id, APP_ID);
        assert_eq!(
            spec.revision,
            get_surface(&db, &surf.id).unwrap().current_revision
        );
    }

    #[test]
    fn disjoint_component_ids_falls_back_to_full_replace() {
        let current_tool = tool_from_create();
        let mut proposed = tool_from_evolve();
        // Replace every stable id so granular diff would remount the whole tree.
        fn reid(c: &mut ToolComponent, prefix: &str, n: &mut usize) {
            c.id = format!("{prefix}-{}", *n);
            *n += 1;
            if let Some(children) = c.children.as_mut() {
                for child in children {
                    reid(child, prefix, n);
                }
            }
        }
        let mut n = 0;
        for c in proposed.components.iter_mut() {
            reid(c, "new", &mut n);
        }
        let spec = ApplicationSpec {
            application_id: "tool-task-tracker".into(),
            revision: 1,
            surface_id: "surf-tool-task-tracker".into(),
            tool: current_tool,
        };
        let compiled = diff_surface_update(
            &spec,
            &proposed,
            "tool-task-tracker",
            Some("Redesign"),
            Some(1),
        )
        .unwrap();
        assert_eq!(compiled.operations.len(), 1);
        assert_eq!(compiled.operations[0].op_type, "tool.full_replace");
    }
}
