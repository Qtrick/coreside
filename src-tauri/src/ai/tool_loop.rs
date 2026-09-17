//! Bounded agent tool execution loop with cancellation and validation.

use std::time::Duration;

use serde_json::json;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use crate::db::Database;
use crate::media::{get_media_asset, media_readable_in_scope, ImportMediaInput};
use crate::projects::{get_project, load_project_context_settings, search_project_context};
use crate::search::{
    build_http_client, fetch_web_page, image_search, video_search, web_search, SearchRegistry,
};
use crate::state::AppState;

use super::capability_registry::{
    validate_tool_call, validate_tool_output, AgentCapability, ToolCallRequest, ToolCallResult,
    TOOL_LOOP_MAX_STEPS,
};

const STEP_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct ToolLoopContext {
    pub project_id: Option<String>,
    pub conversation_id: Option<String>,
}

pub struct ToolLoop {
    registry: Option<SearchRegistry>,
    client: reqwest::Client,
}

impl ToolLoop {
    pub fn new(registry: Option<SearchRegistry>) -> Result<Self, String> {
        let client = build_http_client().map_err(|e| e.to_string())?;
        Ok(Self { registry, client })
    }

    pub fn mock() -> Self {
        Self {
            registry: Some(SearchRegistry::mock()),
            client: build_http_client().expect("http client"),
        }
    }

    pub async fn run(
        &self,
        state: &AppState,
        ctx: &ToolLoopContext,
        calls: Vec<ToolCallRequest>,
        cancel: &CancellationToken,
    ) -> Vec<ToolCallResult> {
        let mut results = Vec::new();
        for (i, call) in calls.into_iter().enumerate() {
            if i >= TOOL_LOOP_MAX_STEPS {
                results.push(ToolCallResult {
                    capability: call.capability,
                    ok: false,
                    output: json!(null),
                    error: Some(format!("exceeded max steps ({TOOL_LOOP_MAX_STEPS})")),
                    pending_approval: None,
                });
                break;
            }
            if cancel.is_cancelled() {
                results.push(ToolCallResult {
                    capability: call.capability,
                    ok: false,
                    output: json!(null),
                    error: Some("cancelled".into()),
                    pending_approval: None,
                });
                break;
            }

            let result = timeout(STEP_TIMEOUT, self.execute_one(state, ctx, &call, cancel)).await;

            match result {
                Ok(Ok(r)) => results.push(r),
                Ok(Err(e)) => results.push(ToolCallResult {
                    capability: call.capability,
                    ok: false,
                    output: json!(null),
                    error: Some(e),
                    pending_approval: None,
                }),
                Err(_) => results.push(ToolCallResult {
                    capability: call.capability,
                    ok: false,
                    output: json!(null),
                    error: Some("step timed out".into()),
                    pending_approval: None,
                }),
            }
        }
        results
    }

