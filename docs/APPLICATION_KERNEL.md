# Application Kernel

**Product:** Coreside  
**Code:** `src-tauri/src/application_kernel/`

Trusted gateway for durable generated-application mutations. Agent-facing apply paths should prefer `apply_change` over calling Runtime V2 helpers directly.

## Gateway: `apply_change`

Entry: `application_kernel::apply_change(db, ChangeRequest)`.

1. `validate_operations` (Runtime V2)
2. Protected-boundary checks (`assert_ops_not_protected`)
3. Risk classification + impact summary + policy evaluation
4. If approval required and not granted → return proposal (no apply)
5. Create Runtime V2 transaction
6. Apply kernel-owned ops: `data::apply_kernel_operations`, `manifest::apply_manifest_operations`
7. `apply_transaction` for surface/component/state ops
8. Record provenance; run post-change verification; mark last-known-good when verified

`ChangeResult` includes apply result (or `proposal_id`), `risk`, `impact_summary`, `policy`, and optional `verification`.

## Persistence

| Layer | Role |
| --- | --- |
| Runtime V2 transactions | Surface/component/state ops, undo snapshots |
| Manifest ops | `manifest.upsert`, restore LKG, disable |
| Data ops | Models, records, migrations (namespaced CRUD) |

## Protected boundaries

Rejected in the gateway:

- Protected tool / surface / application IDs
- `permission.grant` from agent ops
- Forbidden / `unrestricted.*` permissions in payloads
- Arbitrary SQL (`data.execute_sql` or `sql` in payload)

Recovery Mode cannot be disabled via agent operations (`recovery::assert_agent_cannot_disable_recovery`, wired into the gateway). Agent UI mutations are blocked while Recovery Mode / disable-user-surfaces is active.
## Capability catalog

`capability_catalog()` exposes compiler/manifest versions, packs, allowed/forbidden permissions, operation families, and `limits.rs` ceilings — agent cannot raise limits.

## Related docs

- [APPLICATION_MANIFEST.md](./APPLICATION_MANIFEST.md)
- [CHANGE_COMPILER.md](./CHANGE_COMPILER.md)
- [CHANGE_PROPOSALS.md](./CHANGE_PROPOSALS.md)
- [GENERATIVE_INTERFACE_RUNTIME_V2.md](./GENERATIVE_INTERFACE_RUNTIME_V2.md)
