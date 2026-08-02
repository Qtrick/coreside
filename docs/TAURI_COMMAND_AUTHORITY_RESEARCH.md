# Tauri Command Authority Research

**Product:** Coreside  
**Access date:** 2026-08-02  
**Public beta:** **NOT READY**  
**Tauri:** 2.x (`tauri = "2"`, `@tauri-apps/*` ^2.5)

## Finding (authoritative for this track)

In **Tauri 2**, by default **all registered custom commands** are allowed to **all windows** unless you explicitly constrain them with:

1. **`AppManifest::commands`** (allowlist of command names the app may expose), and  
2. **`allow-*` permissions** in capability JSON files for the windows that may call them.

`src-tauri/build.rs` currently only calls:

```rust
fn main() {
    tauri_build::build()
}
```

No command allowlist is generated/enforced at build time beyond the default “all registered commands” behavior.

## Current Coreside posture

| Layer | What exists | Gap |
| --- | --- | --- |
| `generate_handler![…]` in `lib.rs` | ~200 registered commands | Broad surface |
| `capabilities/default.json` | `main` window: `core:*`, `shell:allow-open` | No per-command `allow-*` for custom IPC |
| `capabilities/tool-window.json` | `tool-*`: core window perms only (no shell) | Still inherits default custom-command access unless AppManifest/permissions restrict |
| Inventory | `reports/tauri-command-inventory.json` | Many kernel commands registered; some unused from frontend |

Window capability split (main vs `tool-*`) correctly drops `shell:allow-open` from tool windows, but that does **not** by itself prevent a tool webview from invoking sensitive custom Rust commands if the webview can call `invoke`.

## Risk for public beta

Secondary / generated-tool windows share the protected webview process model. If a future bug or compromised renderer path can `invoke` arbitrary registered names, kernel commands (`kernel_export_package_bytes`, policy overrides, etc.) are in scope unless denied by capabilities + AppManifest.

## Closure direction

1. Enable Tauri 2 command ACL: generate permissions for each custom command; grant narrowly per capability.
2. Use `AppManifest::commands` (via `tauri.conf` / build integration) so undeclared commands are unavailable.
3. Keep high-risk kernel/export/credential commands on **main** capability only.
4. Re-inventory after ACL: update `reports/tauri-command-inventory.json` + security findings.
5. Prove with a negative test: tool window cannot invoke a denied command.

## Non-claims

- Not claiming current tool windows have already been exploited.
- Not claiming capabilities alone (without command permissions) equal least privilege.
- Public beta command-authority gate remains **open** until ACL + evidence land.

## Related

- `src-tauri/capabilities/default.json`
- `src-tauri/capabilities/tool-window.json`
- `reports/tauri-command-inventory.json`
- [AGENT_TRUST_BOUNDARY_RESEARCH.md](./AGENT_TRUST_BOUNDARY_RESEARCH.md)
