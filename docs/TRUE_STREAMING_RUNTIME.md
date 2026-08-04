# True Streaming Runtime (RC3)

**Product:** Coreside  
**Status:** Phase 2 vertical slice landed — **not complete**  
**Public beta:** NOT READY  
**Access date:** 2026-08-03

## Honest current state

| Claim | Reality |
| --- | --- |
| Production send path | **`chat_with_auto` → `AiProvider::chat_stream`** (no longer `chat`-only) |
| Live TextDelta → UI | OpenAI / compatible SSE + mock `live stream probe`; `send_message_inner` forwards peeks via `peek_assistant_message` → `AgentTurnEvent::Text` |
| Auto fallback | **No answer splicing** — after first non-empty `TextDelta`, failure surfaces; no Model B splice |
| OpenAI / compatible SSE adapter | **Implemented** (`stream: true`; `stream_options` only for `openai`) |
| Anthropic / Gemini live SSE | **Not implemented** — trait default buffered `chat_stream` |
| Default / buffered `chat_stream` | Honest (`live: false`, **no** fabricated `TextDelta`, `buffered: true`) |
| `emit_buffered_text_fluidly` | Post-hoc UI typing for **buffered** completions only; skipped when `streamed_live` |
| Hosted gateway | Still `"stream": false` (buffered disclosure via default `chat_stream`) |
| Channel / progressive ops streaming | **Partial** — interactive `send_message` uses Channel for text/action/error/operation; Sync/Conflict still global (see [SCOPED_TURN_STREAMING.md](./SCOPED_TURN_STREAMING.md)) |
| Frontend live UI scope | **Partial** — `sendingConversationId` + active-chat guards; global `sending` lock remains (full turn registry = Phase 3) |
| E2E execution of live probe | Fixture + unit test exist; full desktop E2E not claimed |

## Provider-neutral events

Defined in `src-tauri/src/ai/provider.rs`:

- `ResponseStarted { live }`
- `TextDelta` / `TextCompleted`
- `ToolCallStarted` / `ToolCallArgumentsDelta` / `ToolCallCompleted`
- `UsageUpdated`
- `ResponseCompleted { buffered }`
- `ResponseCancelled` / `ResponseFailed`

## Acceptance progress

1. ~~At least one live adapter emitting real `TextDelta` before completion~~ — OpenAI SSE + mock delayed fixture (`true_streaming_emits_delta_before_completion`, keyword `live stream probe`).
2. ~~`send_message_inner` consumes `chat_stream`~~ — via `chat_with_auto`.
3. S1 delayed-completion fixture — **unit covered**; desktop E2E not claimed.
4. ~~Relabel fake typing~~ — `emit_buffered_text_fluidly` (not marketed as provider streaming).
5. Hosted: buffered path only until a live gateway contract exists.
6. UTF-8 / frame split — partial (OpenAI line SSE + truncate helpers); more coverage still needed.
7. Backpressure + cancel + oversized frame — partial (`MAX_STREAM_*` on OpenAI; cancel in select loops).

## Remaining gaps (do not claim done)

- Anthropic and Gemini live SSE adapters
- Channel / progressive application-operation streaming (interactive text Channel landed; ops progressive apply + Sync-only Channel remain)
- Full turn registry / reconnect snapshot / delta-only UI efficiency
- Full E2E run of the live-stream probe in the desktop shell
- Broader compatible-provider matrix for `stream: true` + `response_format`

## Non-goals

- Fake deltas from buffered text
- Copying Partial Update delimiter HTML protocol
- Claiming Partial Update streaming parity while Anthropic/Gemini/hosted remain buffered
