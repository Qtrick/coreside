# Generated Application Testing

**Product:** Coreside  
**Code:** `application_kernel/testing.rs`

Declarative tests only — no shell, no browser automation harness.

## Declarative tests

`DeclarativeTest`: `testId`, `name`, `testType`, `actions[]`, `assertions[]`, `timeoutMs` (default 5000).

**Allowed actions:** `render_surface`, `find_component`, `click`, `enter_value`, `submit_form`, `navigate_route`, `create_record`

**Allowed assertions:** `visible_text`, `state_equals`, `record_exists`, `route_equals`, `no_overflow`, `has_accessible_label`, `no_console_error`, `render_ok`

Stored in `generated_tests`; runs recorded in `generated_test_runs`.

`run_test` is a trusted bounded runner: structural / DB checks (e.g. `record_exists`); several UI assertions currently pass as schema-level placeholders.

## Post-change verification

`verify_after_change` validates manifests for touched apps and queues enabled tests. Statuses: `verified`, `verified_with_warnings`, `failed`. LKG is marked only when status is `verified`. Failure does **not** auto-rollback.

## Visual checks summary

`visual_checks_summary(width)` returns heuristic IDs for agent/offline context (`engine: "heuristic"`). Full overflow / label / touch / chart checks run in the UI via `verifySurfaceElement` (`src/lib/visual-verification.ts`) on inline surfaces (with layout warnings).

Exposed via `kernel_visual_checks`.
