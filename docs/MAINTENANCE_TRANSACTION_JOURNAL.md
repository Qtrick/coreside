# Maintenance Transaction Journal (RC3)

**Product:** Coreside  
**Status:** Persisted journal + deterministic startup actions + restore stage ladder Unit Verified — packaged restore proof open  
**Access date:** 2026-08-03

## Location

`AppPaths.recovery/maintenance-journal.json` — outside `coreside.db`, inside managed product root.

## Schema (`schema_version: 1`)

Records operation id/type, stage, irreversible flag, timestamps, optional profile paths, safety backup id, error/rollback/rehydration/completion fields. Optional `profile_generation` binds the journal to [`QuiescenceCoordinator`](./DURABLE_PROFILE_ARCHITECTURE.md) generation at maintenance start/stage updates.

## Lifecycle hooks

`AppState::{begin_maintenance,set_maintenance_stage,clear_maintenance}` persist or clear the journal and pause/resume quiescence.

Restore advances: Staging → Validating → Swapping → Reopening → Rehydrating → Completed. Mid-swap failure sets RecoveryRequired and **does not** clear the journal.

## Startup

If a journal exists:

- Validate path fields stay under AppPaths roots (rejects `..` and relative escapes).
- Classify: None / RollBack / EnterRecovery.
- **Apply** via `apply_startup_decision` (deterministic; clear failures → Recovery, not log-only).
- **Never** delete a profile tree merely because a stale journal exists.
- **Never** silently create a blank profile.

Current startup decisions:

- `None` + `completed`/`inactive` → clear stale journal
- `RollBack` + `!irreversible` → clear journal (staged trees left for Recovery inspection)
- `EnterRecovery` / irreversible `RollBack` / unreadable / clear failure → `enter_safe_startup` + quiescence pause + skip ordinary services

Full coherent profile restore / automated rollback resume remain partially open — see [DURABLE_PROFILE_ARCHITECTURE.md](./DURABLE_PROFILE_ARCHITECTURE.md).

## Tests

`maintenance_journal` unit tests cover persist/load/clear, path rejection, pre-swap rollback classification, mid-swap EnterRecovery, restore stage ladder (Validating vs Swapping/Reopening/Rehydrating), and `apply_startup_decision` clear / EnterRecovery outcomes. `data_cmds::restore_exit_paths_journal_and_quiescence` covers AppState journal + quiescence retain vs clear.
