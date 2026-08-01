# Recovery Mode

**Product:** Coreside  
**Code:** `application_kernel/recovery.rs`  
**UI:** Settings → Recovery (`src/components/settings/RecoverySettings.tsx`)

Protected-core only — must not depend on generated surfaces or capability packs.

## `RecoveryState`

| Field | Meaning |
| --- | --- |
| `recoveryMode` | Master switch |
| `disableUserSurfaces` | Hide/disable user surfaces |
| `disableCustomLayouts` | Disable custom layouts |
| `disableCapabilityPacks` | Disable packs |
| `uncleanShutdown` | Set on safe startup |
| `lastFailure` | Optional JSON reason |
| `updatedAt` | Timestamp |

Singleton row: `recovery_state` id `local`.

## APIs

- `set_recovery_mode` / `clear_recovery` / `set_flags`
- `enter_safe_startup(reason)` — enables recovery, disables user surfaces, records failure, **suspends apps with `crash_count >= 3`**

## Crash count suspend

`manifest::record_crash` increments `crash_count`. At ≥ 3, lifecycle/health become `suspended`. Restore LKG clears crash count. Settings Recovery UI can restore LKG per application.

The crash signal comes from `lifecycle::record_build_failure` when the failure is
reported as **non-retryable**: a transient failure is recorded for display but
does not count as a strike. That reaches the trusted core through the
`kernel_record_build_failure` IPC command.

`ToolErrorBoundary` in `src/components/tool-renderer/ToolRenderer.tsx` reports
non-retryable failures through `kernel_record_build_failure` (deduped per
application + tool version). Callers should pass a real kernel `applicationId`
(as `ToolCanvas` does); missing manifests record the failure row without
advancing `crash_count`. Covered by
`manifest::tests::non_retryable_build_failures_suspend_after_three_strikes`.

## Agent boundary

`assert_agent_cannot_disable_recovery` is enforced from `apply_change` via `assert_ops_not_protected` — ops mentioning recovery or payloads setting `recoveryMode: false` / `disableRecovery: true` are rejected.

While Recovery Mode or `disableUserSurfaces` is on, **agent** surface/component/layout/manifest mutations are rejected (`recovery_required`). Settings Recovery UI can still restore LKG and clear recovery.

**Partial:** flags are persisted and apply-path gated; hiding already-rendered user surfaces in every window/view is not fully wired yet.