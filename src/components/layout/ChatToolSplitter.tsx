import {
  useCallback,
  useRef,
  type KeyboardEvent,
  type PointerEvent,
} from "react";
import { clampSplitForWidth, clampSplitRatio } from "@/lib/layout-mode";

type ChatToolSplitterProps = {
  ratio: number;
  mainWidth: number;
  onChange: (ratio: number) => void;
  onCommit: (ratio: number) => void;
  disabled?: boolean;
};

/**
 * Accessible splitter between Chat and Tool Canvas.
 * Keyboard: ArrowLeft/Right adjust; Home/End reset to 50%.
 */
export function ChatToolSplitter({
  ratio,
  mainWidth,
  onChange,
  onCommit,
  disabled,
}: ChatToolSplitterProps) {
  const dragging = useRef(false);

  const applyClientX = useCallback(
    (clientX: number, commit: boolean) => {
      const el = document.querySelector<HTMLElement>(".app-shell-main");
      if (!el) return;
      const rect = el.getBoundingClientRect();
      if (rect.width <= 0) return;
      const next = clampSplitForWidth(
        (clientX - rect.left) / rect.width,
        mainWidth || rect.width,
      );
      onChange(next);
      if (commit) onCommit(next);
    },
    [mainWidth, onChange, onCommit],
  );

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    if (disabled) return;
    dragging.current = true;
    e.currentTarget.setPointerCapture(e.pointerId);
    document.body.style.userSelect = "none";
    applyClientX(e.clientX, false);
  };

  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    if (!dragging.current) return;
    applyClientX(e.clientX, false);
  };

  const endDrag = (e: PointerEvent<HTMLDivElement>) => {
    if (!dragging.current) return;
    dragging.current = false;
    document.body.style.userSelect = "";
    applyClientX(e.clientX, true);
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (disabled) return;
    const step = e.shiftKey ? 0.08 : 0.03;
    let next = ratio;
    if (e.key === "ArrowLeft") next = ratio - step;
    else if (e.key === "ArrowRight") next = ratio + step;
    else if (e.key === "Home" || e.key === "Enter") next = 0.5;
    else return;
    e.preventDefault();
    const clamped = clampSplitForWidth(clampSplitRatio(next), mainWidth);
    onChange(clamped);
    onCommit(clamped);
  };

  const onDoubleClick = () => {
    if (disabled) return;
    const reset = clampSplitForWidth(0.5, mainWidth);
    onChange(reset);
    onCommit(reset);
  };

  return (
    <div
      className="chat-tool-splitter"
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize chat and tool panels"
      aria-valuemin={28}
      aria-valuemax={72}
      aria-valuenow={Math.round(ratio * 100)}
      aria-disabled={disabled || undefined}
      tabIndex={disabled ? -1 : 0}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
      onKeyDown={onKeyDown}
      onDoubleClick={onDoubleClick}
    />
  );
}
