# Release Assurance Plan

**Product:** Coreside v0.1.0  
**Assessment target:** Internal alpha (not public beta)  
**Last updated:** 2026-07-19

## Principles

1. **No fabricated gates** — pass/fail requires evidence from this repo or a recorded manual run.
2. **Rust is authoritative** for credentials, AI access mode, migrations, and protected resources.
3. **Vitest is authoritative** for React logic that does not require Tauri IPC.
4. **Tauri interactive sessions** are required before claiming consumer-ready desktop assurance.

## Phase 0 — Foundation checks (automated)

| Item | Status | Evidence |
| --- | --- | --- |
| TypeScript typecheck | **Done** | `npm run typecheck` exit 0 (2026-07-19) |
| ESLint | **Done** (warnings) | 0 errors, 5 warnings (2026-07-19) |
| Vitest suite | **Done** | 66/66 passed (2026-07-19) |
| Rust lib tests | **Partial** | 204 passed, 1 failed: `config::env::tests::public_status_hides_key` |
| `npm run verify` full chain | **Pending** | Not run end-to-end for this release doc |
| CI workflow | **Pending** | No `.github/` workflows in repo |

## Phase 1 — AI access & disclosure

| Item | Status | Notes |
| --- | --- | --- |
| `AiAccessMode` enum + `resolve_access_presentation` | **Done** | `src-tauri/src/ai/access_mode.rs` |
| BYOK (`user_byok`) disclosure | **Done** | Shows provider + model |
| `.env` / deployment creds (`developer_environment`) | **Done** | Hides OpenRouter/model unless Developer Mode |
| Hosted adapter (`coreside_hosted`) | **Pending** | `hosted_connected` hardcoded `false` in `ai_cmds.rs` |
| Local inference (`user_local`) | **Done** | Ollama / LM Studio paths in access_mode |
| Frontend disclosure tests | **Done** | `src/lib/ai-access-disclosure.test.ts` |
| Rust disclosure tests | **Done** | 5 tests in `access_mode.rs` |
| `npm run test:ai-access` script | **Done** | Added with this assurance pack |
| Manual Tests A–J (disclosure + chat UX) | **Pending** | See `docs/MANUAL_RELEASE_CHECKLIST.md` |

## Phase 2 — Core product journeys (manual)

| Item | Status | Reference |
| --- | --- | --- |
| Scenarios A–J (chat, provider, proposals, queue) | **Pending** | `docs/MANUAL_ACCEPTANCE_A_AB.md` |
| Journeys K–P (multiwindow, packages, recovery) | **Pending** | `docs/USER_JOURNEY_MATRIX.md` |
| Scenarios Q–AB (security, search, media, export) | **Pending** | `docs/MANUAL_ACCEPTANCE_A_AB.md` |

## Phase 3 — Data & migrations

| Item | Status | Notes |
| --- | --- | --- |
| Migrations 001–014 present | **Done** | `src-tauri/migrations/` |
| Fresh install migration | **Done** (implicit) | App boots on empty DB in dev |
| Upgrade fixtures 001→014 | **Pending** | No committed fixture DBs or upgrade test harness |
| Migration rollback | **Not planned** | SQLite forward-only |

## Phase 4 — Security & export assurance

| Item | Status | Reference |
| --- | --- | --- |
| Credential storage policy documented | **Done** | `docs/SECURITY.md` |
| Export redaction tests (Rust) | **Done** | `src-tauri/src/exports/mod.rs` tests |
| Manual export leak check (AB) | **Pending** | Human checklist |
| SSRF / crawler tests | **Partial** | Rust unit tests; no live crawl E2E |

## Phase 5 — Accessibility & performance

| Item | Status | Notes |
| --- | --- | --- |
| Contrast unit tests | **Done** | `src/lib/readability/contrast.test.ts` |
| Keyboard / focus audit | **Pending** | See `docs/ACCESSIBILITY_ASSURANCE.md` |
| Performance baselines | **Pending** | See `docs/PERFORMANCE_ASSURANCE.md` |

## Phase 6 — Platform & packaging

| Item | Status | Notes |
| --- | --- | --- |
| macOS dev (`tauri dev`) | **Primary** | Maintainer environment |
| macOS packaged build | **Pending verification** | `npm run build` not run for this doc |
| Windows / Linux builds | **Pending** | Icons exist; no release evidence in repo |
| WebDriver E2E | **Pending** | Not configured |

## Phase 7 — Release decision

| Gate | Status |
| --- | --- |
| Internal alpha (team dogfood) | **Eligible** with known gaps documented |
| Public beta | **Not eligible** — manual A–AB incomplete, Rust test failure, no E2E, hosted adapter absent |

## Deliverables (this pack)

| File | Purpose |
| --- | --- |
| `docs/RELEASE_ASSURANCE_RESEARCH.md` | Tooling research |
| `docs/RELEASE_ASSURANCE_PLAN.md` | This plan |
| `docs/FEATURE_INVENTORY.md` | Feature area status |
| `docs/USER_JOURNEY_MATRIX.md` | K–P journey checklist |
| `docs/TEST_STRATEGY.md` | Layered test approach |
| `docs/MANUAL_RELEASE_CHECKLIST.md` | Tests A–J + journeys |
| `docs/SECURITY_ASSURANCE.md` | Security focus areas |
| `docs/ACCESSIBILITY_ASSURANCE.md` | a11y status |
| `docs/PERFORMANCE_ASSURANCE.md` | Baseline honesty |
| `docs/MIGRATION_ASSURANCE.md` | Migration inventory |
| `docs/PLATFORM_ASSURANCE.md` | OS targets |
| `docs/KNOWN_ISSUES.md` | Tracked gaps |
| `docs/RELEASE_READINESS.md` | Go / no-go |
| `reports/feature-inventory.json` | Machine-readable IDs |
| `reports/release-findings.json` | Findings tracker |
| `reports/release-gates.json` | Gate statuses |
