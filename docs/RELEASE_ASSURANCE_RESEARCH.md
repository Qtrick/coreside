# Release Assurance Research

**Product:** Coreside v0.1.0  
**Date:** 2026-07-19  
**Purpose:** Ground release assurance in what the repo actually has — not aspirational tooling.

## Tauri 2 testing references

Coreside ships on **Tauri 2** (`@tauri-apps/api` / `@tauri-apps/cli` ^2.5.0). Relevant upstream docs:

| Topic | URL |
| --- | --- |
| Tauri 2 prerequisites | https://v2.tauri.app/start/prerequisites/ |
| Develop / debug | https://v2.tauri.app/develop/ |
| Build / bundle | https://v2.tauri.app/distribute/ |
| WebDriver (desktop E2E) | https://v2.tauri.app/develop/tests/webdriver/ |
| Mock runtime (CI without native shell) | https://v2.tauri.app/develop/tests/mocking/ |

**Current repo state:** No `tauri-driver`, WebDriver config, or CI workflow exists (`.github/` absent). Interactive assurance still depends on `npm run dev` or a packaged build.

## Existing automated stack

### TypeScript / Vitest

- **Runner:** Vitest 3.x via `vite.config.ts` (`environment: jsdom`, `setupFiles: ./tests/setup.ts`)
- **Scripts:** `npm test` (`vitest run`), `npm run test:watch`
- **Test files (11):** `src/lib/*.test.ts`, `tests/*.test.ts`
- **Verified 2026-07-19:** 66 tests passed across 11 files

Targeted scripts already exist for subsystems (`test:preservation`, `test:partial-update-parity`, etc.).

### Rust / Cargo

- **Scripts:** `npm run test:rust`, plus many `test:*` filters (`test:runtime-v2`, `test:application-kernel`, `test:recovery`, …)
- **Verified 2026-07-19:** 204 passed, **1 failed** (`config::env::tests::public_status_hides_key`)

### Static analysis

- `npm run typecheck` — passes (`tsc --noEmit`)
- `npm run lint` — passes with 5 ESLint warnings (react-refresh/only-export-components)
- `npm run check:rust` — available; not re-run for this document
- `npm run verify` — chains typecheck, lint, vitest, check:rust, test:rust, build:web

## AI access disclosure tests

Dedicated script (to be added): `npm run test:ai-access`

- Frontend: `src/lib/ai-access-disclosure.test.ts` (mirrors Rust disclosure policy)
- Rust: `src-tauri/src/ai/access_mode.rs` (`#[cfg(test)]` module)

Policy source of truth: `resolve_access_presentation` in Rust; frontend must not invent mode from scattered booleans.

## WebDriver — future layer

Tauri 2 WebDriver (`tauri-driver` + platform driver) is the intended path for:

- Cold-start boot
- Native window open/close (secondary tool windows)
- Packaged-app smoke (not `vite` dev server alone)
- Cross-window sync / conflict banners

**Not implemented.** `docs/MANUAL_ACCEPTANCE_A_AB.md` scenarios A–AB remain human-driven on a real Tauri session.

## Mock-runtime limitations

Tauri mock runtime is useful for CI command invocation but **cannot** replace:

| Gap | Why mock is insufficient |
| --- | --- |
| OS keychain / keyring | BYOK storage is native; Vitest uses no Tauri IPC |
| SQLite migrations on real app data dir | Rust unit tests use in-memory / temp DBs, not full upgrade paths from user fixtures |
| Crawl4AI sidecar lifecycle | Python subprocess supervision is Rust-only |
| Secondary native windows | Window labels, focus, and IPC differ from jsdom |
| Wallpaper / media asset protocol | `asset://` and filesystem scopes are Tauri-specific |
| Packaged CSP and bundle resources | Dev server CSP ≠ production webview |

Vitest + Rust lib tests give **logic assurance**; they do not prove **desktop integration**.

## Related in-repo docs

- `docs/MANUAL_ACCEPTANCE_A_AB.md` — interactive checklist A–AB
- `docs/GENERATED_APPLICATION_TESTING.md` — kernel / package testing notes
- `docs/SECURITY.md` — credential and export boundaries
- `reports/coreside-feature-evidence.json` — subsystem evidence snapshot (2026-07-18)

## Honest summary

| Layer | Status |
| --- | --- |
| Unit / component (Vitest) | Present; green on 2026-07-19 |
| Rust lib tests | Present; 1 known failure |
| Tauri WebDriver E2E | Not started |
| Mock runtime CI | Not configured |
| Full A–AB manual pass | Pending human session |
| Performance baselines | Not measured |
