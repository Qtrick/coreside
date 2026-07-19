# Manual Acceptance A–AB (Tauri)

**Product:** Coreside  
**Purpose:** Real-build checklist before voice / Wasm / enterprise connectors.  
**Status:** Automated foundation checks passed; **full interactive A–AB on a Tauri build is still required by a human.**

## Automated verification already run

- `cargo test --lib application_kernel` — package ZIP + kernel tests
- `npm run typecheck`
- `npm run test` (includes `visual-verification`)

## Interactive A–AB (run on `npm run tauri dev` or packaged app)

| ID | Scenario | Pass criteria |
| --- | --- | --- |
| A | Cold start | App boots; no panic; chat loads |
| B | Provider connect | BYOK test succeeds; key not in SQLite |
| C | Simple chat | Reply streams; cancel works |
| D | Tool create | Lightweight tool persists after restart |
| E | Strong proposal | Delete/migrate-style ops show `ChangeProposalCard` |
| F | Proposal Apply | Apply succeeds; status becomes applied; no re-prompt on reload |
| G | Proposal Discard | Discard clears sticky + stream card |
| H | Inline surface a11y | Unlabeled control surfaces layout warning when applicable |
| I | Queue | Second send while busy returns queued notice |
| J | Conflict | Stale revision shows ConflictBanner; Reload works |
| K | Secondary window | Open tool window; sync/conflict banner can appear |
| L | ZIP package | Export `.coreside-app` ZIP; import preview + approve |
| M | Legacy JSON package | Import still accepts JSON v1 |
| N | EventBus | subscription.create + event.dispatch persist; restart keeps enabled subs |
| O | NDJSON harvest | Raw multi-line ops parsed when schema ops empty (dev fixture) |
| P | Recovery Mode | Agent cannot disable; settings toggle works |
| Q | Protected resources | Agent cannot rename logos / base settings |
| R | Project isolation | Chat in A never loads B context |
| S | Search budget | Over-budget blocked with clear message |
| T | Media import | Invalid MIME rejected |
| U | Wallpaper readability | Contrast overlays when needed |
| V | Automations | Pause / cancel / no catch-up storm |
| W | Command palette | Cmd+K navigates |
| X | Light + dark | No unreadable tokens |
| Y | Narrow layout | Composer + chat usable ~360px |
| Z | Restart persistence | Tools, proposals status, settings survive |
| AA | Action Log off | No optional events persisted |
| AB | Export security | Secrets redacted from exports |

Mark each row Pass / Fail / Skipped when running a real Tauri session.
