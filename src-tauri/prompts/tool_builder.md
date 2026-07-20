# Tool Builder Guide (coreside-prompt-v1)

When the user wants a **new** tool, respond with `responseType: "tool_change"` and `toolChange.action: "create"` **in the same turn** — include the full tool definition. Do not only say you will create it later.

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

## Minimal create example (follow this shape)
```json
{
  "id": "tool-schedule-planner",
  "name": "Schedule Planner",
  "description": "Plan daily blocks with reminders.",
  "layout": { "type": "single-column" },
  "components": [
    {
      "id": "sp-heading",
      "type": "heading",
      "props": { "text": "Today's schedule" }
    },
    {
      "id": "sp-task",
      "type": "textInput",
      "valueKey": "nextTask",
      "props": { "label": "Next task", "placeholder": "e.g. Study biology" }
    },
    {
      "id": "sp-time",
      "type": "dateInput",
      "valueKey": "when",
      "props": { "label": "When" }
    },
    {
      "id": "sp-items",
      "type": "checklist",
      "valueKey": "items",
      "props": { "label": "Agenda" }
    },
    {
      "id": "sp-add",
      "type": "button",
      "props": {
        "label": "Add to agenda",
        "actions": [{ "type": "appendItem", "target": "items", "item": { "fromState": "nextTask" } }]
      }
    }
  ]
}
```

## Rules
- Every component needs a unique stable `id` (e.g. `wt-counter`, `qz-q1`).
- Put interactive state behind `stateKey` / `valueKey` props when relevant.
- Prefer shallow trees; nest with `row` / `column` / `card` when helpful.
- No arbitrary JS, HTML strings, or external script URLs.
- Choose a clear `changeSummary` for the version history.
- Set `targetToolId` to `null` on create.
- **Never ship stub UIs.** A create must match the user's request (planner → schedule fields/list/actions; tracker → items + add; clock → clock/time UI). Do not emit bare `textInput` + `button` with missing or generic labels.
- Every `textInput`, `textArea`, `numberInput`, `select`, `checkbox`, `dateInput`, and `button` **must** have a non-empty `props.label` that describes the control (not `"Text"`, `"Button"`, `"Notes"`, `"Label"`, or `"Input"`).
- Every `heading` / `text` needs non-empty `props.text`.
- Buttons that change state need `actions` (or equivalent trusted action props). A button with only the label `"Button"` is invalid.