    async fn execute_one(
        &self,
        state: &AppState,
        ctx: &ToolLoopContext,
        call: &ToolCallRequest,
        cancel: &CancellationToken,
    ) -> Result<ToolCallResult, String> {
        if cancel.is_cancelled() {
            return Err("cancelled".into());
        }

        let cap = validate_tool_call(call)?;
        let output = match cap {
            AgentCapability::ProjectContextSearch => {
                let pid = ctx
                    .project_id
                    .as_deref()
                    .ok_or_else(|| "no active project".to_string())?;
                let query = call.arguments["query"].as_str().unwrap_or("");
                let limit = call.arguments["limit"].as_u64().unwrap_or(8) as usize;
                let hits = {
                    let db = state.db.lock();
                    search_project_context(&db, pid, query, limit)
                }
                .map_err(|e| e.to_string())?;
                json!({ "hits": hits })
            }
            AgentCapability::GetProjectSummary => {
                let pid = ctx
                    .project_id
                    .as_deref()
                    .ok_or_else(|| "no active project".to_string())?;
                let project = {
                    let db = state.db.lock();
                    get_project(&db, pid)
                }
                .map_err(|e| e.to_string())?;
                json!({
                    "name": project.name,
                    "instructions": project.instructions,
                    "summary": project.summary,
                })
            }
            AgentCapability::WebSearch => {
                let registry = self.registry.as_ref().ok_or_else(|| {
                    "local research engine not ready: install the Crawl4AI sidecar (Settings → Web Research)".to_string()
                })?;
                let safe = {
                    let db = state.db.lock();
                    load_project_context_settings(&db).map_err(|e| e.to_string())?
                };
                let safe = crate::search::SafeSearchLevel::from_setting(&safe.safe_search);
                let query = call.arguments["query"].as_str().unwrap_or("");
                let domain = call.arguments.get("domain").and_then(|v| v.as_str());
                let count = call.arguments["count"].as_u64().unwrap_or(8) as usize;
                let resp = if domain.is_some() {
                    registry
                        .web_search_with_domain(query, domain, count, safe)
                        .await
                        .map_err(|e| e.to_string())?
                } else {
                    web_search(registry, query, count, safe)
                        .await
                        .map_err(|e| e.to_string())?
                };
                json!(resp)
            }
            AgentCapability::ImageSearch => {
                let registry = self.registry.as_ref().ok_or_else(|| {
                    "local research engine not ready: install the Crawl4AI sidecar (Settings → Web Research)".to_string()
                })?;
                let safe = {
                    let db = state.db.lock();
                    load_project_context_settings(&db).map_err(|e| e.to_string())?
                };
                let safe = crate::search::SafeSearchLevel::from_setting(&safe.safe_search);
                let query = call.arguments["query"].as_str().unwrap_or("");
                let count = call.arguments["count"].as_u64().unwrap_or(12) as usize;
                let resp = image_search(registry, query, count, safe)
                    .await
                    .map_err(|e| e.to_string())?;
                json!(resp)
            }
            AgentCapability::VideoSearch => {
                let registry = self.registry.as_ref().ok_or_else(|| {
                    "local research engine not ready: install the Crawl4AI sidecar (Settings → Web Research)".to_string()
                })?;
                let safe = {
                    let db = state.db.lock();
                    load_project_context_settings(&db).map_err(|e| e.to_string())?
                };
                let safe = crate::search::SafeSearchLevel::from_setting(&safe.safe_search);
                let query = call.arguments["query"].as_str().unwrap_or("");
                let count = call.arguments["count"].as_u64().unwrap_or(8) as usize;
                let resp = video_search(registry, query, count, safe)
                    .await
                    .map_err(|e| e.to_string())?;
                json!(resp)
            }
            AgentCapability::FetchWebPage => {
                let url = call.arguments["url"].as_str().unwrap_or("");
                let page = crate::research::fetch_web_page_orchestrated(url, Some(&state.crawler))
                    .await
                    .map_err(|e| e.to_string())?;
                json!(page)
            }
            AgentCapability::InspectMediaResult => {
                if let Some(id) = call.arguments.get("assetId").and_then(|v| v.as_str()) {
                    let asset = {
                        let db = state.db.lock();
                        get_media_asset(&db, id)
                    }
                    .map_err(|e| e.to_string())?;
                    if !media_readable_in_scope(&asset, ctx.project_id.as_deref()) {
                        return Err(
                            "media asset is not readable in the active project scope".into()
                        );
                    }
                    // Omit local_filename — agents only need library metadata.
                    json!({
                        "id": asset.id,
                        "projectId": asset.project_id,
                        "category": asset.category,
                        "title": asset.title,
                        "mimeType": asset.mime_type,
                        "byteSize": asset.byte_size,
                        "width": asset.width,
                        "height": asset.height,
                        "durationMs": asset.duration_ms,
                        "sourceUrl": asset.source_url,
                        "sourcePageUrl": asset.source_page_url,
                        "creator": asset.creator,
                        "license": asset.license,
                        "attribution": asset.attribution,
                        "validationStatus": asset.validation_status,
                        "createdAt": asset.created_at,
                        "lastUsedAt": asset.last_used_at,
                    })
                } else {
                    json!({
                        "resultId": call.arguments.get("resultId"),
                        "note": "search result inspection is session-scoped; use assetId for library assets"
                    })
                }
            }
            AgentCapability::ImportMediaAsset => {
                let url = call.arguments["url"].as_str().unwrap_or("");
                json!({
                    "pendingApproval": true,
                    "proposedImport": ImportMediaInput {
                        url: url.to_string(),
                        title: call.arguments.get("title").and_then(|v| v.as_str()).map(str::to_string),
                        project_id: ctx.project_id.clone(),
                        source_page_url: None,
                        creator: None,
                        license: None,
                        attribution: None,
                        category: call.arguments.get("category").and_then(|v| v.as_str()).map(str::to_string),
                    }
                })
            }
            AgentCapability::NoAction => json!({ "status": "noop" }),
        };

        validate_tool_output(cap, &output)?;
        let pending = output.get("pendingApproval").and_then(|v| v.as_bool());

        Ok(ToolCallResult {
            capability: cap.as_str().to_string(),
            ok: true,
            output,
            error: None,
            pending_approval: pending,
        })
    }
}

