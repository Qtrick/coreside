# Direct Manipulation

**Product:** Coreside  
**UI:** Customize mode (`CustomizeMode.tsx`)

Consumer-facing name: **Customize** (not Developer Mode).

Supported actions emit the same Runtime V2 / Kernel operations as the agent (`sourceType: direct_manipulation` / `user`), with manual-edit provenance. No raw JSON, CSS, or JavaScript exposure.

When a pending agent proposal targets a manually edited revision, the proposal must be rebased or cancelled — stale apply is rejected.
