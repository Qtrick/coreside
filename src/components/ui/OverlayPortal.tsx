import { createPortal } from "react-dom";
import type { ReactNode } from "react";

/**
 * Non-modal window-local overlay host.
 *
 * Portals to document.body so AppShell grid/z-index rules cannot turn tips into
 * grid participants. Does not trap focus or mark the background inert — use
 * ModalPortal for true dialogs.
 */
export function OverlayPortal({
  children,
  className = "window-overlay-root",
}: {
  children: ReactNode;
  className?: string;
}) {
  if (typeof document === "undefined") return null;

  let host = document.getElementById("coreside-window-overlay-root");
  if (!host) {
    host = document.createElement("div");
    host.id = "coreside-window-overlay-root";
    host.className = className;
    host.setAttribute("data-overlay-mode", "nonmodal");
    document.body.appendChild(host);
  } else if (className && host.className !== className) {
    host.className = className;
  }

  return createPortal(children, host);
}
