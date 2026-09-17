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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchIntent {
    Lookup,
    News,
    DeepResearch,
    KnownUrl,
    DomainSearch,
}

impl ResearchIntent {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Lookup => "lookup",
            Self::News => "news",
            Self::DeepResearch => "deep_research",
            Self::KnownUrl => "known_url",
            Self::DomainSearch => "domain_search",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchResult {
    pub id: String,
    pub title: String,
    pub url: String,
    pub display_domain: Option<String>,
    pub snippet: Option<String>,
    pub age: Option<String>,
    pub rank: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlights: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieval_method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<SearchResultProvenance>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultProvenance {
    pub discovery_provider: String,
    pub original_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_url: Option<String>,
    pub content_fetched: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched_at: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contributing_sources: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ranking_stage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_provider_score: Option<f64>,
}

impl WebSearchResult {
    pub fn ensure_provenance(&mut self) {
        if self.provenance.is_none() {
            let provider = self.provider.clone().unwrap_or_else(|| "unknown".into());
            self.provenance = Some(SearchResultProvenance {
                discovery_provider: provider.clone(),
                original_url: self.url.clone(),
                canonical_url: self.canonical_url.clone(),
                content_fetched: self.content.is_some(),
                fetched_at: self.fetched_at.clone(),
                contributing_sources: vec![provider],
                ranking_stage: Some("normalized".into()),
                raw_provider_score: self.score,
            });
        }
    }
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
