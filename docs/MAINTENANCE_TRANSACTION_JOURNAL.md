# Maintenance Transaction Journal (RC3)

**Product:** Coreside  
**Status:** Persisted journal foundation — Recovery/rollback driver incomplete  
**Access date:** 2026-08-03

## Location

`AppPaths.recovery/maintenance-journal.json` — outside `coreside.db`, inside managed product root.

## Schema (`schema_version: 1`)

Records operation id/type, stage, irreversible flag, timestamps, optional profile paths, safety backup id, error/rollback/rehydration/completion fields.

## Lifecycle hooks

`AppState::{begin_maintenance,set_maintenance_stage,clear_maintenance}` persist or clear the journal.

## Startup

If a journal exists:

- Validate path fields stay under AppPaths roots (rejects `..` and relative escapes).
- Classify: None / RollBack / EnterRecovery.
- **Never** delete a profile tree merely because a stale journal exists.
- **Never** silently create a blank profile.

Current startup decisions (in `lib.rs` setup):

- `None` + `completed` → clear stale completed journal
- `RollBack` + `!irreversible` → clear journal (staged trees left for Recovery inspection); clear failure → enter Recovery / skip ordinary services
- `EnterRecovery` / irreversible `RollBack` / unreadable → `enter_safe_startup` + skip ordinary services

Full coherent profile restore / automated rollback resume remain open.

## Tests

`maintenance_journal` unit tests cover persist/load/clear, path rejection, and pre-swap rollback classification.
