# Application Manifest

**Product:** Coreside  
**Code:** `application_kernel/manifest.rs`  
**Schema version:** `"1"` (`MANIFEST_SCHEMA_VERSION`)

Declarative, versioned identity for a generated application. Stored in `application_manifests` with history in `application_manifest_versions`.

## Fields (`ApplicationManifest`)

| Field | Notes |
| --- | --- |
| `schemaVersion` | Must be `"1"` |
| `applicationId` / `instanceId` | Required; not protected IDs |
| `name` | Required non-empty |
| `description`, `version` | Optional / auto-bumped on upsert |
| `surfaces[]` | `surfaceId`, `placement`, optional `definitionRef` |
| `routes[]` | `routeId`, `title`, optional `surfaceId` |
| `dataModels[]` | `modelId`, optional `schemaRef` |
| `settings`, `capabilities`, `permissions`, `events`, `tests` | String lists |
| `searchKeywords`, `tags`, `agentDescription` | Discovery / agent context |
| `projectId`, `conversationId`, `organizationId`, `ownership` | Optional scoping |

Record metadata also tracks `healthState`, `lifecycleState`, `disabled`, `crashCount`, and `lastKnownGoodVersion`.

## Validation

- Size ≤ `MAX_MANIFEST_JSON_BYTES` (256 KiB)
- No secret-shaped strings (`api_key`, `authorization`, absolute `/users/`, `file://`)
- No executable content (`<script`, `javascript:`)
- Permissions must be in the allowed set (see [APPLICATION_PERMISSIONS.md](./APPLICATION_PERMISSIONS.md))
- Routes must not collide with protected navigation (`settings`, `recovery`, `providers`, `coreside.*`, core settings)
- Capability IDs must not be CDN URLs (`cdn.*` / `://`)

## Last-known-good (LKG)

- `mark_last_known_good` — after successful verification, sets `lastKnownGoodVersion` and flags the version row
- `restore_last_known_good` — restores that snapshot, sets lifecycle `restored`, clears `disabled` / `crashCount`
- Ops: `manifest.upsert`, `manifest.restore_last_known_good`, `manifest.disable`

`ensure_manifest_for_tool` wraps an existing personal tool as a minimal manifest (idempotent).
