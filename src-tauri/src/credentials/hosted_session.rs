//! Hosted Coreside AI session in OS keyring (JWT bundle from future auth UI).
//! Format matches `src/lib/hosted/auth-session.ts`.

use serde::{Deserialize, Serialize};

use once_cell::sync::Lazy;

use super::{get_secret, set_secret, CredentialError};

static REFRESH_LOCK: Lazy<tokio::sync::Mutex<()>> =
    Lazy::new(|| tokio::sync::Mutex::new(()));

pub const KEYRING_ACCOUNT: &str = "coreside:hosted-auth";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredHostedSession {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: f64,
    pub user_id: String,
}

/// Read-only plan presentation (server is authoritative; desktop never writes plans).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostedPlanPresentation {
    pub plan_id: String,
    pub display_name: String,
    pub hosted_ai_enabled: bool,
    pub allowance_amount: u32,
    /// `server` when read from Supabase; `contract_default` / `contract_catalog` otherwise.
    pub source: String,
}

pub fn contract_default_plan_presentation() -> HostedPlanPresentation {
    HostedPlanPresentation {
        plan_id: "free".into(),
        display_name: "Free".into(),
        hosted_ai_enabled: false,
        allowance_amount: 0,
        source: "contract_default".into(),
    }
}

/// Read-only catalog stubs for Settings UI (server is authoritative).
pub fn contract_plan_catalog_summaries() -> Vec<HostedPlanPresentation> {
    vec![
        contract_default_plan_presentation(),
        HostedPlanPresentation {
            plan_id: "personal".into(),
            display_name: "Personal".into(),
            hosted_ai_enabled: true,
            allowance_amount: 500,
            source: "contract_catalog".into(),
        },
        HostedPlanPresentation {
            plan_id: "pro".into(),
            display_name: "Pro".into(),
            hosted_ai_enabled: true,
            allowance_amount: 2000,
            source: "contract_catalog".into(),
        },
    ]
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

fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}

fn load_stored_session() -> Option<StoredHostedSession> {
    let raw = get_secret(KEYRING_ACCOUNT).ok()?;
    parse_session(&raw)
}

/// Whether any hosted session JSON exists in the keyring (may be expired).
pub fn has_stored_session() -> bool {
    load_stored_session().is_some()
}

fn persist_session(session: &StoredHostedSession) -> Result<(), CredentialError> {
    let json = serde_json::to_string(session).map_err(|e| CredentialError::Other(e.to_string()))?;
    set_secret(KEYRING_ACCOUNT, &json)
}

/// Non-expired hosted session with access token.
pub fn load_valid_session(at: Option<f64>) -> Option<StoredHostedSession> {
    let session = load_stored_session()?;
    let now = at.unwrap_or_else(now_ms);
    if session_valid(&session, now) {
        Some(session)
    } else {
        None
    }
}

pub fn access_token() -> Option<String> {
    load_valid_session(None).map(|s| s.access_token)
}

