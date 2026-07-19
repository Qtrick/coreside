# Navigation Continuity

**Product:** Coreside  
**Code:** `runtime_v2/app_routes.rs`, `AppRouteShell.tsx`

Generated applications own namespaced routes in the Application Manifest. State persists in `application_route_state` (current route, params, history, index).

- Same-route navigation is a no-op (no remount, no history push).
- Back/Forward adjust `historyIndex`.
- Routes cannot collide with protected Coreside routes.
- Loading / empty / error fragment fallbacks remain protected in the shell.
