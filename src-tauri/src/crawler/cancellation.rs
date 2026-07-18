use serde_json::json;

use super::errors::CrawlerError;
use super::supervisor::CrawlerSupervisor;

/// Ask the sidecar to cancel an in-flight request by id.
pub async fn cancel_request(
    supervisor: &CrawlerSupervisor,
    target_request_id: &str,
) -> Result<bool, CrawlerError> {
    let trimmed = target_request_id.trim();
    if trimmed.is_empty() {
        return Err(CrawlerError::Invalid("targetRequestId is required".into()));
    }
    let payload = supervisor
        .send_command(
            "cancel",
            json!({ "targetRequestId": trimmed }),
        )
        .await?;
    Ok(payload
        .get("cancelled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false))
}
