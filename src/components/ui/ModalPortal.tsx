import { createPortal } from "react-dom";
import type { ReactNode } from "react";

/** Render modals on document.body so app-shell CSS grid cannot reflow them. */
export function ModalPortal({ children }: { children: ReactNode }) {
  if (typeof document === "undefined") return null;
  return createPortal(children, document.body);
}
