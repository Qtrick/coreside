//! OS secure credential storage for provider API keys.
//!
//! Secrets never enter SQLite. Account ids are stored as metadata only.

mod resolve;

pub use resolve::{defaults_for, resolve_connection_by_id, resolve_credentials};

use thiserror::Error;

const SERVICE: &str = "coreside.provider";

#[derive(Debug, Error)]
pub enum CredentialError {
    #[error("Secure credential storage is unavailable on this system: {0}")]
    Unavailable(String),
    #[error("Credential not found")]
    NotFound,
    #[error("Credential storage error: {0}")]
    Other(String),
}

pub fn account_for_connection(connection_id: &str) -> String {
    format!("coreside:{connection_id}")
}

fn map_keyring_error(err: keyring::Error) -> CredentialError {
    match err {
        keyring::Error::NoEntry => CredentialError::NotFound,
        keyring::Error::PlatformFailure(e) => CredentialError::Unavailable(e.to_string()),
        keyring::Error::NoStorageAccess(e) => CredentialError::Unavailable(e.to_string()),
        other => CredentialError::Other(other.to_string()),
    }
}

fn entry(account: &str) -> Result<keyring::Entry, CredentialError> {
    keyring::Entry::new(SERVICE, account).map_err(map_keyring_error)
}

/// Store or replace an API key for the given keyring account id.
pub fn set_secret(account: &str, secret: &str) -> Result<(), CredentialError> {
    let secret = secret.trim();
    if secret.is_empty() {
        return Err(CredentialError::Other("API key cannot be empty".into()));
    }
    entry(account)?
        .set_password(secret)
        .map_err(map_keyring_error)
}

/// Read an API key. Returns NotFound if absent.
pub fn get_secret(account: &str) -> Result<String, CredentialError> {
    entry(account)?.get_password().map_err(map_keyring_error)
}

/// Delete an API key. Missing entries are treated as success.
pub fn delete_secret(account: &str) -> Result<(), CredentialError> {
    match entry(account)?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(map_keyring_error(e)),
    }
}

pub fn has_secret(account: &str) -> bool {
    matches!(get_secret(account), Ok(s) if !s.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_format() {
        assert_eq!(account_for_connection("abc"), "coreside:abc");
    }
}
