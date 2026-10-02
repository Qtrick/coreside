# Vendo ↔ Coreside Adoption Matrix

Vendo is a secondary reference (archive sha256 `c667ff74…`). Coreside remains local-first and declarative; do not clone hosted SaaS assumptions.

| Vendo mechanism | Why it exists | Coreside analogue | Adopt | Adapt | Reject | Why / Evidence |
|---|---|---|---|---|---|---|
| One guard choke point | Privileged actions must not bypass a single gateway | `registered_actions` + `ActionRunContext` + Tauri commands | Yes | Keep Kernel as choke point | — | Matches Rust-authoritative model |
| Descriptors + hashing | Bind approvals to exact action identity | Descriptor registry + `call_hash` v2 | Yes | Already includes conversation/venue/presence/project/surface/component | — | Continue deepening turn/agent binding |
| Approvals / grants | Human gate for privileged work | approvals.rs / grants.rs | Yes | Lifetime/revocation semantics | Persistent destructive grants | Safe consumer defaults |
| Exact approval replay | Resume frozen call after restart | Durable approval rows | Yes | SQLite resume without renderer stack | — | Parked interruption; desktop restart journey still open |
| Interrupted turns | Human delay outlives process | Turn journal + pending approvals | Yes | Adapt to desktop restart | Hosted turn parking SaaS | Local SQLite |
| Stream resume | Identical result after disconnect | Progressive ops + catch-up | Adapt | Prove real transport/store path | Fake resume-only-reload tests as proof | Unit catch-up + cursor stale/future/dup; Journey 24–25 desktop wired; multi-window Cases H–I open |
| Forming preview | Safe geometry while building | `runtime_v2/preview_transaction.rs` + `progressive_ops.rs` (+ `app-store` preview overlay) | Adapt | Speculative ≠ durable; `mark_interrupted` discards | Value-leaking HTML forming; Vendo `apps/persistence/forming.ts` in-memory only | `preview_transaction` unit tests; Journey 19–20 e2e |
| Idempotency ledger | Safe retries | outbox / idempotency tables | Yes | Bounded retention cleanup | Unbounded growth | Systematize keys |
| Turn envelopes | Correlate provider work | turn_id + journal | Yes | Keep local | Multi-tenant envelopes | Consumer local-first |
| App ownership | Isolate app data | project/conversation/application lineage | Yes | DB lineage + ApplicationPlan exclusive channel; fail-closed scope; fork remaps conversation-touched apps + tools.id; resolve_bound_surface_for_tool_cmd | Hosted org tenancy as default | compile_plan_against_db; Journey 27 branch isolation desktop |
| Build → validate → repair → success | Do not claim done early | ApplicationPlan → compile → apply → declarative tests → repair.rs | Yes | Bounded repair, no privilege escalation | Unbounded repair loops | Lifecycle UX: Testing before Ready |
| Surface placement | Where apps live | placements / ToolCanvas / InlineSurface | Yes | Keep | Marketplace placement | Consumer personal tools |
| App version/history | Evolve safely | revisions + transactions + proposals + surface_diff | Yes | OCC-real granular evolve | — | baseRevision required; ApplicationSpec diff |
| App SQL isolation | Per-app data safety | generated_data_* scoped by application_id | Yes | Keep | Arbitrary app SQL | Declarative models only |
| Hosted/local store split | Cloud control plane | Hosted gateway secrets vs local SQLite | Adapt | Keep hosted keys off device | Hosted control-plane as product core | Local-first Coreside |
| MCP marketplace / arbitrary JS | Extensibility | Capability packs | — | — | Reject | Unsafe for protected webview |
| Multi-tenant hosted persistence | SaaS scale | — | — | — | Reject for consumer core | Conflicts with local-first |
| In-memory SSE resume registries | Dev-server hot reload | — | — | — | Reject | Use SQLite catch-up |

## Explicit rejects

- Hosted SaaS control-plane as the primary architecture
- MCP marketplace of unrestricted packages
- Arbitrary generated JS
- Unrestricted app code execution
- Multi-tenant architecture that replaces local SQLite authority
- DB-less privileged ApplicationPlan compilation as Apply authority
- Client-secret form/WebSocket authority
