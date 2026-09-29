/** Trusted preservation helpers — mirrors Rust runtime_v2::preservation. */

export type PreservationPolicy =
  | "replace"
  | "preserve_instance"
  | "preserve_state"
  | "preserve_user_input"
  | "preserve_media_state"
  | "preserve_scroll"
  | "preserve_focus"
  | "preserve_selection"
  | "preserve_if_compatible"
  | "reset_explicitly";

export type FocusSnapshot = {
  componentId?: string | null;
  fieldId?: string | null;
  selectionStart?: number | null;
  selectionEnd?: number | null;
  /** Contenteditable caret/selection as text offsets within the focused node. */
  selectionAnchorOffset?: number | null;
  selectionFocusOffset?: number | null;
};

export type ScrollSnapshot = Record<
  string,
  { scrollTop: number; scrollLeft?: number }
>;

export type MediaSnapshot = Record<
  string,
  {
    currentTime?: number;
    paused?: boolean;
    muted?: boolean;
    volume?: number;
  }
>;

function activeElementSnapshot(root: HTMLElement | null): FocusSnapshot {
  if (!root) return {};
  const active = document.activeElement;
  if (!active || !root.contains(active)) return {};
  const componentId =
    active.closest<HTMLElement>("[data-component-id]")?.dataset.componentId ?? null;
  const fieldId =
    active instanceof HTMLElement && active.id ? active.id : null;
  const selectionStart =
    active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement
      ? active.selectionStart
      : null;
  const selectionEnd =
    active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement
      ? active.selectionEnd
      : null;
  let selectionAnchorOffset: number | null = null;
  let selectionFocusOffset: number | null = null;
  const contentEditable =
    active instanceof HTMLElement &&
    (active.isContentEditable ||
      active.getAttribute("contenteditable") === "true" ||
      active.getAttribute("contenteditable") === "");
  if (
    contentEditable &&
    !(active instanceof HTMLInputElement) &&
    !(active instanceof HTMLTextAreaElement)
  ) {
    const sel = document.getSelection();
    if (sel && sel.rangeCount > 0 && sel.anchorNode && active.contains(sel.anchorNode)) {
      selectionAnchorOffset = textOffsetInRoot(active, sel.anchorNode, sel.anchorOffset);
      selectionFocusOffset = textOffsetInRoot(active, sel.focusNode, sel.focusOffset);
    }
  }
  return {
    componentId,
    fieldId,
    selectionStart,
    selectionEnd,
    selectionAnchorOffset,
    selectionFocusOffset,
  };
}

function subtreeTextLength(node: Node): number {
  if (node.nodeType === Node.TEXT_NODE) return node.textContent?.length ?? 0;
  let total = 0;
  for (let i = 0; i < node.childNodes.length; i++) {
    total += subtreeTextLength(node.childNodes[i]!);
  }
  return total;
}

/** Map a DOM Selection point to a UTF-16 code-unit offset within `root`'s text. */
function textOffsetInRoot(
  root: Node,
  node: Node | null,
  offset: number,
): number | null {
  if (!node) return null;
  if (node.nodeType === Node.TEXT_NODE) {
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    let total = 0;
    let current: Node | null = walker.nextNode();
    while (current) {
      if (current === node) {
        return total + Math.max(0, Math.min(offset, current.textContent?.length ?? 0));
      }
      total += current.textContent?.length ?? 0;
      current = walker.nextNode();
    }
    return null;
  }
  // Element caret: `offset` is a child index.
  if (node !== root && !root.contains(node)) return null;
  let total = 0;
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  let current: Node | null = walker.nextNode();
  while (current) {
    if (node.contains(current)) break;
    if (node.compareDocumentPosition(current) & Node.DOCUMENT_POSITION_FOLLOWING) break;
    total += current.textContent?.length ?? 0;
    current = walker.nextNode();
  }
  const childLimit = Math.max(0, Math.min(offset, node.childNodes.length));
  for (let i = 0; i < childLimit; i++) {
    total += subtreeTextLength(node.childNodes[i]!);
  }
  return total;
}

