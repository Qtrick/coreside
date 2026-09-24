//! Software Document (UI Intermediate Representation)
//!
//! A declarative, structured representation of personal software in Coreside.
//! Provides stable, addressable section and component targets for incremental
//! model mutations without permitting arbitrary HTML or JavaScript execution.
//!
//! Architecture:
//! Model -> Software Document / IR -> Validation & Self-Repair -> Runtime Surface -> ToolRenderer

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

use crate::ai::response_schema::{ActionDefinition, ToolComponent, ToolDefinition};

/// A semantic section in a Software Document.
/// Sections provide stable addresses for partial updates (akin to Partial Update markers
/// or Chrome declarative partial-update templates) while preserving security boundaries.
/// A semantic section or region in a Software Document.
/// Regions provide stable addresses for partial updates (akin to Partial Update markers
/// or Chrome declarative partial-update templates) while preserving security boundaries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSection {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_region_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub responsive: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
    #[serde(default)]
    pub components: Vec<ToolComponent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

/// Explicit State Scope separating persistent, session, ephemeral, in-flight, and error states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateScope {
    /// Persisted across turns and saved to disk.
    Persistent,
    /// Preserved in-session across turns/interactions, reset on app restart.
    Session,
    /// Component-local ephemeral state (hover, open dropdown, active focus).
    Ephemeral,
    /// Pending async action, submitting, or optimistic mutation state.
    InFlight,
    /// Query failure, network error, or validation failure state.
    Error,
}

impl Default for StateScope {
    fn default() -> Self {
        Self::Persistent
    }
}

/// Explicit contract for state managed by the application.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StateContract {
    pub key: String,
    #[serde(rename = "type", default = "default_type_name")]
    pub type_name: String,
    #[serde(default)]
    pub initial_value: Value,
    #[serde(default)]
    pub scope: StateScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preservation_policy: Option<String>,
    #[serde(default = "default_read_policy")]
    pub read_policy: String,
    #[serde(default = "default_write_policy")]
    pub write_policy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensitivity: Option<String>,
    #[serde(default = "default_origin")]
    pub origin: String,
}

fn default_origin() -> String {
    "user".to_string()
}

fn default_type_name() -> String {
    "string".to_string()
}

fn default_read_policy() -> String {
    "public".to_string()
}

fn default_write_policy() -> String {
    "model".to_string()
}

impl StateContract {
    /// Create a new state contract with an explicit origin.
    /// Use `origin = "model"` for model-declared contracts, `"user"` for user-authorized,
    /// `"system"` for system-managed, `"legacy"` for pre-contract state.
    pub fn new_with_origin(
        key: impl Into<String>,
        initial_value: Value,
        scope: StateScope,
        origin: impl Into<String>,
    ) -> Self {
        Self {
            key: key.into(),
            type_name: "string".to_string(),
            initial_value,
            scope,
            description: None,
            preservation_policy: Some("preserve".into()),
            read_policy: "public".into(),
            write_policy: "model".into(),
            sensitivity: None,
            origin: origin.into(),
        }
    }

    /// Create a model-declared state contract.
    pub fn new_for_model(key: impl Into<String>, initial_value: Value, scope: StateScope) -> Self {
        Self::new_with_origin(key, initial_value, scope, "model")
    }

    /// Create a user-authorized state contract (user explicitly defined this key).
    pub fn new_for_user(key: impl Into<String>, initial_value: Value, scope: StateScope) -> Self {
        Self::new_with_origin(key, initial_value, scope, "user")
    }

    /// Legacy compatibility: same as new_with_origin but defaults to "model" origin
    /// (the safer assumption when provenance is unknown).
    /// Prefer new_for_model() or new_for_user() at call sites where origin is known.
    pub fn new(key: impl Into<String>, initial_value: Value, scope: StateScope) -> Self {
        Self::new_for_model(key, initial_value, scope)
    }
}

impl Default for StateContract {
    fn default() -> Self {
        Self {
            key: String::new(),
            type_name: default_type_name(),
            initial_value: Value::Null,
            scope: StateScope::Persistent,
            description: None,
            preservation_policy: None,
            read_policy: default_read_policy(),
            write_policy: default_write_policy(),
            sensitivity: None,
            origin: default_origin(),
        }
    }
}

/// Declared action contract for runtime effects.
///
/// `component_id` is required for Runtime V2 applications; it identifies which component
/// in the SoftwareDocument declares this action. The kernel verifies this at invocation
/// time so that a model cannot invoke a contract that belongs to a different component.
///
/// `descriptor_hash` is the SHA-256 hex of the canonical bundled ActionDescriptor JSON.
/// If supplied, the gateway verifies it before execution so that descriptor tampering
/// is detected at the boundary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActionContract {
    pub action_id: String,
    pub action_name: String,
    /// The component that declares this action within the SoftwareDocument.
    /// Required for Runtime V2 canonical applications; optional for legacy compatibility.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component_id: Option<String>,
    /// SHA-256 hex of the bundled ActionDescriptor JSON for tamper detection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descriptor_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_from_state: Option<HashMap<String, String>>,
}

/// Note generated when the self-repair loop rectifies a document defect.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RepairNote {
    pub kind: String,
    pub target_id: String,
    pub detail: String,
}

/// Declarative Software Document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SoftwareDocument {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default = "default_doc_version")]
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<Value>,
    #[serde(default)]
    pub sections: Vec<DocumentSection>,
    #[serde(default)]
    pub components: Vec<ToolComponent>,
    #[serde(default)]
    pub state_contracts: Vec<StateContract>,
    #[serde(default)]
    pub action_contracts: Vec<ActionContract>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_tokens: Option<Value>,
    #[serde(default)]
    pub capability_packs: Vec<String>,
}

fn default_doc_version() -> u32 {
    1
}

