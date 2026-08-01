# Generated Application Runtime

**Status:** Implemented (consumer desktop)  
**Related:** `REGISTERED_ACTIONS.md`, `APPLICATION_APPROVALS_AND_GRANTS.md`, `APPLICATION_KERNEL.md`

## Principle

Coreside turns conversations into persistent personal applications without running unrestricted generated code inside the trusted shell.

A generated application is:

1. An **ApplicationManifest** (canonical identity + lifecycle)
2. One or more **surfaces** (declarative UI)
3. Optional **data models**
4. Declared **permissions** and **registered action access**
5. Optional **automations**
6. Version history + last-known-good

## Execution flow

```text
Declarative component action (invokeRegisteredAction)
        ↓
Trusted parent (ToolRenderer → Tauri IPC)
        ↓
kernel_invoke_registered_action
        ↓
ActionRunContext (forged-safe: actor/venue/presence/session)
        ↓
execute_registered_action (single gateway)
        ↓
recovery → size/schema/depth → lifecycle → declared access
→ permission → breakers → approval/grant/policy → handler → audit
        ↓
ActionOutcome: ok | error | pendingApproval | blocked
```

Local deterministic actions (`setValue`, `toggle`, …) stay in the frontend action engine and never enter the gateway.

## Lifecycle states

`active` · `disabled` · `suspended` · `failed` · `restored` (plus soft-delete via GC)

Failed edits preserve the prior known-good version. Build failures are stored in `application_build_failures` with safe messages only.

## Recovery Mode

When Recovery Mode is on (or user surfaces are disabled):

- Gateway returns `recovery_required` for Application and Automation venue calls
- Tool canvas blocks unavailable applications
- Automations cannot execute generated privileged actions (same recovery gate)

## What is deferred

- Arbitrary generated React/JS
- External account connectors / `connectionRequired`
- Background execution while Coreside is closed
- SaaS embedding / MCP door
