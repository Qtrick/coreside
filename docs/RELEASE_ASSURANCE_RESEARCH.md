# Release Assurance Research

**Product:** Coreside  
**Access date:** 2026-08-01  
**Public beta:** **NOT READY**  
**Purpose:** Evidence required before claiming public beta — not aspirational tooling.

## Verdict

Public beta is **not ready**. Unit/lib coverage and offline doctor checks are useful but **do not** substitute for packaged desktop evidence, Settings IA completion, or honest security verification records.

## Evidence required for public beta

| Gate | Required evidence | Current (2026-08-01) |
| --- | --- | --- |
| **Build** | Reproducible web + Tauri package on primary OS (macOS) | Scripts exist; full packaged smoke **not claimed** |
| **Static** | `typecheck` + `lint` + `check:rust` green | Historically pass; re-run at gate |
| **Unit** | Vitest + targeted Rust `test:*` green | Present; treat known Rust failures as blockers until fixed/waived |
| **Migrations** | `test:migrations` on fresh + upgrade fixtures | Present (`docs/MIGRATION_ASSURANCE.md`) |
| **Security** | ASVS L2–**style** checklist with findings JSON; no false certification | Docs exist; open P0s must be fixed or waived (`SECURITY_VERIFICATION_STANDARD.md`) |
| **Agent choke** | Registered-action Allow / RequireApproval / Block + injection boundary tests | Runtime present; regression suite must be recorded green |
| **Settings IA** | Consumer categories live; BYOK + Recovery discoverable | **Priority gap** — flat panel today |
| **Manual matrix** | Recorded Pass on A–AB (or equivalent) on packaged build | **gap** |
| **E2E desktop** | Cold start + chat + tool persist at minimum | Scripts (`e2e*`) exist; full matrix **not recorded** |
| **Privacy** | Keys absent from SQLite/exports; Action Log off by default | Code paths exist; export journey AB **gap** |
| **Performance** | Cold-start + chat-stream baseline on packaged build | Not a public-beta claim yet |
| **Wallpaper** | Visual proof / residual occlusion | **Out of scope this phase** — do not block Settings IA; track separately |

## What is insufficient alone

- Vitest in jsdom (no keychain, no native windows, no packaged CSP)
- In-memory / temp SQLite tests (not user app-data upgrade at scale)
- Mock Tauri runtime without WebDriver / real shell
- “Controls exist in code” without recorded verification

## Archives used for continuity

| Archive | SHA-256 |
| --- | --- |
| Coreside Chat AI.zip | `5cdaa9f461d96322ca35de138e69ed04f4fc5ea57e9a75040de0abab08b09322` |
| Vendo main.zip | `516d00b41ca5051087b2e9838ef84bde6b42bad48d16df924fd35152f55e4a55` |

Extracts: `.reference/coreside-uploaded-2026-08-01`, `.reference/vendo-uploaded-2026-08-01`.

## Related

- `docs/RELEASE_ASSURANCE_PLAN.md` · `docs/RELEASE_READINESS.md` · `docs/E2E_EXECUTION.md`
- `docs/MANUAL_ACCEPTANCE_A_AB.md` · `docs/SECURITY_VERIFICATION_STANDARD.md`
- `reports/release-gates.json` · `reports/local-beta-readiness.json`
