//! Trusted external-link opening.

use super::CommandError;

/// Validate and open an external http(s) URL in the system browser.
///
/// All frontend link opens must go through this command so SSRF checks run in
/// Rust. Secondary tool windows do not receive shell/opener plugin permissions;
/// they invoke this command instead.
#[tauri::command]
pub async fn open_external_url(url: String) -> Result<(), CommandError> {
    let validated = crate::search::validate_public_http_url(&url).map_err(|err| {
        CommandError::new(
            "invalid_url",
            format!("That link cannot be opened safely: {err}"),
        )
    })?;

    open_http_url_in_browser(validated.as_str()).map_err(|err| {
        tracing::warn!(error = %err, "failed to open external URL");
        CommandError::new(
            "open_failed",
            "Coreside could not open that link in your browser.",
        )
    })?;
    Ok(())
}

/// Open a validated http(s) URL with the OS default browser (no shell plugin).
///
/// Spawns the opener and returns once the process has started — do not wait on
/// the browser (that would block the async runtime until the user quits it).
fn open_http_url_in_browser(url: &str) -> Result<(), String> {
    // ponytail: URL already passed validate_public_http_url; use the OS opener
    // directly so we do not need deprecated shell.open / a second opener plugin.
    let child = {
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open").arg(url).spawn()
        }
        #[cfg(target_os = "windows")]
        {
            // Avoid cmd.exe: URL query metacharacters must never enter a shell parser.
            std::process::Command::new("rundll32.exe")
                .args(["url.dll,FileProtocolHandler", url])
                .spawn()
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            std::process::Command::new("xdg-open").arg(url).spawn()
        }
    }
    .map_err(|err| err.to_string())?;
    // Detach: drop Child without waiting so the opener can outlive this call.
    drop(child);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::open_external_url;

    #[tokio::test]
    async fn rejects_loopback_before_opening() {
        let err = open_external_url("http://127.0.0.1/".into())
            .await
            .expect_err("loopback must fail SSRF validation");
        assert_eq!(err.code, "invalid_url");
    }
}
