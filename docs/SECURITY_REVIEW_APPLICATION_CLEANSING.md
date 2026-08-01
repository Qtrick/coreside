# Security Review — Application Cleansing Phase

**Date:** 2026-08-01  
**Commit:** `4b5fb1a288b5180860fe1e4fb984defdf296f036`  
**Machine-readable twin:** `reports/security-findings.json`  
**Production code changed:** yes — capability split, `open_external_url`, DNS-aligned URL validation (see resolutions below)

This review inspects the surfaces listed in the cleansing brief. Findings are evidence-based. Severity uses P0–P3.

---

## Scope inspected

| Surface | Path |
| --- | --- |
| Tauri capabilities | `src-tauri/capabilities/default.json` |
| CSP | `src-tauri/tauri.conf.json` → `app.security.csp` |
| Search URL safety | `src-tauri/src/search/safety.rs` |
| Registered-action URL handling | `src-tauri/src/application_kernel/registered_actions/handlers.rs` (`validate_external_url`, `external_link.open`) |
| Credential storage | `src-tauri/src/credentials/mod.rs`, `resolve.rs`, `hosted_session.rs` |
| Secondary tool windows | `src-tauri/src/windows/mod.rs` + capability `windows: ["main", "tool-*"]` |
| Frontend shell open | `src/lib/open-url.ts` |

---

## Summary counts

| Severity | Count |
| --- | --- |
| P0 | 0 |
| P1 | 2 |
| P2 | 3 |
| P3 | 3 |

---

## P1 findings

### SEC-CAP-001 — `tool-*` windows inherit `shell:allow-open` — **FIXED**

**Evidence:** `default.json` applies `shell:allow-open` to both `main` and `tool-*`. `open_tool_window` creates labels `tool-{id}` that load the protected app route `/#/tool/{id}`.

**Assessment:** Tool windows are not arbitrary HTML sandboxes; they run the same React shell. Generated surfaces still cannot execute unrestricted JS. The concern is privilege parity: any code path reachable in a tool window can call `@tauri-apps/plugin-shell` `open` the same way main can.

**Fixable:** yes (suggested, not applied). Split capabilities so `shell:allow-open` remains on `main` only unless a dedicated bridge is required.

### SEC-URL-001 — `openExternalUrl` lacks private-host guards — **FIXED**

**Evidence:** `src/lib/open-url.ts` only requires an `http://` or `https://` prefix, then calls shell `open`. By contrast, `validate_external_url` blocks localhost/private IPs, and `validate_public_http_url` additionally DNS-resolves and fail-closes.

**Assessment:** Search UI cards open result URLs through this helper. A crafted `https://127.0.0.1/...` (or similar) is not blocked on the frontend path.

**Fixable:** yes (suggested, not applied). Port the registered-action host checks into `open-url.ts` before invoking shell open.

---

## P2 findings

### SEC-URL-002 — `external_link.open` does not DNS-resolve — **FIXED**

Literal private IPs and `localhost` suffixes are blocked; hostnames that resolve to private addresses are not. Impact is reduced because the handler only returns a URL and `ToolRenderer` does not currently auto-open that result via shell.

### SEC-CSP-001 — CSP `img-src` allows arbitrary `https:`

Remote images may load from any HTTPS origin. `script-src` remains `'self'`; `connect-src` is limited to `'self' ipc: http://ipc.localhost`.

### SEC-IPC-001 — High-risk kernel commands without frontend wrappers

Inventory shows uninvoked registrations including `kernel_mark_last_known_good`, `kernel_export_package_bytes`, `kernel_set_policy_override`, and `kernel_clear_policy_override`. Missing wrappers reduce accidental UI use but do not remove the IPC surface from webviews that share default capabilities.

---

## P3 findings / positive controls

### SEC-CSP-002 — `style-src` includes `'unsafe-inline'`

Expected for current React styling; `script-src` does not allow `'unsafe-inline'` / `'unsafe-eval'`.

### SEC-CRED-001 — Credential storage (positive)

Provider secrets use OS keyring service `coreside.provider` with accounts `coreside:{connection_id}`. Module docs and code keep secrets out of SQLite. Cleansing fix R3 released the DB mutex before keychain reads.

### SEC-ACT-001 — Registered-action policy (positive)

Doctor checks `permissions.no_unrestricted_fs` / related gateway policy pass in the cleansing audit (47/47). Unrestricted filesystem / `shell.exec` categories are rejected on the gateway path.

---

## Explicit non-findings this pass

- No evidence that generated tool components can call arbitrary Tauri commands beyond the protected `api` / registered-action bridge.
- Packaged-build and Windows/Linux capability behavior were **not** re-verified here (environment-blocked); see subsystem matrix entries `packaging` and `cross-platform`.

---

## Recommended next actions (do not implement in the inventory task)

1. Split tool-window capabilities (SEC-CAP-001).
2. Harden `openExternalUrl` (SEC-URL-001) — small, safe frontend change.
3. Decide disposition for REVIEW-listed high-risk kernel commands (SEC-IPC-001).


## Resolutions applied

| ID | Resolution |
| --- | --- |
| SEC-CAP-001 | `capabilities/tool-window.json` for `tool-*` without `shell:allow-open`; main keeps create-window + shell |
| SEC-URL-001 | `api.openExternalUrl` → Rust `open_external_url` → `validate_public_http_url` → `ShellExt::open` |
| SEC-URL-002 | `validate_external_url` delegates to `validate_public_http_url` |
