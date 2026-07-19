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
