# Known Issues

**Product:** Coreside v0.1.0  
**Last updated:** 2026-08-01  
**Evidence:** regenerate with `npm run release:evidence` — do not treat July `release-gates.json` counts as current.

Tracked gaps affecting release assurance. Not an exhaustive bug list.

## Assurance infrastructure

| Issue | Severity | Status |
| --- | --- | --- |
| Desktop E2E harness | High (for beta) | **Partial** — WebdriverIO + feature-gated plugins; journeys 1–10 automated; journeys **7** and **10** honestly `passed_partial`. See `docs/E2E_EXECUTION.md` |
| Packaged Tauri build | High (for beta) | **Partial** — local `npm run build` produced macOS `.app` + `.dmg`; packaging CI added; clean-profile packaged smoke still needs recorded evidence |
| Manual A–AB / journeys K–P not recorded | High (for beta) | **Open** — see `docs/MANUAL_RELEASE_CHECKLIST.md` |
| CI workflows | Medium | **Fixed** — verify + cross-compile + e2e-desktop + packaging |
| Migration fixture testing | Medium | **Fixed** — `npm run test:migrations` |
| Performance baselines (desktop) | Medium | **Open** — bundle sizes measured; cold-start timings still thin |
| Stale July readiness reports | Medium | **Fixed** — `release:evidence` supersedes `release-gates.json` |

## Generated application runtime (2026-08-01)

| Issue | Severity | Status |
| --- | --- | --- |
| `surface.delete` was a no-op | P0 | **Fixed** — real delete/archive/restore + draft/continuity cleanup |
| Dictation “coming soon” generatable | P1 | **Fixed** — pack removed; compatibility fallback only |
| Draft cleanup on conversation delete | P1 | **Fixed** |
| Crawler per-request cancel unused | P2 | **Fixed** — `cancel_active` uses shared cancel protocol; agent Stop cancels crawls |
| Production tauri bridge included mocks | P1 | **Fixed** — `src/lib/tauri/` split; mocks async-only outside Tauri |
| Multi-window approval race desktop | P1 evidence | **Partial** — E2E journey 7 opens secondary window; duplicate-approval UI in secondary still partial |
| InlineSurface registered-action wiring | P2 | **Fixed** |
| Event-bus loop suspension permanent | P1 | **Fixed** |
| Event-bus idempotency unbounded | P1 | **Fixed** |
| `record_crash` unused | P1 | **Fixed** — three-strike suspend via build failures |
| Create-side dependency edges unwired | P2 | **Open** — delete cleans edges; create-side still TODO |

## Automated test status

Re-run before quoting. Prefer `reports/release-evidence.json`.

| Suite | Command |
| --- | --- |
| Typecheck / lint / vitest | `npm run typecheck` / `lint` / `test` |
| Rust / migrations | `npm run test:rust` / `test:migrations` |
| Doctor | `npm run doctor` |
| Desktop E2E | `npm run e2e:desktop` |
| Packaged build | `npm run build` + `package:scan` |
