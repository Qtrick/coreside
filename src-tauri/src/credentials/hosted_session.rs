//! Hosted Coreside AI session in OS keyring (JWT bundle from future auth UI).
//! Format matches `src/lib/hosted/auth-session.ts`.

use serde::Deserialize;

use super::{get_secret, CredentialError};

pub const KEYRING_ACCOUNT: &str = "coreside:hosted-auth";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredHostedSession {
    pub access_token: String,
    #[allow(dead_code)]
    pub refresh_token: String,
    pub expires_at: f64,
    pub user_id: String,
}

fn parse_session(raw: &str) -> Option<StoredHostedSession> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    serde_json::from_str(trimmed).ok()
}

fn session_valid(session: &StoredHostedSession, now_ms: f64) -> bool {
    if session.access_token.trim().is_empty() {
        return false;
    }
    if !session.expires_at.is_finite() {
        return false;
    }
    // Small skew so we refresh before edge expiry.
    now_ms + 5_000.0 < session.expires_at
}

/// Non-expired hosted session with access token.
pub fn load_valid_session(now_ms: Option<f64>) -> Option<StoredHostedSession> {
    let raw = get_secret(KEYRING_ACCOUNT).ok()?;
    let session = parse_session(&raw)?;
    let now = now_ms.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as f64)
            .unwrap_or(0.0)
    });
    if session_valid(&session, now) {
        Some(session)
    } else {
        None
    }
}

pub fn access_token() -> Option<String> {
    load_valid_session(None).map(|s| s.access_token)
}

/// Session + publishable Supabase client config (from env).
pub fn adapter_ready() -> bool {
    access_token().is_some() && crate::config::supabase_publishable_config().is_some()
}

/// Hosted is effective when adapter is ready and no active BYOK connection wins.
pub fn adapter_connected(db: &crate::db::Database) -> bool {
    if !adapter_ready() {
        return false;
    }
    match crate::db::get_active_provider_connection(db) {
        Ok(Some(conn)) => match super::get_secret(&conn.keyring_account) {
            Ok(key) if !key.trim().is_empty() => false,
            _ => true,
        },
        _ => true,
    }
}

pub fn store_session_json(json: &str) -> Result<(), CredentialError> {
    let session = parse_session(json)
        .ok_or_else(|| CredentialError::Other("Invalid hosted auth session JSON".into()))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0);
    if !session_valid(&session, now) {
        return Err(CredentialError::Other(
            "Hosted auth session is expired or invalid".into(),
        ));
    }
    if !session.access_token.trim().is_empty() {
        super::set_secret(KEYRING_ACCOUNT, json.trim())
    } else {
        Err(CredentialError::Other(
            "Hosted auth session missing access token".into(),
        ))
    }
}

pub fn clear_session() -> Result<(), CredentialError> {
    super::delete_secret(KEYRING_ACCOUNT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_camel_case_session() {
        let raw =
            r#"{"accessToken":"tok","refreshToken":"ref","expiresAt":9999999999999,"userId":"u1"}"#;
        let s = parse_session(raw).unwrap();
        assert_eq!(s.access_token, "tok");
        assert!(session_valid(&s, 1.0));
    }

    #[test]
    fn rejects_expired_session() {
        let raw = r#"{"accessToken":"tok","refreshToken":"ref","expiresAt":1000,"userId":"u1"}"#;
        let s = parse_session(raw).unwrap();
        assert!(!session_valid(&s, 10_000.0));
    }
}
