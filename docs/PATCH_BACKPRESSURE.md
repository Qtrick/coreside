# Patch Backpressure

**Product:** Coreside  
**Limits:** `MAX_PATCH_QUEUE`, `MAX_PATCH_QUEUE_BYTES`, `MAX_PREVIEW_HZ` in `runtime_v2/limits.rs`

- Queue fullness rejects further schedule calls with a clear error.
- Preview supersession drops older `active_turn_preview` items for the same surface.
- Compatible transient updates may coalesce in the UI; destructive/persistent ops never coalesce away.
- Overload diagnostics are redacted and bounded.
