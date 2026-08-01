# Registered Actions

**Status:** Implemented  
**Module:** `src-tauri/src/application_kernel/registered_actions/`

## Terminology

A **RegisteredAction** is an executable capability described by an `ActionDescriptor`. It is not a Coreside Tool (personal app).

## Descriptor fields

| Field | Role |
|---|---|
| `name` | Stable id (`local_data.write`) |
| `title` / `description` | User-facing copy (not in descriptor hash) |
| `inputSchema` | Validated before handler |
| `risk` | `read` · `write` · `destructive` |
| `critical` | Always requires fresh approval when present |
| `permissionCategory` | Must match application permissions |

Descriptor hash covers: name, input schema, risk, critical, permission category.

## Bundled actions

| Name | Risk | Permission |
|---|---|---|
| `local_data.query` | read | `local_data.read` |
| `local_data.write` | write | `local_data.write` |
| `local_data.delete` | destructive | `local_data.write` |
| `tool_state.set` | write | `local_data.write` |
| `web_search.request` | write | `web_search.request` |
| `media.read` | read | `media.read` |
| `external_link.open` | write | (validated URL only; shell opens) |
| `export.prepare` | write | `export.prepare` |
| `automation.propose` | write | `automation.propose` |
| `agent.submit_event` | write | (chat context required) |

Unknown actions fail closed. Generated UI cannot call arbitrary Tauri commands.

## Declarative invocation

```json
{
  "type": "invokeRegisteredAction",
  "actionName": "local_data.write",
  "input": { "modelId": "notes" },
  "inputFromState": { "data": "formValues" }
}
```

## Circuit breakers (protected)

- 60 calls / application / minute
- 20 writes / run
- Depth ≤ 8
- 64 KiB input / output
- Duplicate in-flight suppression
- 5 consecutive failures → temporary suspension
