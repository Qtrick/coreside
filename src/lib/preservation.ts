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
  return { componentId, fieldId, selectionStart, selectionEnd };
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
    target = root.querySelector<HTMLElement>(
      `[data-component-id="${CSS.escape(snapshot.componentId)}"] input, [data-component-id="${CSS.escape(snapshot.componentId)}"] textarea, [data-component-id="${CSS.escape(snapshot.componentId)}"] button, [data-component-id="${CSS.escape(snapshot.componentId)}"] select`,
    );
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
