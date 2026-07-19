# Provider Conformance

**Product:** Coreside  
**Code:** `runtime_v2/provider_conformance.rs`

Profiles: `native_streaming_operations`, `buffered_structured_response`, `tool_call_operations`, `json_schema_response`, `text_protocol_fallback`, `unsupported_for_application_changes`.

Seeded capability records exist for Gemini, OpenAI, Anthropic, OpenRouter, and mock. Benchmark fields are empty until a local run records them — never fabricate measurements.

When streaming structured ops is unsupported: buffer → validate complete response → apply or propose. Fake progressive op streams are forbidden.
