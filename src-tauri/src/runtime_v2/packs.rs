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
                "container",
                "row",
                "column",
                "card",
                "tabs",
                "divider",
                "spacer",
                "heading",
                "text",
                "badge",
                "image",
                "emptyState",
                "textInput",
                "textArea",
                "numberInput",
                "select",
                "checkbox",
                "dateInput",
                "list",
                "checklist",
                "table",
                "counter",
                "progress",
                "stat",
                "button",
                "buttonGroup",
                "quiz",
                "clock",
            ],
            "Built-in layout, form, and feedback components.",
            &["render", "local_state"],
        ),
        pack(
            "coreside.forms",
            "Rich Forms",
            "1.0.0",
            &[
                "form",
                "fieldGroup",
                "radioGroup",
                "slider",
                "switch",
                "colorInput",
                "timeInput",
                "dateTimeInput",
                "filePicker",
                "mediaPicker",
                "submitButton",
                "resetButton",
                "validationMessage",
            ],
            "Trusted structured forms with validation and submitToAgent.",
            &["render", "local_state", "submit_to_agent"],
        ),
        pack(
            "coreside.svg",
            "SVG Scenes",
            "1.0.0",
            &[
                "svgScene",
                "svgRect",
                "svgCircle",
                "svgEllipse",
                "svgLine",
                "svgPath",
                "svgText",
                "svgGroup",
            ],
            "Declarative SVG without script or foreignObject.",
            &["render", "pointer_events"],
        ),
        pack(
            "coreside.charts",
            "Charts",
            "1.0.0",
            &[
                "chartLine",
                "chartBar",
                "chartPie",
                "chartDonut",
                "chartArea",
                "chartScatter",
            ],
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
        // ponytail: dictationButton removed from generatable packs. Old tools that
        // still carry the type render via UnsupportedNode; do not re-add until mic
        // permissions and a real dictation runtime exist.
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

/// Normalize the explicit capability set for one surface. `coreside.core` is
/// implicit for every surface; all other packs remain opt-in.
pub fn normalize_capability_packs(pack_ids: &[String]) -> Result<Vec<String>, String> {
    let registry = pack_registry();
    let mut normalized = pack_ids.to_vec();
    normalized.push("coreside.core".into());
    normalized.sort();
    normalized.dedup();
    for id in &normalized {
        match registry.get(id) {
            Some(pack) if pack.enabled => {}
            Some(_) => return Err(format!("capability pack '{id}' is disabled")),
            None => return Err(format!("unknown capability pack '{id}'")),
        }
    }
    Ok(normalized)
}

/// Derive the least set of bundled packs that can represent an existing trusted
/// definition. This is used only for creation and legacy backfill; it never
/// authorizes a later patch to expand a surface's assigned packs.
pub fn required_packs_for_definition(definition: &Value) -> Result<Vec<String>, String> {
    let mut packs = vec!["coreside.core".to_string()];
    collect_required_packs(definition, &mut packs)?;
    normalize_capability_packs(&packs)
}

fn collect_required_packs(value: &Value, packs: &mut Vec<String>) -> Result<(), String> {
    if let Some(component_type) = value.get("type").and_then(|v| v.as_str()) {
        let pack = resolve_pack_for_component(component_type).ok_or_else(|| {
            format!("component type '{component_type}' is not in any enabled capability pack")
        })?;
        packs.push(pack.id);
    }
    if let Some(components) = value.get("components").and_then(|v| v.as_array()) {
        for component in components {
            collect_required_packs(component, packs)?;
        }
    }
    if let Some(children) = value.get("children").and_then(|v| v.as_array()) {
        for child in children {
            collect_required_packs(child, packs)?;
        }
    }
    Ok(())
}

pub fn validate_component_type_allowed_for_packs(
    component_type: &str,
    allowed_pack_ids: &[String],
) -> Result<(), String> {
    validate_component_type_allowed(component_type)?;
    let allowed = normalize_capability_packs(allowed_pack_ids)?;
    let pack = resolve_pack_for_component(component_type)
        .expect("global component validation resolved a pack");
    if allowed.iter().any(|id| id == &pack.id) {
        Ok(())
    } else {
        Err(format!(
            "component type '{component_type}' requires capability pack '{}' which this surface has not been granted",
            pack.id
        ))
    }
}

/// Walk a surface/tool definition and reject unknown component types.
pub fn validate_definition_components(definition: &Value) -> Result<(), String> {
    validate_definition_components_for_packs(
        definition,
        &bundled_packs()
            .into_iter()
            .map(|p| p.id)
            .collect::<Vec<_>>(),
    )
}

struct TreeValidationState {
    ids: std::collections::HashSet<String>,
    total_count: usize,
    animated_count: usize,
}

/// Validate a definition against both the global trusted registry and one
/// surface's assigned packs, enforcing unique IDs, depth, count, and size limits.
pub fn validate_definition_components_for_packs(
    definition: &Value,
    allowed_pack_ids: &[String],
) -> Result<(), String> {
    let Some(components_val) = definition.get("components") else {
        return Ok(());
    };
    let components: Vec<crate::ai::ToolComponent> = serde_json::from_value(components_val.clone())
        .map_err(|e| format!("invalid component definition: {e}"))?;
    validate_tool_components_for_packs(&components, allowed_pack_ids)
}

/// Validate a typed component tree (insert/replace/update_children paths).
pub fn validate_tool_components(components: &[crate::ai::ToolComponent]) -> Result<(), String> {
    validate_tool_components_for_packs(
        components,
        &bundled_packs()
            .into_iter()
            .map(|p| p.id)
            .collect::<Vec<_>>(),
    )
}

pub fn validate_tool_components_for_packs(
    components: &[crate::ai::ToolComponent],
    allowed_pack_ids: &[String],
) -> Result<(), String> {
    let allowed = normalize_capability_packs(allowed_pack_ids)?;
    let mut state = TreeValidationState {
        ids: std::collections::HashSet::new(),
        total_count: 0,
        animated_count: 0,
    };
    validate_tree_recursive(components, 1, &allowed, &mut state)
}

fn validate_tree_recursive(
    components: &[crate::ai::ToolComponent],
    depth: usize,
    allowed_packs: &[String],
    state: &mut TreeValidationState,
) -> Result<(), String> {
    use super::limits::{
        MAX_ANIMATED_COMPONENTS, MAX_CHART_POINTS, MAX_COMPONENTS_PER_SURFACE,
        MAX_COMPONENT_ID_LEN, MAX_COMPONENT_TREE_DEPTH, MAX_PROPS_JSON_BYTES, MAX_SVG_NODES,
        MAX_TEXT_CONTENT_CHARS,
    };

    if depth > MAX_COMPONENT_TREE_DEPTH {
        return Err(format!(
            "component tree depth ({depth}) exceeds maximum of {MAX_COMPONENT_TREE_DEPTH}"
        ));
    }

    for component in components {
        state.total_count += 1;
        if state.total_count > MAX_COMPONENTS_PER_SURFACE {
            return Err(format!(
                "component count ({}) exceeds maximum of {MAX_COMPONENTS_PER_SURFACE}",
                state.total_count
            ));
        }

        let id = component.id.trim();
        if id.is_empty() {
            return Err("component id cannot be empty".into());
        }
        if id.len() > MAX_COMPONENT_ID_LEN {
            return Err(format!(
                "component id '{id}' exceeds maximum length of {MAX_COMPONENT_ID_LEN}"
            ));
        }
        if !state.ids.insert(component.id.clone()) {
            return Err(format!("duplicate component id: {}", component.id));
        }

        validate_component_type_allowed_for_packs(&component.component_type, allowed_packs)?;

        if matches!(component.component_type.as_str(), "canvasScene" | "clock") {
            state.animated_count += 1;
            if state.animated_count > MAX_ANIMATED_COMPONENTS {
                return Err(format!(
                    "animated components count ({}) exceeds maximum of {MAX_ANIMATED_COMPONENTS}",
                    state.animated_count
                ));
            }
        }

        if let Some(props) = &component.props {
            if props.get("action").is_some() || props.get("actions").is_some() {
                return Err(format!(
                    "component '{}' uses legacy action props ('action'/'actions'); actions must be specified in the authoritative 'actions' field",
                    component.id
                ));
            }
            let s = props.to_string();
            if s.len() > MAX_PROPS_JSON_BYTES {
                return Err(format!(
                    "component '{}' props exceed maximum size of {MAX_PROPS_JSON_BYTES} bytes",
                    component.id
                ));
            }
            validate_text_strings_bounded(props, MAX_TEXT_CONTENT_CHARS, &component.id)?;

            if component.component_type.starts_with("chart") {
                if let Some(data) = props
                    .get("data")
                    .or_else(|| props.get("points"))
                    .and_then(|v| v.as_array())
                {
                    if data.len() > MAX_CHART_POINTS {
                        return Err(format!(
                            "chart '{}' data points ({}) exceed maximum of {MAX_CHART_POINTS}",
                            component.id,
                            data.len()
                        ));
                    }
                }
            }

            if component.component_type == "svgScene" {
                let node_count = component.children.as_ref().map(|c| c.len()).unwrap_or(0)
                    + props
                        .get("elements")
                        .and_then(|v| v.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);
                if node_count > MAX_SVG_NODES {
                    return Err(format!(
                        "svgScene '{}' node count ({node_count}) exceeds maximum of {MAX_SVG_NODES}",
                        component.id
                    ));
                }
            }
        }

        validate_labeled_props(
            &component.component_type,
            &component.id,
            component.props.as_ref(),
        )?;

        if let Some(actions) = &component.actions {
            if actions.len() > crate::ai::response_schema::MAX_ACTIONS_PER_COMPONENT {
                return Err(format!(
                    "component '{}' actions count ({}) exceeds maximum of {}",
                    component.id,
                    actions.len(),
                    crate::ai::response_schema::MAX_ACTIONS_PER_COMPONENT
                ));
            }
            for action in actions {
                action
                    .validate()
                    .map_err(|e| format!("component '{}' invalid action: {e}", component.id))?;
            }
        }

        if let Some(children) = &component.children {
            validate_tree_recursive(children, depth + 1, allowed_packs, state)?;
        }
    }

    Ok(())
}

fn validate_text_strings_bounded(
    val: &Value,
    max_len: usize,
    component_id: &str,
) -> Result<(), String> {
    match val {
        Value::String(s) => {
            if s.chars().count() > max_len {
                return Err(format!(
                    "text content in component '{component_id}' exceeds maximum length of {max_len}"
                ));
            }
        }
        Value::Array(arr) => {
            for item in arr {
                validate_text_strings_bounded(item, max_len, component_id)?;
            }
        }
        Value::Object(map) => {
            for (k, v) in map {
                if k.len() > super::limits::MAX_COMPONENT_ID_LEN {
                    return Err(format!(
                        "prop key '{k}' in component '{component_id}' exceeds maximum length of {}",
                        super::limits::MAX_COMPONENT_ID_LEN
                    ));
                }
                validate_text_strings_bounded(v, max_len, component_id)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_labeled_props(
    component_type: &str,
    component_id: &str,
    props: Option<&Value>,
) -> Result<(), String> {
    use super::limits::{MAX_CANVAS_OBJECTS, MAX_CODE_EDITOR_CHARS};

    const NEEDS_LABEL: &[&str] = &[
        "textInput",
        "textArea",
        "numberInput",
        "select",
        "checkbox",
        "dateInput",
        "button",
    ];
    // Renderer defaults — treat as invalid agent output (screenshot: "Text" / "Button").
    const PLACEHOLDER_LABELS: &[&str] = &[
        "text", "button", "notes", "number", "select", "checkbox", "date", "label", "input",
        "untitled", "field",
    ];

    if component_type == "codeEditor" {
        let value_len = props
            .and_then(|p| {
                p.get("value")
                    .or_else(|| p.get("defaultValue"))
                    .and_then(|v| v.as_str())
            })
            .map(|s| s.chars().count())
            .unwrap_or(0);
        if value_len > MAX_CODE_EDITOR_CHARS {
            return Err(format!(
                "codeEditor '{component_id}' exceeds max {MAX_CODE_EDITOR_CHARS} characters"
            ));
        }
    }
    if component_type == "canvasScene" {
        let count = props
            .and_then(|p| p.get("objects"))
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        if count > MAX_CANVAS_OBJECTS {
            return Err(format!(
                "canvasScene '{component_id}' exceeds max {MAX_CANVAS_OBJECTS} objects"
            ));
        }
    }

    if NEEDS_LABEL.contains(&component_type) {
        let label = props
            .and_then(|p| p.get("label"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let Some(label) = label else {
            return Err(format!(
                "{component_type} '{component_id}' requires non-empty props.label"
            ));
        };
        if PLACEHOLDER_LABELS
            .iter()
            .any(|g| label.eq_ignore_ascii_case(g))
        {
            return Err(format!(
                "{component_type} '{component_id}' has placeholder label '{label}' — use a specific label for the tool"
            ));
        }
    }

    if matches!(component_type, "heading" | "text") {
        let content = props
            .and_then(|p| p.get("text").or_else(|| p.get("children")))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty());
        if content.is_none() {
            return Err(format!(
                "{component_type} '{component_id}' requires non-empty props.text"
            ));
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

    lines.push("\n## Declarative Action Types\n\
        Attach actions ONLY via `component.actions: [...]` on interactive components (e.g. `button`). Never use legacy action props.\n\
        - `setValue`: `{ \"type\": \"setValue\", \"target\": \"stateKey\", \"value\": ... }`\n\
        - `toggle`: `{ \"type\": \"toggle\", \"target\": \"boolKey\" }`\n\
        - `increment` / `decrement`: `{ \"type\": \"increment\", \"target\": \"key\", \"amount\": 1 }`\n\
        - `reset`: `{ \"type\": \"reset\", \"target\": \"stateKey\", \"value\": ... }`\n\
        - `appendItem`: `{ \"type\": \"appendItem\", \"target\": \"itemsKey\", \"item\": { \"id\": \"...\", ... } }`\n\
        - `removeItem`: `{ \"type\": \"removeItem\", \"target\": \"itemsKey\", \"id\": \"itemId\" }`\n\
        - `updateItem`: `{ \"type\": \"updateItem\", \"target\": \"itemsKey\", \"id\": \"itemId\", \"patch\": { ... } }`\n\
        - `selectTab`: `{ \"type\": \"selectTab\", \"target\": \"tabsId\", \"tabId\": \"tab-1\" }`\n\
        - `submitToAgent`: `{ \"type\": \"submitToAgent\", \"eventName\": \"submitted\", \"includeFields\": [\"field1\", \"field2\"] }` (includeFields is required and non-empty)\n\
        - `invokeRegisteredAction`: `{ \"type\": \"invokeRegisteredAction\", \"actionName\": \"local_data.query\", \"input\": { ... }, \"resultKey\": \"targetStateKey\" }`".into());

    lines.push("\n## Registered Kernel Actions\n\
        - `local_data.query`: `{ \"model\": string, \"filter\"?: object, \"orderBy\"?: string, \"limit\"?: number }` -> returns `{ \"records\": [...], \"count\": number }`\n\
        - `local_data.write`: `{ \"model\": string, \"record\": object }` -> returns `{ \"id\": string }`\n\
        - `local_data.delete`: `{ \"model\": string, \"id\": string }` -> returns `{ \"deleted\": boolean }`\n\
        - `media.read`: `{ \"assetId\": string }` -> returns `{ \"assetUrl\": string, \"mimeType\": string }`\n\
        - `external_link.open`: `{ \"url\": string }`\n\
        - `web_search.request`: `{ \"query\": string }`".into());

    lines.push("\n## Explicit State & Data Binding Rules\n\
        - Component IDs are identifiers for rendering and DOM patching; they are NEVER automatic permissions to read or write state.\n\
        - Inputs (textInput, select, dateInput, etc.) must declare an explicit `valueKey: \"myKey\"` to bind to durable state; without `valueKey`, input is local ephemeral UI state.\n\
        - Display components (stat, heading) read from state only if `valueKey` is specified; otherwise they show static `props.value`.\n\
        - Data tables and lists bind via `rowsKey: \"itemsKey\"` or `dataKey: \"itemsKey\"` and automatically unwrap `{ records, count }`.\n\
        - `resultKey` on `invokeRegisteredAction` sets the state destination key for action outputs; it does not authorize reading state.".into());

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
        // Dictation is intentionally not generatable; legacy definitions still
        // render via UnsupportedNode on the frontend.
        assert!(validate_component_type_allowed("dictationButton").is_err());
    }

    #[test]
    fn no_cdn_permissions() {
        for p in bundled_packs() {
            assert!(!p
                .permissions
                .iter()
                .any(|x| x.contains("cdn") || x.contains("network")));
        }
    }

    #[test]
    fn per_surface_pack_boundary_rejects_an_ungranted_known_component() {
        let core_only = vec!["coreside.core".into()];
        assert!(validate_component_type_allowed_for_packs("textInput", &core_only).is_ok());
        let err = validate_component_type_allowed_for_packs("svgScene", &core_only).unwrap_err();
        assert!(err.contains("has not been granted"));

        let definition = serde_json::json!({
            "components": [{ "id": "svg", "type": "svgScene" }]
        });
        assert!(validate_definition_components_for_packs(&definition, &core_only).is_err());
        assert!(
            validate_definition_components_for_packs(&definition, &["coreside.svg".into()],)
                .is_ok()
        );
    }

    #[test]
    fn packs_are_deduplicated_and_core_is_implicit() {
        assert_eq!(
            normalize_capability_packs(&["coreside.svg".into(), "coreside.svg".into()]).unwrap(),
            vec!["coreside.core", "coreside.svg"],
        );
        assert!(normalize_capability_packs(&["unknown.pack".into()]).is_err());
    }

    #[test]
    fn rejects_placeholder_text_and_button_labels() {
        use crate::ai::ToolComponent;
        let stub = vec![
            ToolComponent {
                id: "a".into(),
                component_type: "textInput".into(),
                value_key: None,
                props: None,
                children: None,
                ..Default::default()
            },
            ToolComponent {
                id: "b".into(),
                component_type: "button".into(),
                value_key: None,
                props: Some(serde_json::json!({ "label": "Button" })),
                children: None,
                ..Default::default()
            },
        ];
        assert!(validate_tool_components(&stub).is_err());

        let ok = vec![ToolComponent {
            id: "c".into(),
            component_type: "textInput".into(),
            value_key: None,
            props: Some(serde_json::json!({ "label": "Next task" })),
            children: None,
            ..Default::default()
        }];
        assert!(validate_tool_components(&ok).is_ok());
    }

    #[test]
    fn rejects_duplicate_component_ids() {
        use crate::ai::ToolComponent;
        let comps = vec![
            ToolComponent {
                id: "dup-id".into(),
                component_type: "text".into(),
                value_key: None,
                props: Some(serde_json::json!({ "text": "First" })),
                children: None,
                ..Default::default()
            },
            ToolComponent {
                id: "dup-id".into(),
                component_type: "text".into(),
                value_key: None,
                props: Some(serde_json::json!({ "text": "Second" })),
                children: None,
                ..Default::default()
            },
        ];
        let err = validate_tool_components(&comps).unwrap_err();
        assert!(err.contains("duplicate component id: dup-id"));
    }

    #[test]
    fn rejects_component_count_over_200() {
        use crate::ai::ToolComponent;
        let mut comps = Vec::new();
        for i in 0..205 {
            comps.push(ToolComponent {
                id: format!("comp-{i}"),
                component_type: "text".into(),
                value_key: None,
                props: Some(serde_json::json!({ "text": format!("Text {i}") })),
                children: None,
                ..Default::default()
            });
        }
        let err = validate_tool_components(&comps).unwrap_err();
        assert!(err.contains("component count"));
    }

    #[test]
    fn rejects_tree_depth_over_24() {
        use crate::ai::ToolComponent;
        let mut root = ToolComponent {
            id: "deep-25".into(),
            component_type: "text".into(),
            value_key: None,
            props: Some(serde_json::json!({ "text": "leaf" })),
            children: None,
            ..Default::default()
        };
        for i in (0..25).rev() {
            root = ToolComponent {
                id: format!("node-{i}"),
                component_type: "column".into(),
                value_key: None,
                props: None,
                children: Some(vec![root]),
                ..Default::default()
            };
        }
        let err = validate_tool_components(&[root]).unwrap_err();
        assert!(err.contains("component tree depth"));
    }

    #[test]
    fn rejects_oversized_component_id() {
        use crate::ai::ToolComponent;
        let long_id = "a".repeat(70);
        let comps = vec![ToolComponent {
            id: long_id,
            component_type: "text".into(),
            value_key: None,
            props: Some(serde_json::json!({ "text": "hello" })),
            children: None,
            ..Default::default()
        }];
        let err = validate_tool_components(&comps).unwrap_err();
        assert!(err.contains("exceeds maximum length"));
    }

    #[test]
    fn rejects_oversized_props() {
        use crate::ai::ToolComponent;
        let huge_str = "x".repeat(150_000);
        let comps = vec![ToolComponent {
            id: "huge-props".into(),
            component_type: "text".into(),
            value_key: None,
            props: Some(serde_json::json!({ "text": "valid", "extra": huge_str })),
            children: None,
            ..Default::default()
        }];
        let err = validate_tool_components(&comps).unwrap_err();
        assert!(err.contains("props exceed maximum size"));
    }

    #[test]
    fn rejects_chart_points_over_limit() {
        use crate::ai::ToolComponent;
        let points: Vec<i32> = (0..2_500).collect();
        let comps = vec![ToolComponent {
            id: "big-chart".into(),
            component_type: "chartLine".into(),
            value_key: None,
            props: Some(serde_json::json!({ "data": points })),
            children: None,
            ..Default::default()
        }];
        let err =
            validate_tool_components_for_packs(&comps, &["coreside.charts".into()]).unwrap_err();
        assert!(err.contains("data points (2500) exceed maximum of 2000"));
    }

    #[test]
    fn full_replace_definition_rejects_duplicate_ids() {
        let def = serde_json::json!({
            "id": "tool-test",
            "name": "Test Tool",
            "layout": { "type": "stack" },
            "components": [
                { "id": "dup", "type": "text", "props": { "text": "A" } },
                { "id": "dup", "type": "text", "props": { "text": "B" } }
            ]
        });
        let err = validate_definition_components(&def).unwrap_err();
        assert!(err.contains("duplicate component id: dup"));
    }

    #[test]
    fn rejects_legacy_action_props_bypass() {
        use crate::ai::ToolComponent;
        let comp_with_action = vec![ToolComponent {
            id: "btn-legacy".into(),
            component_type: "button".into(),
            props: Some(serde_json::json!({
                "label": "Click",
                "action": "reset",
                "target": "secretState"
            })),
            ..Default::default()
        }];
        let err = validate_tool_components(&comp_with_action).unwrap_err();
        assert!(err.contains("uses legacy action props ('action'/'actions')"));

        let comp_with_actions = vec![ToolComponent {
            id: "btn-legacy-multi".into(),
            component_type: "button".into(),
            props: Some(serde_json::json!({
                "label": "Click",
                "actions": [{ "type": "toggle", "target": "secretState" }]
            })),
            ..Default::default()
        }];
        let err2 = validate_tool_components(&comp_with_actions).unwrap_err();
        assert!(err2.contains("uses legacy action props ('action'/'actions')"));
    }
}
