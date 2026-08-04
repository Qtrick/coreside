# Progressive Preview Transaction (RC3.3 Phase 5–6)

**Product:** Coreside  
**Status:** Vertical slice landed — **not complete**  
**Access date:** 2026-08-03

## What landed

| Piece | Reality |
| --- | --- |
| `PreviewTransaction` | `runtime_v2/preview_transaction.rs` — turn_id, accepted ops, rejected, base_revision, interrupted |
| Live ingest | `ingest_live_chunk` feeds `NdjsonFrameParser::push` (canonical StreamEvent) during `TextDelta` |
| Channel preview | `AgentTurnEvent::Operation { status: "preview" }` emitted **without** `schedule_and_apply` |
| Durable commit | Turn-end path still prefers payload ops, else preview bag, else legacy post-hoc harvest → **one** `schedule_and_apply` |
| Mock fixture | Keyword `progressive op preview` streams NDJSON `operation.frame_completed` before `ResponseCompleted` |
| Frontend | `agentActions` shows `Preview: …` for status `preview` |
| Attribution | MIT note on Partial Update `UpdateStreamParser` / `runModel` progressive dispatch |

## Honest gaps (do not claim done)

- **OpenAI / JSON-blob path:** Provider text is usually one JSON assistant object. Progressive ops only appear when the model emits **newline-delimited StreamEvent** frames. Single-blob JSON still harvests **after** completion via `push_legacy_compat`.
- **No live durable apply:** Preview does not mutate surfaces mid-stream; commit remains turn-end only.
- **No preview UI for surfaces:** Channel label only — no speculative surface paint / rollback on interrupt.
- **Anthropic / Gemini:** Still buffered `chat_stream`; no live progressive ops there.
- **Tool-round reset:** One parser/preview bag spans the turn (including tool follow-ups); not yet a multi-attempt registry.
- **Desktop E2E:** Fixture + unit/vitest covered; packaged Journey for progressive ops **not_run**.

## How to verify

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib runtime_v2::preview_transaction
cargo test --manifest-path src-tauri/Cargo.toml --lib progressive_op_preview
npm run typecheck
npx vitest run src/lib/tauri/scoped-turn-streaming.test.ts
```

## Related

- [TRUE_STREAMING_RUNTIME.md](./TRUE_STREAMING_RUNTIME.md)
- [PARTIAL_UPDATE_PORT_PROVENANCE.md](./PARTIAL_UPDATE_PORT_PROVENANCE.md)
- [KNOWN_ISSUES.md](./KNOWN_ISSUES.md)