/// Minimal pre-prompt project context when conversation has a project.
pub fn project_context_for_prompt(
    db: &Database,
    project_id: &str,
    user_query: &str,
) -> Result<Option<super::prompt_builder::ProjectPromptContext>, String> {
    let settings = load_project_context_settings(db).map_err(|e| e.to_string())?;
    if !settings.include_project_context {
        return Ok(None);
    }

    let project = get_project(db, project_id).map_err(|e| e.to_string())?;
    let hits = search_project_context(db, project_id, user_query, 6).unwrap_or_default();

    let snippets: Vec<String> = hits
        .iter()
        .map(|h| format!("[{}] {}: {}", h.conversation_title, h.role, h.snippet))
        .collect();

    Ok(Some(super::prompt_builder::ProjectPromptContext {
        project_name: project.name,
        instructions: project.instructions,
        summary: project.summary,
        retrieval_snippets: snippets,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::capability_registry::TOOL_LOOP_MAX_STEPS;
    use serde_json::json;

    #[tokio::test]
    async fn step_limit_enforced() {
        let loop_ = ToolLoop::mock();
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open_path(&dir.path().join("tl.db")).unwrap();
        let state = crate::state::AppState::new_for_test(db);
        let ctx = ToolLoopContext {
            project_id: None,
            conversation_id: None,
        };
        let calls: Vec<ToolCallRequest> = (0..TOOL_LOOP_MAX_STEPS + 2)
            .map(|_| ToolCallRequest {
                capability: "no_action".into(),
                arguments: json!({}),
            })
            .collect();
        let cancel = CancellationToken::new();
        let results = loop_.run(&state, &ctx, calls, &cancel).await;
        assert_eq!(results.len(), TOOL_LOOP_MAX_STEPS + 1);
        assert!(!results.last().unwrap().ok);
    }

    #[tokio::test]
    async fn web_search_without_registry_returns_not_configured() {
        let loop_ = ToolLoop::new(None).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open_path(&dir.path().join("tl-nc.db")).unwrap();
        let state = crate::state::AppState::new_for_test(db);
        let ctx = ToolLoopContext {
            project_id: None,
            conversation_id: None,
        };
        let calls = vec![ToolCallRequest {
            capability: "web_search".into(),
            arguments: json!({ "query": "test" }),
        }];
        let cancel = CancellationToken::new();
        let results = loop_.run(&state, &ctx, calls, &cancel).await;
        assert_eq!(results.len(), 1);
        assert!(!results[0].ok);
        assert!(results[0]
            .error
            .as_deref()
            .unwrap_or("")
            .contains("research engine"));
    }

    #[tokio::test]
    async fn inspect_media_rejects_cross_project_asset() {
        use crate::media::{media_readable_in_scope, MediaAsset};
        use crate::projects::{create_project, CreateProjectInput};

        let loop_ = ToolLoop::mock();
        let dir = tempfile::tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("tl-media.db")).unwrap();
        let p1 = create_project(
            &mut db,
            &CreateProjectInput {
                name: "Alpha".into(),
                description: None,
                icon_key: None,
                instructions: None,
                pinned: None,
            },
        )
        .unwrap();
        let p2 = create_project(
            &mut db,
            &CreateProjectInput {
                name: "Beta".into(),
                description: None,
                icon_key: None,
                instructions: None,
                pinned: None,
            },
        )
        .unwrap();

        // Insert row directly — avoid writing under the real media data dir in unit tests.
        db.conn()
            .execute(
                "INSERT INTO media_assets
                 (id, project_id, category, title, local_filename, mime_type, byte_size,
                  content_hash, validation_status)
                 VALUES ('asset-b', ?1, 'image', 'secret', 'asset-b.png', 'image/png', 10,
                         'hash-cross-project-test', 'ok')",
                rusqlite::params![p2.id],
            )
            .unwrap();

        let asset = MediaAsset {
            id: "asset-b".into(),
            project_id: Some(p2.id.clone()),
            category: "image".into(),
            title: "secret".into(),
            local_filename: "asset-b.png".into(),
            mime_type: "image/png".into(),
            byte_size: 10,
            width: None,
            height: None,
            duration_ms: None,
            content_hash: "hash-cross-project-test".into(),
            source_url: None,
            source_page_url: None,
            creator: None,
            license: None,
            attribution: None,
            validation_status: "ok".into(),
            created_at: "now".into(),
            last_used_at: None,
            thumbnail_filename: None,
        };
        assert!(!media_readable_in_scope(&asset, Some(&p1.id)));

        let state = crate::state::AppState::new_for_test(db);
        let ctx = ToolLoopContext {
            project_id: Some(p1.id),
            conversation_id: None,
        };
        let calls = vec![ToolCallRequest {
            capability: "inspect_media_result".into(),
            arguments: json!({ "assetId": "asset-b" }),
        }];
        let cancel = CancellationToken::new();
        let results = loop_.run(&state, &ctx, calls, &cancel).await;
        assert_eq!(results.len(), 1);
        assert!(!results[0].ok);
        assert!(results[0]
            .error
            .as_deref()
            .unwrap_or("")
            .contains("not readable"));
    }
}
