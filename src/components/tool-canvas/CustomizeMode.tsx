import { useMemo, useState, useEffect } from "react";
import {
  Wand2,
  Sparkles,
  MoveUp,
  MoveDown,
  EyeOff,
  Check,
  AlertCircle,
  Plus,
  Copy,
  Trash2,
  X,
  Sliders,
  Database,
  Layout,
  Type,
  CheckCircle2,
  Undo2,
} from "lucide-react";
import { api } from "@/lib/tauri";
import type { ToolComponent, ToolDefinition, LayoutRole } from "@/types/tool";
import {
  applyDirectManipulationOps,
  flattenComponents,
  makeHideComponentOp,
  makeInsertComponentOp,
  makeMoveComponentOp,
  makeRemoveComponentOp,
  makeReplaceComponentOp,
  makeUpdatePropsOp,
  safeDuplicateComponent,
} from "@/lib/surface-ops";

type CustomizeModeProps = {
  surfaceId: string;
  conversationId?: string | null;
  tool: ToolDefinition;
  baseRevision: number;
  compact?: boolean;
  enabled?: boolean;
  onToggleEnabled?: () => void;
  selectedId?: string | null;
  onSelectId?: (id: string | null) => void;
  onAskAi?: (prompt: string) => void;
  onApplied?: () => void;
};

const LAYOUT_ROLES: Array<{ value: LayoutRole | ""; label: string }> = [
  { value: "", label: "Default flow" },
  { value: "header", label: "Header" },
  { value: "stats", label: "Stats Bar" },
  { value: "main", label: "Main Content" },
  { value: "sidebar", label: "Sidebar / Secondary" },
  { value: "detail", label: "Detail Pane" },
  { value: "footer", label: "Footer" },
  { value: "actions", label: "Actions Row" },
];

interface PaletteItem {
  type: string;
  name: string;
  category: "text" | "inputs" | "actions" | "data" | "charts" | "layout";
  description: string;
  defaultProps: Record<string, unknown>;
}

