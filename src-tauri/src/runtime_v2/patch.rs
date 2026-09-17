//! Fine-grained component-tree patching with revision checks.

use serde_json::{json, Map, Value};

use super::limits::{MAX_COMPONENTS_PER_SURFACE, MAX_COMPONENT_TREE_DEPTH};
use crate::ai::ToolComponent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchConflict {
    StaleRevision { expected: i64, actual: i64 },
    MissingComponent { component_id: String },
    DuplicateId { component_id: String },
    DepthExceeded,
    ComponentLimitExceeded,
    InvalidPayload(String),
}

impl std::fmt::Display for PatchConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StaleRevision { expected, actual } => {
                write!(f, "stale revision: base {expected}, current {actual}")
            }
            Self::MissingComponent { component_id } => {
                write!(f, "component not found: {component_id}")
            }
            Self::DuplicateId { component_id } => {
                write!(f, "duplicate component id: {component_id}")
            }
            Self::DepthExceeded => write!(f, "component tree depth exceeded"),
            Self::ComponentLimitExceeded => write!(f, "component count exceeded"),
            Self::InvalidPayload(m) => write!(f, "invalid patch payload: {m}"),
        }
    }
}

pub type PatchResult<T> = Result<T, PatchConflict>;

fn walk_mut<F>(nodes: &mut [ToolComponent], f: &mut F) -> bool
where
    F: FnMut(&mut ToolComponent) -> bool,
{
    for node in nodes {
        if f(node) {
            return true;
        }
        if let Some(children) = node.children.as_mut() {
            if walk_mut(children, f) {
                return true;
            }
        }
    }
    false
}

fn find_mut<'a>(nodes: &'a mut [ToolComponent], id: &str) -> Option<&'a mut ToolComponent> {
    let mut found: Option<*mut ToolComponent> = None;
    walk_mut(nodes, &mut |n| {
        if n.id == id {
            found = Some(n as *mut ToolComponent);
            true
        } else {
            false
        }
    });
    found.map(|p| unsafe { &mut *p })
}

fn count_components(nodes: &[ToolComponent]) -> usize {
    let mut n = 0;
    fn walk(nodes: &[ToolComponent], n: &mut usize) {
        for c in nodes {
            *n += 1;
            if let Some(ch) = &c.children {
                walk(ch, n);
            }
        }
    }
    walk(nodes, &mut n);
    n
}

fn max_depth(nodes: &[ToolComponent], depth: usize) -> usize {
    let mut max = depth;
    for c in nodes {
        if let Some(ch) = &c.children {
            max = max.max(max_depth(ch, depth + 1));
        } else {
            max = max.max(depth);
        }
    }
    max
}

fn collect_ids(
    nodes: &[ToolComponent],
    ids: &mut std::collections::HashSet<String>,
) -> Result<(), PatchConflict> {
    for c in nodes {
        if !ids.insert(c.id.clone()) {
            return Err(PatchConflict::DuplicateId {
                component_id: c.id.clone(),
            });
        }
        if let Some(ch) = &c.children {
            collect_ids(ch, ids)?;
        }
    }
    Ok(())
}

/// Find a component by id in an immutable tree.
pub fn find_component<'a>(nodes: &'a [ToolComponent], id: &str) -> Option<&'a ToolComponent> {
    for n in nodes {
        if n.id == id {
            return Some(n);
        }
        if let Some(ch) = &n.children {
            if let Some(found) = find_component(ch, id) {
                return Some(found);
            }
        }
    }
    None
}

/// Find a component by id in a mutable tree.
pub fn find_component_mut<'a>(
    nodes: &'a mut [ToolComponent],
    id: &str,
) -> Option<&'a mut ToolComponent> {
    for n in nodes {
        if n.id == id {
            return Some(n);
        }
        if let Some(ch) = n.children.as_mut() {
            if let Some(found) = find_component_mut(ch, id) {
                return Some(found);
            }
        }
    }
    None
}

