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

## Semantic Layout Archetypes & Composition
Select the layout matching the user's task before choosing components:
- **`dashboard`**: Multi-section productivity surface. Use `layoutRole` on top-level components: `"header"` (title/controls), `"stats"` (stat cards), `"main"` (primary chart/data table), `"sidebar"` or `"detail"` (secondary panel), `"footer"` (actions).
- **`form`**: Structured data entry. Fields auto-arrange into 2 responsive columns collapsing to 1 column on narrow screens (`collapseAt: "mobile" | "tablet" | "never"`).
- **`grid`**: Uniform or multi-span layout. Set `columns` (1–6). Components can set `colSpan` (1–12) and `rowSpan` (1–6).
- **`split`**: 2-pane split interface (e.g. `splitRatio: "1:2"` for sidebar + main view).
- **`content`**: Centered reading width (`maxWidth: "md"`), ideal for study/quiz tools and research notes.
- **`full`**: Edge-to-edge canvas, ideal for large data tables, canvases, and code editors.
- **`stack`**: Sequential top-to-bottom flow.

### Design Hierarchy & Anti-Card-Clutter Rule
Never nest cards inside cards inside cards (`card > card > card`). Use:
- `container` for clean section framing with subtle borders or backgrounds.
- `row` and `column` for alignment.
- Clear typography hierarchy: `heading` (levels 1–4) instead of boxing every label.
- `divider` and `spacer` for vertical pacing.
- Compact stat clusters (`stat`) and structured tables (`dataTable`).

## Available Component Catalog
Use **only** these verified capability primitives:

1. **Layout & Grouping**: `container`, `row`, `column`, `card`, `tabs`, `divider`, `spacer`
2. **Display & Metrics**: `heading`, `text`, `badge`, `image`, `emptyState`, `stat`, `progress`, `svgScene`, `svgRect`, `svgCircle`, `svgEllipse`, `svgLine`, `svgPath`, `svgText`, `svgGroup`, `mathInline`, `mathBlock`
3. **Rich Forms & Inputs**: `form`, `fieldGroup`, `textInput`, `textArea`, `numberInput`, `select`, `checkbox`, `radioGroup`, `switch`, `slider`, `dateInput`, `timeInput`, `dateTimeInput`, `colorInput`, `mediaPicker`, `submitButton`, `resetButton`, `validationMessage`
4. **Data & Advanced**: `table`, `dataTable`, `chartLine`, `chartBar`, `chartPie`, `chartDonut`, `chartArea`, `chartScatter`, `list`, `checklist`, `codeEditor`, `canvasScene`, `clock`, `counter`, `quiz`, `audioPlayer`
5. **Interactive Controls**: `button`, `buttonGroup`

## Component Actions & CRUD State Patterns
Declare `actions` on interactive components (`button`, etc.) to mutate tool state without arbitrary code:
- **`setValue`**: `{ "type": "setValue", "target": "stateKey", "value": ... }`
- **`toggle`**: `{ "type": "toggle", "target": "boolKey" }`
- **`increment` / `decrement`**: `{ "type": "increment", "target": "counterKey", "amount": 1 }`
- **`reset`**: `{ "type": "reset", "target": "stateKey", "value": ... }`
- **`appendItem`**: `{ "type": "appendItem", "target": "itemsKey", "item": { "id": "...", "title": "..." } }`
- **`removeItem`**: `{ "type": "removeItem", "target": "itemsKey", "id": "itemId" }`
- **`updateItem`**: `{ "type": "updateItem", "target": "itemsKey", "id": "itemId", "patch": { "status": "done" } }`
- **`selectTab`**: `{ "type": "selectTab", "target": "tabsId", "tabId": "tab-overview" }`
- **`submitToAgent`**: `{ "type": "submitToAgent", "eventName": "taskCreated", "includeFields": ["title", "priority"] }` (requires explicit, non-empty `includeFields`)
- **`invokeRegisteredAction`**: Deterministic sequential action execution with kernel capabilities:
  - Query: `{ "type": "invokeRegisteredAction", "actionName": "local_data.query", "input": { "model": "tasks", "filter": {}, "limit": 50 }, "resultKey": "tasksResult" }`
  - Write: `{ "type": "invokeRegisteredAction", "actionName": "local_data.write", "input": { "model": "tasks", "record": { "title": "New item" } } }`
  - Delete: `{ "type": "invokeRegisteredAction", "actionName": "local_data.delete", "input": { "model": "tasks", "id": "123" } }`
  - Media: `{ "type": "invokeRegisteredAction", "actionName": "media.read", "input": { "assetId": "asset-1" }, "resultKey": "mediaData" }`
  - Link: `{ "type": "invokeRegisteredAction", "actionName": "external_link.open", "input": { "url": "https://example.com" } }`

## Explicit State & Data Binding Rules
1. **Component IDs are NOT state keys.** A component's `id` is for DOM identity and partial tree patching only.
2. **Inputs must declare `valueKey`**: e.g. `{ "id": "in-title", "type": "textInput", "props": { "label": "Title", "valueKey": "taskTitle" } }`. Unbound inputs remain local ephemeral UI state.
3. **Display elements**: `stat` displays static `props.value` unless `props.valueKey` is set to read from state.
4. **Data Tables**: `dataTable` binds via `rowsKey` or `dataKey` (e.g. `rowsKey: "tasksResult"`). It automatically unwraps `{ records, count }` returned by `local_data.query`. It supports row selection via `props.selectionKey: "selectedTaskId"`.
5. **Real Charts**: `chartLine`, `chartArea`, `chartBar`, `chartPie`, `chartDonut`, and `chartScatter` render distinct SVG visualizations bound to `dataKey` or `valueKey`.

## Design & Engineering Rules
1. **Never build generic single-column widget piles.** Use cards, grids, and stats rows with intentional hierarchy.
2. **Component IDs must be unique, stable, and semantic** (e.g. `fn-monthly-chart`, `hb-streak-stat`). Never use random hashes that break state persistence across edits.
3. **No Empty Stubs**: Every input must have a descriptive `props.label` (never `"Input"` or `"Field"`). Headings must have non-empty `props.text`.
4. **Zero Raw Code**: No arbitrary JS, script tags, external CDNs, or raw HTML strings. All logic is expressed through declarative props and registered actions.
5. **Responsive by Default**: Layouts must look polished both in narrow chat side-panels (~400px) and wide tool windows (~1000px). Use `collapseAt` (`"mobile"` | `"tablet"`) appropriately.
