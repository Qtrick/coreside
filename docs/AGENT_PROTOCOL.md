# Agent Protocol

Prompt version: `coreside-prompt-v1`  
Schema version: `"1"`

## Agent request (conceptual)

Built in Rust for each turn:

| Field | Description |
| --- | --- |
| system_prompt | Assembled from `prompts/*.md` + supported components + active tool |
| messages | Recent user/assistant turns from the active conversation |
| cancel | Cancellation token for in-flight provider calls |

Context limits: recent messages only (not the entire database). Active tool definition is included when a tool is selected.

## Agent response schema

```json
{
  "schemaVersion": "1",
  "assistantMessage": "Human-readable reply",
  "responseType": "message",
  "toolChange": null
}
```

Tool change:

```json
{
  "schemaVersion": "1",
  "assistantMessage": "I created a water tracker.",
  "responseType": "tool_change",
  "toolChange": {
    "action": "create",
    "targetToolId": null,
    "changeSummary": "Add water tracker",
    "tool": {
      "id": "water-tracker",
      "name": "Water Tracker",
      "description": "Tracks daily glasses of water.",
      "layout": { "type": "single-column" },
      "components": []
    }
  }
}
```

`responseType` values: `message` | `tool_change` | `noop`  
`action` values: `create` | `update` | `replace`

## Tool schema

- `id`, `name`, `description`
- `layout`: `{ "type": "..." }` (string layouts are normalized)
- `components[]`: `{ id, type, props?, children?, actions?, valueKey? }`

Stable IDs are required. Rust and the frontend generate/repair IDs when omitted or duplicated.

## Action schema

Closed set: `setValue`, `toggle`, `increment`, `decrement`, `reset`, `appendItem`, `removeItem`, `updateItem`, `selectTab`, `submitToAgent`.

`submitToAgent` emits a structured tool interaction event (tool id, component id, event name, values) rather than an opaque string.

## Validation

1. Provider returns text (prefer JSON / structured output mode)
2. Rust parser extracts JSON object
3. Schema validation rejects invalid tool changes
4. Frontend Zod re-validates before rendering
5. Unvalidated model output is never applied to the live tool

## Error recovery

| Failure | Behavior |
| --- | --- |
| Parse failure | Recover readable assistant text if possible; no tool mutation |
| Schema failure | User-facing error; keep last stable tool |
| Provider not configured | Polished setup guidance; app remains usable |
| Cancel | In-flight request aborted |

## Prompt versioning

Prompt files live in `src-tauri/prompts/`. Diagnostics include `promptVersion` for debugging without exposing secrets.

## Examples

Normal chat → `responseType: "message"`, `toolChange: null`.  
Create tool → `action: "create"`.  
Edit active tool → `action: "update"` / `"replace"` with stable `tool.id` and `targetToolId`.
