# Generated Application Testing

**Product:** Coreside  
**Code:** `application_kernel/testing.rs`

Declarative tests only — no shell, no browser automation harness.

## Declarative tests

`DeclarativeTest`: `testId`, `name`, `testType`, `actions[]`, `assertions[]`, `timeoutMs` (default 5000).

**Allowed actions:** `render_surface`, `find_component`, `click`, `enter_value`, `submit_form`, `navigate_route`, `create_record`

**Allowed assertions:** `visible_text`, `state_equals`, `record_exists`, `route_equals`, `no_overflow`, `has_accessible_label`, `no_console_error`, `render_ok`

Stored in `generated_tests`; runs recorded in `generated_test_runs`.

`run_test` is a trusted bounded runner: structural / DB checks (e.g. `record_exists`, `state_equals`, `visible_text` against definition JSON, `route_equals`).

**UI assertions that require a live renderer** (`render_ok`, `no_console_error`, `no_overflow`, `has_accessible_label`) are **not** auto-passed. When present without a renderer verification channel, `run_test` returns status `not_verified` and lists them under `unverifiedAssertions`. Do not treat `not_verified` as a green UI proof.

## Post-change verification

`verify_after_change` validates manifests for touched apps and **runs** up to 32 enabled declarative tests per app (bounded). Statuses:

- `verified` — structural OK and every executed test passed (no unverified UI assertions)
- `verified_with_warnings` — structural OK but at least one UI assertion returned `not_verified`
- `failed` — manifest invalid or a structural/declarative test failed

LKG (`mark_last_known_good`) is marked only when status is `verified`. Failure does **not** auto-rollback.

## Visual checks summary

`visual_checks_summary(width)` returns heuristic IDs for agent/offline context (`engine: "heuristic"`). Full overflow / label / touch / chart checks run in the UI via `verifySurfaceElement` (`src/lib/visual-verification.ts`) on inline surfaces (with layout warnings).

Exposed via `kernel_visual_checks`.

## Renderer verification channel

`authorize_renderer_verify` + `accept_renderer_fact` in `testing.rs`:

1. Request must name `applicationId`, `surfaceId`, `mountInstanceId`, and a closed-vocabulary `assertion`.
2. Rust checks surface ownership, conversation scope, and `MountRegistry.has_fresh_instance`.
3. Without a fresh mount → `not_verified` (never a fake pass).
4. Only after `status: authorized` may a renderer fact be accepted; unknown assertions fail closed.
5. No eval, Function(), arbitrary selectors, FS, shell, or Tauri from generated tests.

UI assertions in `run_test` still return `not_verified` until a live authorized fact is supplied for that assertion.
