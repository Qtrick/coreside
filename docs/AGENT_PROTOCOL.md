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
  "toolChange": null,
  "settingsChange": null
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

Settings change (theme / accents / backgrounds / live wallpaper):

```json
{
  "schemaVersion": "1",
  "assistantMessage": "Enabled a Matrix-style live wallpaper.",
  "responseType": "settings_change",
  "settingsChange": {
    "theme": "dark",
    "background": { "light": "#f5f6f1", "dark": "#050805" },
    "wallpaper": {
      "kind": "matrix",
      "color": "#33ff66",
      "speed": 1.15,
      "density": 0.7,
      "opacity": 0.42
    },
    "changeSummary": "Matrix live wallpaper"
  }
}
```

Allowlisted wallpaper kinds: `none` | `matrix` | `aurora` | `particles` | `rain` | `pulse`.

## Agent tool capabilities (bounded loop)

`send_message` supports `responseType: "tool_use"` with `toolCalls[]`. Coreside runs a trusted Rust tool loop (max **8** steps per round, max **2** outer rounds), then calls the model again with results. Cancellation uses the same `CancellationToken` as generation.

| Capability | Description |
| --- | --- |
| `project_context_search` | FTS snippets within active project |
| `get_project_summary` | Instructions + summary |
| `web_search` / `image_search` / `video_search` | Local Web Research (Crawl4AI sidecar) via settings `safeSearch`; requires engine Ready |
| `fetch_web_page` | Prefer Crawl4AI crawl; SSRF-safe HTTP fallback |
| `inspect_media_result` | Media library asset metadata (project-scoped) |
| `import_media_asset` | Returns `pendingApproval: true` — surfaced in chat for explicit user confirm |
| `no_action` | Explicit noop |

Schemas: `capability_registry.rs`. Per-step timeout 30s. Production research never falls back to a cloud search API or mock results.

`responseType` values: `message` | `tool_change` | `settings_change` | `tool_use` | `noop`  
`action` values: `create` | `update` | `replace`

## Tool schema

- `id`, `name`, `description`
- `layout`: `{ "type": "..." }` (string layouts are normalized)
- `components[]`: `{ id, type, props?, children?, actions?, valueKey? }`

Stable IDs are required. Rust and the frontend generate/repair IDs when omitted or duplicated.

## Action schema

Closed set: `setValue`, `toggle`, `increment`, `decrement`, `reset`, `appendItem`, `removeItem`, `updateItem`, `selectTab`, `submitToAgent`.

`submitToAgent` emits a structured tool interaction event (tool id, component id, event name, values) rather than an opaque string.

## Protected targets

Any operation whose ID starts with `core.` or matches the protected-resource registry is rejected before persistence with a clear user-facing error.

The agent may create **Added Settings** (tool-owned) and may change **theme / accent / background / border / text / live wallpaper preferences** via `settings_change`. It must never modify logos, dock icons, or Base Settings structure.

## Added Settings (user-editable)

Persisted via `list_added_settings` / `upsert_added_setting` / `delete_added_setting`.

Required fields: `id`, `ownerToolId`, `label`, `type` (`boolean` | `text` | `number` | `select` | `color` | `date`), default/current values.

IDs must not be protected (`core.*`).

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
Theme / colors / backgrounds / borders / text / live wallpapers → `responseType: "settings_change"` (wallpaper kinds are allowlisted presets).
