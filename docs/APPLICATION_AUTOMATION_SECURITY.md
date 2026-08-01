# Application Automation Security

**Status:** Implemented (phase 1)  
**Scheduler:** existing `src-tauri/src/automations/` (not replaced)

## Association

Automations may set `application_id`. App-bound automations:

- Execute only through `execute_registered_action_trusted` with `presence: away`
- Currently support `SetToolStateValue` → `tool_state.set`
- Surface `waiting_approval` and `permission_ready` flags

## Away rules

- Writes require a standing application-bound grant
- Exact / session / other-app grants do not authorize away runs
- Destructive / critical away calls are blocked
- Missing authority parks the automation (`waiting_approval = true`) instead of silently succeeding
- Approving a parked request clears waiting flags and re-executes the frozen call

## Failure handling

Existing consecutive-failure pause thresholds still apply. Away parking is not counted as a failure.

## UI

Automations panel shows owning application, waiting-approval, and permission-readiness badges.
