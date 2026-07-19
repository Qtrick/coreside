# Generative UI Security Model

**Product:** Coreside  
**Access date:** 2026-07-18

## Trust boundary

Rust validates and persists. React renders trusted components only. Generated surfaces never receive:

- API keys / keychain access  
- Arbitrary Tauri commands  
- Filesystem / shell  
- Arbitrary network / CDN  
- Model-authored JavaScript  

## Threat → control (selected)

| Threat | Prevention | Detection | Recovery |
|---|---|---|---|
| Arbitrary script | No HTML/JS ops; component registry | Reject unknown types | Keep last good revision |
| CDN supply chain | Bundled packs only | Pack permission check | Disable pack |
| Event/provider loops | Rate limits, idempotency, suspend | EventBus strikes | Unsuspend control |
| Stale overwrite | baseRevision required | Conflict error | Retry/rebase |
| Protected resource edit | `assert_not_protected` | Validation error | No change |
| Cross-project event | Project id equality | CrossProjectDenied | Drop event |
| Diagnostic leak | `redact_secrets` | Review export | Retention trim |
| Oversized trees | Central limits | Limit errors | Reject op |

## Sandbox research (deferred)

Evaluated iframe / Worker / QuickJS / Wasmtime. Missing guarantees for CPU/memory/network isolation in-process with the protected webview. **Decision:** ship capability packs; defer Wasm modules to a later milestone with proof tests.

## Safe embeds

Sandboxed iframe embeds not shipped until Tauri isolation proves no same-origin/credential leakage. Prefer Open in Browser / Crawl4AI extraction.
