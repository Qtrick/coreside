//! Provider conformance profiles — static seeded capability records.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::db::{now_rfc3339, Database, DbResult};
use rusqlite::params;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderProfile {
    NativeStreamingOperations,
    BufferedStructuredResponse,
    ToolCallOperations,
    JsonSchemaResponse,
    TextProtocolFallback,
    UnsupportedForApplicationChanges,
}

impl ProviderProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NativeStreamingOperations => "native_streaming_operations",
            Self::BufferedStructuredResponse => "buffered_structured_response",
            Self::ToolCallOperations => "tool_call_operations",
            Self::JsonSchemaResponse => "json_schema_response",
            Self::TextProtocolFallback => "text_protocol_fallback",
            Self::UnsupportedForApplicationChanges => "unsupported_for_application_changes",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConformanceRecord {
    pub id: String,
    pub provider_id: String,
    pub model_id: String,
    pub profile: ProviderProfile,
    pub capabilities: Value,
    pub last_tested_at: Option<String>,
    pub benchmark: Option<Value>,
    pub created_at: String,
    pub updated_at: String,
}

fn static_profile(provider_id: &str) -> (ProviderProfile, Value) {
    match provider_id {
        "gemini" => (
            ProviderProfile::NativeStreamingOperations,
            json!({
                "profiles": [
                    "native_streaming_operations",
                    "tool_call_operations",
                    "json_schema_response"
                ],
                "supportsApplicationChanges": true,
            }),
        ),
        "openai" => (
            ProviderProfile::NativeStreamingOperations,
            json!({
                "profiles": [
                    "native_streaming_operations",
                    "tool_call_operations",
                    "json_schema_response"
                ],
                "supportsApplicationChanges": true,
            }),
        ),
        "anthropic" => (
            ProviderProfile::BufferedStructuredResponse,
            json!({
                "profiles": [
                    "buffered_structured_response",
                    "tool_call_operations",
                    "json_schema_response"
                ],
                "supportsApplicationChanges": true,
            }),
        ),
        "openrouter" => (
            ProviderProfile::TextProtocolFallback,
            json!({
                "profiles": ["text_protocol_fallback", "tool_call_operations"],
                "supportsApplicationChanges": true,
            }),
        ),
        "mock" => (
            ProviderProfile::NativeStreamingOperations,
            json!({
                "profiles": ["native_streaming_operations", "json_schema_response"],
                "supportsApplicationChanges": true,
            }),
        ),
        _ => (
            ProviderProfile::UnsupportedForApplicationChanges,
            json!({
                "profiles": ["unsupported_for_application_changes"],
                "supportsApplicationChanges": false,
            }),
        ),
    }
}

pub fn seed_provider_profiles(db: &mut Database) -> DbResult<()> {
    for provider in ["gemini", "openai", "anthropic", "openrouter", "mock"] {
        let _ = get_provider_profile(db, provider, "*");
    }
    Ok(())
}

pub fn get_provider_profile(
    db: &mut Database,
    provider_id: &str,
    model_id: &str,
) -> DbResult<ProviderConformanceRecord> {
    let existing = db
        .conn()
        .query_row(
            "SELECT id, provider_id, model_id, profile, capabilities_json,
                    last_tested_at, benchmark_json, created_at, updated_at
             FROM provider_conformance WHERE provider_id = ?1 AND model_id = ?2",
            params![provider_id, model_id],
            |row| parse_conformance_row(row),
        )
        .ok();

    if let Some(rec) = existing {
        return Ok(rec);
    }

    let (profile, capabilities) = static_profile(provider_id);
    let now = now_rfc3339();
    let id = format!("pconf-{provider_id}-{model_id}");
    let caps_json = capabilities.to_string();
    db.conn().execute(
        "INSERT INTO provider_conformance (
            id, provider_id, model_id, profile, capabilities_json, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(provider_id, model_id) DO UPDATE SET
            profile = excluded.profile,
            capabilities_json = excluded.capabilities_json,
            updated_at = excluded.updated_at",
        params![id, provider_id, model_id, profile.as_str(), caps_json, now, now],
    )?;
    db.conn()
        .query_row(
            "SELECT id, provider_id, model_id, profile, capabilities_json,
                    last_tested_at, benchmark_json, created_at, updated_at
             FROM provider_conformance WHERE provider_id = ?1 AND model_id = ?2",
            params![provider_id, model_id],
            |row| parse_conformance_row(row),
        )
        .map_err(crate::db::DbError::Sqlite)
}

fn parse_conformance_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProviderConformanceRecord> {
    let profile_str: String = row.get(3)?;
    let caps_json: String = row.get(4)?;
    let bench: Option<String> = row.get(6)?;
    let profile = match profile_str.as_str() {
        "native_streaming_operations" => ProviderProfile::NativeStreamingOperations,
        "buffered_structured_response" => ProviderProfile::BufferedStructuredResponse,
        "tool_call_operations" => ProviderProfile::ToolCallOperations,
        "json_schema_response" => ProviderProfile::JsonSchemaResponse,
        "text_protocol_fallback" => ProviderProfile::TextProtocolFallback,
        _ => ProviderProfile::UnsupportedForApplicationChanges,
    };
    Ok(ProviderConformanceRecord {
        id: row.get(0)?,
        provider_id: row.get(1)?,
        model_id: row.get(2)?,
        profile,
        capabilities: serde_json::from_str(&caps_json).unwrap_or(Value::Null),
        last_tested_at: row.get(5)?,
        benchmark: bench.and_then(|s| serde_json::from_str(&s).ok()),
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

/// Select the best application-change profile for a provider/model pair.
pub fn select_application_profile(
    db: &mut Database,
    provider_id: &str,
    model_id: &str,
) -> DbResult<ProviderProfile> {
    let rec = get_provider_profile(db, provider_id, model_id)?;
    if rec.profile == ProviderProfile::UnsupportedForApplicationChanges {
        return Ok(ProviderProfile::UnsupportedForApplicationChanges);
    }
    let profiles = rec
        .capabilities
        .get("profiles")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if profiles
        .iter()
        .any(|p| p.as_str() == Some("native_streaming_operations"))
    {
        return Ok(ProviderProfile::NativeStreamingOperations);
    }
    if profiles
        .iter()
        .any(|p| p.as_str() == Some("buffered_structured_response"))
    {
        return Ok(ProviderProfile::BufferedStructuredResponse);
    }
    if profiles
        .iter()
        .any(|p| p.as_str() == Some("text_protocol_fallback"))
    {
        return Ok(ProviderProfile::TextProtocolFallback);
    }
    Ok(rec.profile)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn test_db() -> crate::db::Database {
        let dir = tempdir().unwrap();
        crate::db::Database::open_path(&dir.path().join("t.db")).unwrap()
    }

    #[test]
    fn provider_profile_selection() {
        let mut db = test_db();
        let gemini = select_application_profile(&mut db, "gemini", "*").unwrap();
        assert_eq!(gemini, ProviderProfile::NativeStreamingOperations);
        let anthropic = select_application_profile(&mut db, "anthropic", "*").unwrap();
        assert_eq!(anthropic, ProviderProfile::BufferedStructuredResponse);
        let openrouter = select_application_profile(&mut db, "openrouter", "*").unwrap();
        assert_eq!(openrouter, ProviderProfile::TextProtocolFallback);
        let unknown = select_application_profile(&mut db, "unknown", "*").unwrap();
        assert_eq!(unknown, ProviderProfile::UnsupportedForApplicationChanges);
    }
}
