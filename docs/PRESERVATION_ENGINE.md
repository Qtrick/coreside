# Preservation Engine

**Product:** Coreside  
**Code:** `runtime_v2/preservation.rs`, `src/lib/preservation.ts`

## Keys and policies

Components may carry a trusted `preservationKey` and `PreservationPolicy`:

`replace` · `preserve_instance` · `preserve_state` · `preserve_user_input` · `preserve_media_state` · `preserve_scroll` · `preserve_focus` · `preserve_selection` · `preserve_if_compatible` · `reset_explicitly`

The model cannot invent policies outside this enum.

Matching uses stable surface ID + component ID + optional preservation key, then type and capability compatibility.

## Drafts

`surface_drafts` stores unsaved form/editor values with `baseRevision`. Stale agent patches surface a conflict banner: Keep draft / Apply agent / Cancel.

## Continuity

`surface_continuity` stores focus, scroll, and media snapshots per surface/window. Restore never autoplays media.
