# Partial Update Feature Matrix

Access date: 2026-07-18

Total classified features: 56

Every identified Partial Update capability receives exactly one disposition.

| ID | Name | Status | Action | PU | Coreside | Video |
|---|---|---|---|---|---|---|
| PU-HTML-RESP | HTML not Markdown responses | `rejected_security` | Reject for security — use trusted components | `src/initialPrompt.md` | `src-tauri/src/ai/response_schema.rs` | 00:00:34 |
| PU-FORMS | Structured forms to LLM | `implemented_differently` | Keep submitToAgent; extend rich forms | `src/initialPrompt.md` | `src/types/tool.ts` | 00:00:44 |
| PU-TTT | Game-styled forms | `safe_adaptation_required` | Adapt via quiz/button components | `src/initialPrompt.md` | `src/components/tool-renderer/nodes.tsx` | 00:00:52 |
| PU-PROTOCOL | Delimiter protocol PpqUtcLGQdYN4oqc | `implemented_differently` | JSON schema v2 operations instead of delimiters | `spec.md` | `src-tauri/src/runtime_v2/operations.rs` | 00:00:28 |
| PU-MARKERS | template for + <?marker> | `implemented_differently` | Stable component/surface IDs | `src/initialPrompt.md` | `src-tauri/src/runtime_v2/patch.rs` | — |
| PU-MULTI-MSG | Multiple messages per turn | `implemented_equivalent` | assistantMessages[] in schema v2 | `spec.md` | `src-tauri/src/runtime_v2/operations.rs` | — |
| PU-INCLUDE-EXCLUDE | Client include/exclude routing | `deferred_collaboration_scope` | audience extension point only | `spec.md` | `src-tauri/src/runtime_v2/operations.rs` | — |
| PU-SUBSCRIBE | window.partialupdates.subscribe | `safe_adaptation_required` | Typed EventBus with loop limits | `spec.md` | `src-tauri/src/runtime_v2/events.rs` | — |
| PU-WS | Hibernatable WebSockets | `deferred_collaboration_scope` | Defer multiuser sync | `src/index.ts` | `—` | — |
| PU-QUEUE | LLM request queue limit 5 | `implemented_differently` | Local per-conversation queue | `src/index.ts` | `src-tauri/src/runtime_v2/queue.rs` | — |
| PU-RATELIMIT | Browser/IP rate limits | `implemented_differently` | BYOK + event/search budgets | `src/index.ts` | `src-tauri/src/runtime_v2/limits.rs` | — |
| PU-FORK | Fork chat + read-only fork pages | `implemented_differently` | Local chat_branches + snapshots | `src/index.ts` | `src-tauri/src/runtime_v2/branch.rs` | — |
| PU-REPLAY | Replay page with pacing | `partially_implemented` | Transaction list + replay UI follow-up | `src/index.ts` | `src-tauri/src/runtime_v2/transactions.rs` | — |
| PU-UNDO | Hard undo multi-turn | `implemented_differently` | Cross-surface undo_transaction | `src/index.ts` | `src-tauri/src/runtime_v2/transactions.rs` | — |
| PU-DEBUG | /debug raw/pretty history | `partially_implemented` | Developer Mode diagnostics | `src/debug.ts` | `src-tauri/src/runtime_v2/mod.rs` | — |
| PU-AUTH | Better Auth OAuth roles | `deferred_enterprise_scope` | Local consumer; defer cloud auth | `src/auth/roles.ts` | `—` | — |
| PU-CDN | CDN Tailwind/D3/CodeMirror | `rejected_security` | Bundled capability packs only | `README.md` | `src-tauri/src/runtime_v2/packs.rs` | 00:08:53 |
| PU-JS | Arbitrary generated JS | `rejected_security` | Reject | `src/initialPrompt.md` | `docs/GENERATIVE_UI_SECURITY_MODEL.md` | 00:00:40 |
| PU-TRANSLATE | Per-client translation | `deferred_consumer_scope` | After Runtime V2 stable | `src/initialPrompt.md` | `—` | 00:02:04 |
| PU-MULTIPLAYER | Multiplayer Connect 4 | `deferred_collaboration_scope` | Defer | `README.md` | `—` | 00:03:14 |
| PU-PRIVATE | Private model history include ids | `deferred_collaboration_scope` | Defer | `src/initialPrompt.md` | `—` | 00:11:46 |
| PU-WIKI | Full page redesign Wikipedia | `rejected_product_direction` | Safe layout zones only; no full chrome replace | `README.md` | `src-tauri/src/runtime_v2/` | 00:03:47 |
| PU-SVG | SVG in responses | `safe_adaptation_required` | coreside.svg pack | `src/initialPrompt.md` | `src/components/tool-renderer/nodes.tsx` | 00:04:02 |
| PU-MATH | MathML | `safe_adaptation_required` | coreside.math pack | `—` | `src/components/tool-renderer/nodes.tsx` | 00:05:12 |
| PU-CANVAS | Canvas/3D whimsical | `safe_adaptation_required` | Declarative canvas limits; no WebGL JS | `—` | `src/components/tool-renderer/nodes.tsx` | 00:05:17 |
| PU-CODEMIRROR | CodeMirror playground | `safe_adaptation_required` | Bundled textarea editor; no auto-exec | `README.md` | `src/components/tool-renderer/nodes.tsx` | 00:09:01 |
| PU-TAILWIND | Tailwind via CDN | `rejected_security` | Reject CDN; use design tokens | `—` | `—` | 00:10:45 |
| PU-SILENT | Silent updates | `implemented_equivalent` | schema v2 silent | `src/initialPrompt.md` | `src-tauri/src/runtime_v2/operations.rs` | 00:02:09 |
| PU-INSTANCES | Instance numbers ttt/1 ttt/2 | `implemented_equivalent` | instance_id per surface | `src/initialPrompt.md` | `src-tauri/src/runtime_v2/surfaces.rs` | — |
| PU-STREAM | Progressive stream apply | `implemented_differently` | NDJSON validated frames | `src/index.ts` | `src-tauri/src/runtime_v2/streaming.rs` | 00:00:23 |
| CS-V1-TOOL | Coreside schema v1 tool_change | `partially_implemented` | Keep + add patches | `—` | `src-tauri/src/ai/response_schema.rs` | — |
| CS-INLINE | Inline generative surfaces | `missing_high_value` | Implement now | `—` | `src/components/chat/InlineSurface.tsx` | — |
| CS-PATCH | Fine-grained patches | `missing_high_value` | Implement now | `—` | `src-tauri/src/runtime_v2/patch.rs` | — |
| CS-PACKS | Capability packs | `missing_high_value` | Implement now | `—` | `src-tauri/src/runtime_v2/packs.rs` | — |
| CS-BRANCH | Chat branching | `missing_high_value` | Implement now | `—` | `src-tauri/src/runtime_v2/branch.rs` | — |
| CS-QUEUE | Request queue UI | `missing_high_value` | Implement now | `—` | `src-tauri/src/runtime_v2/queue.rs` | — |
| WICG-DPU | Declarative Partial Updates platform | `obsolete_or_reference_only` | Reference only; not adopted into webview | `WICG repo` | `—` | — |
| PU-DICTATION | Voice dictation self-mod | `deferred_consumer_scope` | Extension point; follow-up | `README.md` | `src-tauri/src/runtime_v2/packs.rs` | — |
| PU-PARSE | UpdateStreamParser incremental parse | `implemented_differently` | NDJSON frames | `src/index.ts` | `src-tauri/src/runtime_v2/streaming.rs` | — |
| PU-FORM-ROUTE | POST c/:chatId/form | `rejected_security` | In-process events only | `src/index.ts` | `src/lib/actions.ts` | — |
| PU-PROMPT-ROUTE | Prompt submission route | `implemented_differently` | Tauri send_message | `src/index.ts` | `src/stores/app-store.ts` | — |
| PU-CLIENT-SECRET | Client secret tokens | `deferred_collaboration_scope` | Defer | `src/index.ts` | `—` | — |
| PU-CLIENT-COLOR | User-specific colors | `deferred_collaboration_scope` | Defer | `src/initialPrompt.md` | `—` | — |
| PU-CLEAR-HIST | Clear history debug action | `partially_implemented` | Existing clear chats | `src/debug.ts` | `src/commands` | — |
| PU-DO | Cloudflare Durable Object | `rejected_product_direction` | Local-first Tauri | `src/index.ts` | `src-tauri` | — |
| PU-D1 | Cloudflare D1 auth DB | `deferred_enterprise_scope` | Defer | `migrations/0001_better_auth.sql` | `src-tauri/migrations` | — |
| PU-GATEWAY | Cloudflare AI Gateway | `rejected_product_direction` | Provider-neutral BYOK | `src/env.ts` | `src-tauri/src/ai` | — |
| PU-SCOPED-CSS | Scoped style markers | `safe_adaptation_required` | No arbitrary CSS; tokens only | `src/initialPrompt.md` | `src/styles` | — |
| PU-DISMOUNT | MutationObserver cleanup | `safe_adaptation_required` | React lifecycle; no model scripts | `src/initialPrompt.md` | `src/components/tool-renderer` | — |
| PU-FULLWIDTH | message-full-width class | `implemented_differently` | Inline surface full-width toggle | `src/initialPrompt.md` | `src/components/chat/InlineSurface.tsx` | — |
| PU-JSON-PATH | JSON client props path updates | `implemented_differently` | state.* operations | `spec.md` | `src-tauri/src/runtime_v2/operations.rs` | — |
| CS-TXN | Cross-surface transactions | `missing_high_value` | Implement now | `—` | `src-tauri/src/runtime_v2/transactions.rs` | — |
| CS-LAYOUT | Workspace layout model | `partially_implemented` | Table + ops; UI follow-up | `—` | `src-tauri/migrations/012_runtime_v2.sql` | — |
| CS-INSPECTOR | Developer inspector UI | `partially_implemented` | API ready; Settings Developer Mode UI next | `—` | `src-tauri/src/runtime_v2/mod.rs` | — |
| CS-EVENT-LOOP | Event loop protection | `implemented_equivalent` | Implement now | `—` | `src-tauri/src/runtime_v2/events.rs` | — |
| CS-V1-COMPAT | Schema v1 compatibility | `implemented_equivalent` | Preserve | `—` | `src-tauri/src/ai/response_parser.rs` | — |

## Disposition counts

- `deferred_collaboration_scope`: 6
- `deferred_consumer_scope`: 2
- `deferred_enterprise_scope`: 2
- `implemented_differently`: 12
- `implemented_equivalent`: 5
- `missing_high_value`: 6
- `obsolete_or_reference_only`: 1
- `partially_implemented`: 6
- `rejected_product_direction`: 3
- `rejected_security`: 5
- `safe_adaptation_required`: 8
