# Security Review P0.4 — 2026-08-09

**Model selection: Auto** (security-review subagent usage-limited; completed by main agent)

| Field | Value |
| --- | --- |
| Commit | `9ed96b923f158a25f92104b3c600ca551f8a141a` (+ dirty P0.4 work) |
| Source fingerprint | see `reports/current-source-fingerprint.json` |
| Machine report | `reports/security-findings-p0.4-2026-08-09.json` |
| P0 / P1 | **0 / 0** |
| npm audit | 0 vulnerabilities (baseline) |

## Sensitive Kernel commands

Main-window + `require_profile()` enforcement for sensitive mutations including:

- `kernel_apply_change`
- `kernel_mark_last_known_good` / `kernel_restore_last_known_good`
- `kernel_export_package` / `_bytes` / `kernel_import_package`
- `kernel_set_policy_override` / `kernel_clear_policy_override`
- `kernel_grant_permission` / `kernel_revoke_permission`
- recovery mode / flags / clear / safe-startup
- `kernel_decide_approval` / `kernel_revoke_runtime_grant` / `kernel_clear_audit_events`
- `kernel_set_application_lifecycle`

Tool windows remain allowlist-isolated. Residual risk: main XSS (mitigated by CSP `script-src 'self'` + `skipHtml` markdown). Label checks are defense-in-depth, not a claim that the main webview is fully trusted.

## Main-renderer injection

- No model `dangerouslySetInnerHTML` sinks found
- Tool renderer strips `on*` and `dangerouslySetInnerHTML`
- Chat uses `ReactMarkdown` with `skipHtml`

## CSP

`unsafe-inline` styles + Google Fonts retained (deferred local-font migration). Scripts remain self-only.

## Dev/release sharing

Documented in `docs/DEV_RELEASE_STATE_ISOLATION_DECISION_2026-08-09.md`. Concurrent fail-closed shipped; data-path split deferred.
