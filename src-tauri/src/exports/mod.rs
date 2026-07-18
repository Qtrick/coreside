//! Tool export package builders (no secrets, user-chosen path).

use serde_json::{json, Value};

pub fn sanitize_filename(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch.to_ascii_lowercase());
        } else if ch.is_whitespace() {
            if !out.ends_with('-') {
                out.push('-');
            }
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() || trimmed.contains("..") {
        "coreside-tool".into()
    } else {
        trimmed.chars().take(80).collect()
    }
}

fn is_secret_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "apikey"
            | "api_key"
            | "authorization"
            | "secret"
            | "token"
            | "credentials"
            | "password"
            | "passwd"
            | "private_key"
            | "privatekey"
            | "access_token"
            | "accesstoken"
            | "refresh_token"
            | "refreshtoken"
            | "bearer"
            | "client_secret"
            | "clientsecret"
    ) || lower.contains("apikey")
        || lower.contains("api_key")
        || lower.ends_with("_secret")
        || lower.ends_with("secret")
        || lower.ends_with("_token")
        || lower.ends_with("password")
}

/// Recursively remove secret-bearing keys from tool definitions and state.
pub fn strip_secrets_from_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, child) in map {
                if is_secret_key(key) {
                    continue;
                }
                out.insert(key.clone(), strip_secrets_from_value(child));
            }
            Value::Object(out)
        }
        Value::Array(items) => {
            Value::Array(items.iter().map(strip_secrets_from_value).collect())
        }
        other => other.clone(),
    }
}

pub fn strip_secrets_from_tool(def: &Value) -> Value {
    strip_secrets_from_value(def)
}

pub fn coreside_tool_package(
    tool: &Value,
    state: Option<&Value>,
    app_version: &str,
) -> Value {
    let safe_state = state
        .map(strip_secrets_from_value)
        .unwrap_or_else(|| json!({}));
    json!({
        "schemaVersion": "1",
        "format": "coreside-tool",
        "exportedAt": chrono::Utc::now().to_rfc3339(),
        "coresideVersion": app_version,
        "tool": strip_secrets_from_tool(tool),
        "state": safe_state,
    })
}

pub fn standalone_html_for_clock(tool_name: &str, props: &Value) -> String {
    let title = tool_name.replace('<', "").replace('>', "");
    let show_seconds = props
        .get("showSeconds")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let show_date = props
        .get("showDate")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let hour24 = props
        .get("hour24")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1"/>
<title>{title}</title>
<style>
  :root {{ color-scheme: light dark; }}
  body {{
    margin: 0; min-height: 100vh; display: grid; place-items: center;
    font-family: ui-sans-serif, system-ui, sans-serif;
    background: #141714; color: #eef2ec;
  }}
  .clock {{ text-align: center; }}
  .time {{ font-size: clamp(2.5rem, 10vw, 5rem); font-variant-numeric: tabular-nums; letter-spacing: 0.04em; }}
  .date {{ margin-top: 0.75rem; opacity: 0.72; font-size: 1.05rem; }}
</style>
</head>
<body>
  <div class="clock" data-coreside-export="clock">
    <div class="time" id="time">--:--</div>
    <div class="date" id="date"></div>
  </div>
<script>
(function() {{
  var showSeconds = {show_seconds};
  var showDate = {show_date};
  var hour24 = {hour24};
  function pad(n) {{ return String(n).padStart(2, '0'); }}
  function tick() {{
    var now = new Date();
    var h = now.getHours();
    var suffix = '';
    if (!hour24) {{
      suffix = h >= 12 ? ' PM' : ' AM';
      h = h % 12; if (h === 0) h = 12;
    }}
    var t = pad(h) + ':' + pad(now.getMinutes());
    if (showSeconds) t += ':' + pad(now.getSeconds());
    t += suffix;
    document.getElementById('time').textContent = t;
    var dateEl = document.getElementById('date');
    if (showDate) {{
      dateEl.textContent = now.toLocaleDateString(undefined, {{ weekday: 'long', year: 'numeric', month: 'long', day: 'numeric' }});
      dateEl.style.display = '';
    }} else {{
      dateEl.style.display = 'none';
    }}
  }}
  tick();
  setInterval(tick, 250);
}})();
</script>
</body>
</html>
"#,
        title = title,
        show_seconds = if show_seconds { "true" } else { "false" },
        show_date = if show_date { "true" } else { "false" },
        hour24 = if hour24 { "true" } else { "false" },
    )
}

