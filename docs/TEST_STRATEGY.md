# Test Strategy

**Product:** Coreside v0.1.0  
**Last updated:** 2026-07-19

## Layer overview

```
┌─────────────────────────────────────────────────────────┐
│  Manual Tauri (A–AB, journeys K–P)     [NOT AUTOMATED]  │
├─────────────────────────────────────────────────────────┤
│  E2E WebDriver (future)                [NOT STARTED]    │
├─────────────────────────────────────────────────────────┤
│  Vitest component/unit (jsdom)         [66 tests, green] │
├─────────────────────────────────────────────────────────┤
│  Rust lib / integration (cargo test)   [204 pass, 1 fail]│
└─────────────────────────────────────────────────────────┘
```

## Layer 1 — Rust unit / integration

**Command:** `npm run test:rust` or filtered `npm run test:<area>`

**Scope:** Trusted logic — parsing, validation, DB repositories (in-memory), security, migrations logic, kernel, runtime_v2.

**Strengths:**
- AI response parsing, operations schema, protected resources
- Project FTS isolation, export redaction, media validation
- AI access mode / disclosure policy (`ai::access_mode`)

**Limits:**
- Does not launch Tauri webview or keyring UI
- One known failure: `config::env::tests::public_status_hides_key` (2026-07-19)
- Migration upgrade paths lack fixture DB regression suite

**Targeted scripts (examples):**

| Script | Rust module focus |
| --- | --- |
| `test:runtime-v2` | `runtime_v2::*` |
| `test:application-kernel` | `application_kernel::*` |
| `test:recovery` | `application_kernel::recovery` |
| `test:provider-conformance` | Provider profiles |
| `test:ai-access` | `ai::access_mode` (+ Vitest mirror) |

## Layer 2 — Vitest unit / component

**Command:** `npm test`  
**Config:** `vite.config.ts` → `test.environment: jsdom`, `tests/setup.ts`

**Scope:** TypeScript utilities, React-free logic, light component tests.

| File | Focus |
| --- | --- |
| `ai-access-disclosure.test.ts` | Consumer disclosure mirror |
| `preservation.test.ts` | Preservation engine TS side |
| `navigation/chat.test.ts` | Chat navigation no-op rules |
| `readability/contrast.test.ts` | WCAG contrast helpers |
| `wallpaper.test.ts` | Wallpaper selection logic |
| `action-log.test.ts` | Action log mode behavior |
| `mentions/mentions.test.ts` | `@` mention parsing |
| `visual-verification.test.ts` | Layout/visual checks |
| `tests/tool-schema.test.ts` | Tool schema validation |
| `tests/actions.test.ts` | Action definitions |
| `tests/response-fixtures.test.ts` | Response fixture parsing |

**Limits:**
- No `@tauri-apps/api` IPC unless mocked
- No real SQLite from frontend
- No native window or asset protocol

## Layer 3 — Static analysis

| Command | Role |
| --- | --- |
| `npm run typecheck` | TS compile gate |
| `npm run lint` | ESLint (5 warnings as of 2026-07-19) |
| `npm run check:rust` | `cargo check` compile gate |

## Layer 4 — Manual desktop acceptance

**Reference:** `docs/MANUAL_ACCEPTANCE_A_AB.md`, `docs/MANUAL_RELEASE_CHECKLIST.md`

Required for: keychain BYOK, streaming cancel, secondary windows, packaged CSP, wallpaper readability on real display.

## Layer 5 — E2E (future)

**Planned stack:** Tauri WebDriver (`tauri-driver`) per https://v2.tauri.app/develop/tests/webdriver/

**Candidate first scenarios:** A (cold start), B (provider), C (chat), K (secondary window).

**Status:** Not configured. Do not claim E2E green.

## Layer 6 — Crawl4AI smoke (optional integration)

Scripts: `crawl4ai:doctor`, `crawl4ai:smoke`, `crawl4ai:test` — require local Python sidecar setup (`crawl4ai:setup`). Not part of default `npm test`.

## Release gate mapping

See `reports/release-gates.json`.

## What “verify” means today

`npm run verify` runs typecheck → lint → vitest → check:rust → test:rust → build:web.

A green `verify` still does **not** imply desktop or migration-fixture readiness.
