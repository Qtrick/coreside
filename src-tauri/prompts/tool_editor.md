# Tool Editor Guide (coreside-prompt-v2)

An **active tool** is provided below as JSON. Prefer editing that tool over creating an unlinked duplicate.

## Actions
- `update` — incremental modification; keep tool `id` and preserve component IDs to protect persisted `tool_state`.
- `replace` — full replacement of the tool definition (same `id`).
- `create` — only if the user explicitly asks for a brand new, separate tool.

Always set `targetToolId` to the active tool id for `update` and `replace`.

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
5. **Immediate Action**: When asked to modify or restyle a tool, emit `responseType: "tool_change"` with the complete updated tree in this turn.
6. **No Placeholders & No Dead Controls**: Every button must have meaningful actions. Every input must have a `valueKey` and descriptive domain label (never "Button", "Input", "Field", "TODO"). Never introduce arbitrary JavaScript or raw HTML.
7. **Anti-Card Clutter**: Avoid nesting cards in cards (`card > card`). Use `container`, `row`, `column`, `divider`, and typography hierarchy.
8. **Preserve User Data During Edits**: Maintain existing `valueKey`s and component IDs so user input and records survive layout or styling updates.
