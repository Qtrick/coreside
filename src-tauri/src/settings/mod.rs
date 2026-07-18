//! Base Settings (product-owned) vs Added Settings (tool-owned extensions).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::security::{assert_not_protected, is_protected};

/// Built-in Base Setting categories. These are product-owned and not AI-editable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub enum BaseSettingCategory {
    Appearance,
    AiAgent,
    Data,
    Accessibility,
    About,
}

#[allow(dead_code)]
impl BaseSettingCategory {
    pub fn id(self) -> &'static str {
        match self {
            Self::Appearance => "core.settings.appearance",
            Self::AiAgent => "core.settings.ai",
            Self::Data => "core.settings.data",
            Self::Accessibility => "core.settings.accessibility",
            Self::About => "core.settings.about",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::AiAgent => "AI Agent",
            Self::Data => "Data",
            Self::Accessibility => "Accessibility",
            Self::About => "About",
        }
    }

    pub fn all() -> &'static [BaseSettingCategory] {
        &[
            Self::Appearance,
            Self::AiAgent,
            Self::Data,
            Self::Accessibility,
            Self::About,
        ]
    }
}

/// A setting contributed by a personal tool (or the user), never under `core.*`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddedSettingRecord {
    pub id: String,
    pub owner_tool_id: Option<String>,
    pub label: String,
    pub description: String,
    pub setting_type: String,
    pub default_value: Value,
    pub current_value: Value,
    pub constraints: Value,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// Payload for creating or updating an Added Setting.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertAddedSettingInput {
    pub id: String,
    pub owner_tool_id: Option<String>,
    pub label: String,
    pub description: Option<String>,
    pub setting_type: String,
    pub default_value: Option<Value>,
    pub current_value: Option<Value>,
    pub constraints: Option<Value>,
}

/// Validates that an Added Setting id is allowed (not `core.*` / protected).
pub fn validate_added_setting_id(id: &str) -> Result<(), String> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return Err("Added Setting id cannot be empty".into());
    }
    if trimmed.starts_with("core.") {
        return Err(format!(
            "Added Setting ids cannot start with 'core.': {trimmed}"
        ));
    }
    if is_protected(trimmed) {
        return Err(format!(
            "Added Setting id collides with a protected resource: {trimmed}"
        ));
    }
    assert_not_protected(trimmed)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_core_prefix_ids() {
        assert!(validate_added_setting_id("core.settings.appearance").is_err());
        assert!(validate_added_setting_id("core.branding").is_err());
        assert!(validate_added_setting_id("core.foo").is_err());
    }

    #[test]
    fn allows_tool_owned_ids() {
        assert!(validate_added_setting_id("tool.water.units").is_ok());
        assert!(validate_added_setting_id("added.quiz.difficulty").is_ok());
    }

    #[test]
    fn base_categories_use_protected_ids() {
        for cat in BaseSettingCategory::all() {
            assert!(is_protected(cat.id()));
        }
    }
}
