//! Google Gemini provider (generateContent + JSON schema).

use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::errors::AiError;
use super::provider::{AgentRequest, AgentResponse, AiProvider, ProviderHealth, UsageMetadata};
use super::response_schema::SCHEMA_VERSION;
use crate::security::redact_secrets;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

pub struct GeminiProvider {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
}

impl GeminiProvider {
    pub fn new(api_key: String, model: String, base_url: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            api_key,
            model,
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
        }
    }

    fn generate_url(&self) -> String {
        format!("{}/models/{}:generateContent", self.base_url, self.model)
    }

    fn list_models_url(&self) -> String {
        format!("{}/models", self.base_url)
    }

    fn response_schema() -> Value {
        // Keep this conservative: Gemini rejects many OpenAPI features
        // (`nullable`, complex `anyOf`) which previously caused opaque 400s.
        json!({
            "type": "object",
            "properties": {
                "schemaVersion": { "type": "string" },
                "assistantMessage": { "type": "string" },
                "responseType": {
                    "type": "string",
                    "enum": ["message", "tool_change", "tool_use", "settings_change", "noop"]
                },
                "toolCalls": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "capability": { "type": "string" },
                            "arguments": { "type": "object" }
                        },
                        "required": ["capability"]
                    }
                },
                "citations": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "title": { "type": "string" },
                            "url": { "type": "string" },
                            "displayDomain": { "type": "string" },
                            "snippet": { "type": "string" }
                        },
                        "required": ["id", "title", "url"]
                    }
                },
                "toolChange": {
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["create", "update", "replace"]
                        },
                        "targetToolId": { "type": "string" },
                        "changeSummary": { "type": "string" },
                        "tool": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "name": { "type": "string" },
                                "description": { "type": "string" },
                                "layout": {
                                    "type": "object",
                                    "properties": {
                                        "type": { "type": "string" }
                                    }
                                },
                                "components": {
                                    "type": "array",
                                    "items": { "type": "object" }
                                }
                            },
                            "required": ["id", "name", "components"]
                        }
                    },
                    "required": ["action", "changeSummary"]
                },
                "settingsChange": {
                    "type": "object",
                    "properties": {
                        "theme": {
                            "type": "string",
                            "enum": ["system", "light", "dark"]
                        },
                        "accentPrimary": {
                            "type": "object",
                            "properties": {
                                "light": { "type": "string" },
                                "dark": { "type": "string" }
                            }
                        },
                        "accentSecondary": {
                            "type": "object",
                            "properties": {
                                "light": { "type": "string" },
                                "dark": { "type": "string" }
                            }
                        },
                        "background": {
                            "type": "object",
                            "properties": {
                                "light": { "type": "string" },
                                "dark": { "type": "string" }
                            }
                        },
                        "surface": {
                            "type": "object",
                            "properties": {
                                "light": { "type": "string" },
                                "dark": { "type": "string" }
                            }
                        },
                        "surfaceMuted": {
                            "type": "object",
                            "properties": {
                                "light": { "type": "string" },
                                "dark": { "type": "string" }
                            }
                        },
                        "border": {
                            "type": "object",
                            "properties": {
                                "light": { "type": "string" },
                                "dark": { "type": "string" }
                            }
                        },
                        "textPrimary": {
                            "type": "object",
                            "properties": {
                                "light": { "type": "string" },
                                "dark": { "type": "string" }
                            }
                        },
                        "textSecondary": {
                            "type": "object",
                            "properties": {
                                "light": { "type": "string" },
                                "dark": { "type": "string" }
                            }
                        },
                        "wallpaper": {
                            "type": "object",
                            "properties": {
                                "kind": {
                                    "type": "string",
                                    "enum": ["none", "matrix", "aurora", "particles", "rain", "pulse"]
                                },
                                "color": { "type": "string" },
                                "secondaryColor": { "type": "string" },
                                "speed": { "type": "number" },
                                "density": { "type": "number" },
                                "opacity": { "type": "number" }
                            },
                            "required": ["kind"]
                        },
                        "changeSummary": { "type": "string" }
                    }
                },
                "diagnostics": { "type": "object" }
            },
            "required": ["schemaVersion", "assistantMessage", "responseType"]
        })
    }

    fn build_contents(messages: &[super::provider::AgentMessage]) -> Vec<Value> {
        messages
            .iter()
            .map(|m| {
                let role = match m.role.as_str() {
                    "assistant" | "model" => "model",
                    // tool_result maps to user for API shape; content is untrusted-enveloped.
                    "tool_result" => "user",
                    _ => "user",
                };
                json!({
                    "role": role,
                    "parts": [{ "text": m.content }]
                })
            })
            .collect()
    }

    async fn send_generate(
        &self,
        body: Value,
        cancel: CancellationToken,
    ) -> Result<Value, AiError> {
        let url = self.generate_url();
        let request = self
            .client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body);

        let response = tokio::select! {
            _ = cancel.cancelled() => return Err(AiError::Cancelled),
            result = request.send() => {
                result.map_err(|e| {
                    if e.is_timeout() {
                        AiError::Timeout
                    } else if e.is_connect() || e.is_request() {
                        AiError::Http(redact_secrets(&e.to_string(), Some(&self.api_key)))
                    } else {
                        AiError::Http(redact_secrets(&e.to_string(), Some(&self.api_key)))
                    }
                })?
            }
        };

        let status = response.status();
        let text = tokio::select! {
            _ = cancel.cancelled() => return Err(AiError::Cancelled),
            result = response.text() => {
                result.map_err(|e| AiError::Http(redact_secrets(&e.to_string(), Some(&self.api_key))))?
            }
        };

        if cancel.is_cancelled() {
            return Err(AiError::Cancelled);
        }

        if !status.is_success() {
            return Err(AiError::Provider(format!(
                "Gemini HTTP {}: {}",
                status.as_u16(),
                redact_secrets(&text, Some(&self.api_key))
            )));
        }

        serde_json::from_str(&text).map_err(|e| {
            AiError::Parse(format!(
                "Invalid Gemini JSON: {} — {}",
                e,
                redact_secrets(
                    &text.chars().take(200).collect::<String>(),
                    Some(&self.api_key)
                )
            ))
        })
    }

    fn extract_text(payload: &Value) -> Result<String, AiError> {
        let candidates = payload
            .get("candidates")
            .and_then(|c| c.as_array())
            .ok_or_else(|| AiError::Parse("Missing candidates".into()))?;

        let first = candidates
            .first()
            .ok_or_else(|| AiError::Parse("Empty candidates".into()))?;

        let parts = first
            .pointer("/content/parts")
            .and_then(|p| p.as_array())
            .ok_or_else(|| AiError::Parse("Missing content.parts".into()))?;

        let mut text = String::new();
        for part in parts {
            if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                text.push_str(t);
            }
        }

        if text.is_empty() {
            return Err(AiError::Parse("Empty model text".into()));
        }
        Ok(text)
    }

    fn extract_usage(payload: &Value) -> UsageMetadata {
        let meta = payload.get("usageMetadata");
        UsageMetadata {
            prompt_tokens: meta
                .and_then(|m| m.get("promptTokenCount"))
                .and_then(|v| v.as_u64())
                .map(|v| v as u32),
            completion_tokens: meta
                .and_then(|m| m.get("candidatesTokenCount"))
                .and_then(|v| v.as_u64())
                .map(|v| v as u32),
            total_tokens: meta
                .and_then(|m| m.get("totalTokenCount"))
                .and_then(|v| v.as_u64())
                .map(|v| v as u32),
        }
    }
}

