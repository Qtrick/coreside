import {
  Download,
  ExternalLink,
  History,
  Info,
  MoreHorizontal,
  X,
} from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import { CustomizeMode } from "@/components/tool-canvas/CustomizeMode";
import type { ToolDefinition } from "@/types/tool";
import { surfaceIdForTool } from "@/lib/surface-ops";

type ToolHeaderActionsProps = {
  tool: ToolDefinition;
  conversationId: string | null;
  density: "full" | "icons" | "menu";
  closeLabel?: string;
  onDetails: () => void;
  onExport: () => void;
  onOpen: () => void;
  onUndo: () => void;
  onClose: () => void;
  onCustomizeApplied: () => void;
};

/**
 * Priority: Close always visible; Customize/Open/Undo when space allows;
 * Details/Export overflow-eligible.
 */
export function ToolHeaderActions({
  tool,
  conversationId,
  density,
  closeLabel = "Close tool canvas",
  onDetails,
  onExport,
  onOpen,
  onUndo,
  onClose,
  onCustomizeApplied,
}: ToolHeaderActionsProps) {
  const [menuOpen, setMenuOpen] = useState(false);
  const menuId = useId();
  const rootRef = useRef<HTMLDivElement>(null);
  const menuButtonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!menuOpen) return;
    const onDoc = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) setMenuOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setMenuOpen(false);
        menuButtonRef.current?.focus();
      }
    };
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    const focusFrame = requestAnimationFrame(() => {
      const first = rootRef.current?.querySelector<HTMLElement>(
        '[role="menuitem"]:not([aria-disabled="true"]), .tool-header-menu .btn',
      );
      first?.focus();
    });
    return () => {
      cancelAnimationFrame(focusFrame);
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [menuOpen]);

  const showLabels = density === "full";
  const overflowOnly = density === "menu";

  const renderSecondary = (inMenu: boolean) => (
    <>
      <button
        type="button"
        className="btn btn-secondary"
        role={inMenu ? "menuitem" : undefined}
        onClick={() => {
          setMenuOpen(false);
          onDetails();
        }}
        aria-label="Application details"
      >
        <Info size={16} aria-hidden />
        {showLabels || overflowOnly ? "Details" : null}
      </button>
      <button
        type="button"
        className="btn btn-secondary"
        role={inMenu ? "menuitem" : undefined}
        onClick={() => {
          setMenuOpen(false);
          onExport();
        }}
        aria-label="Export tool"
      >
        <Download size={16} aria-hidden />
        {showLabels || overflowOnly ? "Export" : null}
      </button>
    </>
  );

  return (
    <div className="tool-header-actions" data-density={density} ref={rootRef}>
      {!overflowOnly ? (
        <>
          <CustomizeMode
            surfaceId={surfaceIdForTool(tool.id)}
            conversationId={conversationId}
            tool={tool}
            baseRevision={tool.version ?? 1}
            onApplied={onCustomizeApplied}
          />
          <button
            type="button"
            className="btn btn-secondary"
            onClick={onOpen}
            aria-label="Open tool in new window"
          >
            <ExternalLink size={16} aria-hidden />
            {showLabels ? "Open" : null}
          </button>
          <button
            type="button"
            className="btn btn-secondary"
            onClick={onUndo}
            aria-label="Undo last tool change"
          >
            <History size={16} aria-hidden />
            {showLabels ? "Undo" : null}
          </button>
          {density === "full" ? renderSecondary(false) : null}
          {density === "icons" ? (
            <div className="tool-header-more">
              <button
                ref={menuButtonRef}
                type="button"
                className="btn btn-secondary"
                aria-label="More tool actions"
                aria-haspopup="menu"
                aria-expanded={menuOpen}
                aria-controls={menuId}
                onClick={() => setMenuOpen((v) => !v)}
              >
                <MoreHorizontal size={16} aria-hidden />
                <span className="sr-only">More tool actions</span>
              </button>
              {menuOpen ? (
                <div className="tool-header-menu" role="menu" id={menuId}>
                  {renderSecondary(true)}
                </div>
              ) : null}
            </div>
          ) : null}
        </>
      ) : (
        <div className="tool-header-more">
          <button
            ref={menuButtonRef}
            type="button"
            className="btn btn-secondary"
            aria-label="More tool actions"
            aria-haspopup="menu"
            aria-expanded={menuOpen}
            aria-controls={menuId}
            onClick={() => setMenuOpen((v) => !v)}
          >
            <MoreHorizontal size={16} aria-hidden />
            More
          </button>
          {menuOpen ? (
            <div className="tool-header-menu" role="menu" id={menuId}>
              <CustomizeMode
                surfaceId={surfaceIdForTool(tool.id)}
                conversationId={conversationId}
                tool={tool}
                baseRevision={tool.version ?? 1}
                onApplied={() => {
                  setMenuOpen(false);
                  onCustomizeApplied();
                }}
              />
              <button
                type="button"
                className="btn btn-secondary"
                role="menuitem"
                aria-label="Open tool in new window"
                onClick={() => {
                  setMenuOpen(false);
                  onOpen();
                }}
              >
                <ExternalLink size={16} aria-hidden />
                Open
              </button>
              <button
                type="button"
                className="btn btn-secondary"
                role="menuitem"
                aria-label="Undo last tool change"
                onClick={() => {
                  setMenuOpen(false);
                  onUndo();
                }}
              >
                <History size={16} aria-hidden />
                Undo
              </button>
              {renderSecondary(true)}
            </div>
          ) : null}
        </div>
      )}
      <button
        type="button"
        className={
          closeLabel.toLowerCase().includes("back")
            ? "btn btn-secondary tool-header-close"
            : "icon-btn tool-header-close"
        }
        onClick={onClose}
        aria-label={closeLabel}
      >
        <X size={18} aria-hidden />
        {closeLabel.toLowerCase().includes("back") ? "Back to chat" : null}
      </button>
    </div>
  );
}