function setContentEditableSelection(
  root: HTMLElement,
  anchorOffset: number,
  focusOffset: number,
): void {
  const anchor = pointFromTextOffset(root, anchorOffset);
  const focus = pointFromTextOffset(root, focusOffset);
  if (!anchor || !focus) return;
  const sel = document.getSelection();
  if (!sel) return;
  try {
    // Handles forward and backward selections without Range ordering tricks.
    sel.setBaseAndExtent(anchor.node, anchor.offset, focus.node, focus.offset);
  } catch {
    const forward = anchorOffset <= focusOffset;
    const start = forward ? anchor : focus;
    const end = forward ? focus : anchor;
    const range = document.createRange();
    try {
      range.setStart(start.node, start.offset);
      range.setEnd(end.node, end.offset);
    } catch {
      return;
    }
    sel.removeAllRanges();
    sel.addRange(range);
    if (!forward) {
      try {
        sel.collapseToEnd();
        sel.extend(start.node, start.offset);
      } catch {
        // leave forward range when extend is unavailable
      }
    }
  }
}

function pointFromTextOffset(
  root: Node,
  offset: number,
): { node: Node; offset: number } | null {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  let remaining = Math.max(0, offset);
  let current: Node | null = walker.nextNode();
  let last: Node | null = null;
  while (current) {
    last = current;
    const len = current.textContent?.length ?? 0;
    if (remaining <= len) {
      return { node: current, offset: remaining };
    }
    remaining -= len;
    current = walker.nextNode();
  }
  if (last) {
    return { node: last, offset: last.textContent?.length ?? 0 };
  }
  return { node: root, offset: 0 };
}

export function captureFocusSnapshot(root: HTMLElement | null): FocusSnapshot {
  return activeElementSnapshot(root);
}

export function restoreFocusSnapshot(
  root: HTMLElement | null,
  snapshot: FocusSnapshot | null | undefined,
): void {
  if (!root || !snapshot) return;
  let target: HTMLElement | null = null;
  if (snapshot.fieldId) {
    const byId = root.querySelector<HTMLElement>(`#${CSS.escape(snapshot.fieldId)}`);
    if (byId) target = byId;
  }
  if (!target && snapshot.componentId) {
    const compEl = root.querySelector<HTMLElement>(
      `[data-component-id="${CSS.escape(snapshot.componentId)}"]`,
    );
    if (compEl) {
      if (
        compEl instanceof HTMLInputElement ||
        compEl instanceof HTMLTextAreaElement ||
        compEl instanceof HTMLButtonElement ||
        compEl instanceof HTMLSelectElement ||
        compEl.tabIndex >= 0 ||
        compEl.isContentEditable
      ) {
        target = compEl;
      } else {
        target = compEl.querySelector<HTMLElement>(
          "input, textarea, button, select, [tabindex]:not([tabindex='-1']), [contenteditable='true']",
        );
      }
    }
  }
  if (!target) return;
  target.focus({ preventScroll: true });
  if (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement
  ) {
    const start = snapshot.selectionStart ?? target.value.length;
    const end = snapshot.selectionEnd ?? start;
    try {
      target.setSelectionRange(start, end);
    } catch {
      // Some input types do not support selection.
    }
    return;
  }
  if (
    (target.isContentEditable ||
      target.getAttribute("contenteditable") === "true" ||
      target.getAttribute("contenteditable") === "") &&
    typeof snapshot.selectionAnchorOffset === "number" &&
    typeof snapshot.selectionFocusOffset === "number"
  ) {
    setContentEditableSelection(
      target,
      snapshot.selectionAnchorOffset,
      snapshot.selectionFocusOffset,
    );
  }
}

