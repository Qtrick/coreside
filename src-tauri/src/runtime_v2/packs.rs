//! Trusted capability-pack registry. No CDN / runtime package install.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityPackMeta {
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub component_types: Vec<String>,
    pub action_types: Vec<String>,
    pub event_types: Vec<String>,
    pub permissions: Vec<String>,
    pub agent_description: String,
    pub enabled: bool,
    pub bundled: bool,
}

fn pack(
    id: &str,
    name: &str,
    version: &str,
    components: &[&str],
    desc: &str,
    permissions: &[&str],
) -> CapabilityPackMeta {
    CapabilityPackMeta {
        id: id.into(),
        display_name: name.into(),
        version: version.into(),
        component_types: components.iter().map(|s| (*s).to_string()).collect(),
        action_types: vec!["submitToAgent".into(), "setValue".into()],
        event_types: vec!["state_changed".into(), "form_submitted".into()],
        permissions: permissions.iter().map(|s| (*s).to_string()).collect(),
        agent_description: desc.into(),
        enabled: true,
        bundled: true,
    }
}

pub fn bundled_packs() -> Vec<CapabilityPackMeta> {
    vec![
        pack(
            "coreside.core",
            "Core Components",
            "1.0.0",
            &[
                "container", "row", "column", "card", "tabs", "divider", "spacer",
                "heading", "text", "badge", "image", "emptyState", "textInput",
                "textArea", "numberInput", "select", "checkbox", "dateInput", "list",
                "checklist", "table", "counter", "progress", "stat", "button",
                "buttonGroup", "quiz", "clock",
            ],
            "Built-in layout, form, and feedback components.",
            &["render", "local_state"],
        ),
        pack(
            "coreside.forms",
            "Rich Forms",
            "1.0.0",
            &[
                "form", "fieldGroup", "radioGroup", "slider", "switch", "colorInput",
                "timeInput", "dateTimeInput", "filePicker", "mediaPicker",
                "submitButton", "resetButton", "validationMessage",
            ],
            "Trusted structured forms with validation and submitToAgent.",
            &["render", "local_state", "submit_to_agent"],
        ),
        pack(
            "coreside.svg",
            "SVG Scenes",
            "1.0.0",
            &["svgScene", "svgRect", "svgCircle", "svgEllipse", "svgLine", "svgPath", "svgText", "svgGroup"],
            "Declarative SVG without script or foreignObject.",
            &["render", "pointer_events"],
        ),
        pack(
            "coreside.charts",
            "Charts",
            "1.0.0",
            &["chartLine", "chartBar", "chartPie", "chartDonut", "chartArea", "chartScatter"],
            "Bundled chart components with accessible data tables. No CDN.",
            &["render"],
        ),
        pack(
            "coreside.code",
            "Code Editor",
            "1.0.0",
            &["codeEditor"],
            "Local code editing surface. Does not execute edited code.",
            &["render", "local_state", "submit_to_agent"],
        ),
        pack(
            "coreside.math",
            "Math",
            "1.0.0",
            &["mathInline", "mathBlock"],
            "Safe MathML rendering for equations.",
            &["render"],
        ),
        pack(
            "coreside.canvas",
            "Declarative Canvas",
            "1.0.0",
            &["canvasScene"],
            "Bounded Canvas 2D scenes from declarative objects. No arbitrary JS.",
            &["render", "pointer_events"],
        ),
        pack(
            "coreside.audio",
            "Audio",
            "1.0.0",
            &["audioPlayer"],
            "Local Media Library audio playback with explicit user start.",
            &["render", "media_playback"],
        ),
        pack(
            "coreside.data",
            "Rich Data",
            "1.0.0",
            &["dataTable"],
            "Sortable/filterable tables with bounded rows and CSV export metadata.",
            &["render", "local_state", "export"],
        ),
        pack(
            "coreside.dictation",
            "Dictation (extension point)",
            "0.1.0",
            &["dictationButton"],
            "Permissioned voice input extension point. Follow-up milestone.",
            &["microphone", "render"],
        ),
    ]
}

pub fn pack_registry() -> HashMap<String, CapabilityPackMeta> {
    bundled_packs()
        .into_iter()
        .map(|p| (p.id.clone(), p))
        .collect()
}

pub fn resolve_pack_for_component(component_type: &str) -> Option<CapabilityPackMeta> {
    bundled_packs()
        .into_iter()
        .find(|p| p.enabled && p.component_types.iter().any(|t| t == component_type))
}

pub fn validate_component_type_allowed(component_type: &str) -> Result<(), String> {
    if resolve_pack_for_component(component_type).is_some() {
        Ok(())
    } else {
        Err(format!(
            "component type '{component_type}' is not in any enabled capability pack"
        ))
    }
}

/// Walk a surface/tool definition and reject unknown component types.
pub fn validate_definition_components(definition: &Value) -> Result<(), String> {
    let Some(components) = definition.get("components").and_then(|v| v.as_array()) else {
        return Ok(());
    };
    for component in components {
        validate_component_value(component)?;
    }
    Ok(())
}

fn validate_component_value(component: &Value) -> Result<(), String> {
    if let Some(component_type) = component.get("type").and_then(|v| v.as_str()) {
        validate_component_type_allowed(component_type)?;
    }
    if let Some(children) = component.get("children").and_then(|v| v.as_array()) {
        for child in children {
            validate_component_value(child)?;
        }
    }
    Ok(())
}

/// Validate a typed component tree (insert/replace/update_children paths).
pub fn validate_tool_components(components: &[crate::ai::ToolComponent]) -> Result<(), String> {
    for component in components {
        validate_component_type_allowed(&component.component_type)?;
        if let Some(children) = &component.children {
            validate_tool_components(children)?;
        }
    }
    Ok(())
}

pub fn agent_pack_catalog_markdown() -> String {
    let mut lines = vec![
        "## Capability packs (trusted, bundled)".to_string(),
        "Use only these component types. No CDN libraries. No arbitrary JavaScript.".into(),
    ];
    for p in bundled_packs() {
        if !p.enabled {
            continue;
        }
        lines.push(format!(
            "- **{}** (`{}` v{}): {} — components: {}",
            p.display_name,
            p.id,
            p.version,
            p.agent_description,
            p.component_types.join(", ")
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_and_svg_resolve() {
        assert!(validate_component_type_allowed("progress").is_ok());
        assert!(validate_component_type_allowed("svgScene").is_ok());
        assert!(validate_component_type_allowed("evilScript").is_err());
    }

    #[test]
    fn no_cdn_permissions() {
        for p in bundled_packs() {
            assert!(!p.permissions.iter().any(|x| x.contains("cdn") || x.contains("network")));
        }
    }
}
