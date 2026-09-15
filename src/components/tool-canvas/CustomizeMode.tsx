import { useMemo, useState, useEffect } from "react";
import { Wand2, Sparkles, MoveUp, MoveDown, EyeOff, Check, AlertCircle } from "lucide-react";
import type { ToolComponent, ToolDefinition, LayoutRole } from "@/types/tool";
import {
  applyDirectManipulationOps,
  flattenComponents,
  makeHideComponentOp,
  makeMoveComponentOp,
  makeReplaceComponentOp,
  makeUpdatePropsOp,
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

  const [labelDraft, setLabelDraft] = useState("");
  const [roleDraft, setRoleDraft] = useState<string>("");
  const [colSpanDraft, setColSpanDraft] = useState<number>(1);
  const [aiPrompt, setAiPrompt] = useState("");
  const [busy, setBusy] = useState(false);
  const [successMsg, setSuccessMsg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

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
            : selected.id;
      setLabelDraft(label);
      setRoleDraft(selected.layoutRole ?? "");
      setColSpanDraft(selected.colSpan ?? 1);
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
      await applyDirectManipulationOps({
        conversationId,
        surfaceId,
        operations,
      });
      if (successNote) setSuccessMsg(successNote);
      onApplied?.();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
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

  const renameSelected = async () => {
    if (!selected || !labelDraft.trim()) return;
    const props: Record<string, unknown> = {};
    if ("label" in (selected.props ?? {})) props.label = labelDraft.trim();
    else props.text = labelDraft.trim();
    await runOps(
      [
        makeUpdatePropsOp({
          surfaceId,
          componentId: selected.id,
          props,
          baseRevision,
        }),
      ],
      "Label updated",
    );
  };

  const applyLayoutChanges = async () => {
    if (!selected) return;
    const updated: ToolComponent = {
      ...selected,
      layoutRole: roleDraft ? roleDraft : undefined,
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
      "Layout role updated",
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

  const handleAskAi = () => {
    if (!selected || !aiPrompt.trim()) return;
    const prompt = `In tool '${tool.name}', modify the ${selected.type} component '${selected.id}': ${aiPrompt.trim()}`;
    onAskAi?.(prompt);
    setAiPrompt("");
    setSuccessMsg("Sent request to AI");
  };

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
        {compact ? <span className="sr-only">Customize</span> : "Customize"}
      </button>
      {enabled ? (
        <div className="customize-mode-panel" role="region" aria-label="Tool Customization">
          <div className="customize-panel-header">
            <p className="muted">Click any component on canvas to inspect & edit.</p>
          </div>

          <div className="customize-component-selector">
            <label className="sr-only" htmlFor="customize-select-comp">Select component</label>
            <select
              id="customize-select-comp"
              className="tr-select"
              value={selectedId ?? ""}
              onChange={(e) => setSelectedId(e.target.value || null)}
            >
              <option value="">-- Choose a component --</option>
              {rows.map(({ component }) => (
                <option key={component.id} value={component.id}>
                  {component.type} · {component.id} {component.layoutRole ? `(${component.layoutRole})` : ""}
                </option>
              ))}
            </select>
          </div>

          {selected ? (
            <div className="customize-editor">
              <div className="customize-selected-meta">
                <span className="badge">{selected.type}</span>
                <code>{selected.id}</code>
              </div>

              <div className="customize-form-group">
                <label htmlFor="customize-label-input">Label / Text</label>
                <div className="input-action-row">
                  <input
                    id="customize-label-input"
                    type="text"
                    value={labelDraft}
                    onChange={(e) => setLabelDraft(e.target.value)}
                  />
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    disabled={busy}
                    onClick={() => void renameSelected()}
                  >
                    Save
                  </button>
                </div>
              </div>

              <div className="customize-form-group">
                <label htmlFor="customize-role-input">Layout Role</label>
                <div className="input-action-row">
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
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    disabled={busy}
                    onClick={() => void applyLayoutChanges()}
                  >
                    Apply
                  </button>
                </div>
              </div>

              <div className="button-row">
                <button
                  type="button"
                  className="btn btn-ghost btn-sm"
                  disabled={busy}
                  onClick={() => void moveSelected("up")}
                  title="Move component up"
                >
                  <MoveUp size={14} aria-hidden /> Up
                </button>
                <button
                  type="button"
                  className="btn btn-ghost btn-sm"
                  disabled={busy}
                  onClick={() => void moveSelected("down")}
                  title="Move component down"
                >
                  <MoveDown size={14} aria-hidden /> Down
                </button>
                <button
                  type="button"
                  className="btn btn-ghost btn-sm"
                  disabled={busy}
                  onClick={() => void hideSelected()}
                  title="Hide component"
                >
                  <EyeOff size={14} aria-hidden /> Hide
                </button>
              </div>

              {onAskAi && (
                <div className="customize-ai-box">
                  <label htmlFor="customize-ai-prompt">
                    <Sparkles size={13} aria-hidden /> Ask AI to modify this
                  </label>
                  <div className="input-action-row">
                    <input
                      id="customize-ai-prompt"
                      type="text"
                      placeholder="e.g. make this blue with a reset icon"
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
