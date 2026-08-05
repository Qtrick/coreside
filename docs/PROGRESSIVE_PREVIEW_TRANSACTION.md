# Progressive Preview Transaction (RC3.3 Phase 5–6 / RC3.4 Phase 7)

**Product:** Coreside  
**Status:** Vertical slice landed — **not complete**  
**Access date:** 2026-08-04

## What landed

| Piece | Reality |
| --- | --- |
| `PreviewTransaction` | `runtime_v2/preview_transaction.rs` — preview_transaction_id, turn_id, accepted/rejected ops, base_revision, interrupted/committed, speculative `PreviewSurfaceModel` map |
| Live ingest | `ingest_live_chunk` / `ingest_live_chunk_with_seed` feeds `NdjsonFrameParser::push` during `TextDelta` |
| Speculative paint | Accepted paint ops apply via `apply_component_op` / state merge on an **in-memory** definition/state clone — **no SQLite write** |
| Channel preview | `AgentTurnEvent::Operation { status: "preview" }` plus `AgentTurnEvent::PreviewSurface { definition_json, state_json, revision, sequence, … }` (Channel-only; Sync global-fallback untouched) |
| Parser failures | Incomplete buffer noise stays silent; oversized/halted/fatal → rejected list + optional `Error`; invalid paint target → rejected |
| Interrupt / cancel | Clears speculative surfaces **and accepted ops**; emits `Operation { status: "interrupted" }`; turn-end harvest skips interrupted bags |
| Durable commit | Turn-end path still one `schedule_and_apply`; on Sync success `mark_committed()` clears preview model |
| Frontend overlay | `previewSurfacesByKey` + `src/lib/preview/surface-overlay.ts`; ToolCanvas paints overlay with **Preview** badge when conversation matches; Sync / Conflict (even background) / error / interrupt / turn finalize clears per conversation or tool; **tool/surface clears require non-empty `conversationId`** (unscoped Sync cannot wipe unrelated chats) |
| Mock fixture | Keyword `progressive op preview` streams NDJSON `operation.frame_completed` before `ResponseCompleted` (`chat.status` — label only, no surface paint) |
| Attribution | MIT note on Partial Update `UpdateStreamParser` / `runModel` progressive dispatch |

## Honest gaps (do not claim done)

- **OpenAI / JSON-blob path:** Provider text is usually one JSON assistant object. Progressive ops only appear when the model emits **newline-delimited StreamEvent** frames. Single-blob JSON still harvests **after** completion via `push_legacy_compat`.
- **No live durable apply:** Preview does not mutate SQLite mid-stream; commit remains turn-end only.
- **Component-state preservation engine:** Speculative paint preserves tool identity and applies component/state ops; full preservation-policy parity across live paint is **P2**.
- **Anthropic / Gemini:** Live text SSE unit-verified; progressive NDJSON ops still depend on model emitting frames (same as OpenAI JSON-blob path).
- **Tool-round reset:** One parser/preview bag spans the turn (including tool follow-ups); not yet a multi-attempt registry.
- **Desktop E2E:** Fixture + unit/vitest covered; packaged Journey for progressive paint **not_run**. **Do not claim Desktop Verified.**

## How to verify

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib runtime_v2::preview_transaction
cargo test --manifest-path src-tauri/Cargo.toml --lib progressive_op_preview
npm run typecheck
npx vitest run src/lib/preview/surface-overlay.test.ts src/lib/tauri/scoped-turn-streaming.test.ts
```

## Related

- [TRUE_STREAMING_RUNTIME.md](./TRUE_STREAMING_RUNTIME.md)
- [PARTIAL_UPDATE_PORT_PROVENANCE.md](./PARTIAL_UPDATE_PORT_PROVENANCE.md)
- [KNOWN_ISSUES.md](./KNOWN_ISSUES.md)
