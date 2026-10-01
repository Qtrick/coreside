# Application Approvals and Grants

**Status:** Implemented  
**TTL:** Pending approvals expire after **15 minutes**. Approved-but-unused approvals expire **15 minutes after the decision** (consume must happen within that window).

## Two layers

### Application permissions

Broad categories declared on the manifest and granted by the user (`local_data.write`, `web_search.request`, …). Enforced by `application_permissions` and checked on every registered-action call.

### Runtime action grants

User consent for a specific registered action / input scope. Separate table: `runtime_action_grants`.

| Scope | Meaning |
|---|---|
| `exact` | Same call hash only |
| `action` | That action, any input |
| `application_action` | That application + action |

| Duration | Meaning |
|---|---|
| `once` | Spent after one successful use |
| `session` | Until Coreside process exits |
| `standing` | Until revoked |

Destructive and critical actions cannot receive remembered grants in this phase.

## Approvals

Statuses: `pending` → `approved` / `denied` / `expired` → `consumed`

- Only actor `user` may decide (no agent self-approval)
- Decide + consume use CAS receipts in `runtime_approval_claims`
- `expire_stale` expires both `pending` and `approved` rows whose `expires_at` has passed
- Approving refreshes `expires_at` to decision time + approved-unused TTL
- `consume` sweeps stale rows first, then CAS-updates only while still `approved` and within `expires_at`
- Frozen `input_json` is stored for trusted re-execution after approve
- Frozen `conversation_id` / `project_id` are stored on the approval row (migration `036_approval_call_context`) so Approve-once replay reconstructs the same `call_hash` even when the target personal-tool surface has a null conversation
- `call_hash` **v2** binds: `hashVersion`, applicationId, projectId, conversationId, **surfaceId**, **componentId**, venue, presence, action, descriptorHash, input. Same action+input from a different surface or component cannot reuse an approval. Pre-v2 pending approvals are not reinterpreted (fail-closed until TTL expiry).
- `kernel_decide_approval` rebuilds context via `replay_context_from_approval` and re-runs the frozen call exactly once through the gateway
- `call_hash` binds application, action, input, conversation, venue, and presence (cross-conversation replay is rejected)

## Default policy

| Presence | Risk | Behavior |
|---|---|---|
| Present | read | Auto if declared + permission + not critical |
| Present | write | Ask unless matching grant |
| Present | destructive / critical | Always ask |
| Away | write | Requires standing `application_action` grant |
| Away | destructive / critical | Blocked |

## UI

- Approval cards via `PendingApprovalsHost`
- Remembered grants in Settings → App permissions
- Per-application details panel (Details on tool canvas)
