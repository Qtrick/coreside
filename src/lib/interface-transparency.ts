/**
 * Interface transparency token computation.
 * Separates wallpaper intensity (renderer opacity) from panel translucency.
 */

export const INTERFACE_TRANSPARENCY_MIN = 0;
export const INTERFACE_TRANSPARENCY_MAX = 60;
export const INTERFACE_TRANSPARENCY_DEFAULT = 20;
export const INTERFACE_TRANSPARENCY_STEP = 1;

export const INTERFACE_TRANSPARENCY_PRESETS = [
  { id: "solid", label: "Solid", value: 0 },
  { id: "balanced", label: "Balanced", value: 20 },
  { id: "immersive", label: "Immersive", value: 40 },
] as const;

export type InterfaceTransparencyTokens = {
  /** User preference 0–60. */
  preference: number;
  /** Effective panel transparency after readability clamp. */
  effective: number;
  panelAlpha: number;
  sidebarAlpha: number;
  cardAlpha: number;
  controlAlpha: number;
  headerAlpha: number;
  modalAlpha: number;
  scrimAlpha: number;
};

/** Clamp preference into the allowed consumer range. */
export function clampInterfaceTransparency(value: unknown): number {
  const n = typeof value === "number" ? value : Number(value);
  if (!Number.isFinite(n)) return INTERFACE_TRANSPARENCY_DEFAULT;
  return Math.min(
    INTERFACE_TRANSPARENCY_MAX,
    Math.max(INTERFACE_TRANSPARENCY_MIN, Math.round(n)),
  );
}

/**
 * Compute semantic surface alphas from interface transparency %.
 * Higher transparency → lower alpha (more wallpaper shows through).
 * Cards/controls/modals keep stronger minimum surfaces for readability.
 */
export function computeInterfaceTransparencyTokens(
  preference: number,
  options?: { wallpaperActive?: boolean; readabilityBoost?: number },
): InterfaceTransparencyTokens {
  const pref = clampInterfaceTransparency(preference);
  const wallpaperActive = options?.wallpaperActive ?? true;
  const boost = Math.min(30, Math.max(0, options?.readabilityBoost ?? 0));

  // No wallpaper: keep solid surfaces regardless of saved preference.
  if (!wallpaperActive) {
    return {
      preference: pref,
      effective: 0,
      panelAlpha: 1,
      sidebarAlpha: 1,
      cardAlpha: 1,
      controlAlpha: 1,
      headerAlpha: 1,
      modalAlpha: 1,
      scrimAlpha: 0,
    };
  }

  const effective = Math.max(0, pref - boost);
  const t = effective / 100;
  const panelAlpha = clamp01(1 - t * 0.85);
  const sidebarAlpha = clamp01(panelAlpha - 0.04);
  const cardAlpha = clamp01(Math.max(0.82, panelAlpha + 0.12));
  const controlAlpha = clamp01(Math.max(0.88, panelAlpha + 0.16));
  const headerAlpha = clamp01(Math.max(0.86, panelAlpha + 0.1));
  const modalAlpha = clamp01(Math.max(0.94, panelAlpha + 0.2));
  const scrimAlpha = clamp01(boost / 100 + (effective > 40 ? 0.08 : 0));

  return {
    preference: pref,
    effective,
    panelAlpha,
    sidebarAlpha,
    cardAlpha,
    controlAlpha,
    headerAlpha,
    modalAlpha,
    scrimAlpha,
  };
}

export function applyInterfaceTransparencyCssVars(
  tokens: InterfaceTransparencyTokens,
): void {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  root.style.setProperty("--interface-transparency", String(tokens.preference));
  root.style.setProperty(
    "--interface-transparency-effective",
    String(tokens.effective),
  );
  root.style.setProperty("--core-panel-alpha", String(tokens.panelAlpha));
  root.style.setProperty("--core-sidebar-alpha", String(tokens.sidebarAlpha));
  root.style.setProperty("--core-card-alpha", String(tokens.cardAlpha));
  root.style.setProperty("--core-control-alpha", String(tokens.controlAlpha));
  root.style.setProperty("--core-header-alpha", String(tokens.headerAlpha));
  root.style.setProperty("--core-modal-alpha", String(tokens.modalAlpha));
  root.style.setProperty(
    "--core-readable-scrim-alpha",
    String(tokens.scrimAlpha),
  );
  root.style.setProperty(
    "--core-content-overlay",
    `color-mix(in srgb, var(--surface) ${Math.round(tokens.panelAlpha * 100)}%, transparent)`,
  );
  root.style.setProperty(
    "--core-sidebar-overlay",
    `color-mix(in srgb, var(--surface) ${Math.round(tokens.sidebarAlpha * 100)}%, transparent)`,
  );
}

function clamp01(n: number): number {
  return Math.min(1, Math.max(0, n));
}
