use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaAsset {
    pub id: String,
    pub project_id: Option<String>,
    pub category: String,
    pub title: String,
    pub local_filename: String,
    pub mime_type: String,
    pub byte_size: i64,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_ms: Option<i64>,
    pub content_hash: String,
    pub source_url: Option<String>,
    pub source_page_url: Option<String>,
    pub creator: Option<String>,
    pub license: Option<String>,
    pub attribution: Option<String>,
    pub validation_status: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub thumbnail_filename: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportMediaInput {
    pub url: String,
    pub title: Option<String>,
    pub project_id: Option<String>,
    pub source_page_url: Option<String>,
    pub creator: Option<String>,
    pub license: Option<String>,
    pub attribution: Option<String>,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaMetadata {
    pub mime_type: String,
    pub category: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_ms: Option<i64>,
}
