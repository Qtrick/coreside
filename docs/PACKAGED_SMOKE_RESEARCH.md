# Packaged Smoke Research

**Product:** Coreside  
**Access date:** 2026-08-02  
**Public beta:** **NOT READY**  
**Related:** [PACKAGED_BUILD_ASSURANCE.md](./PACKAGED_BUILD_ASSURANCE.md)

## Verdict

A green `build:web`, Vitest, or doctor run is **not** packaged smoke. Public beta requires recorded evidence from a real Tauri package (or equivalent release binary) on the primary OS (macOS).

## What “packaged smoke” means here

Minimum recorded checklist (manual or automated against the bundle):

1. **Build** — `npm run build` produces `Coreside.app` / DMG under bundle output
2. **Scan** — `npm run package:scan` → `reports/packaging-results.json` with no P0 leaks
3. **Cold start** — packaged app opens without panic; DB path writable
4. **BYOK status** — settings shows connection state without exposing key material
5. **Chat round-trip** — send + receive (or honest offline failure) on packaged binary
6. **Persistence** — restart retains conversation / tool state
7. **E2E note** — `e2e:desktop` uses debug + `e2e` features; it **supplements** but does not replace production-package smoke

## Current gaps

| Gap | Evidence |
| --- | --- |
| Full suite often marks packaged build as `manual_verification_required` | `scripts/release-evidence.mjs` consumes `packaging-results.json` only when status is passed / passed_with_warnings |
| Bootstrap panic on DB open failure | Would hard-crash packaged smoke — [BETA_BLOCKER_CLOSURE_RESEARCH.md](./BETA_BLOCKER_CLOSURE_RESEARCH.md) P0 |
| Production must exclude E2E plugins | Doctor + packaging CI checks exist; re-verify on gate build |

## Production constraints (do not regress)

- Cargo feature `e2e` **not** in default features
- Production capabilities: `default` + `tool-window` only (no wdio)
- No `.env` / fixture DBs inside scanned bundle text

## Non-claims

- Do **not** claim packaged smoke passed for commit `57f80bc…` as of this research date.
- Do **not** equate `assurance:report` quick mode with packaged smoke.

## Exit criteria for this blocker

1. Fresh `npm run build` + `package:scan` on macOS recorded in `reports/packaging-results.json`
2. Manual or scripted smoke checklist attached / linked from release evidence
3. Bootstrap P0 fixed so DB open failure cannot panic the packaged binary
4. Release evidence gate `packaged-tauri-build` status `passed` or `passed_with_warnings` with honest notes
