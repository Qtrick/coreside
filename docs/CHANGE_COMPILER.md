# Change Compiler

**Product:** Coreside  
**Code:** `application_kernel/compiler.rs`, `application_kernel/surface_diff.rs`**Version:** `COMPILER_VERSION = "1"`

Deterministic map from high-level `ChangeIntent` → ordered `AppOperation`s (`CompiledChange`).

## Authority

| API | DB lineage / OCC | Role |
| --- | --- | --- |
| `compile_plan_against_db` | Yes | **Only** admission path for durable Apply |
| `compile_for_inspection` / `compile_plan` | No | Structural / preview diagnostics — never Apply authority |
| `AgentResponsePayload::normalized_operations` | N/A | Refuses `applicationPlan`; legacy `operations` / `toolChange` only |

UpdateSurface against SQLite loads `ApplicationSpec` and emits the smallest safe `component.*` set when metadata/contracts allow. Falls back to `tool.full_replace` for redesigns or explicit contract/layout renames.

## Intents

| Intent | Emits |
| --- | --- |
| `AddSetting` | `setting.create` (rejects protected setting IDs) |
| `UpsertManifest` | `manifest.upsert` (validates manifest first) |
| `UpsertDataModel` | `data.model_upsert` |
| `MigrateDataModel` | `data.model_upsert` + migrationStrategy; destructive requires approval |
| `InsertComponent` | `component.insert` (pack allowlist via `validate_component_type_allowed`) |
| `UpdateComponent` | `component.update_props` |
| `RemoveComponent` | `component.remove` |
| `NavigateRoute` | `route.navigate` (blocks `settings` / `recovery`) — rejected in ApplicationPlan |
| `CreateSurface` | `surface.create` |
| `UpdateSurface` | DB-backed: granular `component.*` via surface_diff; inspection: `tool.full_replace` |

Each compiled op gets an id, `transactionGroup: "compiled"`, and an idempotency key. `CompiledChange` includes `summary` and `rollbackHint`.

Compile is trusted/local — it does not apply changes. Pass resulting operations from `compile_plan_against_db` to `apply_change`.