#[async_trait]
impl AiProvider for GeminiProvider {
    fn provider_id(&self) -> &str {
        "gemini"
    }

    fn display_name(&self) -> &str {
        "Google Gemini"
    }

    async fn health_check(&self, cancel: CancellationToken) -> Result<ProviderHealth, AiError> {
        // Prefer listing models; fall back to a minimal generate.
        let list_url = self.list_models_url();
        let list_req = self
            .client
            .get(&list_url)
            .header("x-goog-api-key", &self.api_key)
            .timeout(Duration::from_secs(20));

        let list_result = tokio::select! {
            _ = cancel.cancelled() => return Err(AiError::Cancelled),
            result = list_req.send() => result,
        };

        if let Ok(resp) = list_result {
            if resp.status().is_success() {
                let body: Value = resp.json().await.unwrap_or(json!({}));
                let models = body
                    .get("models")
                    .and_then(|m| m.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|m| {
                                m.get("name")
                                    .and_then(|n| n.as_str())
                                    .map(|s| s.trim_start_matches("models/").to_string())
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                return Ok(ProviderHealth {
                    ok: true,
                    message: format!("Listed {} models", models.len()),
                    models,
                });
            }
        }

        // Minimal generate fallback
        let body = json!({
            "contents": [{
                "role": "user",
                "parts": [{ "text": "ping" }]
            }],
            "generationConfig": {
                "maxOutputTokens": 8
            }
        });

        let _ = self.send_generate(body, cancel).await?;
        Ok(ProviderHealth {
            ok: true,
            message: "generateContent reachable".into(),
            models: vec![self.model.clone()],
        })
    }

    async fn chat(&self, request: AgentRequest) -> Result<AgentResponse, AiError> {
        if request.cancel.is_cancelled() {
            return Err(AiError::Cancelled);
        }

        let contents = Self::build_contents(&request.messages);
        if contents.is_empty() {
            return Err(AiError::Validation("No messages to send".into()));
        }

        let mut body = json!({
            "systemInstruction": {
                "parts": [{ "text": request.system_prompt }]
            },
            "contents": contents,
            "generationConfig": {
                "temperature": 0.4,
                "responseMimeType": "application/json",
                "responseSchema": Self::response_schema()
            }
        });

        let _ = SCHEMA_VERSION;

        let payload = match self
            .send_generate(body.clone(), request.cancel.clone())
            .await
        {
            Ok(payload) => payload,
            Err(err) => {
                let msg = err.to_string().to_lowercase();
                let looks_like_schema = msg.contains("schema")
                    || msg.contains("invalid argument")
                    || msg.contains("400");
                if !looks_like_schema {
                    return Err(err);
                }
                tracing::warn!(
                    error = %redact_secrets(&err.to_string(), Some(&self.api_key)),
                    "Gemini structured schema rejected; retrying with JSON mime only"
                );
                if let Some(gen) = body.get_mut("generationConfig") {
                    if let Some(obj) = gen.as_object_mut() {
                        obj.remove("responseSchema");
                    }
                }
                self.send_generate(body, request.cancel.clone()).await?
            }
        };
        let raw_text = Self::extract_text(&payload)?;
        let usage = Self::extract_usage(&payload);

        Ok(AgentResponse {
            raw_text,
            usage,
            model: self.model.clone(),
            provider_id: self.provider_id().to_string(),
        })
    }

    async fn probe(&self, cancel: CancellationToken) -> Result<(), AiError> {
        let body = json!({
            "contents": [{
                "role": "user",
                "parts": [{ "text": "ping" }]
            }],
            "generationConfig": {
                "maxOutputTokens": 8
            }
        });
        let _ = self.send_generate(body, cancel).await?;
        Ok(())
    }
}
