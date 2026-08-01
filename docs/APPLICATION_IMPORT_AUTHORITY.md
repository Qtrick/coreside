# Application Import Authority

**Status:** Implemented  
**Principle (Vendo-inspired):** An exported artifact carries structure and data, not authority.

## Export

Packages must not include:

- Active or pending approvals
- Standing / session / once grants
- Approval claim receipts
- Session or run identifiers as authority
- Keychain handles / provider credentials
- Machine-local secret paths

`packages.rs` strips authority-shaped keys (`grants`, `approvals`, `runtime_grants`, …).

## Import

1. Preview package (permissions, capabilities, warnings)
2. Validate size/shape/content
3. Remint identifiers when requested
4. Require user approval to import
5. Start with **no** runtime grants
6. Declared permissions still need local grant
7. Action access must revalidate against the current registry

## Adversarial expectations

Importing a package that embeds grant/approval blobs must strip them and warn. Oversized, executable, or secret-shaped manifests fail validation.
