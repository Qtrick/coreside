//! Agent tool capability schemas (bounded, validated).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const TOOL_LOOP_MAX_STEPS: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentCapability {
    ProjectContextSearch,
    GetProjectSummary,
    WebSearch,
    ImageSearch,
    VideoSearch,
    FetchWebPage,
    InspectMediaResult,
    ImportMediaAsset,
    NoAction,
}

impl AgentCapability {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ProjectContextSearch => "project_context_search",
            Self::GetProjectSummary => "get_project_summary",
            Self::WebSearch => "web_search",
            Self::ImageSearch => "image_search",
            Self::VideoSearch => "video_search",
            Self::FetchWebPage => "fetch_web_page",
            Self::InspectMediaResult => "inspect_media_result",
            Self::ImportMediaAsset => "import_media_asset",
            Self::NoAction => "no_action",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "project_context_search" => Some(Self::ProjectContextSearch),
            "get_project_summary" => Some(Self::GetProjectSummary),
            // Local research — primary + required aliases (Crawl4AI-backed).
            "web_search" | "discover_urls" | "search_known_domain" | "crawl_sources" => {
                Some(Self::WebSearch)
            }
            "image_search" | "search_source_images" => Some(Self::ImageSearch),
            "video_search" | "search_source_videos" => Some(Self::VideoSearch),
            "fetch_web_page"
            | "crawl_source"
            | "fetch_source_content"
            | "get_cached_source"
            | "refresh_source"
            | "inspect_source_links" => Some(Self::FetchWebPage),
            "inspect_media_result" => Some(Self::InspectMediaResult),
            "import_media_asset" | "import_media_candidate" => Some(Self::ImportMediaAsset),
            "propose_live_wallpaper" => Some(Self::ImportMediaAsset),
            "cancel_research" | "no_action" => Some(Self::NoAction),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallRequest {
    pub capability: String,
    #[serde(default)]
    pub arguments: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallResult {
    pub capability: String,
    pub ok: bool,
    pub output: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_approval: Option<bool>,
}

pub fn capability_schemas() -> Vec<Value> {
    vec![
        schema(
            AgentCapability::ProjectContextSearch,
            "Search prior messages in the active project",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 20 }
                },
                "required": ["query"]
            }),
        ),
        schema(
            AgentCapability::GetProjectSummary,
            "Return project instructions and summary",
            json!({ "type": "object", "properties": {} }),
        ),
        schema(
            AgentCapability::WebSearch,
            "Hybrid web research (discover_urls / search_known_domain / crawl_sources): free-text open-web search when Exa is configured; otherwise crawl a URL or discover pages on a known domain. Never invent results.",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Free-text query (Exa), or URL / domain seed for Crawl4AI" },
                    "domain": { "type": "string", "description": "Optional known domain to seed discovery" },
                    "count": { "type": "integer", "minimum": 1, "maximum": 20 }
                },
                "required": ["query"]
            }),
        ),
        schema(
            AgentCapability::ImageSearch,
            "search_source_images: extract image metadata from a URL or domain seed (no auto-download)",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "count": { "type": "integer", "minimum": 1, "maximum": 50 }
                },
                "required": ["query"]
            }),
        ),
        schema(
            AgentCapability::VideoSearch,
            "search_source_videos: extract video references from a URL or domain seed (no DRM bypass)",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "count": { "type": "integer", "minimum": 1, "maximum": 20 }
                },
                "required": ["query"]
            }),
        ),
        schema(
            AgentCapability::FetchWebPage,
            "crawl_source / fetch_source_content / refresh_source: crawl one public URL via the local research engine (SSRF-safe, robots respected)",
            json!({
                "type": "object",
                "properties": { "url": { "type": "string", "format": "uri" } },
                "required": ["url"]
            }),
        ),
        schema(
            AgentCapability::InspectMediaResult,
            "Inspect a media search result or library asset by id",
            json!({
                "type": "object",
                "properties": {
                    "assetId": { "type": "string" },
                    "resultId": { "type": "string" }
                }
            }),
        ),
        schema(
            AgentCapability::ImportMediaAsset,
            "import_media_candidate / propose_live_wallpaper: queue media import for user approval (never auto-import)",
            json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "format": "uri" },
                    "title": { "type": "string" },
                    "category": { "type": "string" }
                },
                "required": ["url"]
            }),
        ),
        schema(
            AgentCapability::NoAction,
            "cancel_research / no_action: stop or skip tools for this step",
            json!({ "type": "object", "properties": {} }),
        ),
    ]
}

fn schema(cap: AgentCapability, description: &str, parameters: Value) -> Value {
    json!({
        "name": cap.as_str(),
        "description": description,
        "parameters": parameters
    })
}

pub fn validate_tool_call(req: &ToolCallRequest) -> Result<AgentCapability, String> {
    let cap = AgentCapability::parse(&req.capability)
        .ok_or_else(|| format!("unknown capability: {}", req.capability))?;
    match cap {
        AgentCapability::ProjectContextSearch => {
            require_str(&req.arguments, "query")?;
        }
        AgentCapability::WebSearch
        | AgentCapability::ImageSearch
        | AgentCapability::VideoSearch => {
            require_str(&req.arguments, "query")?;
        }
        AgentCapability::FetchWebPage => {
            require_str(&req.arguments, "url")?;
        }
        AgentCapability::ImportMediaAsset => {
            require_str(&req.arguments, "url")?;
        }
        AgentCapability::InspectMediaResult => {
            if req.arguments.get("assetId").is_none() && req.arguments.get("resultId").is_none() {
                return Err("inspect_media_result requires assetId or resultId".into());
            }
        }
        AgentCapability::GetProjectSummary | AgentCapability::NoAction => {}
    }
    Ok(cap)
}

fn require_str(args: &Value, key: &str) -> Result<(), String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| format!("{key} is required"))?;
    Ok(())
}

pub fn validate_tool_output(cap: AgentCapability, output: &Value) -> Result<(), String> {
    if !output.is_object() && !output.is_array() && !output.is_string() && !output.is_null() {
        return Err("tool output must be JSON object, array, string, or null".into());
    }
    if cap == AgentCapability::ImportMediaAsset
        && output.get("pendingApproval").and_then(|v| v.as_bool()) != Some(true)
    {
        return Err("import_media_asset output must set pendingApproval=true".into());
    }
    Ok(())
}
