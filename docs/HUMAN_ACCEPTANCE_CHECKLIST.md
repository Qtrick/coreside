# Human Acceptance Checklist

**Product:** Coreside  
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