const PALETTE_ITEMS: PaletteItem[] = [
  // Text & Display
  { type: "heading", name: "Heading", category: "text", description: "Section header text", defaultProps: { text: "Section Title", level: 3 } },
  { type: "text", name: "Text", category: "text", description: "Paragraph or explanatory text", defaultProps: { text: "Description text goes here." } },
  { type: "stat", name: "Metric Stat", category: "text", description: "Large number display with label", defaultProps: { label: "Total Count", value: "0" } },
  { type: "badge", name: "Badge", category: "text", description: "Status or category tag", defaultProps: { label: "Active", variant: "primary" } },
  { type: "progress", name: "Progress Bar", category: "text", description: "Visual completion progress", defaultProps: { value: 50, max: 100, label: "Progress" } },
  { type: "emptyState", name: "Empty State", category: "text", description: "Callout for empty collections", defaultProps: { title: "No records yet", description: "Add your first entry to get started." } },
  { type: "divider", name: "Divider", category: "text", description: "Horizontal dividing line", defaultProps: {} },

  // Inputs
  { type: "textInput", name: "Text Input", category: "inputs", description: "Single-line text entry", defaultProps: { label: "Field Name", placeholder: "Enter text...", valueKey: "inputDraft" } },
  { type: "textArea", name: "Text Area", category: "inputs", description: "Multi-line text entry", defaultProps: { label: "Notes", placeholder: "Enter details...", valueKey: "notesDraft" } },
  { type: "numberInput", name: "Number Input", category: "inputs", description: "Numeric field with stepper", defaultProps: { label: "Quantity", min: 0, max: 100, valueKey: "quantity" } },
  { type: "slider", name: "Range Slider", category: "inputs", description: "Continuous or stepped range slider", defaultProps: { label: "Intensity", min: 0, max: 100, step: 5, valueKey: "rangeValue" } },
  { type: "select", name: "Dropdown Select", category: "inputs", description: "Choose one option from list", defaultProps: { label: "Category", options: [{ label: "Option A", value: "a" }, { label: "Option B", value: "b" }], valueKey: "selectedCategory" } },
  { type: "radioGroup", name: "Radio Group", category: "inputs", description: "Single choice from visible radio items", defaultProps: { label: "Priority", options: [{ label: "Standard", value: "std" }, { label: "Urgent", value: "urg" }], valueKey: "selectedPriority" } },
  { type: "checkbox", name: "Checkbox", category: "inputs", description: "Toggle boolean checkbox", defaultProps: { label: "Enable feature", valueKey: "featureEnabled" } },
  { type: "switch", name: "Toggle Switch", category: "inputs", description: "Sleek toggle switch", defaultProps: { label: "Notifications", valueKey: "notificationsOn" } },
  { type: "dateInput", name: "Date Input", category: "inputs", description: "Calendar date picker", defaultProps: { label: "Due Date", valueKey: "dueDate" } },
  { type: "mediaPicker", name: "Media Picker", category: "inputs", description: "Secure media asset selector", defaultProps: { label: "Attachment", allowedTypes: ["image/png", "image/jpeg"], valueKey: "selectedMediaAssetId" } },

  // Actions
  { type: "button", name: "Button", category: "actions", description: "Clickable button with actions", defaultProps: { label: "Submit", variant: "primary" } },
  { type: "submitButton", name: "Submit Button", category: "actions", description: "Form submission button", defaultProps: { label: "Save Changes" } },

  // Data & Tables
  { type: "dataTable", name: "Data Table", category: "data", description: "Structured table with sorting & selection", defaultProps: { rowsKey: "records", columns: [{ key: "title", label: "Title" }, { key: "status", label: "Status" }], selectionKey: "selectedRecordId" } },
  { type: "table", name: "Simple Table", category: "data", description: "Lightweight table", defaultProps: { columns: [{ key: "name", label: "Name" }, { key: "value", label: "Value" }] } },
  { type: "list", name: "List View", category: "data", description: "Itemized list of records", defaultProps: { itemsKey: "items" } },
  { type: "checklist", name: "Checklist", category: "data", description: "Interactive task list with check state", defaultProps: { itemsKey: "tasks", selectionKey: "completedTasks" } },
  { type: "codeEditor", name: "Code Editor", category: "data", description: "Monospace text/code block", defaultProps: { language: "json", valueKey: "codeContent" } },
  { type: "audioPlayer", name: "Audio Player", category: "data", description: "Media audio player control", defaultProps: { title: "Audio Preview", src: "" } },

  // Charts
  { type: "chartLine", name: "Line Chart", category: "charts", description: "Trend over time", defaultProps: { title: "Activity Trend", dataKey: "trendData" } },
  { type: "chartBar", name: "Bar Chart", category: "charts", description: "Category comparison", defaultProps: { title: "Performance by Category", dataKey: "barData" } },
  { type: "chartPie", name: "Pie Chart", category: "charts", description: "Proportion breakdown", defaultProps: { title: "Distribution", dataKey: "pieData" } },
  { type: "chartDonut", name: "Donut Chart", category: "charts", description: "Ring proportion chart", defaultProps: { title: "Composition", dataKey: "donutData" } },
  { type: "chartArea", name: "Area Chart", category: "charts", description: "Filled trend chart", defaultProps: { title: "Cumulative Volume", dataKey: "areaData" } },
  { type: "chartScatter", name: "Scatter Plot", category: "charts", description: "2D point correlation chart", defaultProps: { title: "Correlation", dataKey: "scatterData" } },

  // Layout
  { type: "container", name: "Container", category: "layout", description: "Box container for grouping", defaultProps: {} },
  { type: "card", name: "Card Panel", category: "layout", description: "Elevated panel with border", defaultProps: { title: "Panel Title" } },
  { type: "tabs", name: "Tabs", category: "layout", description: "Tabbed container for switching panes", defaultProps: { tabs: [{ id: "tab1", label: "Overview" }, { id: "tab2", label: "Details" }] } },
  { type: "row", name: "Horizontal Row", category: "layout", description: "Flex row for side-by-side items", defaultProps: {} },
  { type: "column", name: "Vertical Column", category: "layout", description: "Flex column for stacked items", defaultProps: {} },
];

