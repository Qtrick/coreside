# Application Approvals and Grants

**Status:** Implemented  
**TTL:** Pending approvals expire after **15 minutes**

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
- Frozen `input_json` is stored for trusted re-execution after approve
- `kernel_decide_approval` re-runs the frozen call exactly once through the gateway

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
