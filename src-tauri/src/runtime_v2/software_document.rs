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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSection {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
    #[serde(default)]
    pub components: Vec<ToolComponent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
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
                description: Some(format!("Bound to component '{component_id}'")),
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
                description: Some(format!(
                    "Auto-repaired state contract for component '{}'",
                    comp.id
                )),
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
            metadata: None,
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
            title: None,
            role: None,
            layout: None,
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
            metadata: None,
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
}
