# Application Lifecycle

**Product:** Coreside  
**Code:** `application_kernel/lifecycle.rs` (+ manifest health/lifecycle fields)

## States

`draft`, `preview`, `active`, `suspended`, `archived`, `deleted_pending_cleanup`, `failed`, `disabled`, `restored`

Manifest upserts start as `active` with health `testing`. Disable → `disabled`. Crash loop / safe startup → `suspended`. LKG restore → `restored`.

## Dependencies

`DependencyEdge`: `sourceType` / `sourceId` → `targetType` / `targetId` with `relationshipType`, stored in `application_dependencies`.

`deletion_warnings` / `dependency_impact` list reverse edges before delete.

## Jobs

`application_jobs` tracks long-running work (`create_job`, progress updates). On restart, `interrupt_active_jobs` marks `pending`/`running` as `interrupted` — does not auto-resume provider calls.

## Garbage collection

`garbage_collect` is bounded and **does not** delete user data, LKG versions, credentials, or active manifests. It removes:

- `application_health_events` older than 30 days
- Failed `application_packages` rows older than 7 days
