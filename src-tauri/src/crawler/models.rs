use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::resources::ResourceProfile;

pub const PROTOCOL_VERSION: &str = "1";

pub const TERMINAL_EVENT_TYPES: &[&str] = &["completed", "failed", "cancelled"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolEnvelope {
    pub protocol_version: String,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolRequest {
    pub protocol_version: String,
    pub request_id: String,
    #[serde(rename = "type")]
    pub command: String,
    pub payload: Value,
}

impl ProtocolRequest {
    pub fn new(request_id: impl Into<String>, command: impl Into<String>, payload: Value) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION.to_string(),
            request_id: request_id.into(),
            command: command.into(),
            payload,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InstallationState {
    Ready,
    NeedsSetup,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrawlerStatusView {
    pub installation: InstallationState,
    pub running: bool,
    pub python_path: Option<String>,
    pub reason: Option<String>,
    pub resource_profile: String,
    pub sidecar_version: Option<String>,
    pub crawl4ai_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheStatsView {
    pub root: Option<String>,
    pub size_bytes: u64,
    pub quota_bytes: u64,
    pub usage_ratio: f64,
    pub over_quota: bool,
}

impl CacheStatsView {
    pub fn from_payload(payload: &Value) -> Self {
        Self {
            root: payload
                .get("root")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            size_bytes: payload
                .get("sizeBytes")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            quota_bytes: payload
                .get("quotaBytes")
                .and_then(|v| v.as_u64())
                .unwrap_or(1_000_000_000),
            usage_ratio: payload
                .get("usageRatio")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            over_quota: payload
                .get("overQuota")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrawlUrlPayload {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_profile: Option<ResourceProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverDomainPayload {
    pub domain: String,
    #[serde(default)]
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_profile: Option<ResourceProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelPayload {
    pub target_request_id: String,
}