impl SoftwareDocument {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: None,
            version: 1,
            layout: None,
            sections: Vec::new(),
            components: Vec::new(),
            state_contracts: Vec::new(),
            action_contracts: Vec::new(),
            design_tokens: None,
            capability_packs: vec!["coreside.core".into()],
        }
    }

    pub fn flatten_components(&self) -> Vec<ToolComponent> {
        let mut out = Vec::new();
        for sec in &self.sections {
            for c in &sec.components {
                out.push(c.clone());
            }
        }
        out
    }

    pub fn sync_components(&mut self) {
        self.components = self.flatten_components();
    }

    /// Load or convert a JSON Value into a SoftwareDocument without losing contracts.
    pub fn from_value(val: &Value) -> Result<Self, String> {
        if val.get("sections").is_some() {
            let mut doc: Self = serde_json::from_value(val.clone())
                .map_err(|e| format!("failed to deserialize SoftwareDocument: {e}"))?;
            if doc.components.is_empty() && !doc.sections.is_empty() {
                doc.sync_components();
            }
            Ok(doc)
        } else {
            let tool: ToolDefinition = serde_json::from_value(val.clone())
                .map_err(|e| format!("failed to parse ToolDefinition: {e}"))?;
            let mut doc = Self::from_tool_definition(&tool);
            if let Some(sc) = val
                .get("stateContracts")
                .or_else(|| val.get("state_contracts"))
            {
                let contracts: Vec<StateContract> = serde_json::from_value(sc.clone())
                    .map_err(|e| format!("invalid state contracts: {e}"))?;
                doc.state_contracts = contracts;
            }
            if let Some(ac) = val
                .get("actionContracts")
                .or_else(|| val.get("action_contracts"))
            {
                let contracts: Vec<ActionContract> = serde_json::from_value(ac.clone())
                    .map_err(|e| format!("invalid action contracts: {e}"))?;
                doc.action_contracts = contracts;
            }
            doc.sync_components();
            Ok(doc)
        }
    }

    /// Convert a flat or structured ToolDefinition into a structured SoftwareDocument.
    pub fn from_tool_definition(tool: &ToolDefinition) -> Self {
        let mut doc = Self::new(&tool.id, &tool.name);
        doc.layout = Some(tool.layout.clone());
        doc.description = if tool.description.is_empty() {
            None
        } else {
            Some(tool.description.clone())
        };

        // Check if components contain synthetic section containers
        let has_synthetic_sections = tool.components.iter().any(|c| {
            c.component_type == "container"
                && (c.id.starts_with("section-")
                    || c.props
                        .as_ref()
                        .and_then(|p| p.get("syntheticSection"))
                        .and_then(|v| v.as_bool())
                        == Some(true))
        });

        if has_synthetic_sections {
            fn unpack_container(comp: &ToolComponent, doc: &mut SoftwareDocument) {
                let props = comp.props.as_ref();
                let sec_id = props
                    .and_then(|p| p.get("section"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| {
                        comp.id
                            .strip_prefix("section-")
                            .unwrap_or(&comp.id)
                            .to_string()
                    });
                let title = props
                    .and_then(|p| p.get("title"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let role = props
                    .and_then(|p| p.get("sectionRole"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| comp.layout_role.clone());
                let layout = props
                    .and_then(|p| p.get("sectionLayout"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let parent_region_id = props
                    .and_then(|p| p.get("parentRegionId"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let slot = props
                    .and_then(|p| p.get("slot"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let mut leaf_components = Vec::new();
                let mut child_containers = Vec::new();
                if let Some(ref children) = comp.children {
                    for child in children {
                        let is_child_sec = child.component_type == "container"
                            && (child.id.starts_with("section-")
                                || child
                                    .props
                                    .as_ref()
                                    .and_then(|p| p.get("syntheticSection"))
                                    .and_then(|v| v.as_bool())
                                    == Some(true));
                        if is_child_sec {
                            child_containers.push(child);
                        } else {
                            leaf_components.push(child.clone());
                        }
                    }
                }

                doc.sections.push(DocumentSection {
                    id: sec_id,
                    title,
                    role,
                    layout,
                    parent_region_id,
                    slot,
                    responsive: None,
                    instance_id: None,
                    components: leaf_components,
                    metadata: None,
                });

                for child_sec in child_containers {
                    unpack_container(child_sec, doc);
                }
            }

            for comp in &tool.components {
                let is_sec = comp.component_type == "container"
                    && (comp.id.starts_with("section-")
                        || comp
                            .props
                            .as_ref()
                            .and_then(|p| p.get("syntheticSection"))
                            .and_then(|v| v.as_bool())
                            == Some(true));
                if is_sec {
                    unpack_container(comp, &mut doc);
                } else {
                    // Collect loose components into a main section
                    if let Some(main_sec) = doc.sections.iter_mut().find(|s| s.id == "main") {
                        main_sec.components.push(comp.clone());
                    } else {
                        doc.sections.push(DocumentSection {
                            id: "main".into(),
                            title: None,
                            role: Some("content".into()),
                            layout: Some("stack".into()),
                            parent_region_id: None,
                            slot: None,
                            responsive: None,
                            instance_id: None,
                            components: vec![comp.clone()],
                            metadata: None,
                        });
                    }
                }
            }

            doc.sync_components();
            return doc;
        }

        // Group components by section if explicit in props, or organize into main
        let mut section_map: HashMap<String, Vec<ToolComponent>> = HashMap::new();
        let mut section_order: Vec<String> = Vec::new();

        for comp in &tool.components {
            let sec_id = comp
                .props
                .as_ref()
                .and_then(|p| p.get("section"))
                .and_then(|v| v.as_str())
                .unwrap_or("main")
                .to_string();

            if !section_map.contains_key(&sec_id) {
                section_order.push(sec_id.clone());
            }
            section_map.entry(sec_id).or_default().push(comp.clone());
        }

        for sec_id in section_order {
            let comps = section_map.remove(&sec_id).unwrap_or_default();
            doc.sections.push(DocumentSection {
                id: sec_id.clone(),
                title: if sec_id == "main" {
                    None
                } else {
                    Some(sec_id.clone())
                },
                role: Some("content".into()),
                layout: Some("stack".into()),
                parent_region_id: None,
                slot: None,
                responsive: None,
                instance_id: None,
                components: comps,
                metadata: None,
            });
        }

        if doc.sections.is_empty() {
            doc.sections.push(DocumentSection {
                id: "main".into(),
                title: None,
                role: Some("content".into()),
                layout: Some("stack".into()),
                parent_region_id: None,
                slot: None,
                responsive: None,
                instance_id: None,
                components: Vec::new(),
                metadata: None,
            });
        }

        doc.sync_components();
        doc
    }

    /// Render/flatten SoftwareDocument into a ToolDefinition for runtime presentation.
    pub fn to_tool_definition(&self) -> ToolDefinition {
        // Derive layout from self.layout or design_tokens.layout, falling back to single-column
        let layout = self
            .layout
            .clone()
            .or_else(|| {
                self.design_tokens
                    .as_ref()
                    .and_then(|dt| dt.get("layout"))
                    .cloned()
            })
            .unwrap_or_else(|| json!({ "type": "single-column" }));

        // Check if there is multi-section or structured hierarchy
        let has_rich_structure = self.sections.len() > 1
            || self
                .sections
                .iter()
                .any(|s| s.parent_region_id.is_some() || s.role.is_some() || s.layout.is_some());

        if !has_rich_structure {
            let mut all_components = Vec::new();
            for section in &self.sections {
                for comp in &section.components {
                    let mut c = comp.clone();
                    // Ensure section tagging and hierarchy metadata is recorded in component props
                    let mut props = c.props.unwrap_or_else(|| json!({}));
                    if let Value::Object(ref mut map) = props {
                        if !map.contains_key("section") {
                            map.insert("section".into(), json!(section.id));
                        }
                    }
                    c.props = Some(props);
                    all_components.push(c);
                }
            }
            return ToolDefinition {
                id: self.id.clone(),
                name: self.title.clone(),
                description: self.description.clone().unwrap_or_default(),
                layout,
                components: all_components,
            };
        }

        // Build hierarchical container projection
        let mut containers: HashMap<String, ToolComponent> = HashMap::new();
        for section in &self.sections {
            let mut section_components = section.components.clone();
            for comp in &mut section_components {
                let mut props = comp.props.take().unwrap_or_else(|| json!({}));
                if let Value::Object(ref mut map) = props {
                    if !map.contains_key("section") {
                        map.insert("section".into(), json!(section.id));
                    }
                    if let Some(ref role) = section.role {
                        if !map.contains_key("sectionRole") {
                            map.insert("sectionRole".into(), json!(role));
                        }
                    }
                    if let Some(ref sec_layout) = section.layout {
                        if !map.contains_key("sectionLayout") {
                            map.insert("sectionLayout".into(), json!(sec_layout));
                        }
                    }
                }
                comp.props = Some(props);
            }

            let container_id = format!("section-{}", section.id);
            let sec_props = json!({
                "section": section.id,
                "title": section.title,
                "sectionRole": section.role,
                "sectionLayout": section.layout,
                "parentRegionId": section.parent_region_id,
                "slot": section.slot,
                "syntheticSection": true,
            });

            containers.insert(
                section.id.clone(),
                ToolComponent {
                    id: container_id,
                    component_type: "container".into(),
                    value_key: None,
                    props: Some(sec_props),
                    children: Some(section_components),
                    actions: None,
                    layout_role: section.role.clone(),
                    col_span: None,
                    row_span: None,
                },
            );
        }

        let mut child_ids = HashSet::new();
        for section in &self.sections {
            if let Some(ref pid) = section.parent_region_id {
                if containers.contains_key(pid) {
                    child_ids.insert(section.id.clone());
                }
            }
        }

        for section in &self.sections {
            if let Some(ref pid) = section.parent_region_id {
                if let Some(child_comp) = containers.remove(&section.id) {
                    if let Some(parent_comp) = containers.get_mut(pid) {
                        if let Some(ref mut children) = parent_comp.children {
                            children.push(child_comp);
                        } else {
                            parent_comp.children = Some(vec![child_comp]);
                        }
                    } else {
                        containers.insert(section.id.clone(), child_comp);
                    }
                }
            }
        }

        let mut root_components = Vec::new();
        for section in &self.sections {
            if !child_ids.contains(&section.id) {
                if let Some(comp) = containers.remove(&section.id) {
                    root_components.push(comp);
                }
            }
        }

        ToolDefinition {
            id: self.id.clone(),
            name: self.title.clone(),
            description: self.description.clone().unwrap_or_default(),
            layout,
            components: root_components,
        }
    }

    /// Self-Repair Loop: inspects the document, repairs common model errors,
    /// ensures security constraints, and returns diagnostic repair notes.
    pub fn validate_and_repair(&mut self) -> Vec<RepairNote> {
        let mut notes = Vec::new();
        let mut seen_component_ids: HashSet<String> = HashSet::new();
        let mut declared_state_keys: HashSet<String> =
            self.state_contracts.iter().map(|s| s.key.clone()).collect();

        // 1. Sanitize and validate sections
        if self.sections.is_empty() {
            self.sections.push(DocumentSection {
                id: "main".into(),
                title: None,
                role: Some("content".into()),
                layout: Some("stack".into()),
                parent_region_id: None,
                slot: None,
                responsive: None,
                instance_id: None,
                components: Vec::new(),
                metadata: None,
            });
            notes.push(RepairNote {
                kind: "added_fallback_section".into(),
                target_id: "main".into(),
                detail: "Created fallback 'main' section for empty document".into(),
            });
        }

        // 2. Component inspection & repair
        for section in &mut self.sections {
            for comp in &mut section.components {
                repair_component(
                    comp,
                    &mut seen_component_ids,
                    &mut declared_state_keys,
                    &mut self.state_contracts,
                    &mut notes,
                );
            }
        }

        notes
    }

    /// Find a semantic region by ID.
    pub fn find_region(&self, id: &str) -> Option<&DocumentSection> {
        self.sections.iter().find(|s| s.id == id)
    }

    /// Find a mutable semantic region by ID.
    pub fn find_region_mut(&mut self, id: &str) -> Option<&mut DocumentSection> {
        self.sections.iter_mut().find(|s| s.id == id)
    }

    /// Compute deterministic hierarchy path for a region (e.g. "surface/main/results").
    /// Returns None if the region is not found or a cycle is detected in the parent chain.
    pub fn get_region_path(&self, id: &str) -> Option<String> {
        let mut curr = self.find_region(id)?;
        let mut segments = vec![curr.id.clone()];
        let mut visited = HashSet::new();
        visited.insert(curr.id.clone());
        while let Some(ref parent_id) = curr.parent_region_id {
            if !visited.insert(parent_id.clone()) {
                return None; // cycle detected
            }
            if let Some(parent) = self.find_region(parent_id) {
                segments.push(parent.id.clone());
                curr = parent;
            } else {
                break;
            }
        }
        segments.push(self.id.clone());
        segments.reverse();
        Some(segments.join("/"))
    }

    /// Get all direct child regions of a parent region.
    pub fn get_child_regions(&self, parent_id: &str) -> Vec<&DocumentSection> {
        self.sections
            .iter()
            .filter(|s| s.parent_region_id.as_deref() == Some(parent_id))
            .collect()
    }

    /// Move a component from one region to another with optional target index.
    /// Validates all preconditions before any mutation.
    ///
    /// Same-region moves: the target index is relative to the list BEFORE removal.
    /// When moving within the same region, the removal of the source component
    /// shifts all subsequent indices down by 1, so we adjust accordingly.
    pub fn move_component(
        &mut self,
        component_id: &str,
        target_region_id: &str,
        target_index: Option<usize>,
    ) -> Result<(), String> {
        // 1. Validate target region exists.
        let target_len = self
            .find_region(target_region_id)
            .ok_or_else(|| format!("target region '{target_region_id}' not found"))?
            .components
            .len();

        // 2. Validate target index if provided (before removal — upper bound is len).
        if let Some(idx) = target_index {
            if idx > target_len {
                return Err(format!(
                    "target index {idx} exceeds region length {target_len}"
                ));
            }
        }

        // 3. Determine source region and position for same-region adjustment.
        let source_info: Option<(String, usize)> = self.sections.iter().find_map(|sec| {
            sec.components
                .iter()
                .position(|c| c.id == component_id)
                .map(|pos| (sec.id.clone(), pos))
        });
        let (source_region_id, source_pos) =
            source_info.ok_or_else(|| format!("component '{component_id}' not found"))?;

        // 4. Compute adjusted target index for same-region moves.
        // Removing the source shifts indices at or after source_pos down by 1.
        let adjusted_index = target_index.map(|idx| {
            if source_region_id == target_region_id && source_pos < idx {
                // ponytail: after removal, the slot we want has shifted left by 1.
                idx.saturating_sub(1)
            } else {
                idx
            }
        });

        // 5. Find and extract the component from source region.
        let mut extracted: Option<ToolComponent> = None;
        for sec in &mut self.sections {
            if let Some(pos) = sec.components.iter().position(|c| c.id == component_id) {
                extracted = Some(sec.components.remove(pos));
                break;
            }
        }
        let mut comp = extracted.ok_or_else(|| format!("component '{component_id}' not found"))?;

        // 6. Update component's section prop if present.
        if let Some(Value::Object(ref mut map)) = comp.props {
            map.insert("section".into(), json!(target_region_id));
        }

        // 7. Insert into target region with adjusted index.
        // Safe to unwrap: we validated target exists in step 1.
        let target_sec = self.find_region_mut(target_region_id).unwrap();
        match adjusted_index {
            Some(idx) => {
                target_sec.components.insert(idx, comp);
            }
            None => {
                target_sec.components.push(comp);
            }
        }
        Ok(())
    }

    /// Reorder components in a region by given ID sequence.
    /// Unspecified components retain their previous relative order.
    pub fn reorder_components(
        &mut self,
        region_id: &str,
        component_ids: &[String],
    ) -> Result<(), String> {
        let sec = self
            .find_region_mut(region_id)
            .ok_or_else(|| format!("region '{region_id}' not found"))?;

        // Record original order of all components before any mutation.
        let original_order: Vec<String> = sec.components.iter().map(|c| c.id.clone()).collect();
        let mut comp_map: HashMap<String, ToolComponent> = HashMap::new();
        for c in sec.components.drain(..) {
            comp_map.insert(c.id.clone(), c);
        }

        // Place requested components in the specified order.
        let mut new_components = Vec::new();
        for id in component_ids {
            if let Some(c) = comp_map.remove(id) {
                new_components.push(c);
            }
        }

        // Append unspecified components in their original relative order.
        for id in &original_order {
            if let Some(c) = comp_map.remove(id) {
                new_components.push(c);
            }
        }

        sec.components = new_components;
        Ok(())
    }

    /// Update region layout and responsive configuration.
    pub fn update_region_layout(
        &mut self,
        region_id: &str,
        layout: Option<String>,
        responsive: Option<Value>,
    ) -> Result<(), String> {
        let sec = self
            .find_region_mut(region_id)
            .ok_or_else(|| format!("region '{region_id}' not found"))?;
        if let Some(l) = layout {
            sec.layout = Some(l);
        }
        if let Some(r) = responsive {
            sec.responsive = Some(r);
        }
        Ok(())
    }

    /// Find a component along with its enclosing section/region at arbitrary depth.
    pub fn find_component_with_region(
        &self,
        component_id: &str,
    ) -> Option<(&DocumentSection, &ToolComponent)> {
        for sec in &self.sections {
            if let Some(comp) = find_in_tree(&sec.components, component_id) {
                return Some((sec, comp));
            }
        }
        None
    }

    /// Find a component by ID at arbitrary depth in any section.
    pub fn find_component(&self, component_id: &str) -> Option<&ToolComponent> {
        for sec in &self.sections {
            if let Some(comp) = find_in_tree(&sec.components, component_id) {
                return Some(comp);
            }
        }
        None
    }

    /// Find a mutable reference to a component by ID at arbitrary depth.
    pub fn find_component_mut(&mut self, component_id: &str) -> Option<&mut ToolComponent> {
        for sec in &mut self.sections {
            if let Some(comp) = find_in_tree_mut(&mut sec.components, component_id) {
                return Some(comp);
            }
        }
        None
    }

    /// Insert a component at a specified parent, section, or root.
    pub fn insert_component(
        &mut self,
        target_section_id: Option<&str>,
        target_parent_id: Option<&str>,
        target_index: Option<usize>,
        comp: ToolComponent,
    ) -> Result<(), String> {
        if self.find_component(&comp.id).is_some() {
            return Err(format!("component '{}' already exists", comp.id));
        }
        if let Some(pid) = target_parent_id {
            let parent = self
                .find_component_mut(pid)
                .ok_or_else(|| format!("parent component '{pid}' not found"))?;
            let mut children = parent.children.take().unwrap_or_default();
            match target_index {
                Some(idx) if idx <= children.len() => children.insert(idx, comp),
                _ => children.push(comp),
            }
            parent.children = Some(children);
            return Ok(());
        }

        let sec = if let Some(sid) = target_section_id {
            self.sections
                .iter_mut()
                .find(|s| s.id == sid)
                .ok_or_else(|| format!("section '{sid}' not found"))?
        } else if let Some(first) = self.sections.first_mut() {
            first
        } else {
            self.sections.push(DocumentSection {
                id: "main".into(),
                title: None,
                role: Some("content".into()),
                layout: Some("stack".into()),
                parent_region_id: None,
                slot: None,
                responsive: None,
                instance_id: None,
                components: Vec::new(),
                metadata: None,
            });
            self.sections.first_mut().unwrap()
        };

        match target_index {
            Some(idx) if idx <= sec.components.len() => sec.components.insert(idx, comp),
            _ => sec.components.push(comp),
        }
        Ok(())
    }

    /// Remove a component at arbitrary tree depth.
    pub fn remove_component(&mut self, id: &str) -> Result<ToolComponent, String> {
        for sec in &mut self.sections {
            if let Some(removed) = remove_from_tree(&mut sec.components, id) {
                return Ok(removed);
            }
        }
        Err(format!("component '{id}' not found"))
    }

    /// Replace a component in-place at arbitrary tree depth.
    pub fn replace_component(
        &mut self,
        id: &str,
        new_comp: ToolComponent,
    ) -> Result<ToolComponent, String> {
        for sec in &mut self.sections {
            if let Some(old) = replace_in_tree(&mut sec.components, id, new_comp.clone()) {
                return Ok(old);
            }
        }
        Err(format!("component '{id}' not found"))
    }

    /// Move a component from its current position to another section, parent, or index.
    pub fn move_component_tree(
        &mut self,
        id: &str,
        target_section_id: Option<&str>,
        target_parent_id: Option<&str>,
        target_index: Option<usize>,
    ) -> Result<(), String> {
        let comp = self.remove_component(id)?;
        self.insert_component(target_section_id, target_parent_id, target_index, comp)
    }

    /// Update props of a component at arbitrary tree depth.
    pub fn update_component_props(&mut self, id: &str, patch_props: &Value) -> Result<(), String> {
        let comp = self
            .find_component_mut(id)
            .ok_or_else(|| format!("component '{id}' not found"))?;
        let mut props = comp.props.take().unwrap_or_else(|| json!({}));
        if let (Value::Object(ref mut base), Value::Object(patch)) = (&mut props, patch_props) {
            for (k, v) in patch {
                base.insert(k.clone(), v.clone());
            }
        } else {
            props = patch_props.clone();
        }
        comp.props = Some(props);
        Ok(())
    }

    /// Update children of a component at arbitrary tree depth.
    pub fn update_component_children(
        &mut self,
        id: &str,
        children: Vec<ToolComponent>,
    ) -> Result<(), String> {
        let comp = self
            .find_component_mut(id)
            .ok_or_else(|| format!("component '{id}' not found"))?;
        comp.children = Some(children);
        Ok(())
    }

    /// Update visibility of a component at arbitrary tree depth.
    pub fn update_component_visibility(&mut self, id: &str, visible: bool) -> Result<(), String> {
        let comp = self
            .find_component_mut(id)
            .ok_or_else(|| format!("component '{id}' not found"))?;
        let mut props = comp.props.take().unwrap_or_else(|| json!({}));
        if let Value::Object(ref mut base) = props {
            base.insert("visible".into(), json!(visible));
        }
        comp.props = Some(props);
        Ok(())
    }

    /// Update actions of a component at arbitrary tree depth.
    pub fn update_component_actions(
        &mut self,
        id: &str,
        actions: Vec<ActionDefinition>,
    ) -> Result<(), String> {
        let comp = self
            .find_component_mut(id)
            .ok_or_else(|| format!("component '{id}' not found"))?;
        comp.actions = Some(actions);
        Ok(())
    }

    /// Update or declare state scope for an existing or new key.
    pub fn update_state_scope(&mut self, key: &str, scope: StateScope) -> Result<(), String> {
        if let Some(contract) = self.state_contracts.iter_mut().find(|sc| sc.key == key) {
            contract.scope = scope;
        } else {
            self.state_contracts
                .push(StateContract::new(key, Value::Null, scope));
        }
        Ok(())
    }

    /// Filter and preserve state values according to state contracts.
    /// Ephemeral and InFlight states are safely discarded, while Persistent and Session
    /// states are preserved across model update turns.
    pub fn preserve_state_values(
        &self,
        previous_state: &HashMap<String, Value>,
    ) -> HashMap<String, Value> {
        let mut preserved = HashMap::new();
        for (k, v) in previous_state {
            if let Some(contract) = self.state_contracts.iter().find(|sc| &sc.key == k) {
                if contract.scope != StateScope::Ephemeral && contract.scope != StateScope::InFlight
                {
                    preserved.insert(k.clone(), v.clone());
                }
            } else {
                preserved.insert(k.clone(), v.clone());
            }
        }
        preserved
    }

    /// Filter state by explicit scope.
    pub fn filter_state_by_scope(
        &self,
        active_state: &HashMap<String, Value>,
        scope: StateScope,
    ) -> HashMap<String, Value> {
        let mut filtered = HashMap::new();
        for (k, v) in active_state {
            if let Some(contract) = self.state_contracts.iter().find(|sc| &sc.key == k) {
                if contract.scope == scope {
                    filtered.insert(k.clone(), v.clone());
                }
            }
        }
        filtered
    }

    /// Add a new section at the specified index or end.
    pub fn add_section(
        &mut self,
        mut section: DocumentSection,
        index: Option<usize>,
    ) -> Result<(), String> {
        if section.id.trim().is_empty() {
            return Err("section id cannot be empty".into());
        }
        if self.sections.iter().any(|s| s.id == section.id) {
            return Err(format!("section with id '{}' already exists", section.id));
        }
        if let Some(ref pid) = section.parent_region_id {
            if *pid == section.id {
                return Err(format!("section '{}' cannot be its own parent", section.id));
            }
            if !self.sections.iter().any(|s| &s.id == pid) {
                return Err(format!(
                    "parent region '{pid}' does not exist; add the parent before its children"
                ));
            }
        }
        if section.role.is_none() {
            section.role = Some("content".into());
        }
        if section.layout.is_none() {
            section.layout = Some("stack".into());
        }

        match index {
            Some(idx) if idx <= self.sections.len() => {
                self.sections.insert(idx, section);
            }
            _ => {
                self.sections.push(section);
            }
        }
        Ok(())
    }

    /// Remove a section by its ID.
    pub fn remove_section(&mut self, section_id: &str) -> Result<DocumentSection, String> {
        let has_children = self
            .sections
            .iter()
            .any(|s| s.parent_region_id.as_deref() == Some(section_id));
        if has_children {
            return Err(format!(
                "cannot remove section '{section_id}': it has child sections that would be orphaned"
            ));
        }
        if let Some(pos) = self.sections.iter().position(|s| s.id == section_id) {
            Ok(self.sections.remove(pos))
        } else {
            Err(format!("section '{section_id}' not found"))
        }
    }

    /// Recursively remove a section and all its descendants.
    pub fn remove_section_recursive(&mut self, section_id: &str) -> Result<Vec<String>, String> {
        if !self.sections.iter().any(|s| s.id == section_id) {
            return Err(format!("section '{section_id}' not found"));
        }
        let mut to_remove: Vec<String> = Vec::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(section_id.to_string());
        while let Some(curr_id) = queue.pop_front() {
            to_remove.push(curr_id.clone());
            for s in &self.sections {
                if s.parent_region_id.as_deref() == Some(&curr_id) {
                    queue.push_back(s.id.clone());
                }
            }
        }
        self.sections.retain(|s| !to_remove.contains(&s.id));
        Ok(to_remove)
    }

    /// Update section components or attributes with pre-mutation validation.
    pub fn update_section(
        &mut self,
        section_id: &str,
        new_components: Option<Vec<ToolComponent>>,
        new_title: Option<String>,
        new_layout: Option<String>,
    ) -> Result<(), String> {
        let sec = self
            .sections
            .iter_mut()
            .find(|s| s.id == section_id)
            .ok_or_else(|| format!("section '{section_id}' not found"))?;

        if let Some(comps) = new_components {
            sec.components = comps;
        }
        if let Some(title) = new_title {
            sec.title = Some(title);
        }
        if let Some(layout) = new_layout {
            sec.layout = Some(layout);
        }
        Ok(())
    }

    /// Bind a component to a state contract, enforcing sensitivity and authorization boundaries.
    pub fn bind_state(
        &mut self,
        component_id: &str,
        key: &str,
        initial_value: Option<Value>,
    ) -> Result<(), String> {
        // Enforce state authorization boundaries: sensitive or restricted state keys cannot be bound by model
        if let Some(contract) = self.state_contracts.iter().find(|sc| sc.key == key) {
            if contract.read_policy == "restricted"
                || contract.sensitivity.as_deref() == Some("sensitive")
            {
                return Err(format!(
                    "state key '{key}' is restricted/sensitive and cannot be bound by model component"
                ));
            }
        }

        let comp = self
            .find_component_mut(component_id)
            .ok_or_else(|| format!("component '{component_id}' not found"))?;
        comp.value_key = Some(key.to_string());

        // Add state contract if not already defined
        if !self.state_contracts.iter().any(|sc| sc.key == key) {
            self.state_contracts.push(StateContract::new(
                key,
                initial_value.unwrap_or(Value::Null),
                StateScope::Persistent,
            ));
        }

        Ok(())
    }

    /// Bind an action contract to a component at arbitrary tree depth.
    pub fn bind_action(
        &mut self,
        component_id: &str,
        action: ActionDefinition,
    ) -> Result<(), String> {
        let comp = self
            .find_component_mut(component_id)
            .ok_or_else(|| format!("component '{component_id}' not found"))?;

        let mut actions = comp.actions.take().unwrap_or_default();
        actions.push(action.clone());
        comp.actions = Some(actions);

        if let ActionDefinition::InvokeRegisteredAction {
            action_name,
            result_key,
            input_from_state,
            ..
        } = &action
        {
            let act_id = format!("{}-{}", component_id, action_name.replace('.', "-"));
            if !self
                .action_contracts
                .iter()
                .any(|ac| ac.action_id == act_id)
            {
                self.action_contracts.push(ActionContract {
                    action_id: act_id,
                    action_name: action_name.clone(),
                    component_id: Some(component_id.to_string()),
                    descriptor_hash: None,
                    description: Some(format!("Triggered from component '{component_id}'")),
                    result_key: result_key.clone(),
                    input_from_state: input_from_state.clone(),
                });
            }
        }

        Ok(())
    }

    /// Apply a safe design style token (e.g. density, variant, accent).
    /// Uses an explicit presentation allowlist to prevent unauthorized property mutations.
    pub fn set_style_token(
        &mut self,
        target_id: &str,
        token: &str,
        val: Value,
    ) -> Result<(), String> {
        if !is_allowed_style_token(token) {
            return Err(format!(
                "style token '{token}' is not an allowed presentation token; forbidden mutation rejected"
            ));
        }

        // First check if target is a section
        if let Some(sec) = self.sections.iter_mut().find(|s| s.id == target_id) {
            let mut meta = sec.metadata.take().unwrap_or_else(|| json!({}));
            if let Value::Object(ref mut m) = meta {
                m.insert(token.to_string(), val);
            }
            sec.metadata = Some(meta);
            return Ok(());
        }

        // Next check if target is a component at arbitrary depth
        if let Some(comp) = self.find_component_mut(target_id) {
            let mut props = comp.props.take().unwrap_or_else(|| json!({}));
            if let Value::Object(ref mut m) = props {
                m.insert(token.to_string(), val);
            }
            comp.props = Some(props);
            return Ok(());
        }

        Err(format!(
            "target '{target_id}' not found as section or component"
        ))
    }

    /// Apply any AppOperation directly to this canonical SoftwareDocument.
    pub fn apply_operation(
        &mut self,
        op: &crate::runtime_v2::operations::AppOperation,
    ) -> Result<(), String> {
        match op.op_type.as_str() {
            "surface.add_section" => {
                let section: DocumentSection = serde_json::from_value(
                    op.payload
                        .get("section")
                        .cloned()
                        .unwrap_or_else(|| op.payload.clone()),
                )
                .map_err(|e| format!("invalid section payload: {e}"))?;
                let index = op
                    .payload
                    .get("index")
                    .and_then(|v| v.as_u64())
                    .map(|i| i as usize);
                self.add_section(section, index)?;
            }
            "surface.remove_section" => {
                let sec_id = op
                    .payload
                    .get("sectionId")
                    .and_then(|v| v.as_str())
                    .or_else(|| op.target.component_id.as_deref())
                    .ok_or_else(|| "sectionId required".to_string())?;
                self.remove_section(sec_id)?;
            }
            "surface.update_section" => {
                let sec_id = op
                    .payload
                    .get("sectionId")
                    .and_then(|v| v.as_str())
                    .or_else(|| op.target.component_id.as_deref())
                    .ok_or_else(|| "sectionId required".to_string())?;
                let comps: Option<Vec<ToolComponent>> = op
                    .payload
                    .get("components")
                    .cloned()
                    .and_then(|v| serde_json::from_value(v).ok());
                let title = op
                    .payload
                    .get("title")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let layout = op
                    .payload
                    .get("layout")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                self.update_section(sec_id, comps, title, layout)?;
            }
            "component.insert" => {
                let comp: ToolComponent = serde_json::from_value(
                    op.payload
                        .get("component")
                        .cloned()
                        .unwrap_or_else(|| op.payload.clone()),
                )
                .map_err(|e| format!("invalid component payload: {e}"))?;
                let sec_id = op.payload.get("sectionId").and_then(|v| v.as_str());
                let pid = op
                    .target
                    .parent_id
                    .as_deref()
                    .or_else(|| op.payload.get("parentId").and_then(|v| v.as_str()));
                let index = op
                    .payload
                    .get("index")
                    .and_then(|v| v.as_u64())
                    .map(|i| i as usize);
                self.insert_component(sec_id, pid, index, comp)?;
            }
            "component.remove" => {
                let cid = op
                    .target
                    .component_id
                    .as_deref()
                    .or_else(|| op.payload.get("componentId").and_then(|v| v.as_str()))
                    .ok_or_else(|| "componentId required".to_string())?;
                self.remove_component(cid)?;
            }
            "component.replace" => {
                let cid = op
                    .target
                    .component_id
                    .as_deref()
                    .or_else(|| op.payload.get("componentId").and_then(|v| v.as_str()))
                    .ok_or_else(|| "componentId required".to_string())?;
                let comp: ToolComponent = serde_json::from_value(
                    op.payload
                        .get("component")
                        .cloned()
                        .unwrap_or_else(|| op.payload.clone()),
                )
                .map_err(|e| format!("invalid component payload: {e}"))?;
                self.replace_component(cid, comp)?;
            }
            "component.move" => {
                let cid = op
                    .target
                    .component_id
                    .as_deref()
                    .or_else(|| op.payload.get("componentId").and_then(|v| v.as_str()))
                    .ok_or_else(|| "componentId required".to_string())?;
                let sec_id = op.payload.get("toSectionId").and_then(|v| v.as_str());
                let pid = op.payload.get("toParentId").and_then(|v| v.as_str());
                let index = op
                    .payload
                    .get("index")
                    .and_then(|v| v.as_u64())
                    .map(|i| i as usize);
                self.move_component_tree(cid, sec_id, pid, index)?;
            }
            "component.update_props" => {
                let cid = op
                    .target
                    .component_id
                    .as_deref()
                    .or_else(|| op.payload.get("componentId").and_then(|v| v.as_str()))
                    .ok_or_else(|| "componentId required".to_string())?;
                let props = op.payload.get("props").unwrap_or(&op.payload);
                self.update_component_props(cid, props)?;
            }
            "component.update_children" => {
                let cid = op
                    .target
                    .component_id
                    .as_deref()
                    .or_else(|| op.payload.get("componentId").and_then(|v| v.as_str()))
                    .ok_or_else(|| "componentId required".to_string())?;
                let children: Vec<ToolComponent> = serde_json::from_value(
                    op.payload
                        .get("children")
                        .cloned()
                        .unwrap_or_else(|| json!([])),
                )
                .map_err(|e| format!("invalid children payload: {e}"))?;
                self.update_component_children(cid, children)?;
            }
            "component.update_visibility" => {
                let cid = op
                    .target
                    .component_id
                    .as_deref()
                    .or_else(|| op.payload.get("componentId").and_then(|v| v.as_str()))
                    .ok_or_else(|| "componentId required".to_string())?;
                let visible = op
                    .payload
                    .get("visible")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                self.update_component_visibility(cid, visible)?;
            }
            "component.update_actions" => {
                let cid = op
                    .target
                    .component_id
                    .as_deref()
                    .or_else(|| op.payload.get("componentId").and_then(|v| v.as_str()))
                    .ok_or_else(|| "componentId required".to_string())?;
                let actions: Vec<ActionDefinition> = serde_json::from_value(
                    op.payload
                        .get("actions")
                        .cloned()
                        .unwrap_or_else(|| json!([])),
                )
                .map_err(|e| format!("invalid actions payload: {e}"))?;
                self.update_component_actions(cid, actions)?;
            }
            "component.bind_state" => {
                let cid = op
                    .target
                    .component_id
                    .as_deref()
                    .or_else(|| op.payload.get("componentId").and_then(|v| v.as_str()))
                    .ok_or_else(|| "componentId required".to_string())?;
                let key = op
                    .payload
                    .get("key")
                    .and_then(|v| v.as_str())
                    .or_else(|| op.payload.get("valueKey").and_then(|v| v.as_str()))
                    .ok_or_else(|| "key or valueKey required".to_string())?;
                let initial_val = op.payload.get("initialValue").cloned();
                self.bind_state(cid, key, initial_val)?;
            }
            "component.bind_action" => {
                let cid = op
                    .target
                    .component_id
                    .as_deref()
                    .or_else(|| op.payload.get("componentId").and_then(|v| v.as_str()))
                    .ok_or_else(|| "componentId required".to_string())?;
                let action: ActionDefinition = serde_json::from_value(
                    op.payload
                        .get("action")
                        .cloned()
                        .unwrap_or_else(|| op.payload.clone()),
                )
                .map_err(|e| format!("invalid action payload: {e}"))?;
                self.bind_action(cid, action)?;
            }
            "component.set_style_token" => {
                let target_id = op
                    .target
                    .component_id
                    .as_deref()
                    .or_else(|| op.payload.get("targetId").and_then(|v| v.as_str()))
                    .or_else(|| op.payload.get("componentId").and_then(|v| v.as_str()))
                    .ok_or_else(|| "targetId or componentId required".to_string())?;
                let token = op
                    .payload
                    .get("token")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "token required".to_string())?;
                let val = op.payload.get("value").cloned().unwrap_or(Value::Null);
                self.set_style_token(target_id, token, val)?;
            }
            _ => {}
        }
        self.sync_components();
        Ok(())
    }
}

pub const ALLOWED_STYLE_TOKENS: &[&str] = &[
    "density",
    "variant",
    "accent",
    "spacing",
    "theme",
    "align",
    "size",
    "colorScheme",
    "radius",
    "elevation",
    "layoutRole",
    "colSpan",
    "rowSpan",
    "maxWidth",
    "gap",
    "fontSize",
    "fontWeight",
    "border",
    "background",
    "padding",
    "margin",
];

pub fn is_allowed_style_token(token: &str) -> bool {
    ALLOWED_STYLE_TOKENS.contains(&token)
}

fn find_in_tree<'a>(nodes: &'a [ToolComponent], id: &str) -> Option<&'a ToolComponent> {
    for n in nodes {
        if n.id == id {
            return Some(n);
        }
        if let Some(children) = &n.children {
            if let Some(found) = find_in_tree(children, id) {
                return Some(found);
            }
        }
    }
    None
}

fn find_in_tree_mut<'a>(nodes: &'a mut [ToolComponent], id: &str) -> Option<&'a mut ToolComponent> {
    for n in nodes {
        if n.id == id {
            return Some(n);
        }
        if let Some(children) = n.children.as_mut() {
            if let Some(found) = find_in_tree_mut(children, id) {
                return Some(found);
            }
        }
    }
    None
}

fn remove_from_tree(nodes: &mut Vec<ToolComponent>, id: &str) -> Option<ToolComponent> {
    if let Some(pos) = nodes.iter().position(|c| c.id == id) {
        return Some(nodes.remove(pos));
    }
    for n in nodes.iter_mut() {
        if let Some(children) = n.children.as_mut() {
            if let Some(removed) = remove_from_tree(children, id) {
                return Some(removed);
            }
        }
    }
    None
}

fn replace_in_tree(
    nodes: &mut [ToolComponent],
    id: &str,
    new_comp: ToolComponent,
) -> Option<ToolComponent> {
    let mut to_replace = Some(new_comp);
    fn inner(
        nodes: &mut [ToolComponent],
        id: &str,
        new_comp: &mut Option<ToolComponent>,
    ) -> Option<ToolComponent> {
        for n in nodes.iter_mut() {
            if n.id == id {
                if let Some(replacement) = new_comp.take() {
                    return Some(std::mem::replace(n, replacement));
                }
            }
            if let Some(children) = n.children.as_mut() {
                if let Some(old) = inner(children, id, new_comp) {
                    return Some(old);
                }
            }
        }
        None
    }
    inner(nodes, id, &mut to_replace)
}

/// Canonical admission validation function for SoftwareDocuments.
pub fn admit_software_document(
    existing: Option<&SoftwareDocument>,
    candidate: &SoftwareDocument,
    allowed_packs: &[String],
) -> Result<(), String> {
    admit_software_document_with_state(existing, candidate, allowed_packs, None, false)
}

/// Admission validation with state authority and customize mode context.
pub fn admit_software_document_with_state(
    existing: Option<&SoftwareDocument>,
    candidate: &SoftwareDocument,
    allowed_packs: &[String],
    surface_state: Option<&Value>,
    is_user_customizing: bool,
) -> Result<(), String> {
    // 1. Structure
    if candidate.id.trim().is_empty() {
        return Err("software document id cannot be empty".into());
    }
    if candidate.title.trim().is_empty() {
        return Err("software document title cannot be empty".into());
    }
    let mut section_ids = HashSet::new();
    for sec in &candidate.sections {
        if sec.id.trim().is_empty() {
            return Err("section id cannot be empty".into());
        }
        if !section_ids.insert(sec.id.clone()) {
            return Err(format!("duplicate section id '{}'", sec.id));
        }
        if let Some(ref pid) = sec.parent_region_id {
            if pid == &sec.id {
                return Err(format!("section '{}' cannot be its own parent", sec.id));
            }
        }
    }
    for sec in &candidate.sections {
        if let Some(ref pid) = sec.parent_region_id {
            if !section_ids.contains(pid) {
                return Err(format!(
                    "parent region '{pid}' for section '{}' does not exist",
                    sec.id
                ));
            }
        }
    }
    // Region cycle check
    for sec in &candidate.sections {
        let mut visited = HashSet::new();
        visited.insert(sec.id.clone());
        let mut curr = sec.parent_region_id.as_ref();
        while let Some(pid) = curr {
            if !visited.insert(pid.clone()) {
                return Err(format!(
                    "cycle detected in region hierarchy involving section '{pid}'"
                ));
            }
            curr = candidate
                .sections
                .iter()
                .find(|s| &s.id == pid)
                .and_then(|s| s.parent_region_id.as_ref());
        }
    }

    // 2. Validate all components against capability packs
    let mut all_comps = Vec::new();
    for sec in &candidate.sections {
        all_comps.extend(sec.components.clone());
    }
    super::packs::validate_tool_components_for_packs(&all_comps, allowed_packs)?;

    // 3. State contracts validation & security check
    let mut state_keys = HashSet::new();
    for sc in &candidate.state_contracts {
        if sc.key.trim().is_empty() {
            return Err("state contract key cannot be empty".into());
        }
        if !state_keys.insert(sc.key.clone()) {
            return Err(format!("duplicate state contract key '{}'", sc.key));
        }
        if let Some(ref sens) = sc.sensitivity {
            if sens == "sensitive" && sc.read_policy != "restricted" {
                return Err(format!(
                    "sensitive state contract '{}' must have restricted read policy",
                    sc.key
                ));
            }
        }
    }

    if !is_user_customizing {
        // SECURITY: Rule D: Origin authority enforcement for edits.
        // When editing an existing document, the model cannot escalate origin from "model"
        // to "user" for any contract. The kernel derives origin from trusted persisted state.
        // A model-declared contract with origin "user" would grant the model user-level
        // authority, which is an escalation attack.
        if let Some(existing_doc) = existing {
            for sc in &candidate.state_contracts {
                if let Some(existing_sc) = existing_doc
                    .state_contracts
                    .iter()
                    .find(|s| s.key == sc.key)
                {
                    // Cannot escalate from model to user origin
                    if existing_sc.origin == "model" && sc.origin == "user" {
                        return Err(format!(
                            "cannot escalate state contract '{}' origin from 'model' to 'user'",
                            sc.key
                        ));
                    }
                } else {
                    // New contract in edit context: reject non-model origins from untrusted sources
                    if sc.origin != "model" && sc.origin != "system" && sc.origin != "legacy" {
                        return Err(format!(
                            "new state contract '{}' cannot declare origin '{}' in edit context",
                            sc.key, sc.origin
                        ));
                    }
                }
            }
        }

        // Rule A: Existing opaque key protection.
        // A model cannot create a contract for an already-existing state key that had no contract.
        if let Some(Value::Object(ref state_map)) = surface_state {
            for state_key in state_map.keys() {
                let existed_in_contracts = existing
                    .map(|e| e.state_contracts.iter().any(|c| &c.key == state_key))
                    .unwrap_or(false);
                if !existed_in_contracts
                    && candidate
                        .state_contracts
                        .iter()
                        .any(|c| &c.key == state_key)
                {
                    return Err(format!(
                        "cannot create state contract for existing opaque state key '{state_key}'"
                    ));
                }
            }
        }

        // Rule B: Existing trusted contract preservation and anti-broadening.
        if let Some(existing_doc) = existing {
            for sc in &candidate.state_contracts {
                if let Some(existing_sc) = existing_doc
                    .state_contracts
                    .iter()
                    .find(|s| s.key == sc.key)
                {
                    if (existing_sc.read_policy == "restricted"
                        || existing_sc.sensitivity.as_deref() == Some("sensitive"))
                        && sc.read_policy != "restricted"
                    {
                        return Err(format!(
                            "cannot broaden read policy of sensitive/restricted state key '{}'",
                            sc.key
                        ));
                    }
                    if existing_sc.write_policy == "readonly" && sc.write_policy != "readonly" {
                        return Err(format!(
                            "cannot broaden write policy of readonly state key '{}'",
                            sc.key
                        ));
                    }
                    if existing_sc.write_policy == "user" && sc.write_policy == "model" {
                        return Err(format!(
                            "cannot broaden write policy from user-only to model for state key '{}'",
                            sc.key
                        ));
                    }
                    if existing_sc.sensitivity.as_deref() == Some("sensitive")
                        && sc.sensitivity.as_deref() != Some("sensitive")
                    {
                        return Err(format!(
                            "cannot remove sensitivity classification from state key '{}'",
                            sc.key
                        ));
                    }
                }
            }

            // Rule C: Omission protection.
            // Model replacement cannot omit pre-existing restricted/sensitive/readonly contracts.
            for existing_sc in &existing_doc.state_contracts {
                let is_protected = existing_sc.read_policy == "restricted"
                    || existing_sc.sensitivity.as_deref() == Some("sensitive")
                    || existing_sc.write_policy == "readonly";
                if is_protected
                    && !candidate
                        .state_contracts
                        .iter()
                        .any(|c| c.key == existing_sc.key)
                {
                    return Err(format!(
                        "replacement cannot omit protected state contract '{}'",
                        existing_sc.key
                    ));
                }
            }
        }
    }

    // 4. Action contracts validation
    for ac in &candidate.action_contracts {
        if ac.action_id.trim().is_empty() {
            return Err("action contract id cannot be empty".into());
        }
        if let Some(ref rk) = ac.result_key {
            let sc = candidate
                .state_contracts
                .iter()
                .find(|s| &s.key == rk)
                .ok_or_else(|| {
                    format!(
                        "action '{}' resultKey '{rk}' references undeclared state contract",
                        ac.action_id
                    )
                })?;
            if sc.write_policy == "readonly" {
                return Err(format!(
                    "action '{}' resultKey '{rk}' references readonly state contract",
                    ac.action_id
                ));
            }
        }
        if let Some(ref inputs) = ac.input_from_state {
            for (_param, skey) in inputs {
                let sc = candidate
                    .state_contracts
                    .iter()
                    .find(|s| &s.key == skey)
                    .ok_or_else(|| {
                        format!(
                            "action '{}' inputFromState references undeclared state contract '{skey}'",
                            ac.action_id
                        )
                    })?;
                if sc.read_policy == "restricted" || sc.sensitivity.as_deref() == Some("sensitive")
                {
                    return Err(format!(
                        "action '{}' cannot bind sensitive/restricted state key '{skey}' to input",
                        ac.action_id
                    ));
                }
            }
        }
    }

    // 5. Component state and action binding verification
    for sec in &candidate.sections {
        for comp in &sec.components {
            validate_component_bindings(
                comp,
                &candidate.state_contracts,
                &candidate.action_contracts,
            )?;
        }
    }

    Ok(())
}

fn validate_component_bindings(
    comp: &ToolComponent,
    contracts: &[StateContract],
    action_contracts: &[ActionContract],
) -> Result<(), String> {
    // 1. Validate value_key binding
    if let Some(ref vk) = comp.value_key {
        if !vk.is_empty() {
            let sc = contracts.iter().find(|s| &s.key == vk).ok_or_else(|| {
                format!(
                    "component '{}' binds to undeclared state contract key '{vk}'",
                    comp.id
                )
            })?;
            if sc.read_policy == "restricted" || sc.sensitivity.as_deref() == Some("sensitive") {
                return Err(format!(
                    "component '{}' binds to restricted/sensitive state key '{vk}'",
                    comp.id
                ));
            }
            let is_input_component = matches!(
                comp.component_type.as_str(),
                "input" | "textarea" | "select" | "checkbox" | "date_picker" | "number_input"
            );
            if is_input_component && sc.write_policy == "readonly" {
                return Err(format!(
                    "input component '{}' cannot bind to readonly state key '{vk}'",
                    comp.id
                ));
            }
        }
    }

    // 2. Validate props binding keys (valueKey, rowsKey, dataKey, selectionKey)
    if let Some(Value::Object(ref map)) = comp.props {
        for binding_prop in &["valueKey", "rowsKey", "dataKey", "selectionKey"] {
            if let Some(Value::String(ref vk)) = map.get(*binding_prop) {
                if !vk.is_empty() {
                    let sc = contracts.iter().find(|s| &s.key == vk).ok_or_else(|| {
                        format!(
                            "component '{}' prop '{binding_prop}' binds to undeclared state contract key '{vk}'",
                            comp.id
                        )
                    })?;
                    if sc.read_policy == "restricted"
                        || sc.sensitivity.as_deref() == Some("sensitive")
                    {
                        return Err(format!(
                            "component '{}' prop '{binding_prop}' binds to restricted/sensitive state key '{vk}'",
                            comp.id
                        ));
                    }
                    if *binding_prop == "selectionKey" && sc.write_policy == "readonly" {
                        return Err(format!(
                            "component '{}' prop 'selectionKey' cannot bind to readonly state key '{vk}'",
                            comp.id
                        ));
                    }
                }
            }
        }
    }

    // 3. Validate actions
    if let Some(ref actions) = comp.actions {
        for action in actions {
            match action {
                crate::ai::ActionDefinition::InvokeRegisteredAction {
                    action_name,
                    result_key,
                    input_from_state,
                    ..
                } => {
                    let _ac = action_contracts
                        .iter()
                        .find(|a| {
                            &a.action_name == action_name
                                || &a.action_id == action_name
                                || a.action_id == format!("{}-{}", comp.id, action_name.replace('.', "-"))
                        })
                        .ok_or_else(|| {
                            format!(
                                "component '{}' invokes action '{action_name}' without trusted ActionContract",
                                comp.id
                            )
                        })?;
                    if let Some(ref rk) = result_key {
                        let sc = contracts.iter().find(|s| &s.key == rk).ok_or_else(|| {
                            format!(
                                "component '{}' action '{action_name}' resultKey '{rk}' references undeclared state contract",
                                comp.id
                            )
                        })?;
                        if sc.write_policy == "readonly" {
                            return Err(format!(
                                "component '{}' action '{action_name}' resultKey '{rk}' references readonly state contract",
                                comp.id
                            ));
                        }
                    }
                    if let Some(ref inputs) = input_from_state {
                        for (_param, skey) in inputs {
                            let sc = contracts.iter().find(|s| &s.key == skey).ok_or_else(|| {
                                format!(
                                    "component '{}' action '{action_name}' inputFromState references undeclared state contract '{skey}'",
                                    comp.id
                                )
                            })?;
                            if sc.read_policy == "restricted"
                                || sc.sensitivity.as_deref() == Some("sensitive")
                            {
                                return Err(format!(
                                    "component '{}' action '{action_name}' binds restricted/sensitive state key '{skey}' to input",
                                    comp.id
                                ));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // 4. Validate nested children
    if let Some(ref children) = comp.children {
        for child in children {
            validate_component_bindings(child, contracts, action_contracts)?;
        }
    }

    Ok(())
}

fn repair_component(
    comp: &mut ToolComponent,
    seen_ids: &mut HashSet<String>,
    _declared_state: &mut HashSet<String>,
    contracts: &mut Vec<StateContract>,
    notes: &mut Vec<RepairNote>,
) {
    // Deduplicate component ID
    if !seen_ids.insert(comp.id.clone()) {
        let mut disambiguated = format!("{}-dup", comp.id);
        let mut count = 1;
        while seen_ids.contains(&disambiguated) {
            count += 1;
            disambiguated = format!("{}-dup{}", comp.id, count);
        }
        notes.push(RepairNote {
            kind: "disambiguated_component_id".into(),
            target_id: comp.id.clone(),
            detail: format!("Duplicate id changed to '{disambiguated}'"),
        });
        comp.id = disambiguated.clone();
        seen_ids.insert(disambiguated);
    }

    // Sanitize props against arbitrary script injection
    if let Some(Value::Object(ref mut map)) = comp.props {
        map.retain(|k, v| {
            if k.starts_with("on") && k.len() > 2 {
                notes.push(RepairNote {
                    kind: "stripped_inline_event_handler".into(),
                    target_id: comp.id.clone(),
                    detail: format!("Stripped unsafe inline event handler prop '{k}'"),
                });
                return false;
            }
            if let Value::String(s) = v {
                if s.contains("<script") || s.starts_with("javascript:") {
                    notes.push(RepairNote {
                        kind: "sanitized_script_injection".into(),
                        target_id: comp.id.clone(),
                        detail: format!("Removed unsafe script injection in prop '{k}'"),
                    });
                    return false;
                }
            }
            true
        });

        // Strip unauthorized state binding props (valueKey, rowsKey, dataKey, selectionKey)
        for binding_prop in &["valueKey", "rowsKey", "dataKey", "selectionKey"] {
            if let Some(Value::String(ref vkey)) = map.get(*binding_prop).cloned() {
                if !vkey.is_empty() {
                    let is_unauthorized = if let Some(existing_contract) =
                        contracts.iter().find(|sc| &sc.key == vkey)
                    {
                        existing_contract.read_policy == "restricted"
                            || existing_contract.sensitivity.as_deref() == Some("sensitive")
                    } else {
                        true
                    };
                    if is_unauthorized {
                        map.remove(*binding_prop);
                        notes.push(RepairNote {
                            kind: "stripped_unauthorized_state_binding".into(),
                            target_id: comp.id.clone(),
                            detail: format!("Stripped unauthorized state binding prop '{binding_prop}' for key '{vkey}'"),
                        });
                    }
                }
            }
        }
    }

    // Validate or strip state bindings for bound value keys
    let vkey_opt = comp.value_key.clone();
    if let Some(vkey) = vkey_opt.as_deref() {
        if !vkey.is_empty() {
            if let Some(existing_contract) = contracts.iter().find(|sc| sc.key == vkey) {
                // Reject/strip binding to restricted or sensitive state
                if existing_contract.read_policy == "restricted"
                    || existing_contract.sensitivity.as_deref() == Some("sensitive")
                {
                    comp.value_key = None;
                    notes.push(RepairNote {
                        kind: "stripped_unauthorized_state_binding".into(),
                        target_id: comp.id.clone(),
                        detail: format!(
                            "Stripped unauthorized access to restricted state key '{vkey}'"
                        ),
                    });
                }
            } else {
                comp.value_key = None;
                notes.push(RepairNote {
                    kind: "stripped_unauthorized_state_binding".into(),
                    target_id: comp.id.clone(),
                    detail: format!("Stripped state binding for undeclared state key '{vkey}'"),
                });
            }
        }
    }

    // Recursively repair children
    if let Some(ref mut children) = comp.children {
        for child in children {
            repair_component(child, seen_ids, _declared_state, contracts, notes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_software_document_lifecycle() {
        let mut doc = SoftwareDocument::new("doc-research", "Research Dashboard");
        assert_eq!(doc.sections.len(), 0);

        let section = DocumentSection {
            id: "papers".into(),
            title: Some("Saved Papers".into()),
            role: Some("content".into()),
            layout: Some("stack".into()),
            components: vec![ToolComponent {
                id: "paper-list".into(),
                component_type: "list".into(),
                value_key: Some("selectedPaper".into()),
                props: Some(json!({ "dense": true })),
                ..Default::default()
            }],
            ..Default::default()
        };

        assert!(doc.add_section(section, None).is_ok());
        assert_eq!(doc.sections.len(), 1);

        // Self-repair strips undeclared state binding without inventing authority
        let notes = doc.validate_and_repair();
        assert!(!notes.is_empty());
        assert!(notes
            .iter()
            .any(|n| n.kind == "stripped_unauthorized_state_binding"));
        assert_eq!(doc.sections[0].components[0].value_key, None);

        // When a trusted state contract is explicitly declared:
        doc.state_contracts.push(StateContract::new(
            "selectedPaper",
            Value::Null,
            StateScope::Persistent,
        ));
        doc.sections[0].components[0].value_key = Some("selectedPaper".into());
        let notes2 = doc.validate_and_repair();
        assert!(
            notes2.is_empty()
                || !notes2
                    .iter()
                    .any(|n| n.kind == "stripped_unauthorized_state_binding")
        );
        assert_eq!(
            doc.sections[0].components[0].value_key,
            Some("selectedPaper".into())
        );

        // Admission succeeds with trusted contract
        assert!(admit_software_document(None, &doc, &[]).is_ok());

        // Render to tool definition
        let tool = doc.to_tool_definition();
        assert_eq!(tool.id, "doc-research");
        assert!(!tool.components.is_empty());
    }

    #[test]
    fn test_admit_rejects_undeclared_or_restricted_state() {
        let mut doc = SoftwareDocument::new("doc-sec", "Security Test");
        let section = DocumentSection {
            id: "main".into(),
            components: vec![ToolComponent {
                id: "input-1".into(),
                component_type: "textInput".into(),
                value_key: Some("secretKey".into()),
                props: Some(json!({ "label": "Secret" })),
                ..Default::default()
            }],
            ..Default::default()
        };
        doc.sections.push(section);

        // Undeclared state binding -> rejected by admission
        let packs = vec!["coreside.core".to_string()];
        let err = admit_software_document(None, &doc, &packs).unwrap_err();
        assert!(
            err.contains("undeclared state contract key 'secretKey'"),
            "Actual err was: {err}"
        );

        // Declare contract as restricted/sensitive -> rejected by admission
        let mut restricted_contract =
            StateContract::new("secretKey", Value::Null, StateScope::Persistent);
        restricted_contract.read_policy = "restricted".into();
        doc.state_contracts.push(restricted_contract);

        let err2 = admit_software_document(None, &doc, &packs).unwrap_err();
        assert!(err2.contains("restricted/sensitive state key 'secretKey'"));
    }

    #[test]
    fn test_section_hierarchy_round_trip_preservation() {
        let mut doc = SoftwareDocument::new("doc-hierarchy", "Hierarchy Test");
        doc.sections.push(DocumentSection {
            id: "header".into(),
            title: Some("Header Section".into()),
            role: Some("navigation".into()),
            layout: Some("stack".into()),
            parent_region_id: None,
            slot: None,
            responsive: None,
            instance_id: None,
            components: vec![ToolComponent {
                id: "nav-title".into(),
                component_type: "heading".into(),
                ..Default::default()
            }],
            metadata: None,
        });
        doc.sections.push(DocumentSection {
            id: "sub-sidebar".into(),
            title: Some("Sub Sidebar".into()),
            role: Some("sidebar".into()),
            layout: Some("stack".into()),
            parent_region_id: Some("header".into()),
            slot: Some("left".into()),
            responsive: None,
            instance_id: None,
            components: vec![ToolComponent {
                id: "nav-link".into(),
                component_type: "button".into(),
                ..Default::default()
            }],
            metadata: None,
        });

        let tool = doc.to_tool_definition();
        // Check that synthetic section containers preserved the hierarchy
        let round_trip = SoftwareDocument::from_tool_definition(&tool);
        assert_eq!(round_trip.sections.len(), 2);
        assert_eq!(round_trip.sections[0].id, "header");
        assert_eq!(round_trip.sections[0].role.as_deref(), Some("navigation"));
        assert_eq!(round_trip.sections[1].id, "sub-sidebar");
        assert_eq!(
            round_trip.sections[1].parent_region_id.as_deref(),
            Some("header")
        );
        assert_eq!(round_trip.sections[1].components[0].id, "nav-link");
    }

    #[test]
    fn test_self_repair_deduplicates_and_sanitizes() {
        let mut doc = SoftwareDocument::new("doc-test", "Test");
        let section = DocumentSection {
            id: "main".into(),
            components: vec![
                ToolComponent {
                    id: "btn-1".into(),
                    component_type: "button".into(),
                    props: Some(json!({
                        "label": "Click me",
                        "onclick": "alert('xss')",
                        "dangerHtml": "<script>evil()</script>"
                    })),
                    ..Default::default()
                },
                ToolComponent {
                    id: "btn-1".into(), // Duplicate!
                    component_type: "button".into(),
                    props: Some(json!({ "label": "Second" })),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        doc.add_section(section, None).unwrap();

        let notes = doc.validate_and_repair();
        assert!(notes.iter().any(|n| n.kind == "disambiguated_component_id"));
        assert!(notes
            .iter()
            .any(|n| n.kind == "stripped_inline_event_handler"));
        assert!(notes.iter().any(|n| n.kind == "sanitized_script_injection"));

        // Verify IDs are distinct
        assert_ne!(
            doc.sections[0].components[0].id,
            doc.sections[0].components[1].id
        );
    }

    #[test]
    fn test_regional_hierarchy_and_targeting() {
        let mut doc = SoftwareDocument::new("doc-surface", "Multi-Region Surface");

        let header = DocumentSection {
            id: "header".into(),
            title: Some("Header".into()),
            role: Some("header".into()),
            ..Default::default()
        };
        let main = DocumentSection {
            id: "main".into(),
            title: Some("Main Body".into()),
            role: Some("content".into()),
            ..Default::default()
        };
        let sidebar = DocumentSection {
            id: "sidebar".into(),
            title: Some("Sidebar Filters".into()),
            role: Some("sidebar".into()),
            parent_region_id: Some("main".into()),
            slot: Some("left".into()),
            components: vec![ToolComponent {
                id: "filter-input".into(),
                component_type: "input".into(),
                value_key: Some("active_filter".into()),
                ..Default::default()
            }],
            ..Default::default()
        };
        let content_area = DocumentSection {
            id: "content-area".into(),
            title: Some("Data Grid".into()),
            role: Some("content".into()),
            parent_region_id: Some("main".into()),
            slot: Some("center".into()),
            components: vec![ToolComponent {
                id: "grid-1".into(),
                component_type: "data-table".into(),
                ..Default::default()
            }],
            ..Default::default()
        };

        doc.add_section(header, None).unwrap();
        doc.add_section(main, None).unwrap();
        doc.add_section(sidebar, None).unwrap();
        doc.add_section(content_area, None).unwrap();

        assert_eq!(doc.sections.len(), 4);

        // Test regional queries
        let children = doc.get_child_regions("main");
        assert_eq!(children.len(), 2);
        assert!(children.iter().any(|c| c.id == "sidebar"));
        assert!(children.iter().any(|c| c.id == "content-area"));

        let path = doc.get_region_path("sidebar");
        assert_eq!(path.as_deref(), Some("doc-surface/main/sidebar"));

        // Test component lookup with region
        let (found_sec, found_comp) = doc.find_component_with_region("filter-input").unwrap();
        assert_eq!(found_comp.id, "filter-input");
        assert_eq!(found_sec.id, "sidebar");

        // Move component between regions
        assert!(doc
            .move_component("filter-input", "content-area", Some(0))
            .is_ok());
        let (new_sec, _) = doc.find_component_with_region("filter-input").unwrap();
        assert_eq!(new_sec.id, "content-area");
    }

    #[test]
    fn test_state_scopes_and_preservation() {
        let mut doc = SoftwareDocument::new("doc-stateful", "Stateful App");

        doc.state_contracts.push(StateContract {
            key: "user_draft".into(),
            type_name: "string".into(),
            initial_value: json!("draft text"),
            description: None,
            scope: StateScope::Persistent,
            preservation_policy: Some("keep_on_patch".into()),
            ..Default::default()
        });
        doc.state_contracts.push(StateContract {
            key: "selected_tab".into(),
            type_name: "string".into(),
            initial_value: json!("tab-1"),
            description: None,
            scope: StateScope::Session,
            preservation_policy: None,
            ..Default::default()
        });
        doc.state_contracts.push(StateContract {
            key: "is_loading".into(),
            type_name: "boolean".into(),
            initial_value: json!(false),
            description: None,
            scope: StateScope::InFlight,
            preservation_policy: None,
            ..Default::default()
        });

        let mut active_state = HashMap::new();
        active_state.insert("user_draft".into(), json!("user modified text"));
        active_state.insert("selected_tab".into(), json!("tab-2"));
        active_state.insert("is_loading".into(), json!(true));
        active_state.insert("untracked_scratch".into(), json!("temp"));

        // Filter persistent states
        let persistent = doc.filter_state_by_scope(&active_state, StateScope::Persistent);
        assert_eq!(persistent.len(), 1);
        assert_eq!(
            persistent.get("user_draft").unwrap(),
            &json!("user modified text")
        );
        assert!(!persistent.contains_key("selected_tab"));
        assert!(!persistent.contains_key("is_loading"));

        // Filter session states
        let session = doc.filter_state_by_scope(&active_state, StateScope::Session);
        assert_eq!(session.len(), 1);
        assert_eq!(session.get("selected_tab").unwrap(), &json!("tab-2"));

        // Preserve state values according to declared scopes
        let preserved = doc.preserve_state_values(&active_state);
        assert_eq!(
            preserved.get("user_draft").unwrap(),
            &json!("user modified text")
        );
        assert_eq!(preserved.get("selected_tab").unwrap(), &json!("tab-2"));
        assert!(!preserved.contains_key("is_loading"));
    }

    #[test]
    fn test_region_cycle_detection_self_cycle() {
        // P0.7: add_section now prevents self-reference through the API.
        // To test that get_region_path handles cycles gracefully, we inject one
        // directly into sections (simulating external deserialization or DB corruption).
        let mut doc = SoftwareDocument::new("doc-cycle", "Cycle Test");
        // Direct injection bypasses add_section validation — testing defensive get_region_path.
        doc.sections.push(DocumentSection {
            id: "a".into(),
            parent_region_id: Some("a".into()), // self-cycle: injected directly
            ..Default::default()
        });
        // get_region_path must return None, not loop forever
        assert_eq!(doc.get_region_path("a"), None);
    }

    #[test]
    fn test_add_section_rejects_self_parent() {
        // P0.7: add_section must reject a section that names itself as parent.
        let mut doc = SoftwareDocument::new("doc-self-parent", "Self Parent Test");
        let result = doc.add_section(
            DocumentSection {
                id: "a".into(),
                parent_region_id: Some("a".into()),
                ..Default::default()
            },
            None,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("own parent"));
        // Section must not have been added.
        assert!(doc.sections.is_empty());
    }

    #[test]
    fn test_region_cycle_detection_two_node_cycle() {
        // Inject a two-node cycle directly (not via add_section which would reject it).
        let mut doc = SoftwareDocument::new("doc-cycle2", "Cycle Test 2");
        doc.sections.push(DocumentSection {
            id: "a".into(),
            parent_region_id: Some("b".into()),
            ..Default::default()
        });
        doc.sections.push(DocumentSection {
            id: "b".into(),
            parent_region_id: Some("a".into()),
            ..Default::default()
        });
        assert_eq!(doc.get_region_path("a"), None);
        assert_eq!(doc.get_region_path("b"), None);
    }

    #[test]
    fn test_add_section_rejects_nonexistent_parent() {
        // P0.7: add_section must reject a section whose parent doesn't exist yet.
        let mut doc = SoftwareDocument::new("doc-orphan", "Orphan Test");
        let result = doc.add_section(
            DocumentSection {
                id: "child".into(),
                parent_region_id: Some("nonexistent-parent".into()),
                ..Default::default()
            },
            None,
        );
        assert!(result.is_err());
        assert!(
            result.unwrap_err().contains("does not exist"),
            "error must mention parent does not exist"
        );
        assert!(doc.sections.is_empty());
    }

    #[test]
    fn test_region_cycle_detection_three_node_cycle() {
        // Inject a three-node cycle directly.
        let mut doc = SoftwareDocument::new("doc-cycle3", "Cycle Test 3");
        doc.sections.push(DocumentSection {
            id: "a".into(),
            parent_region_id: Some("b".into()),
            ..Default::default()
        });
        doc.sections.push(DocumentSection {
            id: "b".into(),
            parent_region_id: Some("c".into()),
            ..Default::default()
        });
        doc.sections.push(DocumentSection {
            id: "c".into(),
            parent_region_id: Some("a".into()),
            ..Default::default()
        });
        assert_eq!(doc.get_region_path("a"), None);
    }

    #[test]
    fn test_region_cycle_detection_missing_parent() {
        // P0.7 prevents adding orphans via add_section, but if one is injected
        // directly, get_region_path should stop gracefully at the missing parent.
        let mut doc = SoftwareDocument::new("doc-missing", "Missing Parent");
        // Direct injection — bypasses add_section validation.
        doc.sections.push(DocumentSection {
            id: "orphan".into(),
            parent_region_id: Some("nonexistent".into()),
            ..Default::default()
        });
        // Should stop at the missing parent, not loop
        let path = doc.get_region_path("orphan");
        assert_eq!(path.as_deref(), Some("doc-missing/orphan"));
    }

    #[test]
    fn test_region_valid_deep_hierarchy_path() {
        let mut doc = SoftwareDocument::new("doc-deep", "Deep Hierarchy");
        doc.add_section(
            DocumentSection {
                id: "root".into(),
                ..Default::default()
            },
            None,
        )
        .unwrap();
        doc.add_section(
            DocumentSection {
                id: "level1".into(),
                parent_region_id: Some("root".into()),
                ..Default::default()
            },
            None,
        )
        .unwrap();
        doc.add_section(
            DocumentSection {
                id: "level2".into(),
                parent_region_id: Some("level1".into()),
                ..Default::default()
            },
            None,
        )
        .unwrap();
        let path = doc.get_region_path("level2");
        assert_eq!(path.as_deref(), Some("doc-deep/root/level1/level2"));
    }

    #[test]
    fn test_reorder_deterministic_unspecified_order() {
        let mut doc = SoftwareDocument::new("doc-reorder", "Reorder Test");
        doc.add_section(
            DocumentSection {
                id: "main".into(),
                components: vec![
                    ToolComponent {
                        id: "A".into(),
                        component_type: "text".into(),
                        ..Default::default()
                    },
                    ToolComponent {
                        id: "B".into(),
                        component_type: "text".into(),
                        ..Default::default()
                    },
                    ToolComponent {
                        id: "C".into(),
                        component_type: "text".into(),
                        ..Default::default()
                    },
                    ToolComponent {
                        id: "D".into(),
                        component_type: "text".into(),
                        ..Default::default()
                    },
                    ToolComponent {
                        id: "E".into(),
                        component_type: "text".into(),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
            None,
        )
        .unwrap();

        // Request D, B as the specified order
        doc.reorder_components("main", &["D".into(), "B".into()])
            .unwrap();

        let ids: Vec<&str> = doc.sections[0]
            .components
            .iter()
            .map(|c| c.id.as_str())
            .collect();
        // Specified: D, B. Unspecified (original relative order): A, C, E
        assert_eq!(ids, vec!["D", "B", "A", "C", "E"]);
    }

    #[test]
    fn test_reorder_all_specified() {
        let mut doc = SoftwareDocument::new("doc-reorder2", "Reorder Test 2");
        doc.add_section(
            DocumentSection {
                id: "main".into(),
                components: vec![
                    ToolComponent {
                        id: "A".into(),
                        component_type: "text".into(),
                        ..Default::default()
                    },
                    ToolComponent {
                        id: "B".into(),
                        component_type: "text".into(),
                        ..Default::default()
                    },
                    ToolComponent {
                        id: "C".into(),
                        component_type: "text".into(),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
            None,
        )
        .unwrap();

        // Complete reorder
        doc.reorder_components("main", &["C".into(), "A".into(), "B".into()])
            .unwrap();

        let ids: Vec<&str> = doc.sections[0]
            .components
            .iter()
            .map(|c| c.id.as_str())
            .collect();
        assert_eq!(ids, vec!["C", "A", "B"]);
    }

    #[test]
    fn test_reorder_partial_specified_preserves_order() {
        let mut doc = SoftwareDocument::new("doc-reorder3", "Reorder Test 3");
        doc.add_section(
            DocumentSection {
                id: "main".into(),
                components: vec![
                    ToolComponent {
                        id: "A".into(),
                        component_type: "text".into(),
                        ..Default::default()
                    },
                    ToolComponent {
                        id: "B".into(),
                        component_type: "text".into(),
                        ..Default::default()
                    },
                    ToolComponent {
                        id: "C".into(),
                        component_type: "text".into(),
                        ..Default::default()
                    },
                    ToolComponent {
                        id: "D".into(),
                        component_type: "text".into(),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
            None,
        )
        .unwrap();

        // Only specify B
        doc.reorder_components("main", &["B".into()]).unwrap();

        let ids: Vec<&str> = doc.sections[0]
            .components
            .iter()
            .map(|c| c.id.as_str())
            .collect();
        // B first, then A, C, E (original order minus B)
        assert_eq!(ids, vec!["B", "A", "C", "D"]);
    }

    #[test]
    fn test_move_component_validates_destination_before_mutation() {
        let mut doc = SoftwareDocument::new("doc-move", "Move Test");
        doc.add_section(
            DocumentSection {
                id: "source".into(),
                components: vec![ToolComponent {
                    id: "widget".into(),
                    component_type: "button".into(),
                    ..Default::default()
                }],
                ..Default::default()
            },
            None,
        )
        .unwrap();

        // Try to move to nonexistent region — must fail and leave source untouched
        let result = doc.move_component("widget", "nonexistent", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));

        // Source component must still be there
        assert_eq!(doc.sections[0].components.len(), 1);
        assert_eq!(doc.sections[0].components[0].id, "widget");
    }

    #[test]
    fn test_move_component_validates_index_before_mutation() {
        let mut doc = SoftwareDocument::new("doc-move2", "Move Test 2");
        doc.add_section(
            DocumentSection {
                id: "source".into(),
                components: vec![ToolComponent {
                    id: "widget".into(),
                    component_type: "button".into(),
                    ..Default::default()
                }],
                ..Default::default()
            },
            None,
        )
        .unwrap();
        doc.add_section(
            DocumentSection {
                id: "target".into(),
                components: vec![],
                ..Default::default()
            },
            None,
        )
        .unwrap();

        // Index 5 exceeds target length 0
        let result = doc.move_component("widget", "target", Some(5));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("exceeds"));

        // Source untouched
        assert_eq!(doc.sections[0].components.len(), 1);
        assert_eq!(doc.sections[0].components[0].id, "widget");
    }

    #[test]
    fn test_move_component_validates_source_before_mutation() {
        let mut doc = SoftwareDocument::new("doc-move3", "Move Test 3");
        doc.add_section(
            DocumentSection {
                id: "source".into(),
                components: vec![],
                ..Default::default()
            },
            None,
        )
        .unwrap();
        doc.add_section(
            DocumentSection {
                id: "target".into(),
                components: vec![],
                ..Default::default()
            },
            None,
        )
        .unwrap();

        // Component doesn't exist
        let result = doc.move_component("nonexistent", "target", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_move_component_success() {
        let mut doc = SoftwareDocument::new("doc-move4", "Move Test 4");
        doc.add_section(
            DocumentSection {
                id: "source".into(),
                components: vec![ToolComponent {
                    id: "widget".into(),
                    component_type: "button".into(),
                    props: Some(json!({"label": "Click"})),
                    ..Default::default()
                }],
                ..Default::default()
            },
            None,
        )
        .unwrap();
        doc.add_section(
            DocumentSection {
                id: "target".into(),
                components: vec![],
                ..Default::default()
            },
            None,
        )
        .unwrap();

        assert!(doc.move_component("widget", "target", Some(0)).is_ok());
        // Source empty, target has the component
        assert_eq!(doc.sections[0].components.len(), 0);
        assert_eq!(doc.sections[1].components.len(), 1);
        assert_eq!(doc.sections[1].components[0].id, "widget");
    }

    // P0.5: same-region move edge cases — these previously panicked.

    fn make_region_with_three_comps(region_id: &str) -> DocumentSection {
        DocumentSection {
            id: region_id.into(),
            components: vec![
                ToolComponent {
                    id: "A".into(),
                    component_type: "text".into(),
                    ..Default::default()
                },
                ToolComponent {
                    id: "B".into(),
                    component_type: "text".into(),
                    ..Default::default()
                },
                ToolComponent {
                    id: "C".into(),
                    component_type: "text".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    fn ids(sec: &DocumentSection) -> Vec<&str> {
        sec.components.iter().map(|c| c.id.as_str()).collect()
    }

    #[test]
    fn test_same_region_move_first_to_last() {
        // P0.5: move A (pos 0) to end (index 3 = original length). Previously panicked.
        let mut doc = SoftwareDocument::new("doc-sr1", "Same Region 1");
        doc.add_section(make_region_with_three_comps("main"), None)
            .unwrap();
        // target_index = 3 means "after C" in the original list.
        assert!(doc.move_component("A", "main", Some(3)).is_ok());
        assert_eq!(ids(&doc.sections[0]), vec!["B", "C", "A"]);
    }

    #[test]
    fn test_same_region_move_last_to_first() {
        // P0.5: move C (pos 2) to index 0.
        let mut doc = SoftwareDocument::new("doc-sr2", "Same Region 2");
        doc.add_section(make_region_with_three_comps("main"), None)
            .unwrap();
        assert!(doc.move_component("C", "main", Some(0)).is_ok());
        assert_eq!(ids(&doc.sections[0]), vec!["C", "A", "B"]);
    }

    #[test]
    fn test_same_region_move_middle_to_end() {
        // P0.5: move B (pos 1) to index 3 (end). After removal B pos=1 is gone,
        // so len=2; adjusted index = 3-1 = 2, which is valid.
        let mut doc = SoftwareDocument::new("doc-sr3", "Same Region 3");
        doc.add_section(make_region_with_three_comps("main"), None)
            .unwrap();
        assert!(doc.move_component("B", "main", Some(3)).is_ok());
        assert_eq!(ids(&doc.sections[0]), vec!["A", "C", "B"]);
    }

    #[test]
    fn test_same_region_move_to_same_position() {
        // Moving to the same position is a no-op (A stays at 0).
        let mut doc = SoftwareDocument::new("doc-sr4", "Same Region 4");
        doc.add_section(make_region_with_three_comps("main"), None)
            .unwrap();
        assert!(doc.move_component("A", "main", Some(0)).is_ok());
        assert_eq!(ids(&doc.sections[0]), vec!["A", "B", "C"]);
    }

    #[test]
    fn test_same_region_move_no_index() {
        // Move to end (no index) in same region: A goes to the end.
        let mut doc = SoftwareDocument::new("doc-sr5", "Same Region 5");
        doc.add_section(make_region_with_three_comps("main"), None)
            .unwrap();
        assert!(doc.move_component("A", "main", None).is_ok());
        assert_eq!(ids(&doc.sections[0]), vec!["B", "C", "A"]);
    }

    // P0.6: remove_section must protect against orphaned children.

    #[test]
    fn test_remove_section_rejects_parent_with_children() {
        let mut doc = SoftwareDocument::new("doc-rem", "Remove Test");
        doc.add_section(
            DocumentSection {
                id: "parent".into(),
                ..Default::default()
            },
            None,
        )
        .unwrap();
        doc.add_section(
            DocumentSection {
                id: "child".into(),
                parent_region_id: Some("parent".into()),
                ..Default::default()
            },
            None,
        )
        .unwrap();
        let result = doc.remove_section("parent");
        assert!(result.is_err());
        assert!(
            result.unwrap_err().contains("orphaned"),
            "error must mention orphaning"
        );
        // Both sections must still be there.
        assert_eq!(doc.sections.len(), 2);
    }

    #[test]
    fn test_remove_section_leaf_succeeds() {
        let mut doc = SoftwareDocument::new("doc-rem2", "Remove Test 2");
        doc.add_section(
            DocumentSection {
                id: "parent".into(),
                ..Default::default()
            },
            None,
        )
        .unwrap();
        doc.add_section(
            DocumentSection {
                id: "leaf".into(),
                parent_region_id: Some("parent".into()),
                ..Default::default()
            },
            None,
        )
        .unwrap();
        // Remove the leaf — no children, must succeed.
        let removed = doc.remove_section("leaf");
        assert!(removed.is_ok());
        assert_eq!(removed.unwrap().id, "leaf");
        assert_eq!(doc.sections.len(), 1);
    }

    #[test]
    fn test_remove_section_recursive_removes_subtree() {
        let mut doc = SoftwareDocument::new("doc-rec-rem", "Recursive Remove");
        doc.add_section(
            DocumentSection {
                id: "root".into(),
                ..Default::default()
            },
            None,
        )
        .unwrap();
        doc.add_section(
            DocumentSection {
                id: "child".into(),
                parent_region_id: Some("root".into()),
                ..Default::default()
            },
            None,
        )
        .unwrap();
        doc.add_section(
            DocumentSection {
                id: "grandchild".into(),
                parent_region_id: Some("child".into()),
                ..Default::default()
            },
            None,
        )
        .unwrap();
        // Also a sibling of child.
        doc.add_section(
            DocumentSection {
                id: "sibling".into(),
                parent_region_id: Some("root".into()),
                ..Default::default()
            },
            None,
        )
        .unwrap();

        let removed = doc.remove_section_recursive("child").unwrap();
        assert!(removed.contains(&"child".to_string()));
        assert!(removed.contains(&"grandchild".to_string()));
        assert!(!removed.contains(&"sibling".to_string()));
        // root and sibling remain; child and grandchild are gone.
        assert_eq!(doc.sections.len(), 2);
        assert!(doc.find_region("root").is_some());
        assert!(doc.find_region("sibling").is_some());
        assert!(doc.find_region("child").is_none());
        assert!(doc.find_region("grandchild").is_none());
    }
}
