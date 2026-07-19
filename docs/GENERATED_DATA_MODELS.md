# Generated Data Models

**Product:** Coreside  
**Code:** `application_kernel/data.rs`

Declarative schemas compiled to **namespaced CRUD**. The agent cannot run arbitrary SQL.

## Field types

`text`, `long_text`, `integer`, `decimal`, `boolean`, `date`, `time`, `date_time`, `duration`, `enum`, `reference`, `media_ref`, `surface_ref`, `tool_ref`, `json`, `created_at`, `updated_at`

`enum` fields require non-empty `enumValues`.

## Model definition

`DataModelDefinition`: `modelId`, `displayName`, `fields[]`, `schemaVersion`.

Validation rejects empty models, SQL-injection-shaped IDs (`;`, `--`, spaces), and unknown field types.

## CRUD (application-scoped)

| Op | Behavior |
| --- | --- |
| `data.model_upsert` | Upsert model definition |
| `data.record_create` | Validate + insert; applies defaults / timestamps |
| `data.record_update` | Optional `baseVersion` → `revision_conflict` on mismatch |
| `data.record_delete` | Delete by record id within application |
| Query helper | `query_records` capped by `MAX_GENERATED_RECORDS_PER_QUERY` (500) |

Writes require granted `local_data.write` (`assert_can_write_data`).

Records live in `generated_data_records` as JSON blobs with `record_version`. Query results inject `_id`, `_version`, `_createdAt`, `_updatedAt`.

## Forbidden

- `data.execute_sql` and any payload containing `sql` (rejected at the kernel gateway)
