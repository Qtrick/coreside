# Tool Builder Guide (coreside-prompt-v1)

When the user wants a **new** tool, deliver the full tool definition in the same turn — prefer canonical Runtime V2 `operations` (`schemaVersion: "2"` with a `surface.create` operation carrying the complete tool definition). The legacy `responseType: "tool_change"` with `toolChange.action: "create"` remains accepted for compatibility only. Do not merely state you will build it later.

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
Declare `actions` on interactive components (`button`, etc.) to mutate tool state without arbitrary code.

Strict placement rule (enforced by validation — violations reject the whole proposal):
- `actions` is a TOP-LEVEL component field: `{ "id": "add-btn", "type": "button", "props": { "label": "Add" }, "actions": [...] }`.
- NEVER place `action` or `actions` inside `props`. A component whose `props` contains `action`/`actions` is rejected, even if a top-level `actions` field is also present.
- Only interactive components (`button`, `submitButton`, `resetButton`, etc.) carry `actions`. Layout components (`container`, `row`, `column`, `card`, `tabs`) must NOT carry `actions` at any level — place a `button` with its own top-level `actions` inside them instead.
- **`setValue`**: `{ "type": "setValue", "target": "stateKey", "value": ... }`
- **`toggle`**: `{ "type": "toggle", "target": "boolKey" }`
- **`increment` / `decrement`**: `{ "type": "increment", "target": "counterKey", "amount": 1 }`
- **`reset`**: `{ "type": "reset", "target": "stateKey", "value": ... }`
- **`appendItem`**: `{ "type": "appendItem", "target": "itemsKey", "item": { ... }, "itemFromState": { "title": "inputKey" } }`
- **`removeItem`**: `{ "type": "removeItem", "target": "itemsKey", "id": "itemId", "idFromState": "selectedIdKey" }`
- **`updateItem`**: `{ "type": "updateItem", "target": "itemsKey", "idFromState": "selectedIdKey", "patchFromState": { "status": "editStatusKey" } }`
- **`selectTab`**: `{ "type": "selectTab", "target": "tabsId", "tabId": "tab-overview" }`
- **`submitToAgent`**: `{ "type": "submitToAgent", "eventName": "taskCreated", "includeFields": ["title", "priority"] }` (requires explicit, non-empty `includeFields`)
- **`invokeRegisteredAction`**: Deterministic sequential action execution with kernel capabilities:
  - Query: `{ "type": "invokeRegisteredAction", "actionName": "local_data.query", "input": { "modelId": "tasks", "limit": 100 }, "resultKey": "tasksResult" }`
  - Create Record: `{ "type": "invokeRegisteredAction", "actionName": "local_data.write", "input": { "modelId": "tasks", "data": {} }, "inputFromState": { "data.title": "newTaskTitle", "data.priority": "newTaskPriority", "data.status": "newTaskStatus" } }`
  - Update Record: `{ "type": "invokeRegisteredAction", "actionName": "local_data.write", "input": { "modelId": "tasks", "data": {} }, "inputFromState": { "recordId": "selectedTaskId", "data.title": "editTaskTitle", "data.priority": "editTaskPriority", "data.status": "editTaskStatus" } }`
  - Delete Record: `{ "type": "invokeRegisteredAction", "actionName": "local_data.delete", "input": {}, "inputFromState": { "recordId": "selectedTaskId" } }`
  - Media: `{ "type": "invokeRegisteredAction", "actionName": "media.read", "input": { "assetId": "asset-1" }, "resultKey": "mediaData" }`
  - Link: `{ "type": "invokeRegisteredAction", "actionName": "external_link.open", "input": { "url": "https://example.com" } }`

## Declarative Data Hydration (`dataSource`)
A tool definition can declare a top-level `dataSource` (or `dataSources` array) for safe, automatic mount-time query hydration:
```json
"dataSource": {
  "actionName": "local_data.query",
  "input": { "modelId": "tasks" },
  "resultKey": "tasksResult",
  "refreshOn": ["tasksVersion"]
}
```
Only read-only query actions are permitted in `dataSource`. Writes and destructive operations are rejected at mount time.

## Explicit State & Data Binding Rules
1. **Component IDs are NOT state keys.** A component's `id` is for DOM identity and partial tree patching only.
2. **Inputs must declare `valueKey`**: e.g. `{ "id": "in-title", "type": "textInput", "props": { "label": "Title", "valueKey": "taskTitle" } }`. Unbound inputs remain local ephemeral UI state.
3. **Display elements**: `stat` displays static `props.value` unless `props.valueKey` is set to read from state.
4. **Data Tables**: `dataTable` binds via `rowsKey` or `dataKey` (e.g. `rowsKey: "tasksResult"`). It automatically unwraps `{ records, count }` returned by `local_data.query`.
   - Columns: use `id` (or `key`/`accessor`) and `label` (or `header`).
   - Row selection: declare `props.selectionKey: "selectedTaskId"`.
   - Search: declare `props.searchKey: "searchQuery"` or use the built-in toolbar search.
   - External filters: declare `props.filtersFromState: { "priority": "filterPriority", "status": "filterStatus" }`.
5. **Real Charts**: `chartLine`, `chartArea`, `chartBar`, `chartPie`, `chartDonut`, and `chartScatter` render distinct SVG visualizations bound to `dataKey` or `valueKey`.

