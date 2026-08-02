# Performance Limits

**Product:** Coreside  
**Code:** `src-tauri/src/runtime_v2/limits.rs`

Agent cannot raise these ceilings. Compact subset exposed via `limits_json()` / capability catalog.

## Ceilings

| Constant | Value |
| --- | --- |
| `MAX_OPERATIONS_PER_TURN` | 32 |
| `MAX_TRANSACTION_GROUPS_PER_TURN` | 8 |
| `MAX_COMPONENT_TREE_DEPTH` | 24 |
| `MAX_COMPONENTS_PER_SURFACE` | 200 |
| `MAX_SURFACES_PER_CONVERSATION` | 64 |
| `MAX_INLINE_SURFACES_VISIBLE` | 12 (UI soft-cap only; list API returns full active set) |
| `MAX_SUBSCRIPTIONS_PER_SURFACE` | 16 |
| `MAX_EVENT_DEPTH` | 8 |
| `MAX_EVENTS_PER_SURFACE_PER_MINUTE` | 20 |
| `MAX_IDENTICAL_EVENTS_PER_INTERVAL` | 3 |
| `MAX_SVG_NODES` | 500 |
| `MAX_CHART_POINTS` | 2_000 |
| `MAX_CANVAS_OBJECTS` | 200 |
| `MAX_CODE_EDITOR_CHARS` | 200_000 |
| `MAX_DEFINITION_JSON_BYTES` | 512_000 |
| `MAX_STATE_JSON_BYTES` | 256_000 |
| `MAX_QUEUED_TURNS` | 5 |
| `MAX_BRANCH_DEPTH` | 32 |
| `MAX_SNAPSHOTS_PER_CONVERSATION` | 50 |
| `MAX_DIAGNOSTICS_RETENTION` | 100 |
| `MAX_REPLAY_OPS_LOADED` | 500 |
| `EVENT_COOLDOWN_MS` | 250 |
| `MAX_MANIFEST_JSON_BYTES` | 256_000 |
| `MAX_GENERATED_RECORDS_PER_QUERY` | 500 |
| `MAX_ANIMATED_COMPONENTS` | 8 |
| `MAX_REPAIR_ATTEMPTS` | 3 |

Package import also rejects bodies larger than 20 MB (`packages.rs`).
