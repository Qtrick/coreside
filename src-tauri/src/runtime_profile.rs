//! Dev vs release runtime profile for data paths and keychain namespaces.
//!
//! Debug builds default to an isolated `dev` profile so `npm run dev` does not
//! share SQLite/media or OS keychain entries with a release `Coreside.app`.
//! Opt into the previous shared layout with `CORESIDE_PROFILE=shared` or
//! `CORESIDE_DEV_SHARE_DATA=1`. Never auto-migrates user data.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProfile {
    /// Consumer data: `…/coreside` (legacy `Coreside` reuse) and unsuffixed keyring.
    Release,
    /// Isolated development: `…/coreside-dev` and `*.dev` keyring services.
    Dev,
}

impl RuntimeProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Release => "release",
            Self::Dev => "dev",
        }
    }

    pub fn product_folder(self) -> &'static str {
        match self {
            Self::Release => "coreside",
            Self::Dev => "coreside-dev",
        }
    }

    pub fn keyring_service(self, base: &str) -> String {
        match self {
            Self::Release => base.to_string(),
            Self::Dev => format!("{base}.dev"),
        }
    }
}

fn env_truthy(name: &str) -> bool {
    matches!(
        std::env::var(name)
            .ok()
            .as_deref()
            .map(|v| v.trim().to_ascii_lowercase())
            .as_deref(),
        Some("1") | Some("true") | Some("yes") | Some("on")
    )
}

/// Resolve the active runtime profile.
///
/// Precedence:
/// 1. `CORESIDE_PROFILE=dev|shared|release|…`
/// 2. `CORESIDE_DEV_SHARE_DATA=1` → release (shared with packaged app)
/// 3. `cfg!(debug_assertions)` → dev; otherwise release
pub fn resolve_runtime_profile() -> RuntimeProfile {
    if let Ok(raw) = std::env::var("CORESIDE_PROFILE") {
        match raw.trim().to_ascii_lowercase().as_str() {
            "dev" | "development" => return RuntimeProfile::Dev,
            "shared" | "release" | "prod" | "production" => return RuntimeProfile::Release,
            "" => {}
            other => {
                tracing::warn!(
                    profile = %other,
                    "unknown CORESIDE_PROFILE; using build default"
                );
            }
        }
    }
    if env_truthy("CORESIDE_DEV_SHARE_DATA") {
        return RuntimeProfile::Release;
    }
    if cfg!(debug_assertions) {
        RuntimeProfile::Dev
    } else {
        RuntimeProfile::Release
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    fn with_profile_env<R>(profile: Option<&str>, share: Option<&str>, f: impl FnOnce() -> R) -> R {
        static LOCK: Mutex<()> = Mutex::new(());
        let _guard: MutexGuard<'_, ()> = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev_profile = std::env::var("CORESIDE_PROFILE").ok();
        let prev_share = std::env::var("CORESIDE_DEV_SHARE_DATA").ok();
        match profile {
            Some(v) => std::env::set_var("CORESIDE_PROFILE", v),
            None => std::env::remove_var("CORESIDE_PROFILE"),
        }
        match share {
            Some(v) => std::env::set_var("CORESIDE_DEV_SHARE_DATA", v),
            None => std::env::remove_var("CORESIDE_DEV_SHARE_DATA"),
        }
        struct Restore(Option<String>, Option<String>);
        impl Drop for Restore {
            fn drop(&mut self) {
                match &self.0 {
                    Some(v) => std::env::set_var("CORESIDE_PROFILE", v),
                    None => std::env::remove_var("CORESIDE_PROFILE"),
                }
                match &self.1 {
                    Some(v) => std::env::set_var("CORESIDE_DEV_SHARE_DATA", v),
                    None => std::env::remove_var("CORESIDE_DEV_SHARE_DATA"),
                }
            }
        }
        let _restore = Restore(prev_profile, prev_share);
        f()
    }

    #[test]
    fn profile_env_dev_and_shared() {
        with_profile_env(Some("dev"), None, || {
            assert_eq!(resolve_runtime_profile(), RuntimeProfile::Dev);
            assert_eq!(
                resolve_runtime_profile().keyring_service("coreside.provider"),
                "coreside.provider.dev"
            );
        });
        with_profile_env(Some("shared"), None, || {
            assert_eq!(resolve_runtime_profile(), RuntimeProfile::Release);
            assert_eq!(
                resolve_runtime_profile().keyring_service("coreside.provider"),
                "coreside.provider"
            );
        });
        with_profile_env(None, Some("1"), || {
            assert_eq!(resolve_runtime_profile(), RuntimeProfile::Release);
        });
    }

    #[test]
    fn debug_default_is_dev_without_overrides() {
        with_profile_env(None, None, || {
            let expected = if cfg!(debug_assertions) {
                RuntimeProfile::Dev
            } else {
                RuntimeProfile::Release
            };
            assert_eq!(resolve_runtime_profile(), expected);
            assert_eq!(
                resolve_runtime_profile().product_folder(),
                expected.product_folder()
            );
        });
    }
}
