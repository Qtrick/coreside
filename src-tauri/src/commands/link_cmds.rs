//! Trusted external-link opening.

use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

use super::CommandError;

/// Validate and open an external http(s) URL in the system browser.
///
/// All frontend link opens must go through this command so SSRF checks run in
/// Rust. Secondary tool windows do not receive `shell:allow-open`; they invoke
/// this command instead of the shell plugin.
#[tauri::command]
pub async fn open_external_url(app: AppHandle, url: String) -> Result<(), CommandError> {
    let validated = crate::search::validate_public_http_url(&url).map_err(|err| {
        CommandError::new(
            "invalid_url",
            format!("That link cannot be opened safely: {err}"),
        )
    })?;

    app.shell().open(validated.as_str(), None).map_err(|err| {
        tracing::warn!(error = %err, "failed to open external URL");
        CommandError::new(
            "open_failed",
            "Coreside could not open that link in your browser.",
        )
    })?;
    Ok(())
}
