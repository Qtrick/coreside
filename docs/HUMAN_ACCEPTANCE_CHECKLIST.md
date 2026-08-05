# Human Acceptance Checklist

**Product:** Coreside  
**Phase:** RC3.6 (unsigned template)  
**Status:** Unsigned template — do **not** treat this file as approval.

Fill this only after real desktop/packaged exercise on a named commit. Leave fields blank until a human records them. Fabricating signatures or Pass results is forbidden.

## Session metadata (required)

| Field | Value |
| --- | --- |
| Date (ISO) | |
| Commit SHA | |
| Dirty worktree? (must be No for beta claims) | |
| Build type (`dev` / `packaged`) | |
| OS / arch | |
| Operator name | |
| Operator signature | |
| Witness (optional) | |

## Acceptance rows

Mark each row **Pass** / **Fail** / **Skipped** / **Blocked**. Skipped and Blocked need a note.

| ID | Claim | Status | Notes |
| --- | --- | --- | --- |
| H1 | Cold start: app boots; chat loads; no panic | | |
| H2 | AI connection: connect your provider; key not in SQLite | | |
| H3 | Simple chat streams; cancel works | | |
| H4 | Channel-scoped text: no global `agent-turn` text eavesdrop in secondary window | | |
| H5 | Wallpaper apply + transparency slider: atomic persist; readable UI | | |
| H6 | Queue: second send while busy queues; UI reflects queue | | |
| H7 | History/replay panel usable for read-only inspection | | |
| H8 | Strong proposal Apply / Discard behaves correctly | | |
| H9 | Restart persistence for chat + created app | | |
| H10 | Packaged smoke (if claiming Packaged Verified) | | |
| H11 | Hosted AI path (only if hosted track in scope) | | |

## RC3.6 — privacy, ACL, multimodal (unsigned)

Automated evidence may exist on HEAD; human rows stay blank until a signed session. **Do not** copy automated Pass into this table.

| ID | Claim | Status | Notes |
| --- | --- | --- | --- |
| R1 | Tool window denies sensitive raw invokes (backup, credentials, send_message, attachments) | | Automated: Journey 11 passed on HEAD (isolated suite); full `npm run e2e` **not_run** |
| R2 | Cross-tool `save_tool_state` denied; own-tool read still works | | Same Journey 11 evidence; human must confirm in real session |
| R3 | Local AI (`user_local`) shows honest disclosure; no outbound provider traffic | | Journey 17 spec registered; harness **not_run** |
| R4 | Hosted Coreside AI Free chat (only if hosted in scope) | | Journey 18 spec registered; requires Supabase session — **not_run** |
| R5 | Image attachment reaches model on configured provider (BYOK) | | Multimodal send path **Unit Verified** only; desktop chat **not_run** |
| R6 | Preview overlay clears on Sync/Conflict without wiping unrelated chats | | Unit + overlay tests; desktop progressive paint **not_run** |
| R7 | Settings category heading matches selection; no stale title during transition | | `aria-busy` landed; manual verify only |

## Consumer usability (RC3.4)

| ID | Claim | Status | Notes |
| --- | --- | --- | --- |
| C1 | After welcome, tester can explain what Coreside does | | |
| C2 | Tester can start without reading developer docs | | |
| C3 | Core tour completes (or skip works) | | |
| C4 | Tour can be reopened from Help & learning | | |
| C5 | Tester can create or inspect an app | | |
| C6 | Preview vs apply is understandable | | |
| C7 | Permission request language is clear | | |
| C8 | Tester can create a project | | |
| C9 | Wallpaper settings found under Appearance | | |
| C10 | Help & learning is discoverable | | |
| C11 | Queued messages are understandable | | |
| C12 | Versions / replay are understandable | | |
| C13 | Failed AI connection errors are actionable | | |
| C14 | Local-vs-cloud privacy language is clear | | |
| C15 | No confusing contradictory labels (App vs Tool) | | |
| C16 | No surprising controls | | |
| C17 | Errors explain next safe action | | |
| C18 | Local AI vs cloud/hosted routing is understandable in Settings | | RC3.6 — see R3 |
| C19 | Attachment / image privacy (what leaves the device) is clear | | RC3.6 — multimodal unit only |
| C20 | Hosted billing / plan language is honest when shown | | Hosted track Deliberately Deferred |

## Verdict block (leave empty until signed)

| Track | Ladder rung claimed | Approved? (Yes/No) | Approver |
| --- | --- | --- | --- |
| Local-first | | | |
| Hosted Coreside AI | | | |

**Overall human acceptance:** _unsigned_

## Non-claims

- This template alone does not advance readiness ladder state.
- Automated Unit Verified evidence does not satisfy Human Accepted.
- Related automated narrative: [MANUAL_RELEASE_CHECKLIST.md](./MANUAL_RELEASE_CHECKLIST.md), [BETA_READINESS_MODEL.md](./BETA_READINESS_MODEL.md).
