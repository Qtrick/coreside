# Patch Scheduler

**Product:** Coreside  
**Code:** `runtime_v2/patch_scheduler.rs`

Flow: stream parser → validate → **scheduler** → Application Kernel → persistence → UI sync.

## Priorities (trusted)

`critical_recovery` > `direct_user_interaction` > `active_turn_preview` > `approved_persistent_change` > `navigation_update` > `background_refresh` > `automation_update` > `low_priority_enrichment`

Agents cannot assign `critical_recovery` or `direct_user_interaction`.

## Dependencies

Operations may declare `dependsOn`. Cycles are rejected. Ready ops apply in topological order.

## Live path

Agent turns call `schedule_and_apply` from `message_cmds` so queued ops pass through backpressure and dependency checks before Kernel apply.

## Two kinds of readiness

Durable readiness asks whether the target surface exists in SQLite and can accept a mutation. Renderer readiness asks whether a Tauri window has a fresh mount for that surface. Creates, data, and approved definition edits are durable: they apply while the UI is unmounted. `route.navigate` and active-turn preview edits of an existing surface wait until a mount exists.

A window may mount the same surface more than once (inline and tool canvas). Readiness is any fresh instance. Closing a window drops that window's mounts and does not delete the application.

`flush_patch_scheduler_cmd` derives the conversation from the surface this window has mounted. It does not accept a caller-chosen conversation, and it does not approve strong-risk operations.