/// Refresh an expired/near-expiry session using the stored refresh token.
pub async fn refresh_session_if_needed() -> Result<Option<StoredHostedSession>, CredentialError> {
    if let Some(session) = load_valid_session(None) {
        return Ok(Some(session));
    }
    let _refresh_guard = REFRESH_LOCK.lock().await;
    // Another caller may have refreshed while we waited.
    if let Some(session) = load_valid_session(None) {
        return Ok(Some(session));
    }
    let Some(stored) = load_stored_session() else {
        return Ok(None);
    };
    if stored.refresh_token.trim().is_empty() {
        return Err(CredentialError::Other(
            "Hosted auth session missing refresh token".into(),
        ));
    }
    let Some((supabase_url, publishable_key)) = crate::config::supabase_publishable_config() else {
        return Err(CredentialError::Other(
            "Supabase publishable config is not available".into(),
        ));
    };
    let base = supabase_url.trim_end_matches('/');
    let url = format!("{base}/auth/v1/token?grant_type=refresh_token");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| CredentialError::Other(e.to_string()))?;
    // Pin Supabase auth origin when DNS resolves publicly.
    let client = match crate::ai::platform::validate_and_build_credential_client(
        base,
        crate::ai::platform::EndpointClass::HostedCoresideGateway,
        false,
        false,
        std::time::Duration::from_secs(20),
    ) {
        Ok((_, pinned)) => pinned,
        Err(_) => client,
    };
    let response = client
        .post(&url)
        .header("apikey", publishable_key.trim())
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "refresh_token": stored.refresh_token.trim() }))
        .send()
        .await
        .map_err(|e| CredentialError::Other(e.to_string()))?;
    if !response.status().is_success() {
        return Err(CredentialError::Other(
            "Hosted auth session refresh failed".into(),
        ));
    }
    #[derive(Deserialize)]
    struct RefreshPayload {
        access_token: String,
        refresh_token: Option<String>,
        expires_in: Option<i64>,
        expires_at: Option<i64>,
        user: Option<RefreshUser>,
    }
    #[derive(Deserialize)]
    struct RefreshUser {
        id: String,
    }
    let payload: RefreshPayload = response
        .json()
        .await
        .map_err(|e| CredentialError::Other(e.to_string()))?;
    if payload.access_token.trim().is_empty() {
        return Err(CredentialError::Other(
            "Hosted auth refresh returned empty access token".into(),
        ));
    }
    let expires_at = payload
        .expires_at
        .map(|s| s as f64 * 1000.0)
        .or_else(|| {
            payload
                .expires_in
                .map(|secs| now_ms() + (secs as f64 * 1000.0))
        })
        .unwrap_or(stored.expires_at);
    let refreshed = StoredHostedSession {
        access_token: payload.access_token,
        refresh_token: payload
            .refresh_token
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| stored.refresh_token.clone()),
        expires_at,
        user_id: payload
            .user
            .map(|u| u.id)
            .filter(|id| !id.trim().is_empty())
            .unwrap_or_else(|| stored.user_id.clone()),
    };
    if !session_valid(&refreshed, now_ms()) {
        return Err(CredentialError::Other(
            "Hosted auth refresh returned an already-expired session".into(),
        ));
    }
    persist_session(&refreshed)?;
    Ok(Some(refreshed))
}

/// Access token for gateway calls, refreshing when needed.
pub async fn ensure_fresh_access_token() -> Result<String, CredentialError> {
    if let Some(session) = refresh_session_if_needed().await? {
        return Ok(session.access_token);
    }
    Err(CredentialError::NotFound)
}

#[derive(Debug, Deserialize)]
struct EntitlementRestRow {
    plan_id: Option<String>,
    hosted_ai_enabled: bool,
    allowance_amount: Option<f64>,
    ai_plan_catalog: Option<PlanCatalogRestRow>,
}

#[derive(Debug, Deserialize)]
struct PlanCatalogRestRow {
    display_name: Option<String>,
}

fn plan_presentation_from_entitlement_row(row: EntitlementRestRow) -> HostedPlanPresentation {
    let plan_id = row
        .plan_id
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| "free".into());
    let display_name = row
        .ai_plan_catalog
        .and_then(|c| c.display_name)
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| {
            if plan_id == "free" {
                "Free".into()
            } else {
                plan_id.clone()
            }
        });
    HostedPlanPresentation {
        plan_id,
        display_name,
        hosted_ai_enabled: row.hosted_ai_enabled,
        allowance_amount: row
            .allowance_amount
            .map(|n| n.max(0.0) as u32)
            .unwrap_or(0),
        source: "server".into(),
    }
}

/// Read the signed-in user's entitlements from Supabase (RLS: own row only).
pub async fn fetch_server_plan_presentation() -> Result<HostedPlanPresentation, CredentialError> {
    let access_token = ensure_fresh_access_token().await?;
    let (supabase_url, publishable_key) = crate::config::supabase_publishable_config()
        .ok_or_else(|| CredentialError::Other("Supabase publishable config is not available".into()))?;
    let base = supabase_url.trim_end_matches('/');
    let url = format!(
        "{base}/rest/v1/ai_entitlements?select=plan_id,hosted_ai_enabled,allowance_amount,ai_plan_catalog(display_name)&limit=1"
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| CredentialError::Other(e.to_string()))?;
    let client = match crate::ai::platform::validate_and_build_credential_client(
        base,
        crate::ai::platform::EndpointClass::HostedCoresideGateway,
        false,
        false,
        std::time::Duration::from_secs(15),
    ) {
        Ok((_, pinned)) => pinned,
        Err(_) => client,
    };
    let response = client
        .get(&url)
        .header("apikey", publishable_key.trim())
        .header("Authorization", format!("Bearer {}", access_token.trim()))
        .send()
        .await
        .map_err(|e| CredentialError::Other(e.to_string()))?;
    if !response.status().is_success() {
        return Err(CredentialError::Other(
            "Could not read hosted plan from server".into(),
        ));
    }
    let rows: Vec<EntitlementRestRow> = response
        .json()
        .await
        .map_err(|e| CredentialError::Other(e.to_string()))?;
    let Some(row) = rows.into_iter().next() else {
        return Err(CredentialError::Other(
            "No hosted entitlements row for signed-in user".into(),
        ));
    };
    Ok(plan_presentation_from_entitlement_row(row))
}

