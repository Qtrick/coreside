//! Registered action descriptors and the bundled registry.
//!
//! A `RegisteredAction` is an executable capability a generated application may
//! call at runtime. This is deliberately *not* called a "Tool": in Coreside a
//! Tool is a user-created personal application.

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::canonical::{canonical_json, sha256_hex};
use crate::application_kernel::permissions::ALLOWED_PERMISSIONS;

/// Runtime action risk. This is a separate axis from kernel operation risk
/// (`automatic` / `lightweight` / `strong`), which classifies durable
/// manifest/data transactions rather than runtime calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActionRisk {
    Read,
    Write,
    Destructive,
}

impl ActionRisk {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Destructive => "destructive",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "read" => Some(Self::Read),
            "write" => Some(Self::Write),
            "destructive" => Some(Self::Destructive),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionDescriptor {
    pub name: String,
    pub title: String,
    pub description: String,
    pub input_schema: Value,
    pub risk: ActionRisk,
    pub critical: bool,
    pub permission_category: String,
}

impl ActionDescriptor {
    /// Stable identity of an action's *authority surface*.
    ///
    /// The hash covers name, input schema, risk, critical flag, and permission
    /// category. `title` and `description` are intentionally excluded so that
    /// improving user-facing wording does not silently invalidate every stored
    /// grant; anything that changes what the action can do does invalidate them.
    pub fn descriptor_hash(&self) -> String {
        sha256_hex(&canonical_json(&json!({
            "name": self.name,
            "inputSchema": self.input_schema,
            "risk": self.risk.as_str(),
            "critical": self.critical,
            "permissionCategory": self.permission_category,
        })))
    }

    pub fn to_catalog_json(&self) -> Value {
        json!({
            "name": self.name,
            "title": self.title,
            "description": self.description,
            "inputSchema": self.input_schema,
            "risk": self.risk.as_str(),
            "critical": self.critical,
            "permissionCategory": self.permission_category,
            "descriptorHash": self.descriptor_hash(),
        })
    }
}

pub fn validate_descriptor(d: &ActionDescriptor) -> Result<(), String> {
    if !is_valid_action_name(&d.name) {
        return Err(format!("invalid action name: {}", d.name));
    }
    if d.title.trim().is_empty() {
        return Err(format!("action {} requires a title", d.name));
    }
    if !d.input_schema.is_object() {
        return Err(format!("action {} inputSchema must be an object", d.name));
    }
    if !ALLOWED_PERMISSIONS.contains(&d.permission_category.as_str()) {
        return Err(format!(
            "action {} declares unknown permission category {}",
            d.name, d.permission_category
        ));
    }
    Ok(())
}

/// Bundled actions covered by a declared permission list. Used when wrapping a
/// personal tool as a manifest so registered-action calls are not silently
/// undeclared.
pub fn default_action_access_for_permissions(permissions: &[String]) -> Vec<String> {
    BUNDLED_ACTIONS
        .iter()
        .filter(|d| permissions.iter().any(|p| p == &d.permission_category))
        .map(|d| d.name.clone())
        .collect()
}

/// `namespace.action`, lowercase, at least one dot, no wildcards.
fn is_valid_action_name(name: &str) -> bool {
    let mut segments = 0;
    for segment in name.split('.') {
        segments += 1;
        let mut chars = segment.chars();
        match chars.next() {
            Some(c) if c.is_ascii_lowercase() => {}
            _ => return false,
        }
        if !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
            return false;
        }
    }
    segments >= 2
}

fn descriptor(
    name: &str,
    title: &str,
    description: &str,
    input_schema: Value,
    risk: ActionRisk,
    critical: bool,
    permission_category: &str,
) -> ActionDescriptor {
    ActionDescriptor {
        name: name.into(),
        title: title.into(),
        description: description.into(),
        input_schema,
        risk,
        critical,
        permission_category: permission_category.into(),
    }
}

