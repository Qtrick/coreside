# Security Assurance

**Product:** Coreside v0.1.0  
**Primary reference:** `docs/SECURITY.md`  
**Last updated:** 2026-07-19

## Assurance focus areas

### 1. Credential handling

| Control | Status | Evidence |
| --- | --- | --- |
| BYOK in OS keyring, not SQLite | **Implemented** | `credentials/`, migration 003 |
| Frontend receives `keyDetected` only | **Implemented** | `ai_cmds.rs`, settings UI |
| `.env` dev fallback | **Implemented** | `config/env.rs` — not for production consumers |
| AI HTTP in Rust only | **Implemented** | Provider adapters in `src-tauri/src/ai/` |
| Log / error redaction | **Implemented** | Sanitization in error paths |
| Public status hides key material | **Partial** | Rust test `public_status_hides_key` **failing** (2026-07-19) |

### 2. AI access disclosure

| Control | Status | Evidence |
| --- | --- | --- |
| Authoritative mode in Rust | **Implemented** | `ai/access_mode.rs` |
| `developer_environment` hides provider/model | **Implemented** | Disclosure policy + tests |
| Hosted mode hides upstream | **Designed** | Tests exist; adapter **not built** |
| Frontend cannot forge mode | **Implemented** | Presentation from Tauri command |

### 3. Export leakage

| Control | Status | Evidence |
| --- | --- | --- |
| Export strips credentials | **Implemented** | `exports/mod.rs` + unit tests |
| Manual AB checklist | **Pending** | Human export inspection |
| Action Log excludes secrets | **Implemented** | `docs/SECURITY.md`, action log tests |
| Chat messages exclude keys | **Policy** | Not stored by design; manual verify recommended |

### 4. Generative UI boundary

| Control | Status | Evidence |
| --- | --- | --- |
| Registry-only components | **Implemented** | Component validation Rust + Zod |
| No arbitrary JS/CSS from model | **Implemented** | Declarative renderer |
| Protected resource IDs | **Implemented** | `security/protected_resources.rs` |
| Agent cannot disable recovery | **Implemented** | `recovery.rs` apply-path gate |

### 5. Network / SSRF

| Control | Status | Evidence |
| --- | --- | --- |
| URL validation before fetch/crawl | **Implemented** | `search/safety.rs`, `crawler/` |
| Exa keys not passed to Crawl4AI | **Implemented** | Architecture separation |
| Private-network block | **Implemented** | Rust tests in `search/safety.rs` |

### 6. Project isolation

| Control | Status | Evidence |
| --- | --- | --- |
| FTS scoped by project | **Implemented** | `projects/retrieval.rs` tests |
| Manual scenario R | **Pending** | Manual acceptance |

## Automated security-related tests

```bash
npm run test:permissions
npm run test:policy
npm run test:recovery
cargo test --manifest-path src-tauri/Cargo.toml --lib exports::
cargo test --manifest-path src-tauri/Cargo.toml --lib search::safety
npm run test:ai-access
```

## Open risks

1. **No penetration test** or third-party audit recorded.
2. **No E2E** proving webview cannot exfiltrate keys via generative surfaces.
3. **One Rust env test failure** — treat as release blocker for public beta until resolved or waived with documented rationale.
4. **Hosted adapter absent** — no cloud credential path to validate yet.

## Manual verification (required)

- **B:** Connect BYOK; confirm SQLite has no key blob.
- **AB:** Export tool/package; grep output for `sk-`, `AI_API_KEY`, bearer tokens.
- **Q:** Agent cannot rename `core.branding*` / base settings.