/// Session + publishable Supabase client config (from env).
pub fn adapter_ready() -> bool {
    access_token().is_some() && crate::config::supabase_publishable_config().is_some()
}

/// Hosted is effective when a session exists and no active BYOK or authless Local AI wins.
/// Must run with no database lock held (may read the keyring).
pub fn hosted_adapter_effective(active: Option<&crate::db::ProviderConnection>) -> bool {
    if !adapter_ready() {
        return false;
    }
    match active {
        Some(conn) => {
            if super::resolve::is_authless_local_connection(conn)
                || super::resolve::connection_is_mandatory_authless(conn)
            {
                return false;
            }
            match super::get_secret(&conn.keyring_account) {
                Ok(key) if !key.trim().is_empty() => false,
                _ => true,
            }
        }
        None => true,
    }
}

/// Read active connection from `db` then evaluate hosted effectiveness.
/// Do not call while holding the app database mutex — use [`hosted_adapter_effective`]
/// with a pre-read active row instead.
pub fn adapter_connected(db: &crate::db::Database) -> bool {
    let active = crate::db::get_active_provider_connection(db).ok().flatten();
    hosted_adapter_effective(active.as_ref())
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

    #[test]
    fn contract_catalog_includes_personal_and_pro() {
        let rows = contract_plan_catalog_summaries();
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().any(|p| p.plan_id == "personal" && p.hosted_ai_enabled));
        assert!(rows.iter().any(|p| p.plan_id == "pro" && p.allowance_amount == 2000));
    }

    #[test]
    fn plan_presentation_from_entitlement_row_uses_catalog_display_name() {
        let row = EntitlementRestRow {
            plan_id: Some("personal".into()),
            hosted_ai_enabled: true,
            allowance_amount: Some(500.0),
            ai_plan_catalog: Some(PlanCatalogRestRow {
                display_name: Some("Personal".into()),
            }),
        };
        let plan = plan_presentation_from_entitlement_row(row);
        assert_eq!(plan.plan_id, "personal");
        assert_eq!(plan.display_name, "Personal");
        assert!(plan.hosted_ai_enabled);
        assert_eq!(plan.allowance_amount, 500);
        assert_eq!(plan.source, "server");
    }

    #[test]
    fn contract_default_plan_is_read_only_stub() {
        let plan = contract_default_plan_presentation();
        assert_eq!(plan.plan_id, "free");
        assert!(!plan.hosted_ai_enabled);
        assert_eq!(plan.allowance_amount, 0);
        assert_eq!(plan.source, "contract_default");
    }

    #[test]
    fn persist_roundtrip_keeps_camel_case_fields() {
        let session = StoredHostedSession {
            access_token: "a".into(),
            refresh_token: "r".into(),
            expires_at: 9_999_999_999_999.0,
            user_id: "u1".into(),
        };
        if persist_session(&session).is_err() {
            eprintln!("skip persist_roundtrip: OS keyring unavailable");
            return;
        }
        let loaded = load_stored_session().expect("stored session");
        assert_eq!(loaded.access_token, "a");
        assert_eq!(loaded.refresh_token, "r");
        let _ = super::super::clear_session();
    }

    mod adapter_connected_tests {
        use super::*;
        use crate::credentials::account_for_connection;
        use crate::db::{self, Database, ProviderConnection};
        use std::sync::{Mutex, MutexGuard};
        use tempfile::tempdir;

        static ENV_LOCK: Mutex<()> = Mutex::new(());

        struct EnvVars {
            keys: Vec<String>,
        }

        impl EnvVars {
            fn hosted_supabase() -> Self {
                std::env::set_var("SUPABASE_URL", "https://test.supabase.co");
                std::env::set_var("SUPABASE_ANON_KEY", "test-anon-key");
                Self {
                    keys: vec![
                        "SUPABASE_URL".into(),
                        "SUPABASE_ANON_KEY".into(),
                    ],
                }
            }
        }

        impl Drop for EnvVars {
            fn drop(&mut self) {
                for key in &self.keys {
                    std::env::remove_var(key);
                }
            }
        }

        fn sample_conn(id: &str, provider: &str) -> ProviderConnection {
            let now = "2026-01-01T00:00:00Z".to_string();
            ProviderConnection {
                id: id.to_string(),
                provider: provider.to_string(),
                label: format!("{provider} label"),
                base_url: None,
                model_default: None,
                keyring_account: account_for_connection(id),
                is_active: true,
                last_status: None,
                last_tested_at: None,
                created_at: now.clone(),
                updated_at: now,
                provider_descriptor_id: Some(provider.to_string()),
                protocol_family: None,
                auth_mode: None,
                endpoint_class: None,
                api_version: None,
                region: None,
                deployment: None,
                organization_id: None,
                project_id: None,
                capability_profile_json: None,
                capability_checked_at: None,
                model_catalog_checked_at: None,
                provider_preset_version: Some("1".into()),
                enabled: true,
            }
        }

        fn ollama_authless_active() -> ProviderConnection {
            ProviderConnection {
                id: "ollama-1".into(),
                provider: "ollama".into(),
                label: "Local Ollama".into(),
                base_url: Some("http://127.0.0.1:11434".into()),
                model_default: Some("llama3.2".into()),
                keyring_account: account_for_connection("ollama-1"),
                is_active: true,
                last_status: Some("connected".into()),
                last_tested_at: None,
                created_at: "2026-01-01T00:00:00Z".into(),
                updated_at: "2026-01-01T00:00:00Z".into(),
                provider_descriptor_id: Some("ollama".into()),
                protocol_family: Some("ollama_native_chat".into()),
                auth_mode: Some("local_authless".into()),
                endpoint_class: Some("loopback_local".into()),
                api_version: None,
                region: None,
                deployment: None,
                organization_id: None,
                project_id: None,
                capability_profile_json: None,
                capability_checked_at: None,
                model_catalog_checked_at: None,
                provider_preset_version: Some("1".into()),
                enabled: true,
            }
        }

        /// Serializes env + keyring for adapter_connected tests (parallel-safe).
        struct HostedAdapterScope {
            _guard: MutexGuard<'static, ()>,
            _env: EnvVars,
        }

        impl HostedAdapterScope {
            fn enter() -> Option<Self> {
                let guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
                let env = EnvVars::hosted_supabase();
                let session =
                    r#"{"accessToken":"tok","refreshToken":"ref","expiresAt":9999999999999,"userId":"u1"}"#;
                if store_session_json(session).is_err() {
                    return None;
                }
                if !adapter_ready() {
                    let _ = clear_session();
                    return None;
                }
                Some(Self {
                    _guard: guard,
                    _env: env,
                })
            }
        }

        impl Drop for HostedAdapterScope {
            fn drop(&mut self) {
                let _ = clear_session();
            }
        }

        #[test]
        fn adapter_connected_respects_active_connection_privacy() {
            let Some(_scope) = HostedAdapterScope::enter() else {
                eprintln!("skip adapter_connected test: OS keyring unavailable");
                return;
            };

            let clear_dir = tempdir().unwrap();
            let clear_db = Database::open_path(&clear_dir.path().join("hosted-clear.db")).unwrap();
            assert!(
                adapter_connected(&clear_db),
                "hosted adapter should be effective with no blocking active connection"
            );

            let ollama_dir = tempdir().unwrap();
            let mut ollama_db =
                Database::open_path(&ollama_dir.path().join("hosted-ollama.db")).unwrap();
            let ollama_conn = ollama_authless_active();
            db::upsert_provider_connection(&mut ollama_db, &ollama_conn).unwrap();
            db::set_active_provider_connection(&mut ollama_db, "ollama-1").unwrap();
            assert!(
                !adapter_connected(&ollama_db),
                "authless Local AI must block hosted adapter"
            );

            let byok_dir = tempdir().unwrap();
            let mut byok_db = Database::open_path(&byok_dir.path().join("hosted-byok.db")).unwrap();
            let byok_conn = sample_conn("byok-1", "openai");
            db::upsert_provider_connection(&mut byok_db, &byok_conn).unwrap();
            db::set_active_provider_connection(&mut byok_db, "byok-1").unwrap();
            let account = account_for_connection("byok-1");
            if crate::credentials::set_secret(&account, "sk-byok-test").is_err() {
                eprintln!("skip BYOK branch: OS keyring unavailable");
                return;
            }
            assert!(
                !adapter_connected(&byok_db),
                "active BYOK must block hosted adapter"
            );
            let _ = crate::credentials::delete_secret(&account);
        }
    }
}
