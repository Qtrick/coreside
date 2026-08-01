import { createPortal } from "react-dom";
import { useEffect, useRef, type ReactNode } from "react";

const FOCUSABLE = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  '[tabindex]:not([tabindex="-1"])',
].join(",");

/** Innermost portal first: only the topmost modal reacts to Tab and Escape. */
const stack: HTMLElement[] = [];

function focusable(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
    (el) => el.offsetParent !== null || el === document.activeElement,
  );
}

/**
 * Render modals on document.body so app-shell CSS grid cannot reflow them.
 *
 * The portal also owns the shared dialog keyboard contract: focus moves into
 * the dialog, Tab cycles inside it, and focus returns to the triggering element
 * on close. Dialogs that manage their own initial focus keep it — the portal
 * only steps in when nothing inside has been focused yet.
 */
export function ModalPortal({
  children,
  onClose,
}: {
  children: ReactNode;
  /** When provided, Escape closes the topmost dialog. */
  onClose?: () => void;
}) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const closeRef = useRef(onClose);
  closeRef.current = onClose;

  useEffect(() => {
    const node = containerRef.current;
    if (!node) return;
    const previouslyFocused = document.activeElement as HTMLElement | null;
    stack.push(node);

    const frame = window.requestAnimationFrame(() => {
      if (node.contains(document.activeElement)) return;
      focusable(node)[0]?.focus();
    });

    const onKeyDown = (event: KeyboardEvent) => {
      if (stack[stack.length - 1] !== node) return;
      if (event.key === "Escape" && closeRef.current) {
        event.stopPropagation();
        event.preventDefault();
        closeRef.current();
        return;
      }
      if (event.key !== "Tab") return;
      const items = focusable(node);
      if (items.length === 0) return;
      const first = items[0];
      const last = items[items.length - 1];
      const active = document.activeElement;
      if (!node.contains(active)) {
        event.preventDefault();
        first.focus();
      } else if (event.shiftKey && active === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && active === last) {
        event.preventDefault();
        first.focus();
      }
    };

    document.addEventListener("keydown", onKeyDown, true);
    return () => {
      window.cancelAnimationFrame(frame);
      document.removeEventListener("keydown", onKeyDown, true);
      const index = stack.indexOf(node);
      if (index >= 0) stack.splice(index, 1);

      // Closing a dialog that is not the topmost one must not pull focus out of
      // the dialog the user is still looking at.
      const top = stack[stack.length - 1];
      if (!top || top.contains(previouslyFocused)) {
        if (previouslyFocused?.isConnected) previouslyFocused.focus();
      } else if (!top.contains(document.activeElement)) {
        focusable(top)[0]?.focus();
      }
    };
  }, []);

  if (typeof document === "undefined") return null;
  return createPortal(
    <div ref={containerRef} style={{ display: "contents" }}>
      {children}
    </div>,
    document.body,
  );
}
