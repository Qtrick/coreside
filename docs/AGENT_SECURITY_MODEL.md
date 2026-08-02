# Agent Security Model

**Product:** Coreside  
**Access date:** 2026-08-01  
**Status:** Normative model aligned with implemented Rust gateways  
**Primary code:** `src-tauri/src/ai/tool_loop.rs`, `src-tauri/src/application_kernel/registered_actions/`, `src-tauri/src/security/`

## Trust boundary

| Layer | Trust | May do |
| --- | --- | --- |
| Model / prompt | Untrusted | Propose text, structured ops, capability calls |
| React webview | Untrusted UI | Render; call allowlisted Tauri commands |
| Rust | Trusted | Validate, persist, fetch, approve, enforce budgets |

Secrets never enter model-visible webview state. Generated surfaces never receive keyring, shell, or arbitrary Tauri access.

## Choke points

All privileged side effects must pass a **single trusted gateway** for their class:

1. **Agent tool loop** — capability registry (`project_context_search`, `web_search`, `fetch_web_page`, …); max steps/rounds/timeouts; cancellation.  
2. **Application operations / kernel** — schema-validated ops; protected-resource checks; revisions.  
3. **Registered actions** — `execute_registered_action` only; unknown actions fail closed.  
4. **Settings / search budgets** — dedicated commands; generic `set_setting` cannot bypass Exa budget/profile protections.

There is no “agent shell.” There is no path for model-authored JavaScript.

## Run / Ask / Block

Registered-action policy (`ActionPolicyDecision`) is the consumer consent model:

| Decision | User meaning | When |
| --- | --- | --- |
| **Run** (`Allow`) | Proceed without a new prompt | Declared + permitted reads; writes with a valid standing grant |
| **Ask** (`RequireApproval`) | Confirm before continuing | Writes without grant; destructive/critical while present; away writes waiting for user |
| **Block** (`Block`) | Refuse; asking would not help now | Undeclared/ungranted; destructive/critical while away; circuit breakers / Recovery Mode agent UI mutations |

Approvals are user-only, short TTL, CAS one-time consume. Destructive/critical never ride standing grants. Away presence cannot run deletions.

Kernel operation risk and registered-action risk are related but separate; both fail closed on unknown or forbidden permissions.

## Injection boundaries

Mapped to OWASP GenAI Top 10 themes without claiming full GenAI certification:

| Boundary | Control |
| --- | --- |
| Prompt injection → exfiltrate keys | Keys only in OS keyring / Rust; UI gets `keyDetected` |
| Injection → arbitrary code | Declarative components + registry; no model JS/HTML |
| Injection → filesystem / network | URL SSRF checks; no unrestricted FS; Crawl4AI/Exa separated |
| Excessive agency | Step limits, budgets, Ask/Block policy, Recovery Mode |
| Cross-project bleed | Project-scoped FTS and context ledgers |
| Stale / confused deputy | Revisions, approval CAS, frozen inputs on decide |
| Log leakage | Action Log sanitized; Off by default; redaction on errors |

User content and retrieved web text are **data**, not authority. Authority is only what Rust grants after validation.

## Agent-editable vs protected

| May change (allowlisted, validated) | Must not change |
| --- | --- |
| Theme, accents, allowlisted wallpapers via settings_change | Branding assets, Base Settings structure, Recovery disable |
| User-editable tools/surfaces via ops | Protected resource IDs (`core.*`) |
| Propose automations / imports pending approval | Shell, credentials, unrestricted network |

## Verification

- Policy unit tests in `registered_actions/policy.rs`  
- `npm run test:permissions`, `test:policy`, `test:recovery`, `test:ai-access`  
- Manual: agent cannot rename `core.branding*` / disable Recovery  

Do not claim beta security complete without regenerating evidence and resolving open P0s in `docs/SECURITY_ASSURANCE.md`.

## Related

- [SECURITY_VERIFICATION_STANDARD.md](./SECURITY_VERIFICATION_STANDARD.md)
- [GENERATIVE_UI_SECURITY_MODEL.md](./GENERATIVE_UI_SECURITY_MODEL.md)
- [REGISTERED_ACTIONS.md](./REGISTERED_ACTIONS.md)
- [AGENT_PROTOCOL.md](./AGENT_PROTOCOL.md)
- [APPLICATION_PERMISSIONS.md](./APPLICATION_PERMISSIONS.md)
