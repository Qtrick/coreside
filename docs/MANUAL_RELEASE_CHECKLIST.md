# Manual Release Checklist

**Product:** Coreside v0.1.0  
**Run on:** Tauri dev (`npm run dev`) or packaged app  
**Default status:** Pending until a human records Pass/Fail/Skipped.

## AI disclosure & access — Tests A–J

Derived from `docs/MANUAL_ACCEPTANCE_A_AB.md` rows A–J. Focus: consumer must not see upstream provider/model when using `.env` credentials without Developer Mode.

| ID | Test | Pass criteria | Status |
| --- | --- | --- | --- |
| **A** | Cold start | App boots; no panic; chat loads | Pending |
| **B** | Provider connect | BYOK test succeeds; key not in SQLite | Pending |
| **C** | Simple chat | Reply streams; cancel works | Pending |
| **D** | Tool create | Lightweight tool persists after restart | Pending |
| **E** | Strong proposal | Delete/migrate-style ops show `ChangeProposalCard` | Pending |
| **F** | Proposal Apply | Apply succeeds; status applied; no re-prompt on reload | Pending |
| **G** | Proposal Discard | Discard clears sticky + stream card | Pending |
| **H** | Inline surface a11y | Unlabeled control surfaces layout warning when applicable | Pending |
| **I** | Queue | Second send while busy returns queued notice | Pending |
| **J** | Conflict | Stale revision shows `ConflictBanner`; Reload works | Pending |

### AI disclosure spot-checks (within A–C)

Run with `.env` key present (`AI_API_KEY` / provider key) and **Developer Mode off**:

| Check | Expected |
| --- | --- |
| Model picker label | Shows **Coreside AI**, not OpenRouter/Gemini model id |
| Provider settings | No upstream provider name in consumer-facing status |
| Developer Mode on | Provider/model visible for debugging |

Automated mirror: `npm run test:ai-access` (Vitest + Rust `ai::access_mode`).

**Known fixed finding:** Env-credential disclosure regression — fixed via `access_mode.rs` + `ai-access-disclosure.test.ts` (see `reports/release-findings.json`).

## Core journeys K–P

| ID | Journey | Status |
| --- | --- | --- |
| **K** | Secondary window sync/conflict | Pending |
| **L** | ZIP package export/import | Pending |
| **M** | Legacy JSON package import | Pending |
| **N** | EventBus persistence across restart | Pending |
| **O** | NDJSON harvest (dev fixture) | Pending |
| **P** | Recovery Mode (agent cannot disable) | Pending |

Detail: `docs/USER_JOURNEY_MATRIX.md`.

## Extended scenarios (Q–AB) — recommended before public beta

| ID | Area | Status |
| --- | --- | --- |
| Q | Protected resources | Pending |
| R | Project isolation | Pending |
| S | Search budget | Pending |
| T | Media import rejection | Pending |
| U | Wallpaper readability | Pending |
| V | Automations | Pending |
| W | Command palette (Cmd+K) | Pending |
| X | Light + dark themes | Pending |
| Y | Narrow layout (~360px) | Pending |
| Z | Restart persistence | Pending |
| AA | Action Log off | Pending |
| AB | Export security (no secrets) | Pending |

Full criteria: `docs/MANUAL_ACCEPTANCE_A_AB.md`.

## Pre-flight (automated — run before manual session)

```bash
npm run typecheck
npm run lint
npm test
npm run test:ai-access
npm run test:rust   # expect 1 failure until env test fixed
```

Record date, commit SHA, OS, and build type (dev vs packaged) when filing results.