/// Bundled registry. Every entry has a real handler in `handlers.rs`.
pub static BUNDLED_ACTIONS: Lazy<Vec<ActionDescriptor>> = Lazy::new(|| {
    vec![
        descriptor(
            "local_data.query",
            "Read application records",
            "Reads records from one of this application's own data models.",
            json!({
                "type": "object",
                "required": ["modelId"],
                "properties": {
                    "modelId": { "type": "string" },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 500 }
                }
            }),
            ActionRisk::Read,
            false,
            "local_data.read",
        ),
        descriptor(
            "local_data.write",
            "Save an application record",
            "Creates or updates a record in one of this application's own data models.",
            json!({
                "type": "object",
                "required": ["modelId", "data"],
                "properties": {
                    "modelId": { "type": "string" },
                    "data": { "type": "object" },
                    "recordId": { "type": "string" },
                    "baseVersion": { "type": "integer" }
                }
            }),
            ActionRisk::Write,
            false,
            "local_data.write",
        ),
        descriptor(
            "local_data.delete",
            "Delete an application record",
            "Permanently deletes one record from this application's own data.",
            json!({
                "type": "object",
                "required": ["recordId"],
                "properties": { "recordId": { "type": "string" } }
            }),
            ActionRisk::Destructive,
            false,
            "local_data.write",
        ),
        descriptor(
            "tool_state.set",
            "Update tool state",
            "Sets one key in this application's own persisted tool state.",
            json!({
                "type": "object",
                "required": ["key", "value"],
                "properties": {
                    "key": { "type": "string" },
                    "value": {}
                }
            }),
            ActionRisk::Write,
            false,
            "local_data.write",
        ),
        descriptor(
            "web_search.request",
            "Request web research",
            "Requests web research. Generated applications cannot run searches \
             directly; the request is reported back so the user can run it in chat.",
            json!({
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": { "type": "string" },
                    "reason": { "type": "string" }
                }
            }),
            ActionRisk::Write,
            false,
            "web_search.request",
        ),
        descriptor(
            "media.read",
            "Read media library items",
            "Lists media assets visible to this application's project scope.",
            json!({
                "type": "object",
                "properties": {
                    "assetId": { "type": "string" },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 200 }
                }
            }),
            ActionRisk::Read,
            false,
            "media.read",
        ),
        descriptor(
            "external_link.open",
            "Open an external link",
            "Validates an external URL and returns it for the protected shell to open.",
            json!({
                "type": "object",
                "required": ["url"],
                "properties": { "url": { "type": "string" } }
            }),
            ActionRisk::Write,
            false,
            "external_link.open",
        ),
        descriptor(
            "export.prepare",
            "Prepare an export",
            "Builds a secret-stripped export payload for the user to save.",
            json!({
                "type": "object",
                "properties": {
                    "modelId": { "type": "string" },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 500 }
                }
            }),
            ActionRisk::Write,
            true,
            "export.prepare",
        ),
        descriptor(
            "automation.propose",
            "Propose an automation",
            "Validates a proposed automation and returns it for user confirmation. \
             Proposing never schedules anything.",
            json!({
                "type": "object",
                "required": ["name", "trigger", "action"],
                "properties": {
                    "name": { "type": "string" },
                    "trigger": { "type": "object" },
                    "action": { "type": "object" }
                }
            }),
            ActionRisk::Write,
            false,
            "automation.propose",
        ),
        descriptor(
            "agent.submit_event",
            "Send an event to the assistant",
            "Appends a bounded entry to the conversation context ledger so the \
             assistant can see what happened inside the application.",
            json!({
                "type": "object",
                "required": ["summary"],
                "properties": {
                    "summary": { "type": "string" },
                    "payload": { "type": "object" }
                }
            }),
            ActionRisk::Write,
            false,
            "project_context.read",
        ),
    ]
});

pub fn find_action(name: &str) -> Option<&'static ActionDescriptor> {
    BUNDLED_ACTIONS.iter().find(|d| d.name == name)
}

pub fn action_names() -> Vec<&'static str> {
    BUNDLED_ACTIONS.iter().map(|d| d.name.as_str()).collect()
}

pub fn catalog_json() -> Value {
    Value::Array(BUNDLED_ACTIONS.iter().map(|d| d.to_catalog_json()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_registry_is_valid() {
        for d in BUNDLED_ACTIONS.iter() {
            validate_descriptor(d).unwrap_or_else(|e| panic!("{}: {e}", d.name));
        }
    }

    #[test]
    fn action_names_are_unique() {
        let mut names = action_names();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total);
    }

    #[test]
    fn descriptor_hash_ignores_description_and_title() {
        let base = find_action("local_data.query").unwrap().clone();
        let mut reworded = base.clone();
        reworded.description = "Completely different wording.".into();
        reworded.title = "Different title".into();
        assert_eq!(base.descriptor_hash(), reworded.descriptor_hash());
    }

    #[test]
    fn descriptor_hash_changes_with_authority() {
        let base = find_action("local_data.query").unwrap().clone();
        let mut escalated = base.clone();
        escalated.risk = ActionRisk::Destructive;
        assert_ne!(base.descriptor_hash(), escalated.descriptor_hash());

        let mut recategorized = base.clone();
        recategorized.permission_category = "local_data.write".into();
        assert_ne!(base.descriptor_hash(), recategorized.descriptor_hash());

        let mut reshaped = base.clone();
        reshaped.input_schema = json!({ "type": "object", "properties": {} });
        assert_ne!(base.descriptor_hash(), reshaped.descriptor_hash());
    }

    #[test]
    fn rejects_bad_names() {
        let mut d = find_action("media.read").unwrap().clone();
        d.name = "MediaRead".into();
        assert!(validate_descriptor(&d).is_err());
        d.name = "media".into();
        assert!(validate_descriptor(&d).is_err());
        d.name = "media.*".into();
        assert!(validate_descriptor(&d).is_err());
    }

    #[test]
    fn rejects_unknown_permission_category() {
        let mut d = find_action("media.read").unwrap().clone();
        d.permission_category = "unrestricted.shell".into();
        assert!(validate_descriptor(&d).is_err());
    }

    #[test]
    fn destructive_actions_exist_for_policy_coverage() {
        assert!(BUNDLED_ACTIONS
            .iter()
            .any(|d| d.risk == ActionRisk::Destructive));
    }
}
