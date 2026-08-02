# Agent Trust Boundary Research

**Product:** Coreside  
**Access date:** 2026-08-02  
**Public beta:** **NOT READY**  
**Normative model:** [AGENT_SECURITY_MODEL.md](./AGENT_SECURITY_MODEL.md)

## Boundary (do not blur)

| Layer | Trust | Authority |
| --- | --- | --- |
| Model / retrieved web text | Untrusted | Propose only |
| React webview (incl. generated surfaces) | Untrusted UI | Render; call allowlisted IPC |
| Rust gateways | Trusted | Validate, persist, network, approve, budgets |

There is no agent shell, no model-authored JS, and no keyring exposure to the webview.

## Choke points that must stay single-path

1. **Tool loop** — registry + step/round/timeouts (`ai/tool_loop.rs`)
2. **Application ops / kernel** — schema + protected resources + revisions
3. **Registered actions** — Allow / RequireApproval / Block (`registered_actions/policy.rs`)
4. **Settings / search budgets** — dedicated commands; no generic bypass

## Public-beta evidence required

| Evidence | Command / artifact | Status |
| --- | --- | --- |
| Policy unit tests | `npm run test:policy` / Rust policy tests | Exists; re-run at gate |
| Permissions / recovery / AI access | `test:permissions`, `test:recovery`, `test:ai-access` | Exists; re-run at gate |
| Registered-action suite | `test:registered-actions` | Exists; re-run at gate |
| Injection / agency boundary notes | Docs map GenAI Top 10 themes | Docs present; not a certification |
| Desktop approval journeys | E2E 06, 07, 08, 09 | 07 still **partial** coverage |

## Coupling to command authority

Even with a correct agent gateway, **webview → Tauri invoke** must not offer a side door around it. See [TAURI_COMMAND_AUTHORITY_RESEARCH.md](./TAURI_COMMAND_AUTHORITY_RESEARCH.md). Agent trust closure is incomplete if secondary windows can call privileged custom commands.

## Non-claims

- Not ASVS / GenAI Top 10 certified.
- Not “agent cannot harm” as a marketing claim.
- Public beta remains **blocked** until regression suite results are recorded in release evidence and E2E partial journeys are closed or explicitly waived.

## Next steps

1. Re-run policy/permissions/recovery/registered-actions at gate; attach to `reports/release-evidence.json`.
2. Finish or waive E2E 07 (multi-window approval race) with recorded rationale.
3. Land Tauri command ACL so tool windows cannot invoke kernel/credential commands.
4. Keep Action Log Off by default; never log secrets or system prompts.
