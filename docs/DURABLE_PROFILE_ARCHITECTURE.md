# Durable Profile Architecture (RC3.3 Phase 13 partial)

**Product:** Coreside  
**Status:** Quiescence + restore journal stage ladder Unit Verified — full packaged restore still open  
**Access date:** 2026-08-03

## Intent

Profile swaps (restore / migration) must quiesce profile-dependent work so the active tree is not mutated mid-swap, and so stale async resumes cannot unpause a newer profile generation.

## QuiescenceCoordinator

`src-tauri/src/quiescence.rs` owns a process-wide coordinator on `AppState`:

| Concern | Behavior |
| --- | --- |
| `pause` / `resume(token)` | Pause token bound to profile generation |
| `bump_generation` | On `replace_profile_database` — invalidates outstanding tokens |
| `force_resume` | Owning maintenance clear path (generation may have bumped) |

Registered subsystems (consult `allows` / `require_active`):

1. **Queue drain** — `try_begin_queue_drain` refuses; in-flight drain loop exits
2. **Patch scheduler** — `schedule_patches_cmd` / `flush_patch_scheduler_cmd` error while paused
3. **Attachment GC** — `reconcile_and_sweep_attachments` returns empty report (also requires healthy profile)

`begin_maintenance` pauses; `clear_maintenance` force-resumes. Automation `SchedulerHandle` pause remains in the restore command (orthogonal ticker).

## Restore transaction (partial)

`restore_profile_backup` advances a persisted journal while quiescence is held:

`CancellingWork` → `Staging` → `Validating` → `Swapping` → `Reopening` → `Rehydrating` (finalizing) → `Completed`

| Failure window | Behavior |
| --- | --- |
| Before irreversible swap (`Validating` and earlier) | Clean staging trees, clear journal, resume quiescence + scheduler |
| Mid-swap / reopen / asset rollback | Set `RecoveryRequired`, **retain** journal, keep quiescence paused, do not pretend success |

“Finalizing” maps to `Rehydrating` + `Completed` in `MaintenanceStage` (no separate Finalizing enum variant).

## Startup journal

See [MAINTENANCE_TRANSACTION_JOURNAL.md](./MAINTENANCE_TRANSACTION_JOURNAL.md). Classifications apply via `apply_startup_decision` (clear or EnterRecovery). Clear failures enter Recovery — not log-only.

## Not claimed (still open)

- Full automated rollback resume of irreversible mid-swap journals (filesystem driver)
- Quiescence of every background job beyond the three subsystems above
- Desktop / packaged proof of restore under load — **do not claim Packaged Verified**

Reports: `reports/subsystem-quiescence-results.json`, `reports/maintenance-startup-results.json`, `reports/restore-transaction-results.json`.
