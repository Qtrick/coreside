# Multiwindow Concurrency

**Product:** Coreside  
**Status:** Partial (conflict UX wired)

## Shared SQLite

Main window and secondary tool windows (`open_tool_window` / `#/tool/{id}`) share the same local SQLite database and Rust command surface. There is no separate per-window store.

## Revision conflicts on records

Generated data updates accept optional `baseVersion`. If it does not match `record_version`, update fails with `revision_conflict` (`data::update_record`). Kernel errors surface a user-safe reload message (`KernelError::RevisionConflict`).

Surface/component ops continue to use Runtime V2 `baseRevision` checks (see [GENERATIVE_INTERFACE_RUNTIME_V2.md](./GENERATIVE_INTERFACE_RUNTIME_V2.md)).

## Conflict UX

1. Apply paths emit `agent-turn` events: `conflict` (revision clash) and `sync` (successful apply with surface ids).
2. Main window and tool windows subscribe via `listenAgentTurn` (bootstrap / `loadToolWindow`).
3. `ConflictBanner` offers **Reload current version** (re-selects active tool) or **Dismiss**.
4. Proposal Apply that hits conflicts also sets `appConflict` in the store.

Not included yet: merge helpers, per-field diff UI, or automatic CRDT sync.
