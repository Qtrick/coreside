# Vendo Reference and Adoption Audit

**Status:** Evidence-based audit for Coreside’s generated-application runtime  
**Date:** 2026-08-01  
**Vendo reference location:** `.reference/vendo/vendo-main` (read-only)  
**License:** Apache 2.0 (`LICENSE` + `NOTICE`)

## Product distinction

| | Vendo | Coreside |
|---|---|---|
| Problem | Make an existing SaaS product customizable by its customers | Give an individual a personal AI environment that grows persistent capabilities |
| Host | Next.js / Express web host | Tauri desktop + Rust trusted core |
| Persistence | Host store / Postgres-oriented | Local SQLite |
| “Tool” | Executable API action for the agent | User-created personal application / surface |
| Execution | TypeScript guard around tool registry | Rust registered-action gateway |

Coreside must not become a Vendo fork, SaaS SDK, or unrestricted app builder.

## Nomenclature

- **Coreside Tool** = personal generated application (surfaces + manifest).
- **Coreside RegisteredAction / ActionDescriptor** = executable privileged capability.
- Vendo’s `ToolDescriptor` maps to Coreside **RegisteredAction**, not to Coreside Tools.

## Decision summary

See `reports/vendo-adoption-matrix.json` for the machine-readable matrix.

| Vendo concept | Decision | Coreside home |
|---|---|---|
| Guard choke point | `ALREADY_EXISTS` / adapted | `application_kernel/registered_actions/gateway.rs` |
| Descriptor hashing | `ALREADY_EXISTS` | `descriptor.rs` + `canonical.rs` (includes `permissionCategory`, excludes description) |
| Approvals (one-time) | `ALREADY_EXISTS` | `approvals.rs` + `runtime_approval_claims` |
| Grants (scope/duration) | `ALREADY_EXISTS` | `grants.rs` |
| Present / away | `ALREADY_EXISTS` | `context.rs` Presence + Venue |
| AppDocument | `ADAPT_NOW` → adapted via manifests | `application_manifests` + Application Kernel |
| Host API sync / OpenAPI | `REJECT` | — |
| MCP door / Composio | `REJECT` | — |
| e2b / Modal sandboxes | `REJECT` | Declarative surfaces only |
| Doctor / conformance | `ADAPT_NOW` | `npm run doctor` (offline) |
| Automations grant capture | `ADAPT_NOW` | App-bound automations + standing grants |
| Cloud / org admin | `REJECT` (this phase) | — |

## Licensing

Vendo is Apache-2.0. Coreside **conceptually reimplemented** the consent/guard model in Rust. No Vendo source modules were transplanted into the application tree. Attribution: `THIRD_PARTY_NOTICES.md`.

## Baseline checks (this session)

| Command | Result |
|---|---|
| `npm run typecheck` | Pass |
| `npm run doctor` | Pass (44 checks) |
| `cargo test --lib application_kernel::registered_actions` | Pass (67–68 tests) |
| Vitest `actions` + `ApprovalCard` | Pass (7) |
| Full `npm run verify` | Recorded in final report |

## Gaps closed in this phase

- Registered-action Rust runtime (gateway, policy, grants, approvals, breakers, audit)
- Migration `015_registered_actions.sql` (including frozen `input_json`)
- Frontend types, Tauri bindings, `invokeRegisteredAction`, approval UI, app details, grants settings
- Doctor + GitHub Actions CI
- Import authority stripping retained and documented
- Decide path re-executes frozen call exactly once

## Remaining (documented, not P0)

- Legacy `kernel_create_record` / `kernel_query_records` still exist but are permission-gated; full retirement deferred
- Agent chat path does not invent registered-action calls itself (proposals still go through Application Kernel apply)
- Richer JSON Schema keywords (`enum`, `oneOf`) not required for bundled actions
- Manual desktop QA of multi-window approval races
