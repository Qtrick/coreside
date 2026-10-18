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
/** Count of open portals that requested a blocking (#root inert) backdrop. */
let inertHolders = 0;

function focusable(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
    (el) => {
      if (el.classList.contains("provider-modal-backdrop")) return false;
      return el.offsetParent !== null || el === document.activeElement;
    },
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
  inertBackground = true,
}: {
  children: ReactNode;
  /** When provided, Escape closes the topmost dialog. */
  onClose?: () => void;
  /**
   * When true (default), mark #root inert while this portal is open.
   * Set false for non-blocking hosts (e.g. pending approvals) that must not
   * freeze the rest of the app.
   */
  inertBackground?: boolean;
}) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const closeRef = useRef(onClose);
  closeRef.current = onClose;
  const inertRef = useRef(inertBackground);
  inertRef.current = inertBackground;

  useEffect(() => {
    const node = containerRef.current;
    if (!node) return;
    const previouslyFocused = document.activeElement as HTMLElement | null;
    stack.push(node);
    const makeInert = inertRef.current;

    // Make the app shell inert while a blocking modal is open so pointer/keyboard
    // cannot reach background controls (ARIA alone is insufficient).
    const appRoot =
      document.getElementById("root") ??
      document.querySelector<HTMLElement>("#coreside-root, [data-coreside-root]");
    if (makeInert) {
      inertHolders += 1;
      if (inertHolders === 1 && appRoot) {
        appRoot.setAttribute("inert", "");
        appRoot.setAttribute("aria-hidden", "true");
      }
    }

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

      if (makeInert) {
        inertHolders = Math.max(0, inertHolders - 1);
        if (inertHolders === 0 && appRoot) {
          appRoot.removeAttribute("inert");
          appRoot.removeAttribute("aria-hidden");
        }
      }

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
