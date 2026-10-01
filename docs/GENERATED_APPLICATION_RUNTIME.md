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

**ApplicationPlan protocol:** Providers (mock, recorded, live) emit an untrusted `applicationPlan`. Rust validates and compiles intents (`ChangeIntent` → `AppOperation`). When present, the plan is authoritative over legacy `toolChange` for operations; `toolChange` may still be derived for preview UI. Evolution uses `kind: "evolve"` (e.g. add due dates) with stable component IDs and optional `migrateDataModel`.

CRUD still goes through `execute_registered_action` (permission + policy + approval/grant + audit). Approve-once `call_hash` v2 binds conversation, venue, presence, **surfaceId**, and **componentId** (migration `036` freezes conversation/project; hash version change fail-closes stale pending approvals).

**Evidence**

- Rust: `application_kernel::application_plan`, `ai::plan_fixtures`, mock fixture → `normalized_operations`.
- Desktop Journey 21: `e2e/specs/21-generated-task-tracker.spec.ts` — ApplicationPlan create path.
- Desktop Journey 22: `e2e/specs/22-application-evolution.spec.ts` — evolve with due dates; existing record preserved.

## What is deferred

- Arbitrary generated React/JS
- External account connectors / `connectionRequired`
- Background execution while Coreside is closed
- SaaS embedding / MCP door
- Secure renderer-side verification channel for UI assertions (see `GENERATED_APPLICATION_TESTING.md`)
- Full Packaged Verified Task Tracker journey (release `.app` launch-only smoke is separate; see `reports/packaged-smoke-results.json` — `launch_passed`, not Packaged Verified)