export function CustomizeMode({
  surfaceId,
  conversationId,
  tool,
  baseRevision,
  compact = false,
  enabled: controlledEnabled,
  onToggleEnabled,
  selectedId: controlledSelectedId,
  onSelectId,
  onAskAi,
  onApplied,
}: CustomizeModeProps) {
  const [internalEnabled, setInternalEnabled] = useState(false);
  const [internalSelectedId, setInternalSelectedId] = useState<string | null>(null);

  const enabled = controlledEnabled ?? internalEnabled;
  const setEnabled = (val: boolean) => {
    if (onToggleEnabled) {
      onToggleEnabled();
    } else {
      setInternalEnabled(val);
    }
  };

  const selectedId = controlledSelectedId !== undefined ? controlledSelectedId : internalSelectedId;
  const setSelectedId = (id: string | null) => {
    if (onSelectId) {
      onSelectId(id);
    } else {
      setInternalSelectedId(id);
    }
  };

  const [activeSection, setActiveSection] = useState<"content" | "layout" | "data" | "actions">("content");
  const [labelDraft, setLabelDraft] = useState("");
  const [placeholderDraft, setPlaceholderDraft] = useState("");
  const [roleDraft, setRoleDraft] = useState<string>("");
  const [colSpanDraft, setColSpanDraft] = useState<number>(1);
  const [valueKeyDraft, setValueKeyDraft] = useState("");
  const [dataKeyDraft, setDataKeyDraft] = useState("");
  const [selectionKeyDraft, setSelectionKeyDraft] = useState("");

  const [aiPrompt, setAiPrompt] = useState("");
  const [busy, setBusy] = useState(false);
  const [successMsg, setSuccessMsg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showPalette, setShowPalette] = useState(false);
  const [paletteCategory, setPaletteCategory] = useState<string>("all");
  const [undoStack, setUndoStack] = useState<string[]>([]);

  const rows = useMemo(
    () => flattenComponents(tool.components ?? []),
    [tool.components],
  );
  const selected = rows.find((row) => row.component.id === selectedId)?.component;

  useEffect(() => {
    if (selected) {
      const label =
        typeof selected.props?.label === "string"
          ? selected.props.label
          : typeof selected.props?.text === "string"
            ? selected.props.text
            : typeof selected.props?.title === "string"
              ? selected.props.title
              : "";
      setLabelDraft(label);
      setPlaceholderDraft(typeof selected.props?.placeholder === "string" ? selected.props.placeholder : "");
      setRoleDraft(selected.layoutRole ?? "");
      setColSpanDraft(selected.colSpan ?? 1);
      setValueKeyDraft(typeof selected.props?.valueKey === "string" ? selected.props.valueKey : "");
      setDataKeyDraft(
        typeof selected.props?.dataKey === "string"
          ? selected.props.dataKey
          : typeof selected.props?.rowsKey === "string"
            ? selected.props.rowsKey
            : "",
      );
      setSelectionKeyDraft(typeof selected.props?.selectionKey === "string" ? selected.props.selectionKey : "");
      setError(null);
      setSuccessMsg(null);
    }
  }, [selected]);

  const runOps = async (
    operations: Parameters<typeof applyDirectManipulationOps>[0]["operations"],
    successNote?: string,
  ) => {
    setBusy(true);
    setError(null);
    setSuccessMsg(null);
    try {
      const patches = await applyDirectManipulationOps({
        conversationId,
        surfaceId,
        operations,
      });
      if (Array.isArray(patches)) {
        for (const patch of patches) {
          if (patch.transactionId) {
            setUndoStack((prev) => [...prev, patch.transactionId!]);
          }
        }
      }
      if (successNote) setSuccessMsg(successNote);
      onApplied?.();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleUndo = async () => {
    if (undoStack.length === 0) return;
    const lastTxnId = undoStack[undoStack.length - 1];
    setBusy(true);
    setError(null);
    try {
      await api.undoTransaction(lastTxnId);
      setUndoStack((prev) => prev.slice(0, -1));
      setSuccessMsg("Undid last change");
      onApplied?.();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const saveContentChanges = async () => {
    if (!selected) return;
    const props: Record<string, unknown> = {};
    if ("label" in (selected.props ?? {})) props.label = labelDraft.trim();
    else if ("text" in (selected.props ?? {})) props.text = labelDraft.trim();
    else if ("title" in (selected.props ?? {})) props.title = labelDraft.trim();
    else props.label = labelDraft.trim();

    if (placeholderDraft.trim()) {
      props.placeholder = placeholderDraft.trim();
    }

    await runOps(
      [
        makeUpdatePropsOp({
          surfaceId,
          componentId: selected.id,
          props,
          baseRevision,
        }),
      ],
      "Content updated",
    );
  };

  const saveLayoutChanges = async () => {
    if (!selected) return;
    const updated: ToolComponent = {
      ...selected,
      layoutRole: roleDraft ? (roleDraft as LayoutRole) : undefined,
      colSpan: colSpanDraft > 1 ? colSpanDraft : undefined,
    };
    await runOps(
      [
        makeReplaceComponentOp({
          surfaceId,
          componentId: selected.id,
          component: updated,
          baseRevision,
        }),
      ],
      "Layout updated",
    );
  };

  const saveDataBindings = async () => {
    if (!selected) return;
    const props: Record<string, unknown> = {};
    if (valueKeyDraft.trim()) {
      props.valueKey = valueKeyDraft.trim();
    }
    if (dataKeyDraft.trim()) {
      if (selected.type === "dataTable" || selected.type === "table") {
        props.rowsKey = dataKeyDraft.trim();
      } else {
        props.dataKey = dataKeyDraft.trim();
      }
    }
    if (selectionKeyDraft.trim()) {
      props.selectionKey = selectionKeyDraft.trim();
    }

    await runOps(
      [
        makeUpdatePropsOp({
          surfaceId,
          componentId: selected.id,
          props,
          baseRevision,
        }),
      ],
      "Data bindings saved",
    );
  };

  const moveSelected = async (direction: "up" | "down") => {
    if (!selected) return;
    const row = rows.find((r) => r.component.id === selected.id);
    if (!row) return;
    const siblings = rows.filter((r) => r.parentId === row.parentId);
    const siblingIndex = siblings.findIndex((r) => r.component.id === selected.id);
    const nextIndex = direction === "up" ? siblingIndex - 1 : siblingIndex + 1;
    if (nextIndex < 0 || nextIndex >= siblings.length) return;
    await runOps(
      [
        makeMoveComponentOp({
          surfaceId,
          componentId: selected.id,
          parentId: row.parentId,
          index: nextIndex,
          baseRevision,
        }),
      ],
      `Moved ${direction}`,
    );
  };

  const duplicateSelected = async () => {
    if (!selected) return;
    const row = rows.find((r) => r.component.id === selected.id);
    const cloned = safeDuplicateComponent(selected);
    await runOps(
      [
        makeInsertComponentOp({
          surfaceId,
          parentId: row?.parentId ?? undefined,
          component: cloned,
          index: (row?.index ?? 0) + 1,
          baseRevision,
        }),
      ],
      "Component duplicated",
    );
    setSelectedId(cloned.id);
  };

  const deleteSelected = async () => {
    if (!selected) return;
    await runOps(
      [
        makeRemoveComponentOp({
          surfaceId,
          componentId: selected.id,
          baseRevision,
        }),
      ],
      "Component removed",
    );
    setSelectedId(null);
  };

  const hideSelected = async () => {
    if (!selected) return;
    await runOps(
      [
        makeHideComponentOp({
          surfaceId,
          componentId: selected.id,
          baseRevision,
          visible: false,
        }),
      ],
      "Component hidden",
    );
  };

  const insertFromPalette = async (item: PaletteItem) => {
    const newId = `${item.type}-${Date.now().toString(36)}`;
    const comp: ToolComponent = {
      id: newId,
      type: item.type,
      props: { ...item.defaultProps },
    };
    await runOps(
      [
        makeInsertComponentOp({
          surfaceId,
          parentId: selected?.children ? selected.id : undefined,
          component: comp,
          baseRevision,
        }),
      ],
      `Added ${item.name}`,
    );
    setSelectedId(newId);
    setShowPalette(false);
  };

  const handleAskAi = () => {
    if (!selected || !aiPrompt.trim()) return;
    const prompt = `In tool '${tool.name}', modify the ${selected.type} component '${selected.id}': ${aiPrompt.trim()}`;
    onAskAi?.(prompt);
    setAiPrompt("");
    setSuccessMsg("Sent request to AI");
  };

  const filteredPalette = useMemo(() => {
    if (paletteCategory === "all") return PALETTE_ITEMS;
    return PALETTE_ITEMS.filter((i) => i.category === paletteCategory);
  }, [paletteCategory]);

  return (
    <div className="customize-mode">
      <button
        type="button"
        className={`${compact ? "icon-btn" : "btn btn-secondary"}${enabled ? " active" : ""}`}
        aria-pressed={enabled}
        aria-label={compact ? "Customize tool" : undefined}
        onClick={() => setEnabled(!enabled)}
      >
        <Wand2 size={16} aria-hidden />
        {compact ? <span className="sr-only">Customize</span> : enabled ? "Done" : "Customize"}
      </button>

      {enabled ? (
        <div className="customize-mode-panel" role="region" aria-label="Tool Customization">
          {/* Builder Top Bar */}
          <div className="customize-panel-header">
            <div className="customize-panel-title">
              <strong>Tool Builder</strong>
              <span className="muted">Click any component to inspect</span>
            </div>
            <div className="button-row">
              {undoStack.length > 0 && (
                <button
                  type="button"
                  className="btn btn-sm btn-secondary"
                  disabled={busy}
                  onClick={() => void handleUndo()}
                  title="Undo last change"
                >
                  <Undo2 size={14} aria-hidden /> Undo
                </button>
              )}
              <button
                type="button"
                className={`btn btn-sm ${showPalette ? "btn-primary" : "btn-secondary"}`}
                onClick={() => setShowPalette(!showPalette)}
              >
                <Plus size={14} aria-hidden /> Add Component
              </button>
              <button
                type="button"
                className="btn btn-ghost btn-sm"
                onClick={() => setEnabled(false)}
                title="Exit Customize Mode"
              >
                <CheckCircle2 size={14} aria-hidden /> Done
              </button>
            </div>
          </div>

          {/* Add Component Palette */}
          {showPalette && (
            <div className="customize-palette" role="region" aria-label="Add Component Palette">
              <div className="palette-header">
                <span className="palette-title">Insert Component</span>
                <button
                  type="button"
                  className="icon-btn icon-btn-sm"
                  onClick={() => setShowPalette(false)}
                  aria-label="Close palette"
                >
                  <X size={14} aria-hidden />
                </button>
              </div>
              <div className="palette-categories" role="tablist">
                {["all", "text", "inputs", "actions", "data", "charts", "layout"].map((cat) => (
                  <button
                    key={cat}
                    type="button"
                    className={`palette-cat-chip${paletteCategory === cat ? " active" : ""}`}
                    onClick={() => setPaletteCategory(cat)}
                  >
                    {cat}
                  </button>
                ))}
              </div>
              <div className="palette-grid">
                {filteredPalette.map((item) => (
                  <button
                    key={item.type}
                    type="button"
                    className="palette-card"
                    disabled={busy}
                    onClick={() => void insertFromPalette(item)}
                  >
                    <div className="palette-card-name">{item.name}</div>
                    <div className="palette-card-desc">{item.description}</div>
                  </button>
                ))}
              </div>
            </div>
          )}

          {/* Component Quick Selector */}
          <div className="customize-component-selector">
            <label className="sr-only" htmlFor="customize-select-comp">
              Select component
            </label>
            <select
              id="customize-select-comp"
              className="tr-select"
              value={selectedId ?? ""}
              onChange={(e) => setSelectedId(e.target.value || null)}
            >
              <option value="">-- Choose a component ({rows.length} total) --</option>
              {rows.map(({ component }) => (
                <option key={component.id} value={component.id}>
                  {component.type} · {component.id} {component.layoutRole ? `(${component.layoutRole})` : ""}
                </option>
              ))}
            </select>
          </div>

          {/* Inspector for Selected Component */}
          {selected ? (
            <div className="customize-editor">
              <div className="customize-selected-meta">
                <div className="meta-info">
                  <span className="badge">{selected.type}</span>
                  <code>{selected.id}</code>
                </div>
                <div className="meta-actions">
                  <button
                    type="button"
                    className="icon-btn icon-btn-sm"
                    disabled={busy}
                    onClick={() => void moveSelected("up")}
                    title="Move up"
                  >
                    <MoveUp size={14} aria-hidden />
                  </button>
                  <button
                    type="button"
                    className="icon-btn icon-btn-sm"
                    disabled={busy}
                    onClick={() => void moveSelected("down")}
                    title="Move down"
                  >
                    <MoveDown size={14} aria-hidden />
                  </button>
                  <button
                    type="button"
                    className="icon-btn icon-btn-sm"
                    disabled={busy}
                    onClick={() => void duplicateSelected()}
                    title="Duplicate component"
                  >
                    <Copy size={14} aria-hidden />
                  </button>
                  <button
                    type="button"
                    className="icon-btn icon-btn-sm"
                    disabled={busy}
                    onClick={() => void hideSelected()}
                    title="Hide component"
                  >
                    <EyeOff size={14} aria-hidden />
                  </button>
                  <button
                    type="button"
                    className="icon-btn icon-btn-sm text-danger"
                    disabled={busy}
                    onClick={() => void deleteSelected()}
                    title="Delete component"
                  >
                    <Trash2 size={14} aria-hidden />
                  </button>
                </div>
              </div>

              {/* Inspector Section Tabs */}
              <div className="inspector-tabs" role="tablist">
                <button
                  type="button"
                  className={`inspector-tab${activeSection === "content" ? " active" : ""}`}
                  onClick={() => setActiveSection("content")}
                >
                  <Type size={13} aria-hidden /> Content
                </button>
                <button
                  type="button"
                  className={`inspector-tab${activeSection === "layout" ? " active" : ""}`}
                  onClick={() => setActiveSection("layout")}
                >
                  <Layout size={13} aria-hidden /> Layout
                </button>
                <button
                  type="button"
                  className={`inspector-tab${activeSection === "data" ? " active" : ""}`}
                  onClick={() => setActiveSection("data")}
                >
                  <Database size={13} aria-hidden /> Data & State
                </button>
                <button
                  type="button"
                  className={`inspector-tab${activeSection === "actions" ? " active" : ""}`}
                  onClick={() => setActiveSection("actions")}
                >
                  <Sliders size={13} aria-hidden /> Actions
                </button>
              </div>

              {/* Inspector Section: Content */}
              {activeSection === "content" && (
                <div className="inspector-panel">
                  <div className="customize-form-group">
                    <label htmlFor="customize-label-input">Label / Heading / Title</label>
                    <input
                      id="customize-label-input"
                      type="text"
                      value={labelDraft}
                      onChange={(e) => setLabelDraft(e.target.value)}
                    />
                  </div>
                  {("placeholder" in (selected.props ?? {}) || selected.type.includes("Input")) && (
                    <div className="customize-form-group">
                      <label htmlFor="customize-placeholder-input">Placeholder</label>
                      <input
                        id="customize-placeholder-input"
                        type="text"
                        value={placeholderDraft}
                        onChange={(e) => setPlaceholderDraft(e.target.value)}
                      />
                    </div>
                  )}
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    disabled={busy}
                    onClick={() => void saveContentChanges()}
                  >
                    Apply Content
                  </button>
                </div>
              )}

              {/* Inspector Section: Layout */}
              {activeSection === "layout" && (
                <div className="inspector-panel">
                  <div className="customize-form-group">
                    <label htmlFor="customize-role-input">Layout Role</label>
                    <select
                      id="customize-role-input"
                      className="tr-select"
                      value={roleDraft}
                      onChange={(e) => setRoleDraft(e.target.value)}
                    >
                      {LAYOUT_ROLES.map((r) => (
                        <option key={r.value} value={r.value}>
                          {r.label}
                        </option>
                      ))}
                    </select>
                  </div>
                  <div className="customize-form-group">
                    <label htmlFor="customize-colspan-input">Column Span (1–12)</label>
                    <input
                      id="customize-colspan-input"
                      type="number"
                      min={1}
                      max={12}
                      value={colSpanDraft}
                      onChange={(e) => setColSpanDraft(Math.max(1, parseInt(e.target.value, 10) || 1))}
                    />
                  </div>
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    disabled={busy}
                    onClick={() => void saveLayoutChanges()}
                  >
                    Apply Layout
                  </button>
                </div>
              )}

              {/* Inspector Section: Data & State */}
              {activeSection === "data" && (
                <div className="inspector-panel">
                  <div className="customize-form-group">
                    <label htmlFor="customize-valkey-input">Value Binding (State Key)</label>
                    <input
                      id="customize-valkey-input"
                      type="text"
                      placeholder="e.g. taskTitle or activeTab"
                      value={valueKeyDraft}
                      onChange={(e) => setValueKeyDraft(e.target.value)}
                    />
                    <span className="muted" style={{ fontSize: "0.75rem" }}>
                      Explicit state binding key. Without this, inputs remain local ephemeral state.
                    </span>
                  </div>
                  {(selected.type === "dataTable" ||
                    selected.type === "table" ||
                    selected.type.startsWith("chart") ||
                    selected.type === "list") && (
                    <div className="customize-form-group">
                      <label htmlFor="customize-datakey-input">Data / Rows Binding Key</label>
                      <input
                        id="customize-datakey-input"
                        type="text"
                        placeholder="e.g. records or queryResult"
                        value={dataKeyDraft}
                        onChange={(e) => setDataKeyDraft(e.target.value)}
                      />
                    </div>
                  )}
                  {(selected.type === "dataTable" || selected.type === "table") && (
                    <div className="customize-form-group">
                      <label htmlFor="customize-selkey-input">Selection Key</label>
                      <input
                        id="customize-selkey-input"
                        type="text"
                        placeholder="e.g. selectedTaskId"
                        value={selectionKeyDraft}
                        onChange={(e) => setSelectionKeyDraft(e.target.value)}
                      />
                    </div>
                  )}
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    disabled={busy}
                    onClick={() => void saveDataBindings()}
                  >
                    Save Data Bindings
                  </button>
                </div>
              )}

              {/* Inspector Section: Actions */}
              {activeSection === "actions" && (
                <div className="inspector-panel">
                  <div className="customize-actions-summary">
                    {selected.actions?.length ? (
                      <ul className="actions-list">
                        {selected.actions.map((act, i) => (
                          <li key={i} className="action-tag">
                            <code>{act.type}</code>
                            {"target" in act && act.target ? <span> → {String(act.target)}</span> : null}
                            {"actionName" in act && act.actionName ? <span> ({String(act.actionName)})</span> : null}
                          </li>
                        ))}
                      </ul>
                    ) : (
                      <p className="muted">No declarative actions attached to this component.</p>
                    )}
                  </div>
                </div>
              )}

              {/* Targeted AI Modification */}
              {onAskAi && (
                <div className="customize-ai-box">
                  <label htmlFor="customize-ai-prompt">
                    <Sparkles size={13} aria-hidden /> Ask AI to modify {selected.type}
                  </label>
                  <div className="input-action-row">
                    <input
                      id="customize-ai-prompt"
                      type="text"
                      placeholder="e.g. make this a 2-column card with delete action"
                      value={aiPrompt}
                      onChange={(e) => setAiPrompt(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") handleAskAi();
                      }}
                    />
                    <button
                      type="button"
                      className="btn btn-primary btn-sm"
                      disabled={!aiPrompt.trim() || busy}
                      onClick={handleAskAi}
                    >
                      Send
                    </button>
                  </div>
                </div>
              )}
            </div>
          ) : null}

          {successMsg && (
            <p className="customize-status success" role="status">
              <Check size={14} aria-hidden /> {successMsg}
            </p>
          )}
          {error && (
            <p className="customize-status error" role="alert">
              <AlertCircle size={14} aria-hidden /> {error}
            </p>
          )}
        </div>
      ) : null}
    </div>
  );
}
