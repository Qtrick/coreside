//! Registered action handlers.
//!
//! Every handler runs behind the gateway, so by the time one is called the
//! action is declared, permitted, approved or granted, and within limits.
//! Handlers still enforce *ownership*: an application may only touch its own
//! records, its own tool state, and media visible in its project scope.

use serde_json::{json, Value};

use crate::db::{Database, DbError};

use super::context::ActionRunContext;
use super::descriptor::ActionDescriptor;

#[derive(Debug, Clone)]
pub struct ActionError {
    pub code: String,
    pub message: String,
}

impl ActionError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self::new("invalid_input", message)
    }
}

impl From<DbError> for ActionError {
    fn from(value: DbError) -> Self {
        let code = match value {
            DbError::NotFound(_) => "not_found",
            DbError::Invalid(_) => "invalid_input",
            _ => "storage_error",
        };
        Self::new(code, value.to_string())
    }
}

pub type HandlerResult = Result<Value, ActionError>;

pub fn dispatch(
    db: &mut Database,
    ctx: &ActionRunContext,
    descriptor: &ActionDescriptor,
    input: &Value,
) -> HandlerResult {
    match descriptor.name.as_str() {
        "local_data.query" => local_data_query(db, ctx, input),
        "local_data.write" => local_data_write(db, ctx, input),
        "local_data.delete" => local_data_delete(db, ctx, input),
        "tool_state.set" => tool_state_set(db, ctx, input),
        "web_search.request" => web_search_request(db, ctx, input),
        "media.read" => media_read(db, ctx, input),
        "external_link.open" => external_link_open(input),
        "export.prepare" => export_prepare(db, ctx, input),
        "automation.propose" => automation_propose(ctx, input),
        "agent.submit_event" => agent_submit_event(db, ctx, input),
        other => Err(ActionError::new(
            "unknown_action",
            format!("no handler for {other}"),
        )),
    }
}

fn application_id(ctx: &ActionRunContext) -> Result<&str, ActionError> {
    ctx.application_id
        .as_deref()
        .ok_or_else(|| ActionError::new("invalid_context", "this action requires an application"))
}

fn str_field<'a>(input: &'a Value, key: &str) -> Result<&'a str, ActionError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| ActionError::invalid(format!("{key} is required")))
}

fn model_id_field<'a>(input: &'a Value) -> Result<&'a str, ActionError> {
    input
        .get("modelId")
        .or_else(|| input.get("model"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| ActionError::invalid("modelId is required"))
}

fn local_data_query(db: &mut Database, ctx: &ActionRunContext, input: &Value) -> HandlerResult {
    let app = application_id(ctx)?;
    let model_id = model_id_field(input)?;
    let limit = input
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(100)
        .min(500) as usize;
    let records = super::super::data::query_records(db, app, model_id, limit)?;
    Ok(json!({ "records": records, "count": records.len() }))
}

fn local_data_write(db: &mut Database, ctx: &ActionRunContext, input: &Value) -> HandlerResult {
    let app = application_id(ctx)?;
    let model_id = model_id_field(input)?.to_string();
    let data = input
        .get("data")
        .cloned()
        .filter(|v| v.is_object())
        .ok_or_else(|| ActionError::invalid("data must be an object"))?;
    match input.get("recordId").and_then(|v| v.as_str()) {
        Some(record_id) => {
            let base = input.get("baseVersion").and_then(|v| v.as_i64());
            super::super::data::update_record(db, app, record_id, data, base)?;
            Ok(json!({ "recordId": record_id, "created": false }))
        }
        None => {
            let id = super::super::data::create_record(db, app, &model_id, data)?;
            Ok(json!({ "recordId": id, "created": true }))
        }
    }
}

fn local_data_delete(db: &mut Database, ctx: &ActionRunContext, input: &Value) -> HandlerResult {
    let app = application_id(ctx)?;
    let record_id = str_field(input, "recordId")?;
    super::super::data::delete_record(db, app, record_id)?;
    Ok(json!({ "recordId": record_id, "deleted": true }))
}

