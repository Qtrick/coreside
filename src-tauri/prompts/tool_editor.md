# Tool Editor Guide (coreside-prompt-v2)

An **active tool** is provided below as JSON. Prefer targeted updates (`schemaVersion: "2"` with granular `operations`) over regenerating the full tree. Full regeneration destroys user focus, scroll position, and transient draft edits.

## Targeted Mutation Model (Partial Update Parity)

When modifying, tuning, restyling, or adding to an existing tool, prefer emitting `schemaVersion: "2"` with targeted `operations`. Only target the specific components that need to change.

### Available Operation Types
- **`component.update_props`**: Modify labels, styles, props, or options on an existing component by `componentId`. All sibling components, hierarchy, and state remain completely untouched.
- **`component.insert`**: Insert a new component under `parentId` at a specific `index` without recreating existing children.
- **`component.remove`**: Delete an existing component by `componentId`.
- **`component.replace`**: Replace a single component subtree with an updated component.
- **`state.patch`**: Update initial tool state keys (`valueKey`) without modifying the component hierarchy.

### Example 1: Targeted Property Update (Changing a label, style, or options)
```json
{
  "schemaVersion": "2",
  "responseType": "message",
  "assistantMessage": "Updated the button label to 'Save Note' and changed its variant to primary.",
  "operations": [
    {
      "type": "component.update_props",
      "surfaceId": "<active-tool-id>",
      "componentId": "btn-submit",
      "props": {
        "label": "Save Note",
        "variant": "primary"
      }
    }
  ]
}
```

### Example 2: Targeted Component Insertion (Adding a field to a form)
```json
{
  "schemaVersion": "2",
  "responseType": "message",
  "assistantMessage": "Added a tags input field to the form.",
  "operations": [
    {
      "type": "component.insert",
      "surfaceId": "<active-tool-id>",
      "parentId": "form-container",
      "index": 2,
      "component": {
        "id": "in-tags",
        "type": "textInput",
        "props": {
          "label": "Tags",
          "placeholder": "biology, research, exam",
          "valueKey": "taskTags"
        }
      }
    }
  ]
}
```

### Example 3: Full Tree Replacement (`tool_change`)
Use `tool_change` **only** when:
1. The user explicitly requests a complete overhaul or redesign from scratch.
2. The fundamental layout archetype changes (e.g. converting a stack into a multi-column dashboard).
When using `tool_change`:
- Always set `targetToolId` to the active tool id.
- Set `action: "update"`.
- Keep existing component `id`s and `valueKey`s for any components whose data should survive.

## Edit & Redesign Rules
1. **Preserve Component IDs & State Keys**: Persistent state depends on stable IDs. If you rearrange or restyle components, keep existing IDs so the user's data is never lost.
2. **Intentional Layouts**: If changing layout or making a tool more compact/responsive, update `tool.layout`:
   - `dashboard` (with `layoutRole`: `"header"`, `"stats"`, `"main"`, `"sidebar"`, `"detail"`, `"footer"`)
   - `form` (responsive 2-column input fields)
   - `grid` (with `columns`, `colSpan`, `rowSpan`)
   - `split` (with `splitRatio`)
   - `content`, `full`, or `stack`
3. **Full Primitive Catalog**:
   - Layout: `container`, `row`, `column`, `card`, `tabs`, `divider`, `spacer`
   - Display: `heading`, `text`, `badge`, `image`, `emptyState`, `stat`, `progress`, `svgScene`, `svgRect`, `svgCircle`, `svgEllipse`, `svgLine`, `svgPath`, `svgText`, `svgGroup`, `mathInline`, `mathBlock`
   - Forms: `form`, `fieldGroup`, `textInput`, `textArea`, `numberInput`, `select`, `checkbox`, `radioGroup`, `switch`, `slider`, `dateInput`, `timeInput`, `dateTimeInput`, `colorInput`, `mediaPicker`, `submitButton`, `resetButton`, `validationMessage`
   - Data & Advanced: `table`, `dataTable`, `chartLine`, `chartBar`, `chartPie`, `chartDonut`, `chartArea`, `chartScatter`, `list`, `checklist`, `codeEditor`, `canvasScene`, `clock`, `counter`, `quiz`, `audioPlayer`
   - Actions: `button`, `buttonGroup` (with declarative `actions`: `setValue`, `toggle`, `increment`, `decrement`, `reset`, `appendItem`, `removeItem`, `updateItem`, `selectTab`, `submitToAgent`, `invokeRegisteredAction`)
4. **Explicit State & Data Bindings**:
   - Component IDs are NOT state keys. Inputs must declare `valueKey` (e.g. `valueKey: "myKey"`).
   - Display components (`stat`, `heading`) read from state if `valueKey` is set; otherwise show static `props.value`.
   - Data tables (`dataTable`) bind via `rowsKey` or `dataKey` and automatically consume `{ records, count }` from `local_data.query`.
   - `submitToAgent` requires explicit non-empty `includeFields: [...]`.
5. **No Placeholders & No Dead Controls**: Every button must have meaningful actions. Every input must have a `valueKey` and descriptive domain label (never "Button", "Input", "Field", "TODO"). Never introduce arbitrary JavaScript or raw HTML.
6. **Anti-Card Clutter**: Avoid nesting cards in cards (`card > card`). Use `container`, `row`, `column`, `divider`, and typography hierarchy.
7. **Preserve User Data During Edits**: Maintain existing `valueKey`s and component IDs so user input and records survive layout or styling updates.
