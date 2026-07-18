//! Exa Search API request / response models.

use serde::{Deserialize, Deserializer, Serialize};

use super::profiles::SearchProfile;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExaSearchRequest {
    pub query: String,
    #[serde(rename = "type")]
    pub search_type: String,
    pub num_results: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_domains: Option<Vec<String>>,
    pub contents: ExaContentsRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExaContentsRequest {
    pub highlights: bool,
}

impl ExaSearchRequest {
    pub fn for_profile(query: &str, profile: SearchProfile) -> Self {
        Self {
            query: query.trim().to_string(),
            search_type: profile.search_type().to_string(),
            num_results: profile.num_results(),
            include_domains: None,
            contents: ExaContentsRequest { highlights: true },
        }
    }

    pub fn with_include_domains(mut self, domains: Vec<String>) -> Self {
        if !domains.is_empty() {
            self.include_domains = Some(domains);
        }
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExaSearchResponse {
    #[serde(default)]
    pub results: Vec<ExaResult>,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_cost_dollars")]
    pub cost_dollars: Option<ExaCostDollars>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExaCostDollars {
    #[serde(default, deserialize_with = "deserialize_optional_f64")]
    pub total: Option<f64>,
}

fn deserialize_optional_f64<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(json_to_f64))
}

fn json_to_f64(value: serde_json::Value) -> Option<f64> {
    match value {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// Accept `costDollars: { total }` or a bare number; ignore unknown shapes.
fn deserialize_cost_dollars<'de, D>(deserializer: D) -> Result<Option<ExaCostDollars>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(match value {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::Number(n)) => Some(ExaCostDollars {
            total: n.as_f64(),
        }),
        Some(serde_json::Value::String(s)) => Some(ExaCostDollars {
            total: s.trim().parse().ok(),
        }),
        Some(serde_json::Value::Object(map)) => {
            let total = map.get("total").cloned().and_then(json_to_f64);
            Some(ExaCostDollars { total })
        }
        Some(_) => None,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExaResult {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    pub url: String,
    #[serde(default)]
    pub published_date: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub highlights: Option<Vec<String>>,
    #[serde(default)]
    pub text: Option<String>,
}

impl ExaResult {
    pub fn snippet(&self) -> Option<String> {
        if let Some(highlights) = &self.highlights {
            let joined = highlights
                .iter()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" … ");
            if !joined.is_empty() {
                return Some(joined);
            }
        }
        self.text
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }
}

impl ExaSearchResponse {
    pub fn actual_cost(&self) -> Option<f64> {
        self.cost_dollars.as_ref().and_then(|c| c.total)
    }
}

/// Rough estimate when the provider omits costDollars (never invent for ledger actual).
pub fn estimate_search_cost(search_type: &str, num_results: usize) -> f64 {
    let base = match search_type {
        "deep" | "deep-lite" => 0.012,
        "deep-reasoning" => 0.015,
        _ => 0.007, // fast / auto / instant
    };
    let extra = num_results.saturating_sub(10) as f64 * 0.001;
    base + extra
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_cost_dollars_total() {
        let raw = json!({
            "results": [],
            "requestId": "req_1",
            "costDollars": { "total": 0.007 }
        });
        let parsed: ExaSearchResponse = serde_json::from_value(raw).unwrap();
        assert_eq!(parsed.request_id.as_deref(), Some("req_1"));
        assert_eq!(parsed.actual_cost(), Some(0.007));
    }

    #[test]
    fn parses_cost_dollars_bare_number_and_string_total() {
        let bare = json!({
            "results": [],
            "costDollars": 0.009
        });
        let parsed: ExaSearchResponse = serde_json::from_value(bare).unwrap();
        assert_eq!(parsed.actual_cost(), Some(0.009));

        let string_total = json!({
            "results": [],
            "costDollars": { "total": "0.011" }
        });
        let parsed: ExaSearchResponse = serde_json::from_value(string_total).unwrap();
        assert_eq!(parsed.actual_cost(), Some(0.011));
    }

    #[test]
    fn snippet_prefers_highlights() {
        let r = ExaResult {
            id: None,
            title: Some("T".into()),
            url: "https://example.com".into(),
            published_date: None,
            author: None,
            highlights: Some(vec!["hello".into(), "world".into()]),
            text: Some("full text ignored".into()),
        };
        assert_eq!(r.snippet().as_deref(), Some("hello … world"));
    }

    #[test]
    fn estimate_cost_by_mode() {
        assert!((estimate_search_cost("fast", 3) - 0.007).abs() < 1e-9);
        assert!((estimate_search_cost("deep", 5) - 0.012).abs() < 1e-9);
        assert!((estimate_search_cost("auto", 12) - 0.009).abs() < 1e-9);
    }
}
