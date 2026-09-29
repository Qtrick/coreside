# Preservation Engine

**Product:** Coreside  
**Code:** `runtime_v2/preservation.rs`, `src/lib/preservation.ts`  
**Authoritative apply path:** `runtime_v2/transactions.rs` (`component.replace`, `component.remove`, `component.update_props`, full `chat.inline_surface_update`)

## Keys and policies

Components may carry a trusted `preservationKey` and `PreservationPolicy`:

`replace` · `preserve_instance` · `preserve_state` · `preserve_user_input` · `preserve_media_state` · `preserve_scroll` · `preserve_focus` · `preserve_selection` · `preserve_if_compatible` · `reset_explicitly`

The model cannot invent policies outside this enum.

Matching uses stable surface ID + component ID + optional preservation key, then type and capability compatibility.

### Dimension semantics

Policies are dimension-scoped. `preserve_focus` does **not** copy `value` / `scrollTop` / `currentTime`. Prop merge uses `prop_keys_for_policy`. Focus/scroll/media continuity also use `surface_continuity` DOM snapshots on the client.

Live surface-state invalidation (on remove / reset) uses OCC (`save_surface_state_occ`) and clears `valueKey` bindings, not only `componentId` keys.

### ToolCanvas dirty reconciliation

Unpersisted typing survives agent Sync reloads via `persistenceScheduler.peekDirtyState` + `reconcileDirtyOverCanonical` in `selectTool`, with store state winning over stale canonical merge order.

## Drafts

`surface_drafts` stores unsaved form/editor values with `baseRevision`. Stale agent patches surface a conflict banner: Keep draft / Apply agent / Cancel.

## Continuity

`surface_continuity` stores focus, scroll, and media snapshots per surface/window. Restore never autoplays media.
