use regex::Regex;
use std::sync::OnceLock;

use super::models::{ImageSearchResult, VideoSearchResult, WebSearchResult};

fn script_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?is)<script[^>]*>.*?</script>").expect("script regex")
    })
}

fn style_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?is)<style[^>]*>.*?</style>").expect("style regex"))
}

fn tag_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"<[^>]+>").expect("tag regex"))
}

/// Strip scripts and HTML tags for safe text extraction.
pub fn extract_text_from_html(html: &str) -> String {
    let no_scripts = script_re().replace_all(html, " ");
    let no_styles = style_re().replace_all(&no_scripts, " ");
    let text = tag_re().replace_all(&no_styles, " ");
    collapse_whitespace(&text)
}

fn collapse_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn normalize_web_results(mut results: Vec<WebSearchResult>) -> Vec<WebSearchResult> {
    for (i, r) in results.iter_mut().enumerate() {
        r.rank = i + 1;
        r.title = r.title.trim().to_string();
        r.url = r.url.trim().to_string();
        if let Some(s) = r.snippet.as_mut() {
            *s = collapse_whitespace(s);
        }
    }
    results
}

pub fn normalize_image_results(mut results: Vec<ImageSearchResult>) -> Vec<ImageSearchResult> {
    for (i, r) in results.iter_mut().enumerate() {
        r.rank = i + 1;
        r.title = r.title.trim().to_string();
    }
    results
}

pub fn normalize_video_results(mut results: Vec<VideoSearchResult>) -> Vec<VideoSearchResult> {
    for (i, r) in results.iter_mut().enumerate() {
        r.rank = i + 1;
        r.title = r.title.trim().to_string();
    }
    results
}

pub fn bound_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        let truncated: String = text.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{truncated}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_script_tags() {
        let html = r#"<html><script>alert(1)</script><body>Hello <b>world</b></body></html>"#;
        let text = extract_text_from_html(html);
        assert!(!text.contains("alert"));
        assert!(text.contains("Hello"));
        assert!(text.contains("world"));
    }
}
