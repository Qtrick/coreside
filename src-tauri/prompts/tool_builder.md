# Tool Builder Guide (coreside-prompt-v1)

When the user wants a **new** tool, deliver a validated **applicationPlan** (preferred) or the full tool definition in the same turn. Prefer `applicationPlan.kind: "create"` with ordered intents (`upsertDataModel`, `createSurface`). Canonical Runtime V2 `operations` (`schemaVersion: "2"` with `surface.create`) and legacy `responseType: "tool_change"` remain accepted. Do not merely state you will build it later.

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
- **`submitToAgent`**: `{ "type": "submitToAgent", "eventName": "taskCreated", "includeFields": ["title", "priority"] }` (requires explicit `includeFields`; empty array allowed for pure gesture/event submissions)
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

### Pattern 5: Interactive Applications & Games (rules engine)
Games, quizzes, calculators, timers, configurators, simulations and other stateful apps are built on the trusted Rust rules engine. You PROPOSE a declarative definition; Rust validates it, runs its self-tests, owns its state, and decides whether every action is legal. You never decide legality and never write engine-owned state with `state.set`/`state.patch` (those operations are rejected for engine-owned keys).
- **Immediate Generation**: deliver the complete playable surface in the same turn (`surface.create` / `toolChange`).
- **Definition**: put an `interactive` object on the tool:
  ```json
  "interactive": {
    "id": "game-tictactoe", "kind": "game", "schemaVersion": "2",
    "stateSchema": [
      { "key": "board", "type": "array", "initialValue": ["","","","","","","","",""] },
      { "key": "currentPlayer", "type": "string", "initialValue": "X" },
      { "key": "status", "type": "string", "initialValue": "playing" },
      { "key": "winner", "type": "string", "initialValue": null, "nullable": true }
    ],
    "actors": ["X", "O"], "aiActors": ["O"],
    "actions": [
      { "id": "move", "actor": "active_player",
        "parameters": { "index": { "type": "integer", "min": 0, "max": 8 } },
        "guards": [ { "op": "eq", "left": { "op": "getState", "key": "status" }, "right": { "op": "const", "value": "playing" } },
                    { "op": "isEmpty", "boardKey": "board", "index": { "op": "getParam", "name": "index" } } ],
        "effects": [ { "effect": "setCellAt", "boardKey": "board", "index": { "op": "getParam", "name": "index" }, "value": { "op": "getPlayer" } },
                     { "effect": "advanceTurn", "players": ["X", "O"] } ] },
      { "id": "reset", "effects": [ { "effect": "resetGame" } ] }
    ],
    "terminalConditions": [ { "condition": { "op": "and", "exprs": [ ... ] }, "status": "won", "winner": { "op": "getCellAt", "boardKey": "board", "index": { "op": "const", "value": 0 } } } ],
    "testCases": [ { "name": "legal move", "actionId": "move", "params": { "index": 4 }, "expectedSuccess": true, "expectedStateSubset": { "currentPlayer": "O" } },
                   { "name": "occupied rejected", "initialState": { "board": ["X","","","","","","","",""], "currentPlayer": "O" }, "actionId": "move", "params": { "index": 0 }, "expectedSuccess": false } ]
  }
  ```
  - Every state key must be declared; unknown keys, wrong types, and nulls without `"nullable": true` are rejected. `status`, `winner`, `turn`, `phase`, `currentPlayer` are engine-managed and may be used without declaring.
  - Parameter types: `string` (minLength/maxLength), `integer`/`number` (min/max), `boolean`, `enum` (allowedValues), `array` (items, maxItems), `object` (properties). Unknown or missing parameters are rejected.
  - Expressions (`"op"`): const, getState, getParam, getActor, getPlayer, getTurn, getPhase, getField, getIndex, getCell, getCellAt, isEmpty, isOccupied, eq, neq, lt, lte, gt, gte, and, or, not, if, add, sub, mul, div, mod, min, max, abs, toNumber, concat, contains, count, in, distance. Out-of-range indexes, non-numeric math and division by zero reject the action.
  - Effects (`"effect"`): set, setCell, setCellAt, swapCells, increment, decrement, pushArray, removeFromArray, shuffleArray, setRandomInt, advanceTurn, setPhase, setActor, endGame, resetGame, if (with then/else effect lists). Randomness requires `randomSeed`; it is deterministic and replayable.
  - Trusted capabilities for rule-heavy games: `{ "effect": "chessMove", "key": "chess", "from": ..., "to": ..., "promotion": ... }` implements full chess rules (castling, en passant, promotion, check, checkmate, stalemate, fifty-move, threefold, insufficient material) on a `chess` object state (`fen`, `board` 8x8 of piece letters, `legalMoves`, `inCheck`, `moves`, `status`). `{ "effect": "grid2048Slide", "key": "grid", "direction": ..., "scoreKey": "score" }` implements 2048. Use these instead of re-describing those rules.
  - `testCases` are executed before admission; a failing test rejects the surface. Include at least one legal and one illegal case.
  - Limits: 128 actions, 64 effects per action, expression depth 16, 256 KB state.
- **Wiring components**: bind display components to state keys (`valueKey`). Buttons use `{ "type": "dispatchInteractive", "actionId": "move", "params": { "index": 4 } }` or `"paramsFromState": { "from": "selectedFrom" }` for values chosen in local UI keys (e.g. a `setValue` selection first). Do not use `setValue` on engine-owned keys.
- **AI opponent**: list the AI's actor in `aiActors`. When it is the AI's turn you receive the public state, the legal actions and `stateRevision`; reply with one operation `{ "type": "interactive.action", "target": { "surfaceId": "..." }, "payload": { "actionId": "move", "params": { ... }, "stateRevision": N } }`. Illegal or stale proposals are rejected without changing state.
- **Hidden information**: set `"readPolicy": "restricted"` on secret keys (quiz answers, hidden cards, mines). They are never shown to the user interface or to you.
- **Non-rules apps** may still use local actions plus `submitToAgent` (`includeFields` must be explicit; `[]` is allowed for pure gestures).
- **Zero Arbitrary Code**: typed components and typed actions only — never `<script>`, `eval`, `innerHTML`, `iframe`, or remote scripts.

## Design & Engineering Rules
1. **Never build generic single-column widget piles.** Use cards, grids, and stats rows with intentional hierarchy.
2. **Component IDs must be unique, stable, and semantic** (e.g. `sp-summary-stats`, `sp-task-table`). Never use random hashes that break state persistence across edits.
3. **No Empty Stubs**: Every input must have a descriptive `props.label` (never `"Input"` or `"Field"`). Headings must have non-empty `props.text`.
4. **Zero Raw Code**: No arbitrary JS, script tags, external CDNs, or raw HTML strings. All logic is expressed through declarative props and registered actions.
5. **Responsive by Default**: Layouts must look polished both in narrow chat side-panels (~400px) and wide tool windows (~1000px). Use `collapseAt` (`"mobile"` | `"tablet"`) appropriately.
6. **Graceful Empty States**: Include `emptyState` components with helpful messages and actionable buttons when collections or queries have zero items.
