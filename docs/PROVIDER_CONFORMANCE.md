# Provider Conformance

**Product:** Coreside  
**Code:** `runtime_v2/provider_conformance.rs`  
**Streaming adapters:** `src-tauri/src/ai/{openai,anthropic,gemini}.rs`

Profiles: `native_streaming_operations`, `buffered_structured_response`, `tool_call_operations`, `json_schema_response`, `text_protocol_fallback`, `unsupported_for_application_changes`.

Seeded capability records exist for Gemini, OpenAI, Anthropic, OpenRouter, and mock. Benchmark fields are empty until a local run records them — never fabricate measurements.

## Live `chat_stream` (RC3.3 Phase 11)

| Provider | Live SSE | Evidence |
| --- | --- | --- |
| OpenAI / compatible | Yes | Unit + prior true-streaming slice |
| Anthropic | Yes (`stream: true` on `/v1/messages`) | **Unit Verified** (delta extraction helpers) |
| Gemini | Yes (`streamGenerateContent?alt=sse`) | **Unit Verified** (delta extraction helpers) |
| Hosted gateway | No | Buffered default `chat_stream` |

When streaming structured ops is unsupported: buffer → validate complete response → apply or propose. Fake progressive op streams are forbidden. Do not claim `live: true` on the buffered trait default.

See `reports/provider-conformance-results.json`.
