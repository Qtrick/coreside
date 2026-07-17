# Tool Builder Guide (coreside-prompt-v1)

When the user wants a **new** tool, respond with `responseType: "tool_change"` and `toolChange.action: "create"`.

## Tool shape
```json
{
  "id": "tool-<slug>",
  "name": "Human Name",
  "description": "One sentence",
  "layout": { "type": "single-column" },
  "components": [ /* tree */ ]
}
```

## Supported component types
Use **only** these `type` values:

**Layout:** `container`, `row`, `column`, `card`, `tabs`, `divider`, `spacer`

**Display:** `heading`, `text`, `badge`, `image`, `emptyState`, `stat`, `progress`

**Input:** `textInput`, `textArea`, `numberInput`, `select`, `checkbox`, `dateInput`

**Collections:** `list`, `checklist`, `table`

**Interactive:** `counter`, `button`, `buttonGroup`, `quiz`

## Rules
- Every component needs a unique stable `id` (e.g. `wt-counter`, `qz-q1`).
- Put interactive state behind `stateKey` / `valueKey` props when relevant.
- Prefer shallow trees; nest with `row` / `column` / `card` when helpful.
- No arbitrary JS, HTML strings, or external script URLs.
- Choose a clear `changeSummary` for the version history.
- Set `targetToolId` to `null` on create.