export function captureScrollSnapshot(root: HTMLElement | null): ScrollSnapshot {
  if (!root) return {};
  const out: ScrollSnapshot = {};
  const mark = (el: HTMLElement, key: string) => {
    if (el.scrollHeight <= el.clientHeight && el.scrollWidth <= el.clientWidth) {
      return;
    }
    out[key] = {
      scrollTop: el.scrollTop,
      scrollLeft: el.scrollLeft,
    };
  };
  mark(root, "root");
  root.querySelectorAll<HTMLElement>("[data-scroll-key]").forEach((el) => {
    const key = el.dataset.scrollKey;
    if (key) mark(el, key);
  });
  return out;
}

export function restoreScrollSnapshot(
  root: HTMLElement | null,
  snapshot: ScrollSnapshot | null | undefined,
): void {
  if (!root || !snapshot) return;
  const rootSnap = snapshot.root;
  if (rootSnap) {
    root.scrollTop = rootSnap.scrollTop;
    if (typeof rootSnap.scrollLeft === "number") {
      root.scrollLeft = rootSnap.scrollLeft;
    }
  }
  for (const [key, pos] of Object.entries(snapshot)) {
    if (key === "root") continue;
    const el = root.querySelector<HTMLElement>(
      `[data-scroll-key="${CSS.escape(key)}"]`,
    );
    if (!el) continue;
    el.scrollTop = pos.scrollTop;
    if (typeof pos.scrollLeft === "number") {
      el.scrollLeft = pos.scrollLeft;
    }
  }
}

export function captureMediaSnapshot(root: HTMLElement | null): MediaSnapshot {
  if (!root) return {};
  const out: MediaSnapshot = {};
  root.querySelectorAll<HTMLMediaElement>("audio, video").forEach((el) => {
    const key =
      el.dataset.componentId ||
      el.getAttribute("data-media-key") ||
      el.id ||
      undefined;
    if (!key) return;
    out[key] = {
      currentTime: el.currentTime,
      paused: el.paused,
      muted: el.muted,
      volume: el.volume,
    };
  });
  return out;
}

export function restoreMediaSnapshot(
  root: HTMLElement | null,
  snapshot: MediaSnapshot | null | undefined,
): void {
  if (!root || !snapshot) return;
  for (const [key, state] of Object.entries(snapshot)) {
    const el = root.querySelector<HTMLMediaElement>(
      `[data-component-id="${CSS.escape(key)}"], [data-media-key="${CSS.escape(key)}"], #${CSS.escape(key)}`,
    );
    if (!el) continue;
    if (typeof state.currentTime === "number") {
      try {
        el.currentTime = state.currentTime;
      } catch {
        // Media may not be ready.
      }
    }
    if (typeof state.muted === "boolean") el.muted = state.muted;
    if (typeof state.volume === "number") el.volume = state.volume;
    // ponytail: never autoplay on restore — always remain paused.
    el.pause();
    if (state.paused === false) {
      // Intentionally ignored — suspension restore must not autoplay.
    }
  }
}

/** Whether component state should survive a definition patch. */
export function shouldPreserveComponent(
  policy: PreservationPolicy,
  oldType: string,
  newType: string,
): boolean {
  const compatible = oldType === newType;
  switch (policy) {
    case "replace":
    case "reset_explicitly":
      return false;
    case "preserve_instance":
    case "preserve_state":
    case "preserve_user_input":
    case "preserve_media_state":
    case "preserve_scroll":
    case "preserve_focus":
    case "preserve_selection":
      return true;
    case "preserve_if_compatible":
      return compatible;
    default:
      return compatible;
  }
}

/**
 * Merge unpersisted local user edits over canonical state after an agent Sync reload.
 * Dirty keys win; keys listed in `resetKeys` (explicit reset / removed bindings) stay canonical.
 */
export function reconcileDirtyOverCanonical(
  canonical: Record<string, unknown>,
  dirty: Record<string, unknown>,
  resetKeys: Iterable<string> = [],
): Record<string, unknown> {
  const blocked = new Set(
    [...resetKeys].map((k) => k.trim()).filter(Boolean),
  );
  const next: Record<string, unknown> = { ...canonical };
  for (const [key, value] of Object.entries(dirty)) {
    if (blocked.has(key)) continue;
    next[key] = value;
  }
  return next;
}