fn tool_state_set(db: &mut Database, ctx: &ActionRunContext, input: &Value) -> HandlerResult {
    let app = application_id(ctx)?;
    let key = str_field(input, "key")?;
    if key.contains("..") || key.starts_with("core.") || crate::security::is_protected(key) {
        return Err(ActionError::new(
            "protected_resource",
            "protected state keys cannot be changed",
        ));
    }
    if crate::security::is_protected(app) {
        return Err(ActionError::new(
            "protected_resource",
            "protected resources cannot be changed",
        ));
    }
    let value = input
        .get("value")
        .cloned()
        .ok_or_else(|| ActionError::invalid("value is required"))?;
    // Ownership: an application only ever writes its own tool state row.
    let mut state = crate::db::get_tool_state(db, app)?.unwrap_or_else(|| json!({}));
    match state.as_object_mut() {
        Some(obj) => {
            obj.insert(key.to_string(), value);
        }
        None => state = json!({ key: value }),
    }
    crate::db::save_tool_state(db, app, &state)?;
    Ok(json!({ "key": key, "saved": true }))
}

/// Generated applications never reach the network. The request is recorded for
/// the user to run in chat, where the real Exa / Crawl4AI budgets apply.
fn web_search_request(db: &mut Database, ctx: &ActionRunContext, input: &Value) -> HandlerResult {
    let query = str_field(input, "query")?;
    if query.chars().count() > 400 {
        return Err(ActionError::invalid("query is too long"));
    }
    let reason = input.get("reason").and_then(|v| v.as_str()).unwrap_or("");
    if let Some(conversation_id) = ctx.conversation_id.as_deref() {
        crate::runtime_v2::context_ledger::append_ledger_entry(
            db,
            conversation_id,
            ctx.project_id.as_deref(),
            None,
            "web_search_request",
            "user_visible",
            &json!({ "query": query, "reason": reason, "applicationId": ctx.application_id }),
            &format!("Application requested web research: {query}"),
            Some("session"),
        )?;
    }
    Ok(json!({
        "status": "requested",
        "performed": false,
        "query": query,
        "runInChat": true,
        "note": "Coreside runs web research in chat, not inside generated applications."
    }))
}

fn media_read(db: &mut Database, ctx: &ActionRunContext, input: &Value) -> HandlerResult {
    let project = ctx.project_id.as_deref();
    if let Some(asset_id) = input.get("assetId").and_then(|v| v.as_str()) {
        let asset = crate::media::get_media_asset(db, asset_id)?;
        if !crate::media::media_readable_in_scope(&asset, project) {
            return Err(ActionError::new(
                "not_found",
                "that media item is not available in this project",
            ));
        }
        return Ok(json!({ "assets": [media_summary(&asset)], "count": 1 }));
    }
    let limit = input
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(50)
        .min(200) as usize;
    let assets = crate::media::list_media_assets(db, project, limit)?;
    let visible: Vec<Value> = assets
        .iter()
        .filter(|a| crate::media::media_readable_in_scope(a, project))
        .map(media_summary)
        .collect();
    Ok(json!({ "count": visible.len(), "assets": visible }))
}

fn media_summary(asset: &crate::media::MediaAsset) -> Value {
    json!({
        "id": asset.id,
        "title": asset.title,
        "category": asset.category,
        "mimeType": asset.mime_type,
        "width": asset.width,
        "height": asset.height,
        "durationMs": asset.duration_ms,
        "license": asset.license,
        "attribution": asset.attribution,
        "sourcePageUrl": asset.source_page_url,
        "createdAt": asset.created_at,
    })
}

/// Validate and normalize a URL. Nothing is opened here; the protected shell
/// decides what to do with the returned value.
fn external_link_open(input: &Value) -> HandlerResult {
    let raw = str_field(input, "url")?;
    Ok(json!({ "url": validate_external_url(raw)?, "openedBy": "shell" }))
}

pub fn validate_external_url(raw: &str) -> Result<String, ActionError> {
    // Reuse the SSRF suite used by search/media so registered-action link
    // opens cannot reach private networks, even via DNS rebinding.
    crate::search::validate_public_http_url(raw)
        .map(|url| url.to_string())
        .map_err(|err| ActionError::invalid(err.to_string()))
}

