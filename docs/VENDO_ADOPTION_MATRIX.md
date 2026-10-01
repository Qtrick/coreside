# Vendo ↔ Coreside Adoption Matrix

Vendo is a secondary reference (archive inspected under analysis extract). Coreside remains local-first and declarative; do not clone hosted SaaS assumptions.

| Vendo mechanism | Why it exists | Coreside analogue | Adopt | Adapt | Reject | Why |
|---|---|---|---|---|---|---|
| One guard choke point | Privileged actions must not bypass a single gateway | `registered_actions` + `ActionRunContext` + Tauri commands | Yes | Keep Kernel as choke point | — | Matches Rust-authoritative model |
| Descriptors + hashing | Bind approvals to exact action identity | Descriptor registry + `call_hash` v2 | Yes | Already includes conversation/venue/presence/project/surface/component | — | Continue deepening turn/agent binding |
| Approvals / grants | Human gate for privileged work | approvals.rs / grants.rs | Yes | Lifetime/revocation semantics | Persistent destructive grants | Safe consumer defaults |
| Exact approval replay | Resume frozen call after restart | Durable approval rows | Yes (in progress) | SQLite resume without renderer stack | — | Parked interruption support |
| Interrupted turns | Human delay outlives process | Turn journal + pending approvals | Yes | Adapt to desktop restart | Hosted turn parking SaaS | Local SQLite |
| Stream resume | Identical result after disconnect | Progressive ops + catch-up | Adapt | Prove real transport/store path | Fake resume-only-reload tests as proof | Vendo test principle is good |
| Idempotency ledger | Safe retries | outbox / idempotency tables | Yes | Bounded retention cleanup | Unbounded growth | Systematize keys |
| Turn envelopes | Correlate provider work | turn_id + journal | Yes | Keep local | Multi-tenant envelopes | Consumer local-first |
| App ownership | Isolate app data | project/conversation/application lineage | Yes | DB lineage authority | Hosted org tenancy as default | Enterprise later |
| Build → validate → repair → success | Do not claim done early | ApplicationPlan → compile → apply → declarative tests → repair.rs | Yes | Bounded repair, no privilege escalation | Unbounded repair loops | Already started |
| Surface placement | Where apps live | placements / ToolCanvas / InlineSurface | Yes | Keep | Marketplace placement | Consumer personal tools |
| App version/history | Evolve safely | revisions + transactions + proposals | Yes | OCC-real evolve | — | baseRevision now required |
| App SQL isolation | Per-app data safety | generated_data_* scoped by application_id | Yes | Keep | Arbitrary app SQL | Declarative models only |
| Hosted/local store split | Cloud control plane | Hosted gateway secrets vs local SQLite | Adapt | Keep hosted keys off device | Hosted control-plane as product core | Local-first Coreside |
| MCP marketplace / arbitrary JS | Extensibility | Capability packs | — | — | Reject | Unsafe for protected webview |
| Multi-tenant hosted persistence | SaaS scale | — | — | — | Reject for consumer core | Conflicts with local-first |

## Explicit rejects

- Hosted SaaS control-plane as the primary architecture
- MCP marketplace of unrestricted packages
- Arbitrary generated JS
- Unrestricted app code execution
- Multi-tenant architecture that replaces local SQLite authority
