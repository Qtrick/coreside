# Known Issues

**Product:** Coreside v0.1.0  
**Last updated:** 2026-08-03  
**Evidence:** regenerate with `npm run release:evidence` — do not treat July counts as current.

Tracked gaps affecting release assurance. Not an exhaustive bug list.

## RC3.2 public-beta blockers (current)

| Issue | Severity | Status |
| --- | --- | --- |
| Wallpaper atomic persistence + slider coalesce | P1 | **Unit landed** — desktop/packaged pixels **not_run** |
| Global `agent-turn` text eavesdrop | P1 | **Mitigated** — Channel for interactive send; eavesdropping E2E **not_run** |
| Journey 12/13 desktop execution | P1 evidence | **Spec only / not_run** |
| Full turn registry / delta-only IPC | P1 | **Open** |
| Typed StructuredUserInput (not text delimiter) | P1 | **Open** |
| Progressive preview transaction | P1 | **Open** |
| Attachment crash suite + scoped ACL | P1 | **Open** |
| Coherent profile restore | P1 | **Open** |

## RC3.1 public-beta blockers (carry-forward)

| Issue | Severity | Status |
| --- | --- | --- |
| Desktop E2E Journey 12 (true streaming) | P1 evidence | **Spec written; not_run** |
| Full desktop E2E suite + packaged smoke | P1 | **Open** |
| Anthropic / Gemini live SSE | P1 product | **Open** (buffered honest fallback) |

## Assurance infrastructure

| Issue | Severity | Status |
| --- | --- | --- |
| Desktop E2E harness | High (for beta) | **Partial** — journeys 1–11 present; 12 added not executed |
| Packaged Tauri build | High (for beta) | **Partial** — clean-profile packaged smoke still needs recorded evidence |
| Misleading evidence script names | Medium | **Fixed** — renamed overclaiming `parity`/`replay`/`inspector` scripts |

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