fn export_prepare(db: &mut Database, ctx: &ActionRunContext, input: &Value) -> HandlerResult {
    let app = application_id(ctx)?;
    let record = super::super::manifest::get_manifest(db, app)?;
    let limit = input
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(200)
        .min(500) as usize;
    let mut payload = json!({
        "schemaVersion": "1",
        "format": "coreside-application-export",
        "applicationId": app,
        "name": record.manifest.name,
        "exportedAt": crate::db::now_rfc3339(),
    });
    if let Some(model_id) = input.get("modelId").and_then(|v| v.as_str()) {
        let records = super::super::data::query_records(db, app, model_id, limit)?;
        payload["modelId"] = json!(model_id);
        payload["records"] = json!(records);
    }
    let safe = crate::exports::strip_secrets_from_value(&payload);
    crate::exports::assert_no_secrets(&safe.to_string())
        .map_err(|e| ActionError::new("export_blocked", e))?;
    Ok(json!({
        "filenameSuggestion": format!("{}.json", crate::exports::sanitize_filename(&record.manifest.name)),
        "payload": safe,
    }))
}

/// Proposing never schedules. The normalized proposal is returned for the user
/// to confirm through the protected automations UI.
fn automation_propose(ctx: &ActionRunContext, input: &Value) -> HandlerResult {
    use crate::automations::{AutomationAction, AutomationTrigger};

    let app = application_id(ctx)?;
    let name = str_field(input, "name")?;
    let trigger: AutomationTrigger = serde_json::from_value(
        input
            .get("trigger")
            .cloned()
            .ok_or_else(|| ActionError::invalid("trigger is required"))?,
    )
    .map_err(|e| ActionError::invalid(format!("trigger is not supported: {e}")))?;
    let action: AutomationAction = serde_json::from_value(
        input
            .get("action")
            .cloned()
            .ok_or_else(|| ActionError::invalid("action is required"))?,
    )
    .map_err(|e| ActionError::invalid(format!("action is not supported: {e}")))?;

    if let AutomationAction::SetToolStateValue { tool_id, .. } = &action {
        if tool_id != app {
            return Err(ActionError::new(
                "permission_denied",
                "an application can only propose automations for itself",
            ));
        }
    }
    if matches!(action, AutomationAction::AiPrompt { .. }) {
        return Err(ActionError::new(
            "permission_denied",
            "applications cannot propose AI-backed automations",
        ));
    }
    crate::automations::validate_automation(name, &trigger, &action, false)
        .map_err(ActionError::invalid)?;

    Ok(json!({
        "status": "proposed",
        "scheduled": false,
        "requiresUserConfirmation": true,
        "applicationId": app,
        "proposal": {
            "name": name,
            "trigger": trigger,
            "action": action,
            "requiresAi": false,
        }
    }))
}

fn agent_submit_event(db: &mut Database, ctx: &ActionRunContext, input: &Value) -> HandlerResult {
    let conversation_id = ctx
        .conversation_id
        .as_deref()
        .ok_or_else(|| ActionError::new("invalid_context", "this action requires an open chat"))?;
    let summary = str_field(input, "summary")?;
    if summary.chars().count() > 500 {
        return Err(ActionError::invalid("summary is too long"));
    }
    let payload = input.get("payload").cloned().unwrap_or_else(|| json!({}));
    let entry = crate::runtime_v2::context_ledger::append_ledger_entry(
        db,
        conversation_id,
        ctx.project_id.as_deref(),
        None,
        "application_event",
        "user_visible",
        &json!({ "applicationId": ctx.application_id, "payload": payload }),
        summary,
        Some("session"),
    )?;
    Ok(json!({ "entryId": entry.id, "submitted": true }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_urls_are_validated() {
        // Literal public IPs avoid DNS in unit tests; hostnames are covered by
        // search::safety tests that exercise resolve_public_host.
        assert!(validate_external_url("https://1.1.1.1/docs").is_ok());
        assert!(validate_external_url("http://8.8.8.8").is_ok());
        assert!(validate_external_url("file:///etc/passwd").is_err());
        assert!(validate_external_url("javascript:alert(1)").is_err());
        assert!(validate_external_url("https://localhost/admin").is_err());
        assert!(validate_external_url("https://127.0.0.1/admin").is_err());
        assert!(validate_external_url("https://192.168.1.5/admin").is_err());
        assert!(validate_external_url("https://[::1]/admin").is_err());
        assert!(validate_external_url("https://user:pass@example.com").is_err());
        assert!(validate_external_url("not a url").is_err());
        assert!(validate_external_url("http://metadata.google.internal/").is_err());
    }
}
