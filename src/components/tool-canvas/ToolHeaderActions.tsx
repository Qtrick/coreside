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

type ToolHeaderActionsProps = {
  tool: ToolDefinition;
  /** Authoritative SQLite surface id when known; never invent surf-* here. */
  surfaceId: string;
  conversationId: string | null;
  density: "full" | "icons" | "menu";
  closeLabel?: string;
  onDetails: () => void;
  onExport: () => void;
  onOpen: () => void;
  onUndo: () => void;
  onClose: () => void;
  onCustomizeApplied: () => void;
  isCustomizing?: boolean;
  onToggleCustomizing?: () => void;
  selectedComponentId?: string | null;
  onSelectComponentId?: (id: string | null) => void;
  onAskAi?: (prompt: string) => void;
};

/**
 * Priority: Close + Open-in-new-window always reachable; Customize/Undo when
 * space allows; Details/Export overflow-eligible. Multi-window must not require
 * opening the overflow menu (Journey 7 / compact 1024px).
 */
export function ToolHeaderActions({
  tool,
  surfaceId,
  conversationId,
  density,
  closeLabel = "Close tool canvas",
  onDetails,
  onExport,
  onOpen,
  onUndo,
  onClose,
  onCustomizeApplied,
  isCustomizing,
  onToggleCustomizing,
  selectedComponentId,
  onSelectComponentId,
  onAskAi,
}: ToolHeaderActionsProps) {
  const [menuOpen, setMenuOpen] = useState(false);
  const menuId = useId();
  const rootRef = useRef<HTMLDivElement>(null);
  const menuButtonRef = useRef<HTMLButtonElement>(null);

  // A responsive reflow can remove the overflow trigger. Do not retain an
  // invisible open menu and resurrect it on a later resize.
  useEffect(() => {
    setMenuOpen(false);
  }, [density]);

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
        ".tool-header-menu button:not(:disabled)",
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

  const renderSecondary = () => (
    <>
      <button
        type="button"
        className="btn btn-secondary"
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
            surfaceId={surfaceId}
            conversationId={conversationId}
            tool={tool}
            baseRevision={tool.version ?? 1}
            compact={density === "icons"}
            enabled={isCustomizing}
            onToggleEnabled={onToggleCustomizing}
            selectedId={selectedComponentId}
            onSelectId={onSelectComponentId}
            onAskAi={onAskAi}
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
          {density === "full" ? renderSecondary() : null}
          {density === "icons" ? (
            <div className="tool-header-more">
              <button
                ref={menuButtonRef}
                type="button"
                className="btn btn-secondary"
                aria-label="More tool actions"
                aria-expanded={menuOpen}
                aria-controls={menuId}
                onClick={() => setMenuOpen((v) => !v)}
              >
                <MoreHorizontal size={16} aria-hidden />
                <span className="sr-only">More tool actions</span>
              </button>
              {menuOpen ? (
                <div
                  className="tool-header-menu"
                  role="group"
                  aria-label="More tool actions"
                  id={menuId}
                >
                  {renderSecondary()}
                </div>
              ) : null}
            </div>
          ) : null}
        </>
      ) : (
        <>
          {/* Compact 1024px: keep Open + Close as fixed-size icon-btn only.
              A labeled More btn-secondary was ~80px and pastRight-overflowed
              when main wrongly sat in the 80px sidebar track (Journey 5). */}
          <button
            type="button"
            className="icon-btn"
            onClick={onOpen}
            aria-label="Open tool in new window"
            title="Open in new window"
          >
            <ExternalLink size={16} aria-hidden />
          </button>
          <div className="tool-header-more">
            <button
              ref={menuButtonRef}
              type="button"
              className="icon-btn"
              aria-label="More tool actions"
              aria-expanded={menuOpen}
              aria-controls={menuId}
              onClick={() => setMenuOpen((v) => !v)}
            >
              <MoreHorizontal size={16} aria-hidden />
              <span className="sr-only">More tool actions</span>
            </button>
            {menuOpen ? (
              <div
                className="tool-header-menu"
                role="group"
                aria-label="More tool actions"
                id={menuId}
              >
                <CustomizeMode
                  surfaceId={surfaceId}
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
                  aria-label="Undo last tool change"
                  onClick={() => {
                    setMenuOpen(false);
                    onUndo();
                  }}
                >
                  <History size={16} aria-hidden />
                  Undo
                </button>
                {renderSecondary()}
              </div>
            ) : null}
          </div>
        </>
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
