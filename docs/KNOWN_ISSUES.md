# Known Issues

**Product:** Coreside v0.1.0  
**Last updated:** 2026-08-01

Tracked gaps affecting release assurance. Not an exhaustive bug list.

## Assurance infrastructure

| Issue | Severity | Status |
| --- | --- | --- |
| Full E2E / WebDriver not configured | High (for beta) | **Open** — no `tauri-driver`, no green E2E suite |
| Manual A–AB / journeys K–P not recorded | High (for beta) | **Open** — see `docs/MANUAL_RELEASE_CHECKLIST.md` |
| No CI workflows | Medium | **Fixed** — `.github/workflows/ci.yml` + `npm run doctor` |
| Migration fixture testing | Medium | **Open** — see `docs/MIGRATION_ASSURANCE.md` (015 added; fixture corpus still thin) |
| Packaged-build clean-profile smoke | High (for beta) | **Open** — not executed this cycle |
| Performance baselines | Medium | **Open** — not measured |

## Generated application runtime (2026-08-01)

| Issue | Severity | Status |
| --- | --- | --- |
| Legacy `kernel_*_record` IPC beside gateway | P2 | **Open** — permission-gated; full retirement deferred |
| Handler output size checked after write | P2 | **Open** — blocked outcome; no automatic rollback |
| Agent chat path inventing registered-action calls | P2 | **Deferred** — UI/automation invoke only |
| Multi-window approval race manual QA | P1 evidence | **Open** — unit CAS covered; desktop not re-shot |
| InlineSurface registered-action wiring | P2 | **Open** — ToolCanvas wired; inline promote path may lag |

## Automated test status (2026-07-19)

| Suite | Result |
| --- | --- |
| `npm run typecheck` | Pass |
| `npm run lint` | Pass (5 warnings) |
| `npm test` | 66/66 pass |
| `npm run test:ai-access` | Pass |
| `cargo test --lib` | **205/205 pass** |

## AI access

| Issue | Severity | Status |
| --- | --- | --- |
| Hosted adapter (`coreside_hosted`) not built | Expected | **Open** — `hosted_connected: false`; never fabricate “Coreside AI connected” |
| Env credentials brand as “Coreside AI” | P1 | **Fixed** — consumer label is **AI Access / Ready**; “Coreside AI” reserved for real hosted |
| Env credentials leaked OpenRouter / model slug | P1 | **Fixed** — disclosure policy + Settings + catalog gating |
| Message bubble model attribution under env mode | P2 | **Fixed** — gated on `showModelIdentity` |
| Real Tauri screenshot retest of Settings card | P1 evidence | **Open** — unit coverage only; desktop not re-shot this cycle |

## Generated tools

| Issue | Severity | Status |
| --- | --- | --- |
| Stub tools render as “Text” / “Button” | P1 | **Mitigated** — prompt examples + pack validation reject missing/placeholder labels; empty creates rejected. Existing stub tools already saved are unchanged until user asks to rebuild. |
| Model still under-builds complex tools | P2 | **Open** — validation blocks worst stubs; quality still depends on the model |

## Product partial implementations

| Issue | Severity | Status |
| --- | --- | --- |
| Recovery Mode incomplete | Medium | **Open** |
| Search requires Exa + Crawl4AI | Low | By design when keys missing |
| Windows / Linux release verification | High (if claimed) | **Open** — macOS primary |

## Lint

| Issue | Severity | Status |
| --- | --- | --- |
| 5 ESLint react-refresh / hooks warnings | Low | **Open** — non-blocking |

## Not issues (explicit non-goals this phase)

- Hosted AI backend / billing / accounts
- Enterprise SSO / org admin
- Marketplace / arbitrary package install

## Reporting

New findings: add to `reports/release-findings.json` with severity P0/P1/P2.
