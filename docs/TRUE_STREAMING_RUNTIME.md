# True Streaming Runtime (RC3)

**Product:** Coreside  
**Status:** Foundation started — **not complete**  
**Public beta:** NOT READY  
**Access date:** 2026-08-03

## Honest current state

| Claim | Reality |
| --- | --- |
| Live provider streaming in production send path | **Not wired** — `chat_with_auto` still uses `chat()` |
| OpenAI / compatible SSE adapter | **Implemented** (`OpenAiProvider::chat_stream`, `stream: true`) |
| Default `chat_stream` | Honest buffered fallback (`live: false`, no fake `TextDelta`) |
| `emit_text_fluidly` | Still used after complete parse — **not** provider streaming |
| Hosted gateway | Still `"stream": false` |
| `NdjsonFrameParser` | Still post-hoc on completed `raw_text` |

## Provider-neutral events

Defined in `src-tauri/src/ai/provider.rs`:

- `ResponseStarted { live }`
- `TextDelta` / `TextCompleted`
- `ToolCallStarted` / `ToolCallArgumentsDelta` / `ToolCallCompleted`
- `UsageUpdated`
- `ResponseCompleted { buffered }`
- `ResponseCancelled` / `ResponseFailed`

## Required before claiming true streaming

1. At least one live adapter (OpenAI-compatible SSE or Anthropic SSE) emitting real `TextDelta` before completion.
2. `send_message_inner` consumes `chat_stream`, not only `chat`.
3. Acceptance S1: first delta reaches UI before provider completion (fixture with delayed finish).
4. Remove or relabel `emit_text_fluidly` so product state never calls simulated typing “streaming.”
5. Hosted: either live stream contract or explicit buffered disclosure.
6. UTF-8 chunk split + frame split tests (partially: truncate helper fixed).
7. Backpressure + cancel + oversized frame gates.

## Non-goals

- Fake deltas from buffered text
- Copying Partial Update delimiter HTML protocol
- Claiming Partial Update streaming parity while `chat` remains the production path
