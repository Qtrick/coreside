# Session Evidence — 2026-09-29 Catch-up / State Authority / Preview Hardening

## Source fingerprint

Working tree dirty at session close (uncommitted). See `git status --short`.

## Implemented (this session)

| Item | Status |
|------|--------|
| Per-window catch-up cursors (migration 034) | Implemented + Unit Verified |
| State `writePolicy: user/system` blocked for model | Implemented + Unit Verified |
| Opaque key protection on model `state.set` | Implemented + Unit Verified |
| `default_origin` fail-closed to `model` | Implemented |
| Preview `state.set` contract auth + merge semantics | Implemented + Unit Verified |
| Approval `decide` CAS on `status='pending'` | Implemented + Unit Verified |
| Queue `complete` requires `active` | Implemented + Unit Verified |
| Drain completes orphan after conversation delete race | Implemented |
| Create-side dependency edges + conversation cleanup | Implemented + Unit Verified |
| Tool upsert admit on full replace | Implemented |
| Tool-window ACL for sync cursor cmds | Implemented |
| User CAS blocks `system` writePolicy | Implemented |

## Intentionally rejected

- Partial Update arbitrary HTML/JS/CSS/CDN/iframe/clientSecret
- Vendo org/cloud multi-tenant machinery
- Experimental WICG Declarative Partial Updates browser APIs (conceptual reference only; Chrome experimental)

## Desktop / Packaged

| Check | Status |
|-------|--------|
| Desktop Verified | **Not Run / Blocked** — no `DISPLAY`, no E2E binary built, WebDriver :4445 not listening |
| Packaged Verified | **Not Run** |
| Journey 19/20 | **Not Run** (blocked by environment) |

## Unit verification (commands)

See session final response for exact pass/fail counts.
