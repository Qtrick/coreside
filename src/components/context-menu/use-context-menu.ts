import { useState } from "react";

/** Context-menu open state. Separate file so ContextMenu.tsx exports only
 * components and keeps Fast Refresh. */
export function useContextMenuTrigger() {
  const [state, setState] = useState<{
    open: boolean;
    x: number;
    y: number;
  }>({ open: false, x: 0, y: 0 });

  const openAt = (x: number, y: number) => setState({ open: true, x, y });
  const openFromEvent = (
    event: globalThis.MouseEvent | { clientX: number; clientY: number },
  ) => {
    if ("preventDefault" in event) event.preventDefault();
    openAt(event.clientX, event.clientY);
  };
  const close = () => setState((s) => ({ ...s, open: false }));

  return { ...state, openAt, openFromEvent, close };
}
