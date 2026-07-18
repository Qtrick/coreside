import {
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";

export type ContextMenuItem = {
  id: string;
  label: string;
  onSelect: () => void;
  destructive?: boolean;
  disabled?: boolean;
  separatorBefore?: boolean;
};

type ContextMenuProps = {
  open: boolean;
  x: number;
  y: number;
  items: ContextMenuItem[];
  onClose: () => void;
  ariaLabel?: string;
};

export function ContextMenu({
  open,
  x,
  y,
  items,
  onClose,
  ariaLabel = "Context menu",
}: ContextMenuProps) {
  const menuId = useId();
  const menuRef = useRef<HTMLDivElement>(null);
  const [focusIndex, setFocusIndex] = useState(0);
  const [position, setPosition] = useState({ left: x, top: y });

  const clampPosition = useCallback(() => {
    const el = menuRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const margin = 8;
    let left = x;
    let top = y;
    if (left + rect.width > window.innerWidth - margin) {
      left = Math.max(margin, window.innerWidth - rect.width - margin);
    }
    if (top + rect.height > window.innerHeight - margin) {
      top = Math.max(margin, window.innerHeight - rect.height - margin);
    }
    setPosition({ left, top });
  }, [x, y]);

  useLayoutEffect(() => {
    if (!open) return;
    clampPosition();
  }, [open, items, clampPosition]);

  useEffect(() => {
    if (!open) return;
    const firstEnabled = items.findIndex((item) => !item.disabled);
    setFocusIndex(firstEnabled >= 0 ? firstEnabled : 0);
    requestAnimationFrame(() => {
      const el = menuRef.current?.querySelector<HTMLElement>(
        '[role="menuitem"]:not([aria-disabled="true"])',
      );
      el?.focus();
    });
  }, [open, items]);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: globalThis.MouseEvent) => {
      const target = event.target as Node;
      if (menuRef.current?.contains(target)) return;
      onClose();
    };
    const onKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      }
    };
    window.addEventListener("mousedown", onPointerDown);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("mousedown", onPointerDown);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [open, onClose]);

  const moveFocus = (delta: number) => {
    const selectable = items
      .map((item, index) => ({ item, index }))
      .filter(({ item }) => !item.disabled);
    if (selectable.length === 0) return;
    const current = selectable.findIndex(({ index }) => index === focusIndex);
    const next =
      selectable[(current + delta + selectable.length) % selectable.length];
    setFocusIndex(next.index);
    const button = menuRef.current?.querySelector<HTMLElement>(
      `[data-menu-index="${next.index}"]`,
    );
    button?.focus();
  };

  const onMenuKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      moveFocus(1);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      moveFocus(-1);
    } else if (event.key === "Home") {
      event.preventDefault();
      const first = items.findIndex((item) => !item.disabled);
      if (first >= 0) setFocusIndex(first);
    } else if (event.key === "End") {
      event.preventDefault();
      const last = [...items].reverse().findIndex((item) => !item.disabled);
      if (last >= 0) setFocusIndex(items.length - 1 - last);
    } else if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      const item = items[focusIndex];
      if (item && !item.disabled) {
        item.onSelect();
        onClose();
      }
    } else if (event.key === "Tab") {
      event.preventDefault();
      onClose();
    }
  };

  if (!open) return null;

  const content: ReactNode = (
    <div
      ref={menuRef}
      id={menuId}
      className="context-menu"
      role="menu"
      aria-label={ariaLabel}
      style={{ left: position.left, top: position.top }}
      onKeyDown={onMenuKeyDown}
    >
      {items.map((item, index) => (
        <div key={item.id}>
          {item.separatorBefore ? (
            <div className="context-menu-separator" role="separator" />
          ) : null}
          <button
            type="button"
            role="menuitem"
            data-menu-index={index}
            className={`context-menu-item${item.destructive ? " destructive" : ""}`}
            disabled={item.disabled}
            aria-disabled={item.disabled || undefined}
            tabIndex={item.disabled ? -1 : index === focusIndex ? 0 : -1}
            onClick={() => {
              if (item.disabled) return;
              item.onSelect();
              onClose();
            }}
            onMouseEnter={() => {
              if (!item.disabled) setFocusIndex(index);
            }}
          >
            {item.label}
          </button>
        </div>
      ))}
    </div>
  );

  return createPortal(content, document.body);
}

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