pub fn assert_tree_limits(components: &[ToolComponent]) -> PatchResult<()> {
    super::packs::validate_tool_components(components).map_err(|e| {
        if e.contains("duplicate component id:") {
            let id = e.split(": ").nth(1).unwrap_or(&e).trim().to_string();
            PatchConflict::DuplicateId { component_id: id }
        } else if e.contains("depth") {
            PatchConflict::DepthExceeded
        } else if e.contains("component count") {
            PatchConflict::ComponentLimitExceeded
        } else {
            PatchConflict::InvalidPayload(e)
        }
    })
}

pub fn check_revision(base: Option<i64>, current: i64) -> PatchResult<()> {
    if let Some(b) = base {
        if b != current {
            return Err(PatchConflict::StaleRevision {
                expected: b,
                actual: current,
            });
        }
    }
    Ok(())
}

pub fn update_props(
    components: &mut [ToolComponent],
    component_id: &str,
    props_patch: &Value,
) -> PatchResult<()> {
    let node =
        find_mut(components, component_id).ok_or_else(|| PatchConflict::MissingComponent {
            component_id: component_id.into(),
        })?;
    let patch_obj = props_patch
        .as_object()
        .ok_or_else(|| PatchConflict::InvalidPayload("props patch must be an object".into()))?;
    let mut current = match node.props.take() {
        Some(Value::Object(m)) => m,
        Some(_) | None => Map::new(),
    };
    for (k, v) in patch_obj {
        current.insert(k.clone(), v.clone());
    }
    node.props = Some(Value::Object(current));
    Ok(())
}

pub fn remove_component(
    components: &mut Vec<ToolComponent>,
    component_id: &str,
) -> PatchResult<()> {
    if let Some(idx) = components.iter().position(|c| c.id == component_id) {
        components.remove(idx);
        return Ok(());
    }
    for c in components.iter_mut() {
        if let Some(ch) = c.children.as_mut() {
            if remove_component(ch, component_id).is_ok() {
                return Ok(());
            }
        }
    }
    Err(PatchConflict::MissingComponent {
        component_id: component_id.into(),
    })
}

pub fn insert_component(
    components: &mut Vec<ToolComponent>,
    parent_id: Option<&str>,
    component: ToolComponent,
    index: Option<usize>,
) -> PatchResult<()> {
    let mut ids = std::collections::HashSet::new();
    collect_ids(components, &mut ids)?;
    if ids.contains(&component.id) {
        return Err(PatchConflict::DuplicateId {
            component_id: component.id.clone(),
        });
    }
    match parent_id {
        None => {
            let i = index.unwrap_or(components.len()).min(components.len());
            components.insert(i, component);
        }
        Some(pid) => {
            let parent =
                find_mut(components, pid).ok_or_else(|| PatchConflict::MissingComponent {
                    component_id: pid.into(),
                })?;
            let children = parent.children.get_or_insert_with(Vec::new);
            let i = index.unwrap_or(children.len()).min(children.len());
            children.insert(i, component);
        }
    }
    assert_tree_limits(components)?;
    Ok(())
}

pub fn replace_component(
    components: &mut [ToolComponent],
    component_id: &str,
    replacement: ToolComponent,
) -> PatchResult<()> {
    let node =
        find_mut(components, component_id).ok_or_else(|| PatchConflict::MissingComponent {
            component_id: component_id.into(),
        })?;
    if replacement.id != component_id && replacement.id != node.id {
        // Allow same id only to avoid collisions.
        return Err(PatchConflict::InvalidPayload(
            "replacement must keep the same component id".into(),
        ));
    }
    *node = replacement;
    Ok(())
}

pub fn update_children(
    components: &mut [ToolComponent],
    component_id: &str,
    children: Vec<ToolComponent>,
) -> PatchResult<()> {
    let node =
        find_mut(components, component_id).ok_or_else(|| PatchConflict::MissingComponent {
            component_id: component_id.into(),
        })?;
    node.children = Some(children);
    Ok(())
}

