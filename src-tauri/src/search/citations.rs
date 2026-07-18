use super::models::{ImageSearchResult, VideoSearchResult, WebSearchResult};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Citation {
    pub index: usize,
    pub title: String,
    pub url: String,
    pub snippet: Option<String>,
}

pub fn citations_from_web(results: &[WebSearchResult]) -> Vec<Citation> {
    results
        .iter()
        .map(|r| Citation {
            index: r.rank,
            title: r.title.clone(),
            url: r.url.clone(),
            snippet: r.snippet.clone(),
        })
        .collect()
}

pub fn format_citations_block(results: &[WebSearchResult]) -> String {
    let cites = citations_from_web(results);
    if cites.is_empty() {
        return String::new();
    }
    let mut lines = vec!["## Sources".to_string()];
    for c in cites {
        let snippet = c
            .snippet
            .as_ref()
            .map(|s| format!(" — {s}"))
            .unwrap_or_default();
        lines.push(format!("[{}] {} ({}){}", c.index, c.title, c.url, snippet));
    }
    lines.join("\n")
}

pub fn citations_from_images(results: &[ImageSearchResult]) -> Vec<Citation> {
    results
        .iter()
        .map(|r| Citation {
            index: r.rank,
            title: r.title.clone(),
            url: r.page_url.clone(),
            snippet: Some(r.image_url.clone()),
        })
        .collect()
}

pub fn citations_from_videos(results: &[VideoSearchResult]) -> Vec<Citation> {
    results
        .iter()
        .map(|r| Citation {
            index: r.rank,
            title: r.title.clone(),
            url: r.url.clone(),
            snippet: r.duration.clone(),
        })
        .collect()
}