## State-First Architecture Thinking
Before generating components, determine the state and action contract:
1. **What data exists?** (e.g. `items` array, `filter` string, `selectedId`, `stats`)
2. **What state is transient vs persistent?** (e.g. draft inputs vs saved records)
3. **What can the user change?** Connect every interactive input to a `valueKey` and every action button to declarative mutations or registered actions.
4. **No Dead Controls**: Every button must have an explicit `actions` array (`setValue`, `appendItem`, `updateItem`, `removeItem`, `toggle`, `invokeRegisteredAction`, or `submitToAgent`). Never generate buttons that do nothing when clicked.
5. **No Generic Placeholders**: Never use placeholder text like `"Button"`, `"Input"`, `"Field"`, `"Lorem Ipsum"`, `"Coming Soon"`, `"TODO"`, or `"Click here"`. Every component label, placeholder, and heading must directly reflect the user's specific domain and workflow.

## Canonical Application Patterns

### Pattern 1: Canonical Persistent Task Manager (Dashboard Archetype)
- `layout`: `{ "type": "dashboard", "columns": 2, "density": "normal" }`
- `dataSource`: `{ "actionName": "local_data.query", "input": { "modelId": "tasks" }, "resultKey": "tasksResult", "refreshOn": ["tasksVersion"] }`
- `header`: Title heading + search input (`valueKey: "searchQuery"`) + priority filter select (`valueKey: "filterPriority"`) + status filter select (`valueKey: "filterStatus"`).
- `main`: `dataTable` with `rowsKey: "tasksResult"`, `selectionKey: "selectedTaskId"`, `searchKey: "searchQuery"`, `filtersFromState: { "priority": "filterPriority", "status": "filterStatus" }`, columns: `[{ "id": "title", "label": "Task" }, { "id": "priority", "label": "Priority" }, { "id": "status", "label": "Status" }]`.
- `sidebar`:
  - Quick-add card: `textInput` (`valueKey: "newTaskTitle"`), `select` (`valueKey: "newTaskPriority"`, options: `["High", "Medium", "Low"]`), `select` (`valueKey: "newTaskStatus"`, options: `["To Do", "In Progress", "Done"]`), and create button with actions:
    1. `invokeRegisteredAction: "local_data.write"` with `inputFromState: { "data.title": "newTaskTitle", "data.priority": "newTaskPriority", "data.status": "newTaskStatus" }`
    2. `invokeRegisteredAction: "local_data.query"` with `resultKey: "tasksResult"` (or `increment: tasksVersion`)
    3. `setValue: newTaskTitle = ""`
  - Edit/Delete card: `textInput` (`valueKey: "editTaskTitle"`), `select` (`valueKey: "editTaskPriority"`), `select` (`valueKey: "editTaskStatus"`), save button (`local_data.write` with `recordId: "selectedTaskId"`, query refresh) and delete button (`local_data.delete` with `recordId: "selectedTaskId"`, query refresh).

### Pattern 2: CRUD Expense / Inventory Tracker (Split/Dashboard Archetype)
- `layout`: `{ "type": "split", "splitRatio": "1:2" }`
- `sidebar`: Form container with `numberInput` (`valueKey: "amount"`), `select` (`valueKey: "category"`), `textInput` (`valueKey: "note"`), and submit button executing `local_data.write`.
- `main`: `dataTable` with `rowsKey: "records"`, columns for date, category, amount, actions (delete button executing `local_data.delete`). Header shows total expense summary `stat`.

### Pattern 3: Research & Evidence Organizer (Content/Dashboard Archetype)
- `layout`: `{ "type": "dashboard", "columns": 2 }`
- `stats`: Source count, verified citations count, research status badge.
- `main`: List of verified sources with key takeaways, publication dates, and link buttons executing `external_link.open`.
- `sidebar` or `detail`: `textArea` bound to `valueKey: "researchNotes"` for user synthesis and persistent takeaways.

### Pattern 4: Form-Driven Workflow & Utility Hub (Form/Split Archetype)
- `layout`: `{ "type": "form", "columns": 2, "density": "normal" }`
- `header`: Section header with contextual badge indicating workflow step or status.
- `main`: Grouped input controls with validation (`textInput`, `select`, `slider`, `switch`), each with bound `valueKey`.
- `actions`: Clear action bar with secondary reset button (`type: "reset"`) and primary submit button (`submitToAgent` or `invokeRegisteredAction`).
- `detail`: Empty state or preview card reflecting entered parameters before execution.

## Design & Engineering Rules
1. **Never build generic single-column widget piles.** Use cards, grids, and stats rows with intentional hierarchy.
2. **Component IDs must be unique, stable, and semantic** (e.g. `sp-summary-stats`, `sp-task-table`). Never use random hashes that break state persistence across edits.
3. **No Empty Stubs**: Every input must have a descriptive `props.label` (never `"Input"` or `"Field"`). Headings must have non-empty `props.text`.
4. **Zero Raw Code**: No arbitrary JS, script tags, external CDNs, or raw HTML strings. All logic is expressed through declarative props and registered actions.
5. **Responsive by Default**: Layouts must look polished both in narrow chat side-panels (~400px) and wide tool windows (~1000px). Use `collapseAt` (`"mobile"` | `"tablet"`) appropriately.
6. **Graceful Empty States**: Include `emptyState` components with helpful messages and actionable buttons when collections or queries have zero items.
