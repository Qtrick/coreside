# Optimistic Reconciliation

**Product:** Coreside

Deterministic local actions (`local_deterministic`): checkbox, switch, tab, local counters — update immediately, persist through Kernel/surface state, roll back on failure.

Agent-dependent actions (`agent_request`) remain explicit and never silently convert from local controls.

Destructive migrations, permission grants, and paid actions are never optimistic.
