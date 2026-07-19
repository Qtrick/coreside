# Generated Data Migrations

**Product:** Coreside  
**Code:** `application_kernel/data.rs` → `apply_migration`  
**Op:** `data.migrate`

Supported `migrationType` values only:

| Type | Effect | Confirmation |
| --- | --- | --- |
| `add_field` | Append field; optionally backfill `default` on existing records | Not required |
| `rename_field` | Rename in schema + rewrite record keys | Not required |
| `remove_field` | Drop field from schema | Requires `confirmDestructive: true` |

Payload shape: `migration` object (`fieldId`, optional `newFieldId` / `fieldType` / `default` / `required`) plus optional `confirmDestructive`.

Each applied migration:

1. Snapshots prior schema into `rollback_json`
2. Writes `generated_data_migrations` with impact summary
3. Bumps `schemaVersion` and upserts the model

Unsupported migration types fail closed. Impact summarizer warns when `remove_field` or migrations touch apps with existing records (see `impact.rs`).
