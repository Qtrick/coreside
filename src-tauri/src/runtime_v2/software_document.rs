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
}

fn default_type_name() -> String {
    "string".to_string()
}

/// Declared action contract for runtime effects.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActionContract {
    pub action_id: String,
    pub action_name: String,
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
    #[serde(default)]
    pub sections: Vec<DocumentSection>,
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
            sections: Vec::new(),
            state_contracts: Vec::new(),
            action_contracts: Vec::new(),
            design_tokens: None,
            capability_packs: vec!["core".into(), "base-layout".into()],
        }
    }

    /// Load or convert a JSON Value into a SoftwareDocument without losing contracts.
    pub fn from_value(val: &Value) -> Result<Self, String> {
        if val.get("sections").is_some() {
            serde_json::from_value(val.clone())
                .map_err(|e| format!("failed to deserialize SoftwareDocument: {e}"))
        } else {
            let tool: ToolDefinition = serde_json::from_value(val.clone())
                .map_err(|e| format!("failed to parse ToolDefinition: {e}"))?;
            Ok(Self::from_tool_definition(&tool))
        }
    }

    /// Convert a flat ToolDefinition into a structured SoftwareDocument.
    pub fn from_tool_definition(tool: &ToolDefinition) -> Self {
        let mut doc = Self::new(&tool.id, &tool.name);
        doc.description = if tool.description.is_empty() {
            None
        } else {
            Some(tool.description.clone())
        };

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

        doc
    }

    /// Render/flatten SoftwareDocument into a ToolDefinition for runtime presentation.
    pub fn to_tool_definition(&self) -> ToolDefinition {
        let mut all_components = Vec::new();

        for section in &self.sections {
            for comp in &section.components {
                let mut c = comp.clone();
                // Ensure section tagging is recorded in component props
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

        ToolDefinition {
            id: self.id.clone(),
            name: self.title.clone(),
            description: self.description.clone().unwrap_or_default(),
            layout: json!({ "type": "single-column" }),
            components: all_components,
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

        // 2. Validate target index if provided.
        if let Some(idx) = target_index {
            if idx > target_len {
                return Err(format!(
                    "target index {idx} exceeds region length {target_len}"
                ));
            }
        }

        // 3. Find and extract the component from source region.
        let mut extracted: Option<ToolComponent> = None;
        for sec in &mut self.sections {
            if let Some(pos) = sec.components.iter().position(|c| c.id == component_id) {
                extracted = Some(sec.components.remove(pos));
                break;
            }
        }
        let mut comp =
            extracted.ok_or_else(|| format!("component '{component_id}' not found"))?;

        // 4. Update component's section prop if present.
        if let Some(Value::Object(ref mut map)) = comp.props {
            map.insert("section".into(), json!(target_region_id));
        }

        // 5. Insert into target region.
        // Safe to unwrap: we validated target exists in step 1.
        let target_sec = self.find_region_mut(target_region_id).unwrap();
        match target_index {
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

    /// Find a component along with its enclosing section/region.
    pub fn find_component_with_region(
        &self,
        component_id: &str,
    ) -> Option<(&DocumentSection, &ToolComponent)> {
        for sec in &self.sections {
            for comp in &sec.components {
                if comp.id == component_id {
                    return Some((sec, comp));
                }
            }
        }
        None
    }

    /// Update or declare state scope for an existing or new key.
    pub fn update_state_scope(&mut self, key: &str, scope: StateScope) -> Result<(), String> {
        if let Some(contract) = self.state_contracts.iter_mut().find(|sc| sc.key == key) {
            contract.scope = scope;
        } else {
            self.state_contracts.push(StateContract {
                key: key.to_string(),
                type_name: "string".to_string(),
                initial_value: Value::Null,
                scope,
                description: None,
                preservation_policy: Some("preserve".into()),
            });
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
                // If not explicitly declared ephemeral, preserve by default for safety
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
        // Ensure default layout role if not provided
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
        if let Some(pos) = self.sections.iter().position(|s| s.id == section_id) {
            Ok(self.sections.remove(pos))
        } else {
            Err(format!("section '{section_id}' not found"))
        }
    }

    /// Update section components or attributes.
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

    /// Bind a component to a state contract, generating one if missing.
    pub fn bind_state(
        &mut self,
        component_id: &str,
        key: &str,
        initial_value: Option<Value>,
    ) -> Result<(), String> {
        let mut found = false;
        for sec in &mut self.sections {
            for comp in &mut sec.components {
                if comp.id == component_id {
                    comp.value_key = Some(key.to_string());
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }
        if !found {
            return Err(format!("component '{component_id}' not found"));
        }

        // Add state contract if not already defined
        if !self.state_contracts.iter().any(|sc| sc.key == key) {
            self.state_contracts.push(StateContract {
                key: key.to_string(),
                type_name: "string".to_string(),
                initial_value: initial_value.unwrap_or(Value::Null),
                scope: StateScope::Persistent,
                description: Some(format!("Bound to component '{component_id}'")),
                preservation_policy: Some("preserve".into()),
            });
        }

        Ok(())
    }

    /// Bind an action contract to a component.
    pub fn bind_action(
        &mut self,
        component_id: &str,
        action: ActionDefinition,
    ) -> Result<(), String> {
        let mut found = false;
        for sec in &mut self.sections {
            for comp in &mut sec.components {
                if comp.id == component_id {
                    let mut actions = comp.actions.take().unwrap_or_default();
                    actions.push(action.clone());
                    comp.actions = Some(actions);
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }
        if !found {
            return Err(format!("component '{component_id}' not found"));
        }

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
                    description: Some(format!("Triggered from component '{component_id}'")),
                    result_key: result_key.clone(),
                    input_from_state: input_from_state.clone(),
                });
            }
        }

        Ok(())
    }

    /// Apply a safe design style token (e.g. density, variant, accent).
    pub fn set_style_token(
        &mut self,
        target_id: &str,
        token: &str,
        val: Value,
    ) -> Result<(), String> {
        // First check if target is a section
        if let Some(sec) = self.sections.iter_mut().find(|s| s.id == target_id) {
            let mut meta = sec.metadata.take().unwrap_or_else(|| json!({}));
            if let Value::Object(ref mut m) = meta {
                m.insert(token.to_string(), val);
            }
            sec.metadata = Some(meta);
            return Ok(());
        }

        // Next check if target is a component
        for sec in &mut self.sections {
            for comp in &mut sec.components {
                if comp.id == target_id {
                    let mut props = comp.props.take().unwrap_or_else(|| json!({}));
                    if let Value::Object(ref mut m) = props {
                        m.insert(token.to_string(), val);
                    }
                    comp.props = Some(props);
                    return Ok(());
                }
            }
        }

        Err(format!(
            "target '{target_id}' not found as section or component"
        ))
    }
}

fn repair_component(
    comp: &mut ToolComponent,
    seen_ids: &mut HashSet<String>,
    declared_state: &mut HashSet<String>,
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
    }

    // Auto-repair missing state contracts for bound value keys
    if let Some(vkey) = comp.value_key.as_deref() {
        if !vkey.is_empty() && declared_state.insert(vkey.to_string()) {
            contracts.push(StateContract {
                key: vkey.to_string(),
                type_name: "string".to_string(),
                initial_value: Value::Null,
                scope: StateScope::Persistent,
                description: Some(format!(
                    "Auto-repaired state contract for component '{}'",
                    comp.id
                )),
                preservation_policy: Some("preserve".into()),
            });
            notes.push(RepairNote {
                kind: "added_missing_state_contract".into(),
                target_id: comp.id.clone(),
                detail: format!("Created implicit state contract for valueKey '{vkey}'"),
            });
        }
    }

    // Recursively repair children
    if let Some(ref mut children) = comp.children {
        for child in children {
            repair_component(child, seen_ids, declared_state, contracts, notes);
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

        // Self-repair adds missing state contract
        let notes = doc.validate_and_repair();
        assert!(!notes.is_empty());
        assert!(doc
            .state_contracts
            .iter()
            .any(|sc| sc.key == "selectedPaper"));

        // Render to tool definition
        let tool = doc.to_tool_definition();
        assert_eq!(tool.id, "doc-research");
        assert_eq!(tool.components.len(), 1);
        assert_eq!(tool.components[0].id, "paper-list");
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
        });
        doc.state_contracts.push(StateContract {
            key: "selected_tab".into(),
            type_name: "string".into(),
            initial_value: json!("tab-1"),
            description: None,
            scope: StateScope::Session,
            preservation_policy: None,
        });
        doc.state_contracts.push(StateContract {
            key: "is_loading".into(),
            type_name: "boolean".into(),
            initial_value: json!(false),
            description: None,
            scope: StateScope::InFlight,
            preservation_policy: None,
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
        let mut doc = SoftwareDocument::new("doc-cycle", "Cycle Test");
        let sec = DocumentSection {
            id: "a".into(),
            parent_region_id: Some("a".into()), // self-cycle
            ..Default::default()
        };
        doc.add_section(sec, None).unwrap();
        // get_region_path must return None, not loop forever
        assert_eq!(doc.get_region_path("a"), None);
    }

    #[test]
    fn test_region_cycle_detection_two_node_cycle() {
        let mut doc = SoftwareDocument::new("doc-cycle2", "Cycle Test 2");
        doc.add_section(
            DocumentSection {
                id: "a".into(),
                parent_region_id: Some("b".into()),
                ..Default::default()
            },
            None,
        )
        .unwrap();
        doc.add_section(
            DocumentSection {
                id: "b".into(),
                parent_region_id: Some("a".into()),
                ..Default::default()
            },
            None,
        )
        .unwrap();
        assert_eq!(doc.get_region_path("a"), None);
        assert_eq!(doc.get_region_path("b"), None);
    }

    #[test]
    fn test_region_cycle_detection_three_node_cycle() {
        let mut doc = SoftwareDocument::new("doc-cycle3", "Cycle Test 3");
        doc.add_section(
            DocumentSection {
                id: "a".into(),
                parent_region_id: Some("b".into()),
                ..Default::default()
            },
            None,
        )
        .unwrap();
        doc.add_section(
            DocumentSection {
                id: "b".into(),
                parent_region_id: Some("c".into()),
                ..Default::default()
            },
            None,
        )
        .unwrap();
        doc.add_section(
            DocumentSection {
                id: "c".into(),
                parent_region_id: Some("a".into()),
                ..Default::default()
            },
            None,
        )
        .unwrap();
        assert_eq!(doc.get_region_path("a"), None);
    }

    #[test]
    fn test_region_cycle_detection_missing_parent() {
        let mut doc = SoftwareDocument::new("doc-missing", "Missing Parent");
        doc.add_section(
            DocumentSection {
                id: "orphan".into(),
                parent_region_id: Some("nonexistent".into()),
                ..Default::default()
            },
            None,
        )
        .unwrap();
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
}
