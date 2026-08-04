# Known Issues

**Product:** Coreside v0.1.0  
**Last updated:** 2026-08-03  
**Evidence:** regenerate with `npm run release:evidence` — do not treat July counts as current.

Tracked gaps affecting release assurance. Not an exhaustive bug list.

## RC3.2 public-beta blockers (current)

| Issue | Severity | Status |
| --- | --- | --- |
| Tauri `commands.deny` is global (blocks main) | P0 | **Fixed** — removed `coreside-tool-deny-sensitive` from `tool-window` capability; isolation = allowlist `coreside-tool-scoped` only (Tauri 2.11 `resolve_access` ignores window on deny) |
| Wallpaper atomic persistence + slider coalesce | P1 | **Unit landed** — same-target in-flight dedupe; desktop/packaged pixels **not_run** |
| Global `agent-turn` text eavesdrop | P1 | **Mitigated** — Channel for interactive send; eavesdropping E2E **not_run** |
| Journey 12/13 desktop execution | P1 evidence | **In progress** — ACL deny fixed; re-run E2E required |
| Full turn registry / delta-only IPC | P1 | **Partial** — frontend `turnsById` + delta apply landed; reconnect / delta-only wire / concurrent UI still open |
| Typed StructuredUserInput (not text delimiter) | P1 | **Unit Verified** — typed seal + trust gate; delimiter not authority; desktop E2E **not_run** (see [STRUCTURED_FORMS_AND_CONTEXT.md](./STRUCTURED_FORMS_AND_CONTEXT.md)) |
| Progressive preview transaction | P1 | **Partial** — live NDJSON preview + `PreviewTransaction` landed; JSON-blob / surface paint / E2E remain open (see [PROGRESSIVE_PREVIEW_TRANSACTION.md](./PROGRESSIVE_PREVIEW_TRANSACTION.md)) |
| Attachment crash suite + scoped ACL | P1 | **Partial** — crash/reconcile suite + authorize **Unit Verified** (`reports/attachment-crash-results.json`); window ACL / desktop proof **open** |
| Coherent profile restore | P1 | **Partial** — journal stage ladder + mid-swap → Recovery **Unit Verified** (`reports/restore-transaction-results.json`); packaged FS swap proof **open** (see [DURABLE_PROFILE_ARCHITECTURE.md](./DURABLE_PROFILE_ARCHITECTURE.md)) |
| Global queue metadata emit | P1 | **Closed** — conversation-scoped `subscribe_conversation_queue` Channel; no process-wide `agent-queue-changed` (see [QUEUE_COORDINATION.md](./QUEUE_COORDINATION.md); Unit Verified, desktop **not_run**) |
| Sync / Conflict scope match | P1 | **Partial** — Sync carries `conversationId` + `surfaceIds`/`toolIds`; frontend `shouldApplyAgentTurnSync` / `shouldShowAppConflict` skip cross-conversation reload/banner; desktop multi-window **not_run** |
| Multimodal Image parts | P1 | **Partial** — `AgentContentPart::Image` + OpenAI data-URL mapping **Unit Verified**; send path wires authorized bytes; Anthropic/Gemini native parts + desktop **not_run** (see [ATTACHMENT_LIFECYCLE.md](./ATTACHMENT_LIFECYCLE.md)) |
| Replay player vs true event replay | P2 | **Partial** — paced read-only `ReplayPlayer` steps synthetic “Committed transaction” events from `list_transactions`; **not** true provider/form event-stream replay (see [BRANCH_SNAPSHOT_REPLAY.md](./BRANCH_SNAPSHOT_REPLAY.md)) |

## RC3.1 public-beta blockers (carry-forward)

| Issue | Severity | Status |
| --- | --- | --- |
| Desktop E2E Journey 12 (true streaming) | P1 evidence | **Spec written; not_run** |
| Full desktop E2E suite + packaged smoke | P1 | **Open** |
| Anthropic / Gemini live SSE | P1 product | **Unit Verified** — live `chat_stream` SSE adapters; desktop against real keys **not_run** |

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
