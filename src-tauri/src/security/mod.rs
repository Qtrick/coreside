//! Secret redaction, protected resources, and user-facing error sanitization.

mod protected_resources;

pub use protected_resources::{
    assert_not_protected, is_protected, list_protected_ids, PROTECTED_IDS,
};

use once_cell::sync::Lazy;
use regex::Regex;

static KEY_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r"(?i)(AIza[0-9A-Za-z\-_]{20,})").expect("valid regex"),
        Regex::new(r"(?i)(sk-[A-Za-z0-9\-_]{20,})").expect("valid regex"),
        Regex::new(r"(?i)(Bearer\s+[A-Za-z0-9\-._~+/]+=*)").expect("valid regex"),
        Regex::new(r#"(?i)(api[_-]?key[=:]\s*)([^\s&"']+)"#).expect("valid regex"),
        Regex::new(r"(?i)(key[=:]\s*)([A-Za-z0-9\-_]{16,})").expect("valid regex"),
    ]
});

/// Redact known API key shapes and an optional concrete key value from text.
pub fn redact_secrets(text: &str, key: Option<&str>) -> String {
    let mut out = text.to_string();

    if let Some(k) = key {
        if !k.is_empty() {
            out = out.replace(k, "[REDACTED]");
        }
    }

    for re in KEY_PATTERNS.iter() {
        out = re
            .replace_all(&out, |caps: &regex::Captures| {
                if caps.len() >= 3 {
                    format!("{}[REDACTED]", &caps[1])
                } else {
                    "[REDACTED]".to_string()
                }
            })
            .into_owned();
    }

    out
}

/// Produce a short, safe error message for the frontend.
pub fn sanitize_error(err: &str, key: Option<&str>) -> String {
    let redacted = redact_secrets(err, key);
    let trimmed = redacted.trim();
    if trimmed.is_empty() {
        return "An unexpected error occurred.".to_string();
    }
    // Keep messages readable but bounded. Truncate on a char boundary (never panic on UTF-8).
    const MAX_CHARS: usize = 400;
    let mut out = String::new();
    for (i, ch) in trimmed.chars().enumerate() {
        if i >= MAX_CHARS {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_concrete_key() {
        let s = redact_secrets("using key=abcSECRET123xyz failed", Some("abcSECRET123xyz"));
        assert!(!s.contains("abcSECRET123xyz"));
        assert!(s.contains("[REDACTED]"));
    }

    #[test]
    fn redacts_gemini_style_key() {
        let s = redact_secrets("AIzaSyA-test-key-value-1234567890abcd", None);
        assert_eq!(s, "[REDACTED]");
    }

    #[test]
    fn sanitize_error_truncates_unicode_safely() {
        // 500 CJK chars cross the 400-char bound mid-string without ASCII.
        let long = "测".repeat(500);
        let out = sanitize_error(&long, None);
        assert!(out.ends_with('…'));
        assert!(out.chars().count() <= 401);
        assert!(std::str::from_utf8(out.as_bytes()).is_ok());
    }
}