/// Apply a named component operation from a JSON payload.
pub fn apply_component_op(
    components: &mut Vec<ToolComponent>,
    op_type: &str,
    component_id: Option<&str>,
    parent_id: Option<&str>,
    base_revision: Option<i64>,
    current_revision: i64,
    payload: &Value,
) -> PatchResult<()> {
    check_revision(base_revision, current_revision)?;
    match op_type {
        "component.update_props" => {
            let id = component_id
                .ok_or_else(|| PatchConflict::InvalidPayload("componentId required".into()))?;
            let props_patch = payload
                .get("props")
                .cloned()
                .unwrap_or_else(|| payload.clone());
            update_props(components, id, &props_patch)?;
        }
        "component.remove" => {
            let id = component_id
                .ok_or_else(|| PatchConflict::InvalidPayload("componentId required".into()))?;
            remove_component(components, id)?;
        }
        "component.insert" => {
            let comp: ToolComponent = serde_json::from_value(
                payload
                    .get("component")
                    .cloned()
                    .unwrap_or_else(|| payload.clone()),
            )
            .map_err(|e| PatchConflict::InvalidPayload(e.to_string()))?;
            let index = payload
                .get("index")
                .and_then(|v| v.as_u64())
                .map(|u| u as usize);
            super::packs::validate_tool_components(std::slice::from_ref(&comp))
                .map_err(PatchConflict::InvalidPayload)?;
            insert_component(components, parent_id, comp, index)?;
        }
        "component.replace" => {
            let id = component_id
                .ok_or_else(|| PatchConflict::InvalidPayload("componentId required".into()))?;
            let comp: ToolComponent = serde_json::from_value(
                payload
                    .get("component")
                    .cloned()
                    .unwrap_or_else(|| payload.clone()),
            )
            .map_err(|e| PatchConflict::InvalidPayload(e.to_string()))?;
            super::packs::validate_tool_components(std::slice::from_ref(&comp))
                .map_err(PatchConflict::InvalidPayload)?;
            replace_component(components, id, comp)?;
        }
        "component.update_children" => {
            let id = component_id
                .ok_or_else(|| PatchConflict::InvalidPayload("componentId required".into()))?;
            let children: Vec<ToolComponent> = serde_json::from_value(
                payload
                    .get("children")
                    .cloned()
                    .unwrap_or_else(|| json!([])),
            )
            .map_err(|e| PatchConflict::InvalidPayload(e.to_string()))?;
            super::packs::validate_tool_components(&children)
                .map_err(PatchConflict::InvalidPayload)?;
            update_children(components, id, children)?;
        }
        "component.update_visibility" => {
            let id = component_id
                .ok_or_else(|| PatchConflict::InvalidPayload("componentId required".into()))?;
            let visible = payload
                .get("visible")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            update_props(components, id, &json!({ "hidden": !visible }))?;
        }
        "component.update_actions" => {
            let id = component_id
                .ok_or_else(|| PatchConflict::InvalidPayload("componentId required".into()))?;
            let raw_actions = payload.get("actions").cloned().unwrap_or(json!([]));
            let actions: Vec<crate::ai::ActionDefinition> = serde_json::from_value(raw_actions)
                .map_err(|e| PatchConflict::InvalidPayload(format!("invalid actions: {e}")))?;
            if actions.len() > crate::ai::response_schema::MAX_ACTIONS_PER_COMPONENT {
                return Err(PatchConflict::InvalidPayload(
                    "too many actions on component".into(),
                ));
            }
            for a in &actions {
                a.validate()
                    .map_err(|e| PatchConflict::InvalidPayload(e.into()))?;
            }
            let node = find_mut(components, id).ok_or_else(|| PatchConflict::MissingComponent {
                component_id: id.into(),
            })?;
            node.actions = Some(actions);
        }
        "component.move" => {
            let id = component_id
                .ok_or_else(|| PatchConflict::InvalidPayload("componentId required".into()))?;
            let node = {
                fn extract(nodes: &mut Vec<ToolComponent>, id: &str) -> Option<ToolComponent> {
                    if let Some(i) = nodes.iter().position(|c| c.id == id) {
                        return Some(nodes.remove(i));
                    }
                    for c in nodes.iter_mut() {
                        if let Some(ch) = c.children.as_mut() {
                            if let Some(n) = extract(ch, id) {
                                return Some(n);
                            }
                        }
                    }
                    None
                }
                extract(components, id)
            };
            let node = node.ok_or_else(|| PatchConflict::MissingComponent {
                component_id: id.into(),
            })?;
            let index = payload
                .get("index")
                .and_then(|v| v.as_u64())
                .map(|u| u as usize);
            insert_component(components, parent_id, node, index)?;
        }
        other => {
            return Err(PatchConflict::InvalidPayload(format!(
                "unsupported component op: {other}"
            )));
        }
    }
    assert_tree_limits(components)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::ToolComponent;

    fn sample() -> Vec<ToolComponent> {
        vec![ToolComponent {
            id: "root".into(),
            component_type: "container".into(),
            value_key: None,
            props: Some(json!({"title": "A"})),
            children: Some(vec![ToolComponent {
                id: "goal".into(),
                component_type: "progress".into(),
                value_key: None,
                props: Some(json!({"maximum": 8})),
                children: None,
                ..Default::default()
            }]),
            ..Default::default()
        }]
    }

    #[test]
    fn patches_props_without_full_replace() {
        let mut tree = sample();
        apply_component_op(
            &mut tree,
            "component.update_props",
            Some("goal"),
            None,
            Some(4),
            4,
            &json!({"maximum": 10}),
        )
        .unwrap();
        let props = tree[0].children.as_ref().unwrap()[0]
            .props
            .as_ref()
            .unwrap();
        assert_eq!(props["maximum"], 10);
        assert_eq!(tree[0].id, "root");
    }

    #[test]
    fn rejects_stale_revision() {
        let mut tree = sample();
        let err = apply_component_op(
            &mut tree,
            "component.update_props",
            Some("goal"),
            None,
            Some(3),
            4,
            &json!({"maximum": 10}),
        )
        .unwrap_err();
        assert!(matches!(err, PatchConflict::StaleRevision { .. }));
    }

    #[test]
    fn update_actions_updates_node_actions_directly_and_validates() {
        let mut tree = sample();
        apply_component_op(
            &mut tree,
            "component.update_actions",
            Some("goal"),
            None,
            Some(4),
            4,
            &json!({
                "actions": [
                    {
                        "type": "increment",
                        "target": "goalValue",
                        "amount": 2.0
                    },
                    {
                        "type": "invokeRegisteredAction",
                        "actionName": "local_data.query",
                        "input": { "modelId": "goals" },
                        "resultKey": "goalRecords"
                    }
                ]
            }),
        )
        .unwrap();

        let goal = &tree[0].children.as_ref().unwrap()[0];
        let actions = goal.actions.as_ref().expect("actions must be Some");
        assert_eq!(actions.len(), 2);
        assert_eq!(
            actions[0],
            crate::ai::ActionDefinition::Increment {
                target: "goalValue".into(),
                amount: Some(2.0),
            }
        );
        // Crucial verification: props must NOT have _actions
        let props = goal.props.as_ref().unwrap();
        assert!(props.get("_actions").is_none());
        assert_eq!(props["maximum"], 8);
    }

    #[test]
    fn update_actions_rejects_unknown_action_type() {
        let mut tree = sample();
        let err = apply_component_op(
            &mut tree,
            "component.update_actions",
            Some("goal"),
            None,
            Some(4),
            4,
            &json!({
                "actions": [
                    {
                        "type": "maliciousScript",
                        "target": "window"
                    }
                ]
            }),
        )
        .unwrap_err();

        assert!(matches!(err, PatchConflict::InvalidPayload(_)));
    }
}
