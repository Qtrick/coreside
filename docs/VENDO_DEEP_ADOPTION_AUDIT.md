# Vendo Deep Adoption Audit

**Product:** Coreside  
**Access date:** 2026-08-01  
**Public beta:** **NOT READY**  
**Reference:** `.reference/vendo-uploaded-2026-08-01` / `.reference/vendo` (Apache-2.0)  
**Mode:** Conceptual reimplementation only — **no Vendo source transplanted**  
**Companion:** [VENDO_REFERENCE_AND_ADOPTION_AUDIT.md](./VENDO_REFERENCE_AND_ADOPTION_AUDIT.md) · `reports/vendo-adoption-matrix.json`

## High-level decisions (beta-relevant)

| Vendo concept | Decision | Coreside mapping / rationale |
| --- | --- | --- |
| **Guard choke point** | **ADOPT** (already) | Single Rust gateway: `application_kernel/registered_actions/gateway.rs` — policy → grants → approvals → breakers → audit |
| **Grants** (scope / duration / present vs away) | **ADOPT** (already) | `grants.rs` + `context.rs` Presence/Venue; automation standing grants |
| **Breakers** | **ADOPT** (already) | `breakers.rs` — fail closed on repeated faults |
| **Approvals** (one-time, frozen input) | **ADOPT** (already) | `approvals.rs` + frozen `input_json` |
| **Component manifests** | **ADAPT** | Coreside `ApplicationManifest` + capability packs / declarative surfaces — not Vendo AppDocument or arbitrary React trees |
| **Storage declarations** | **ADAPT** | Declared permissions + generated data models / local records via kernel — not Vendo host Postgres store adapters |
| **Descriptor hashing** | **ADOPT** (already) | Canonical preimage includes permission category; excludes marketing copy |
| **Doctor / conformance** | **ADAPT** | `npm run doctor` offline checks |
| **MCP door / Composio / OAuth federation** | **REJECT** (beta) | Out of local-first consumer scope; expands trust boundary |
| **Voice** | **REJECT** (beta) | No voice I/O in consumer beta; avoid mic/privacy surface until chat+tools+settings are solid |
| **e2b / Modal / arbitrary code sandboxes** | **REJECT** | Trusted declarative components only |
| **Host OpenAPI / SaaS action sync** | **REJECT** | Desktop product, not embeddable SaaS SDK |
| **Cloud org admin / marketplace** | **REJECT** / defer | Enterprise vision only |

## Choke-point rule (non-negotiable)

All privileged generated-app side effects must pass the Rust registered-action gateway. Frontend `invoke` helpers are conveniences, not the security boundary.

## Explicit beta non-goals from Vendo

- Shipping an MCP client/server door
- Voice agent loops
- Copying Vendo UI chrome or branding
- Unrestricted executable tool registries aimed at SaaS host APIs

## Licensing

Vendo is Apache-2.0. Coreside reimplemented consent/guard concepts in Rust. Attribution: `THIRD_PARTY_NOTICES.md`.
