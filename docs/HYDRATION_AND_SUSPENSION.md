# Hydration and Suspension

**Product:** Coreside  
**Code:** `runtime_v2/continuity.rs`, `get_surface_state`

On restart / remount: restore definitions, surface state, route state, policy-allowed drafts, continuity snapshots. Do **not** auto-resume audio, video with sound, mic, provider generation, or destructive jobs.

Offscreen or collapsed surfaces may enter `suspended` — expensive rendering pauses; wake restores compatible state without duplicate subscriptions.
