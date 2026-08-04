# AI Provider Platform (RC3.5)

**Product:** Coreside  
**Status:** Integrated – Not Verified (platform foundation landed; live provider conformance incomplete)

## Purpose

Coreside routes chat and generated-app work through a capability-driven provider platform so new models and vetted presets do not require a bespoke application release for every model ID.

## Concepts (trusted Rust)

| Concept | Module | Role |
| --- | --- | --- |
| `ProviderDescriptor` | `ai/platform/descriptors.rs` | Built-in preset: endpoint, auth, protocol, discovery, capability profile |
| `ProtocolFamily` | `ai/platform/types.rs` | Adapter wire protocol (Chat Completions, Anthropic Messages, Gemini, Ollama native, …) |
| `AuthMode` | `ai/platform/types.rs` | Includes `local_authless` — no fake API key |
| `EndpointClass` | `ai/platform/types.rs` | Fixed remote, vetted preset, custom HTTPS, loopback Local AI, private LAN |
| `CapabilityProfile` | `ai/platform/types.rs` | Flags + confidence; restrictive merge |
| Endpoint policy | `ai/platform/endpoint_policy.rs` | HTTPS for remote, loopback HTTP for Local AI, no userinfo, block metadata |

## Connection storage

Migration `019_provider_platform` adds non-secret metadata columns on `provider_connections`. API keys remain in the OS keychain only.

## First-class presets (researched)

| ID | Endpoint | Auth | Notes |
| --- | --- | --- | --- |
| `gemini` / `openai` / `anthropic` / `openrouter` | Existing defaults | Key required | Migrated onto descriptors |
| `kimi` | `https://api.moonshot.ai/v1` | Bearer | Official Kimi Open Platform Chat Completions |
| `mistral` | `https://api.mistral.ai/v1` | Bearer | Official Mistral Chat Completions |
| `ollama` | `http://127.0.0.1:11434` | Authless | Native protocol family declared; temporary OpenAI-compat `/v1` send path |
| `lmstudio` / `vllm` / `llama_cpp` | Loopback defaults | Authless | Experimental where noted |
| `compatible` | User HTTPS URL | Key | Custom remote; generated-app eligibility unknown until probe |

## Explicit non-goals in this slice

- Amazon Bedrock / Azure / Vertex (need deployment credentials — planned, not fake-compat)
- Native Ollama NDJSON adapter (descriptor exists; send path still uses `/v1` compat)
- Dynamic model catalog TTL/cache as sole authority (hardcoded catalog still used for picker hints)
- Claiming Generated-app eligibility from provider name alone

## Verification

```bash
cargo test --lib ai::platform
npm run audit:current-source
```
