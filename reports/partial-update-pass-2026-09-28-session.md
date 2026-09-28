# Coreside Deep Implementation Pass — 2026-09-28

**Commit base:** `4ea70a4`  
**Dirty:** yes (this pass)  
**Public beta:** NOT READY

## Implemented this pass

1. **Visibility egress hardening** — progressive preview paint, create/update/promote/branch IPC, `get_tool_state` fail-closed projection.
2. **Bounded AI illegal-action retry** — Rust `probe_ai_*` + `MAX_REPAIR_ATTEMPTS` repair loop in `message_cmds` with model-safe rejection feedback.
3. **Chess FIDE claim protocol** — claimable threefold/fifty-move; automatic fivefold/75-move; resign; fixture actions.
4. **Reconnect catch-up client** — `getConversationEvents` API + subscribe-then-catch-up in app-store.
5. **AppRouteShell interactive hydrate** — `interactiveView` instead of raw state seeding.

## Verification

| Suite | Result |
| --- | --- |
| `runtime_v2::interactive` | 34 passed |
| `runtime_v2::chess::tests` | 22 passed |
| `runtime_v2::preview_transaction` | 23 passed |
| `runtime_v2::progressive_ops` | 37 passed |
| `runtime_v2::visibility` | 9 passed |
| `runtime_v2::replay` | 4 passed |
| `test:runtime-v2` | 350 passed |
| `test:queue` | 7 passed |
| `test:transactions` | 11 passed |
| `test:true-streaming` | 1 passed |
| `test:stream-parser` | 22 passed |
| `test:structured-forms` | 12 passed |
| `test:branching` | 6 passed |
| `test:registered-actions` | 96 passed |
| `test:prompt-injection` | 4 passed |
| AppRouteShell + interactive-surface Vitest | passed |
| `npm run typecheck` | passed |
| `npm run check:rust` | passed |
| `npm run fmt:check` | passed |
| `npm run e2e:desktop` / `npm run e2e` | **failed** — embedded WebDriver health check timeout on :4445 (plugin init logged; environment/blocker, not attributed to product regressions in this diff) |
| `npm run release:evidence:quick` | passed (2 checks; quick_partial) |

## Intentionally rejected (unchanged)

- Arbitrary HTML/JS/CDN/iframe form POST (Partial Update unsafe model)
