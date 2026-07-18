use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SafeSearchLevel {
    Strict,
    Standard,
    Off,
}

impl SafeSearchLevel {
    pub fn from_setting(raw: &str) -> Self {
        match raw.trim().to_lowercase().as_str() {
            "strict" => Self::Strict,
            "off" | "none" => Self::Off,
            _ => Self::Standard,
        }
    }

    /// Safe-search setting string for local filtering / future providers.
    pub fn as_query_value(&self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::Standard => "moderate",
            Self::Off => "off",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchResult {
    pub id: String,
    pub title: String,
    pub url: String,
    pub display_domain: Option<String>,
    pub snippet: Option<String>,
    pub age: Option<String>,
    pub rank: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSearchResult {
    pub id: String,
    pub title: String,
    pub page_url: String,
    pub image_url: String,
    pub thumbnail_url: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub source: Option<String>,
    pub rank: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoSearchResult {
    pub id: String,
    pub title: String,
    pub url: String,
    pub thumbnail_url: Option<String>,
    pub duration: Option<String>,
    pub creator: Option<String>,
    pub age: Option<String>,
    pub rank: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchResponse {
    pub query: String,
    pub provider: String,
    pub results: Vec<WebSearchResult>,
    pub session_id: Option<String>,
    /// Honest-empty / setup guidance for local research (optional; historical rows omit it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSearchResponse {
    pub query: String,
    pub provider: String,
    pub results: Vec<ImageSearchResult>,
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoSearchResponse {
    pub query: String,
    pub provider: String,
    pub results: Vec<VideoSearchResult>,
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchedWebPage {
    pub url: String,
    pub final_url: String,
    pub title: Option<String>,
    pub text: String,
    pub content_type: Option<String>,
    pub byte_size: usize,
}