pub fn assert_no_secrets(blob: &str) -> Result<(), String> {
    let lower = blob.to_lowercase();
    for needle in [
        "apikey",
        "api_key",
        "authorization: bearer",
        "sk-ant-",
        "sk-or-v1-",
        "sk-proj-",
        "aizasy",
        "client_secret",
        "private_key",
    ] {
        if lower.contains(needle) {
            return Err(format!("Export contains forbidden secret pattern: {needle}"));
        }
    }
    Ok(())
}

/// Assert only high-confidence credential token prefixes (safe for chat exports).
pub fn assert_no_credential_tokens(blob: &str) -> Result<(), String> {
    let lower = blob.to_lowercase();
    for needle in ["sk-ant-", "sk-or-v1-", "sk-proj-", "aizasy"] {
        if lower.contains(needle) {
            return Err(format!("Export contains forbidden secret pattern: {needle}"));
        }
    }
    Ok(())
}

/// Redact common credential substrings from free-text (e.g. pasted keys in chat).
pub fn redact_secret_patterns(text: &str) -> String {
    let mut out = text.to_string();
    // Order longer prefixes first where relevant.
    for pattern in [
        "sk-ant-",
        "sk-or-v1-",
        "sk-proj-",
        "AIzaSy",
        "aizasy",
    ] {
        out = redact_token_runs(&out, pattern);
    }
    out
}

fn redact_token_runs(text: &str, prefix: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let prefix_lower = prefix.to_ascii_lowercase();
    let mut result = String::with_capacity(text.len());
    let mut idx = 0;
    let bytes = text.as_bytes();
    let lower_bytes = lower.as_bytes();
    let prefix_bytes = prefix_lower.as_bytes();
    while idx < bytes.len() {
        if idx + prefix_bytes.len() <= lower_bytes.len()
            && lower_bytes[idx..idx + prefix_bytes.len()] == *prefix_bytes
        {
            result.push_str("[REDACTED]");
            idx += prefix_bytes.len();
            while idx < bytes.len() {
                let c = bytes[idx] as char;
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    idx += 1;
                } else {
                    break;
                }
            }
        } else {
            // Copy one UTF-8 char
            let ch = text[idx..].chars().next().unwrap_or('\0');
            result.push(ch);
            idx += ch.len_utf8();
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_path_traversal_names() {
        assert_eq!(sanitize_filename("../../secret"), "secret");
        assert_eq!(sanitize_filename("Digital Clock"), "digital-clock");
    }

    #[test]
    fn clock_html_has_no_keys() {
        let html = standalone_html_for_clock("Clock", &json!({"showSeconds": true}));
        assert!(assert_no_secrets(&html).is_ok());
        assert!(html.contains("setInterval"));
    }

    #[test]
    fn strips_nested_secrets_from_tool_and_state() {
        let tool = json!({
            "name": "Demo",
            "props": { "apiKey": "sk-secret", "title": "ok" },
            "nested": [{ "token": "abc", "label": "keep" }]
        });
        let cleaned = strip_secrets_from_tool(&tool);
        let text = cleaned.to_string();
        assert!(!text.contains("sk-secret"));
        assert!(!text.contains("abc"));
        assert!(text.contains("ok"));
        assert!(text.contains("keep"));

        let package = coreside_tool_package(
            &tool,
            Some(&json!({"password": "nope", "count": 1})),
            "0.0.0",
        );
        let package_text = package.to_string();
        assert!(!package_text.contains("nope"));
        assert!(package_text.contains("\"count\":1") || package_text.contains("\"count\": 1"));
    }

    #[test]
    fn redacts_credential_token_prefixes() {
        let cleaned = redact_secret_patterns("key sk-ant-abc123XYZ rest");
        assert!(!cleaned.contains("sk-ant-"));
        assert!(cleaned.contains("[REDACTED]"));
        assert!(cleaned.contains("rest"));
    }
}
