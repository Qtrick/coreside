import { useMemo, useState } from "react";
import { Wand2 } from "lucide-react";
import type { ToolComponent, ToolDefinition } from "@/types/tool";
import {
  applyDirectManipulationOps,
  flattenComponents,
  makeHideComponentOp,
  makeMoveComponentOp,
  makeUpdatePropsOp,
} from "@/lib/surface-ops";

type CustomizeModeProps = {
  surfaceId: string;
  conversationId?: string | null;
  tool: ToolDefinition;
  baseRevision: number;
  compact?: boolean;
  onApplied?: () => void;
};

export function CustomizeMode({
  surfaceId,
  conversationId,
  tool,
  baseRevision,
  compact = false,
  onApplied,
}: CustomizeModeProps) {
  const [enabled, setEnabled] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [labelDraft, setLabelDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const rows = useMemo(
    () => flattenComponents(tool.components ?? []),
    [tool.components],
  );
  const selected = rows.find((row) => row.component.id === selectedId)?.component;

  const select = (component: ToolComponent) => {
    setSelectedId(component.id);
    const label =
      typeof component.props?.label === "string"
        ? component.props.label
        : typeof component.props?.text === "string"
          ? component.props.text
          : component.id;
    setLabelDraft(label);
  };

  const runOps = async (operations: Parameters<typeof applyDirectManipulationOps>[0]["operations"]) => {
    setBusy(true);
    setError(null);
    try {
      await applyDirectManipulationOps({
        conversationId,
        surfaceId,
        operations,
      });
      onApplied?.();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const hideSelected = async () => {
    if (!selected) return;
    await runOps([
      makeHideComponentOp({
        surfaceId,
        componentId: selected.id,
        baseRevision,
        visible: false,
      }),
    ]);
  };

  const renameSelected = async () => {
    if (!selected || !labelDraft.trim()) return;
    const props: Record<string, unknown> = {};
    if ("label" in (selected.props ?? {})) props.label = labelDraft.trim();
    else props.text = labelDraft.trim();
    await runOps([
      makeUpdatePropsOp({
        surfaceId,
        componentId: selected.id,
        props,
        baseRevision,
      }),
    ]);
  };

  const moveSelected = async (direction: "up" | "down") => {
    if (!selected) return;
    const row = rows.find((r) => r.component.id === selected.id);
    if (!row) return;
    const siblings = rows.filter((r) => r.parentId === row.parentId);
    const siblingIndex = siblings.findIndex((r) => r.component.id === selected.id);
    const nextIndex = direction === "up" ? siblingIndex - 1 : siblingIndex + 1;
    if (nextIndex < 0 || nextIndex >= siblings.length) return;
    await runOps([
      makeMoveComponentOp({
        surfaceId,
        componentId: selected.id,
        parentId: row.parentId,
        index: nextIndex,
        baseRevision,
      }),
    ]);
  };

  return (
    <div className="customize-mode">
      <button
        type="button"
        className={`${compact ? "icon-btn" : "btn btn-secondary"}${enabled ? " active" : ""}`}
        aria-pressed={enabled}
        aria-label={compact ? "Customize tool" : undefined}
        onClick={() => setEnabled((v) => !v)}
      >
        <Wand2 size={16} aria-hidden />
        {compact ? <span className="sr-only">Customize</span> : "Customize"}
      </button>
      {enabled ? (
        <div className="customize-mode-panel">
          <p className="muted">Select a component to edit safely.</p>
          <ul className="customize-component-list">
            {rows.map(({ component }) => (
              <li key={component.id}>
                <button
                  type="button"
                  className={`btn btn-ghost${selectedId === component.id ? " active" : ""}`}
                  onClick={() => select(component)}
                >
                  {component.type} · {component.id}
                </button>
              </li>
            ))}
          </ul>
          {selected ? (
            <div className="customize-editor">
              <label>
                Label
                <input
                  type="text"
                  value={labelDraft}
                  onChange={(e) => setLabelDraft(e.target.value)}
                />
              </label>
              <div className="button-row">
                <button
                  type="button"
                  className="btn btn-secondary"
                  disabled={busy}
                  onClick={() => void renameSelected()}
                >
                  Rename label
                </button>
                <button
                  type="button"
                  className="btn btn-secondary"
                  disabled={busy}
                  onClick={() => void hideSelected()}
                >
                  Hide
                </button>
                <button
                  type="button"
                  className="btn btn-ghost"
                  disabled={busy}
                  onClick={() => void moveSelected("up")}
                >
                  Move up
                </button>
                <button
                  type="button"
                  className="btn btn-ghost"
                  disabled={busy}
                  onClick={() => void moveSelected("down")}
                >
                  Move down
                </button>
              </div>
            </div>
          ) : null}
          {error ? <p className="tr-validation">{error}</p> : null}
        </div>
      ) : null}
    </div>
  );
}
