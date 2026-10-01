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

## First-create bootstrap (Task Tracker path)

When a user-approved transaction creates a surface from a tool definition for the first time:

1. `permissions_from_tool` declares registered actions found on the tool (top-level components, nested children, and `props.dataSource`).
2. `ensure_manifest_for_tool_with_permissions` upserts the manifest and **auto-grants only `local_data.*`** (`grant_source: surface_create`). Higher-risk permissions may be declared but are not auto-granted.
3. `ensure_models_from_tool_actions` upserts empty typed models for every `modelId` referenced by `local_data.*` actions (`tasks` gets a Task-shaped default; other ids get a required `title` field).

CRUD still goes through `execute_registered_action` (permission + policy + approval/grant + audit). Approve-once replays use conversation/project frozen on the approval row (migration `036_approval_call_context`) so personal-tool surfaces with null `conversation_id` still match `call_hash`.

**Evidence**

- Rust vertical slice: `application_kernel::tests::task_tracker_vertical_slice_local_data_crud` (mock fixture → proposal → apply → write/approve → query).
- Desktop Journey 21: `e2e/specs/21-generated-task-tracker.spec.ts` — generate without seeding the final app, Apply, open from Personal apps, create task via `local_data.write`, Approve once, durable title visible. Results: `reports/generated-task-tracker-results.json` (`status: passed`, `seededFinalApplication: false`).

## What is deferred

- Arbitrary generated React/JS
- External account connectors / `connectionRequired`
- Background execution while Coreside is closed
- SaaS embedding / MCP door
- Secure renderer-side verification channel for UI assertions (see `GENERATED_APPLICATION_TESTING.md`)
- Full Packaged Verified Task Tracker journey (release `.app` launch-only smoke is separate; see `reports/packaged-smoke-results.json` — `launch_passed`, not Packaged Verified)
