# Partial Update Adoption Decisions

Access date: 2026-07-18

## API ready; Settings Developer Mode UI next

- **CS-INSPECTOR** — Developer inspector UI (`partially_implemented`) — phase 18

## Adapt via quiz/button components

- **PU-TTT** — Game-styled forms (`safe_adaptation_required`) — phase 12

## After Runtime V2 stable

- **PU-TRANSLATE** — Per-client translation (`deferred_consumer_scope`) — phase 40

## BYOK + event/search budgets

- **PU-RATELIMIT** — Browser/IP rate limits (`implemented_differently`) — phase 19

## Bundled capability packs only

- **PU-CDN** — CDN Tailwind/D3/CodeMirror (`rejected_security`) — phase 14

## Bundled textarea editor; no auto-exec

- **PU-CODEMIRROR** — CodeMirror playground (`safe_adaptation_required`) — phase 14

## Cross-surface undo_transaction

- **PU-UNDO** — Hard undo multi-turn (`implemented_differently`) — phase 17

## Declarative canvas limits; no WebGL JS

- **PU-CANVAS** — Canvas/3D whimsical (`safe_adaptation_required`) — phase 14

## Defer

- **PU-MULTIPLAYER** — Multiplayer Connect 4 (`deferred_collaboration_scope`) — phase 39
- **PU-PRIVATE** — Private model history include ids (`deferred_collaboration_scope`) — phase 39
- **PU-CLIENT-SECRET** — Client secret tokens (`deferred_collaboration_scope`) — phase 39
- **PU-CLIENT-COLOR** — User-specific colors (`deferred_collaboration_scope`) — phase 39
- **PU-D1** — Cloudflare D1 auth DB (`deferred_enterprise_scope`) — phase 41

## Defer multiuser sync

- **PU-WS** — Hibernatable WebSockets (`deferred_collaboration_scope`) — phase 39

## Developer Mode diagnostics

- **PU-DEBUG** — /debug raw/pretty history (`partially_implemented`) — phase 18

## Existing clear chats

- **PU-CLEAR-HIST** — Clear history debug action (`partially_implemented`) — phase 18

## Extension point; follow-up

- **PU-DICTATION** — Voice dictation self-mod (`deferred_consumer_scope`) — phase 26

## Implement now

- **CS-INLINE** — Inline generative surfaces (`missing_high_value`) — phase 10
- **CS-PATCH** — Fine-grained patches (`missing_high_value`) — phase 9
- **CS-PACKS** — Capability packs (`missing_high_value`) — phase 14
- **CS-BRANCH** — Chat branching (`missing_high_value`) — phase 16
- **CS-QUEUE** — Request queue UI (`missing_high_value`) — phase 19
- **CS-TXN** — Cross-surface transactions (`missing_high_value`) — phase 17
- **CS-EVENT-LOOP** — Event loop protection (`implemented_equivalent`) — phase 12

## In-process events only

- **PU-FORM-ROUTE** — POST c/:chatId/form (`rejected_security`) — phase 12

## Inline surface full-width toggle

- **PU-FULLWIDTH** — message-full-width class (`implemented_differently`) — phase 10

## JSON schema v2 operations instead of delimiters

- **PU-PROTOCOL** — Delimiter protocol PpqUtcLGQdYN4oqc (`implemented_differently`) — phase 7

## Keep + add patches

- **CS-V1-TOOL** — Coreside schema v1 tool_change (`partially_implemented`) — phase 9

## Keep submitToAgent; extend rich forms

- **PU-FORMS** — Structured forms to LLM (`implemented_differently`) — phase 12

## Local chat_branches + snapshots

- **PU-FORK** — Fork chat + read-only fork pages (`implemented_differently`) — phase 16

## Local consumer; defer cloud auth

- **PU-AUTH** — Better Auth OAuth roles (`deferred_enterprise_scope`) — phase 41

## Local per-conversation queue

- **PU-QUEUE** — LLM request queue limit 5 (`implemented_differently`) — phase 19

## Local-first Tauri

- **PU-DO** — Cloudflare Durable Object (`rejected_product_direction`) — phase —

## NDJSON frames

- **PU-PARSE** — UpdateStreamParser incremental parse (`implemented_differently`) — phase 11

## NDJSON validated frames

- **PU-STREAM** — Progressive stream apply (`implemented_differently`) — phase 11

## No arbitrary CSS; tokens only

- **PU-SCOPED-CSS** — Scoped style markers (`safe_adaptation_required`) — phase 15

## Preserve

- **CS-V1-COMPAT** — Schema v1 compatibility (`implemented_equivalent`) — phase 7

## Provider-neutral BYOK

- **PU-GATEWAY** — Cloudflare AI Gateway (`rejected_product_direction`) — phase —

## React lifecycle; no model scripts

- **PU-DISMOUNT** — MutationObserver cleanup (`safe_adaptation_required`) — phase —

## Reference only; not adopted into webview

- **WICG-DPU** — Declarative Partial Updates platform (`obsolete_or_reference_only`) — phase —

## Reject

- **PU-JS** — Arbitrary generated JS (`rejected_security`) — phase —

## Reject CDN; use design tokens

- **PU-TAILWIND** — Tailwind via CDN (`rejected_security`) — phase —

## Reject for security — use trusted components

- **PU-HTML-RESP** — HTML not Markdown responses (`rejected_security`) — phase —

## Safe layout zones only; no full chrome replace

- **PU-WIKI** — Full page redesign Wikipedia (`rejected_product_direction`) — phase 15

## Stable component/surface IDs

- **PU-MARKERS** — template for + <?marker> (`implemented_differently`) — phase 9

## Table + ops; UI follow-up

- **CS-LAYOUT** — Workspace layout model (`partially_implemented`) — phase 15

## Tauri send_message

- **PU-PROMPT-ROUTE** — Prompt submission route (`implemented_differently`) — phase —

## Transaction list + replay UI follow-up

- **PU-REPLAY** — Replay page with pacing (`partially_implemented`) — phase 17

## Typed EventBus with loop limits

- **PU-SUBSCRIBE** — window.partialupdates.subscribe (`safe_adaptation_required`) — phase 13

## assistantMessages[] in schema v2

- **PU-MULTI-MSG** — Multiple messages per turn (`implemented_equivalent`) — phase 7

## audience extension point only

- **PU-INCLUDE-EXCLUDE** — Client include/exclude routing (`deferred_collaboration_scope`) — phase 39

## coreside.math pack

- **PU-MATH** — MathML (`safe_adaptation_required`) — phase 14

## coreside.svg pack

- **PU-SVG** — SVG in responses (`safe_adaptation_required`) — phase 14

## instance_id per surface

- **PU-INSTANCES** — Instance numbers ttt/1 ttt/2 (`implemented_equivalent`) — phase 8

## schema v2 silent

- **PU-SILENT** — Silent updates (`implemented_equivalent`) — phase 7

## state.* operations

- **PU-JSON-PATH** — JSON client props path updates (`implemented_differently`) — phase 12

