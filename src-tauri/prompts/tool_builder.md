# Tool Builder Guide (coreside-prompt-v2)

When the user wants a **new** tool, respond with `responseType: "tool_change"` and `toolChange.action: "create"` in the same turn — include the full tool definition. Do not merely state you will build it later.

## Tool Structure
```json
{
  "id": "tool-<slug>",
  "name": "Human Readable Name",
  "description": "Clear concise summary of what this tool does",
  "layout": {
    "type": "dashboard | form | grid | split | content | full | stack",
    "columns": 2,
    "gap": "sm | md | lg",
    "maxWidth": "sm | md | lg | xl | full",
    "density": "compact | normal | comfortable"
  },
  "components": [ /* tree */ ]
}
```

## Semantic Layout Archetypes
Select the layout matching the user's task before choosing components:
- **`dashboard`**: Multi-section productivity surface. Use `layoutRole` on top-level components: `"header"` (title/controls), `"stats"` (stat cards), `"main"` (primary chart/data), `"sidebar"` or `"detail"` (secondary panel), `"footer"` (actions).
- **`form`**: Structured data entry. Fields auto-arrange into 2 responsive columns collapsing to 1 column on narrow screens.
- **`grid`**: Uniform or multi-span layout. Set `columns` (1–6). Components can set `colSpan` (1–12) and `rowSpan` (1–6).
- **`split`**: 2-pane split interface (e.g. `splitRatio: "1:2"` for sidebar + main view).
- **`content`**: Centered reading width (`maxWidth: "md"`), ideal for study/quiz tools and research notes.
- **`full`**: Edge-to-edge canvas, ideal for large data tables, canvases, and code editors.
- **`stack`**: Sequential top-to-bottom flow.

## Available Component Catalog
Use **only** these verified capability primitives:

1. **Layout & Grouping**: `container`, `row`, `column`, `card`, `tabs`, `divider`, `spacer`
2. **Display & Metrics**: `heading`, `text`, `badge`, `image`, `emptyState`, `stat`, `progress`, `svgScene`, `svgRect`, `svgCircle`, `svgEllipse`, `svgLine`, `svgPath`, `svgText`, `svgGroup`, `mathInline`, `mathBlock`
3. **Rich Forms & Inputs**: `form`, `fieldGroup`, `textInput`, `textArea`, `numberInput`, `select`, `checkbox`, `radioGroup`, `switch`, `slider`, `dateInput`, `timeInput`, `dateTimeInput`, `colorInput`, `filePicker`, `mediaPicker`, `submitButton`, `resetButton`, `validationMessage`
4. **Data & Advanced**: `table`, `dataTable`, `chartLine`, `chartBar`, `chartPie`, `chartDonut`, `chartArea`, `chartScatter`, `list`, `checklist`, `codeEditor`, `canvasScene`, `clock`, `counter`, `quiz`, `audioPlayer`
5. **Interactive Controls**: `button`, `buttonGroup`

## Component Actions & CRUD State Patterns
Declare `actions` on interactive components (`button`, `card`, etc.) to mutate tool state without arbitrary code:
- **`setValue`**: `{ "type": "setValue", "target": "stateKey", "value": ... }`
- **`toggle`**: `{ "type": "toggle", "target": "boolKey" }`
- **`increment` / `decrement`**: `{ "type": "increment", "target": "counterKey", "amount": 1 }`
- **`reset`**: `{ "type": "reset", "target": "stateKey", "value": ... }`
- **`appendItem`**: `{ "type": "appendItem", "target": "itemsKey", "item": { "id": "...", "title": "..." } }`
- **`removeItem`**: `{ "type": "removeItem", "target": "itemsKey", "id": "itemId" }`
- **`updateItem`**: `{ "type": "updateItem", "target": "itemsKey", "id": "itemId", "patch": { "status": "done" } }`
- **`selectTab`**: `{ "type": "selectTab", "target": "tabsId", "tabId": "tab-overview" }`
- **`invokeRegisteredAction`**: `{ "type": "invokeRegisteredAction", "actionName": "kernel.action", "input": { ... }, "resultKey": "outputStateKey" }`
- **Dynamic Table Data**: `dataTable` and `table` dynamically read records from `state[valueKey]` or `state[props.dataKey]` or `state[props.rowsKey]` before falling back to `props.rows`.

## Design & Engineering Rules
1. **Never build generic single-column widget piles.** Use cards, grids, and stats rows with intentional hierarchy.
2. **Component IDs must be unique, stable, and semantic** (e.g. `fn-monthly-chart`, `hb-streak-stat`). Never use random hashes that break state persistence across edits.
3. **State Binding**: Interactive inputs must bind via `valueKey` or `props.valueKey`. Buttons that alter state must declare explicit `actions`.
4. **No Empty Stubs**: Every input must have a descriptive `props.label` (never `"Input"` or `"Field"`). Headings must have non-empty `props.text`.
5. **Zero Raw Code**: No arbitrary JS, script tags, external CDNs, or raw HTML strings. All logic is expressed through declarative props and registered actions.
6. **Responsive by Default**: Layouts must look polished both in narrow chat side-panels (~400px) and wide tool windows (~1000px).
