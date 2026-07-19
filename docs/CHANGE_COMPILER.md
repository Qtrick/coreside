# Change Compiler

**Product:** Coreside  
**Code:** `application_kernel/compiler.rs`  
**Version:** `COMPILER_VERSION = "1"`

Deterministic map from high-level `ChangeIntent` → ordered `AppOperation`s (`CompiledChange`).

## Intents

| Intent | Emits |
| --- | --- |
| `AddSetting` | `setting.create` (rejects protected setting IDs) |
| `UpsertManifest` | `manifest.upsert` (validates manifest first) |
| `UpsertDataModel` | `data.model_upsert` |
| `InsertComponent` | `component.insert` (pack allowlist via `validate_component_type_allowed`) |
| `NavigateRoute` | `route.navigate` (blocks `settings` / `recovery`) |

Each compiled op gets an id, `transactionGroup: "compiled"`, and an idempotency key. `CompiledChange` includes `summary` and `rollbackHint`.

Compile is trusted/local — it does not apply changes. Pass resulting operations to `apply_change`.
