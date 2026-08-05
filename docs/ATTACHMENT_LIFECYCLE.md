# Attachment Lifecycle (RC3)

**Product:** Coreside  
**Phase:** RC3.3 Phase 13 partial — authorization + periodic GC + crash-consistency suite  
**Status:** Partial — atomic DB claim landed; conversation-scoped read auth + `run_attachment_gc` **Unit Verified**; crash/reconcile suite **Unit Verified** (`reports/attachment-crash-results.json`)  
**Access date:** 2026-08-03

## Authoritative owner

SQLite `chat_attachments` (migration `016`) is authoritative for attachment truth. Opaque IDs only on public IPC. Opaque IDs alone are **not** sufficient for read/download/bytes — callers must supply conversation scope.

## States in use

| State | Meaning |
| --- | --- |
| `staged` | Composer/draft staging; `backup_eligible=0` |
| `failed` | Staging/validation failure |
| `cancelled` | User/queue cancel |
| `expired` | Past `expires_at`; GC marks + deletes files |
| `attached` | Claimed to a message (DB); files may still be promoting |

## Authorization (Phase 13 partial)

Helper: `authorize_attachment_access(conn, attachment_id, conversation_id)`.

Read paths that must authorize:

- `get_chat_attachment_src` — requires `conversationId`; returns scoped opaque URL
- Custom protocol bytes — URL shape `/attachment/{conversationId}/{attachmentId}`; calls the same helper before reading

Rules:

1. Conversation must exist in the caller's healthy profile DB (project ownership via conversation membership).
2. Attachment row must be bound to that `conversation_id` (unbound staged rows are denied on scoped reads).
3. When `message_id` is set, the message must belong to the same conversation.
4. Wrong `conversation_id` → deny (`not_found`, no enumeration leak).

Reports: `reports/attachment-authorization-results.json` → **Unit Verified**.

## Message commit (current)

1. Validate staged IDs.
2. `BEGIN IMMEDIATE`: insert message + `claim_attachments_on_conn` + metadata.
3. `COMMIT`.
4. Promote files staging → durable (including `already_bound` rows — crash heal).
5. On promote failure: demote/unbind + delete orphan message.

`claim_attachments_on_db` wraps the same claim helper in its own transaction for standalone bind callers.

Invariant after successful send: message with N attachments has N `attached` rows. Crash between commit and promote may leave files in staging; protocol searches both roots; startup sweep promotes orphans.

## Crash-consistency suite (Unit Verified)

Temp-dir + rusqlite tests (`CORESIDE_DATA_DIR` override) prove:

1. Crash after message insert but before claim (txn rollback) → no message, attachment stays `staged`, no durable file.
2. Crash after claim DB commit but before promote → startup `reconcile_and_sweep_attachments` promotes staging → durable.
3. Visible message with N attachments → N `attached` rows + N durable files after reconcile; second reconcile is idempotent.
4. Wrong-conversation authorize still denies after claim/promote.

Command: `npm run test:attachment-crash-consistency`. Report: `reports/attachment-crash-results.json`.

## Queue

- Enqueued turns store opaque attachment IDs.
- Cancel only `queued` (not `active`).
- Cancel/remove/failed-drain release staged IDs best-effort (skips committed).
- Invalid queue prompt (`mentions` / `attachmentIds`): **complete item first**, then release staged IDs.
- Startup calls `recover_stale_active`.

## GC

`reconcile_and_sweep_attachments` (startup + command):

- Healthy profile only (`require_profile`).
- Expire staged/failed/cancelled past `expires_at` (bounded, `LIMIT 200`).
- Promote attached-but-still-in-staging files under managed roots.
- File deletes use `remove_managed_file_no_follow` (no symlink follow).
- Summary is counts-only (`expired` / `promotedOrphans` / `missingDurable`) — redacted.

`run_attachment_gc` Tauri command wraps the same sweep for scheduler / manual invoke. **Scheduler wiring is next** (not hooked into the automation ticker in this slice).

Reports: `reports/attachment-gc-results.json` → **Unit Verified**.

## Remaining P1/P2

- Protocol whole-file reads remain bounded but still load into memory (P1)
- Multimodal provider parts (P1) — **Integrated (unit)**: `AgentContentPart::Image { attachment_id, mime_type, data_base64 }` after `authorize_attachment_access`; OpenAI-family → `image_url` data URLs; Ollama → `images[]`; Anthropic → native base64 `image` blocks; Gemini → `inlineData`. Tool results use sealed JSON envelope (`envelopeHash`, `trust=untrusted_tool_output`); Gemini wraps envelope in `functionResponse`. Send path blocks unsupported image/tool via `validate_provider_send` (draft preserved). Report: `reports/multimodal-provider-results.json`. Desktop E2E **not_run**.
- Parser-level image bomb budgets (P2)
- Wire `run_attachment_gc` into a periodic scheduler tick (P2)
- Desktop / packaged attachment crash proof (open)
