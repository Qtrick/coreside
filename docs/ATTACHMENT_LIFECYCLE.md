# Attachment Lifecycle (RC3)

**Product:** Coreside  
**Status:** Partial — atomic DB claim landed; FS promote crash window remains P2  
**Access date:** 2026-08-03

## Authoritative owner

SQLite `chat_attachments` (migration `016`) is authoritative for attachment truth. Opaque IDs only on public IPC.

## States in use

| State | Meaning |
| --- | --- |
| `staged` | Composer/draft staging; `backup_eligible=0` |
| `failed` | Staging/validation failure |
| `cancelled` | User/queue cancel |
| `expired` | Past `expires_at`; GC marks + deletes files |
| `attached` | Claimed to a message (DB); files may still be promoting |

## Message commit (current)

1. Validate staged IDs.
2. `BEGIN IMMEDIATE`: insert message + `claim_attachments_on_conn` + metadata.
3. `COMMIT`.
4. Promote files staging → durable (including `already_bound` rows — crash heal).
5. On promote failure: demote/unbind + delete orphan message.

`claim_attachments_on_db` wraps the same claim helper in its own transaction for standalone bind callers.

Invariant after successful send: message with N attachments has N `attached` rows. Crash between commit and promote may leave files in staging; protocol searches both roots; startup sweep promotes orphans.

## Queue

- Enqueued turns store opaque attachment IDs.
- Cancel only `queued` (not `active`).
- Cancel/remove/failed-drain release staged IDs best-effort (skips committed).
- Invalid queue prompt (`mentions` / `attachmentIds`): **complete item first**, then release staged IDs.
- Startup calls `recover_stale_active`.

## GC

`reconcile_and_sweep_attachments` (startup):

- Expire staged/failed/cancelled past `expires_at` (bounded).
- Promote attached-but-still-in-staging files.
- Log missing durable files (no silent blanking).

## Remaining P1/P2

- Protocol whole-file reads (P1)
- Protocol window/capability auth beyond opaque ID (P1)
- Multimodal provider parts (P1)
- Parser-level image bomb budgets (P2)
- Full productized periodic sweeper beyond startup (P2)
