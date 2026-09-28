/**
 * Interface transparency token computation.
 * Separates wallpaper intensity (renderer opacity) from panel translucency.
 */

export const INTERFACE_TRANSPARENCY_MIN = 0;
export const INTERFACE_TRANSPARENCY_MAX = 100;
export const INTERFACE_TRANSPARENCY_DEFAULT = 35;
export const INTERFACE_TRANSPARENCY_STEP = 1;

export const INTERFACE_TRANSPARENCY_PRESETS = [
  { id: "solid", label: "Solid", value: 0 },
  { id: "balanced", label: "Balanced", value: 35 },
  { id: "immersive", label: "Immersive", value: 70 },
  { id: "maximum", label: "Maximum", value: 100 },
] as const;

export type InterfaceTransparencyTokens = {
  /** User preference 0–100. */
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
  /**
   * Panel frosted-glass blur in px. Must fall with transparency — fixed blur
   * keeps chrome looking opaque even when panel alpha approaches 0.
   */
  backdropBlurPx: number;
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
 *
 * SEMANTICS (Section 45):
 * - 0% transparency = normal opaque interface (alphas = 1.0, no wallpaper shows through).
 * - 100% transparency = maximum wallpaper show-through:
 *   - main panels are substantially transparent (~0.02 alpha);
 *   - cards drop to subtle glass translucency (~0.08 alpha) instead of heavy blocks;
 *   - controls remain legible with sleek translucent backing (~0.18 alpha);
 *   - headers maintain a subtle boundary scrim (~0.06 alpha);
 *   - modals keep sufficient contrast (~0.45 alpha).
 * - Distinct from wallpaper layer opacity (how bright the wallpaper image/canvas renders).
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
      backdropBlurPx: 0,
    };
  }

  const effective = Math.max(0, pref - boost);
  if (effective === 0) {
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
      backdropBlurPx: 0,
    };
  }

  // Perceptual curve: compress low values so early transparency shows visible
  // steps rather than feeling mostly opaque until 50%. The quadratic segment
  // (t < 0.5) provides smoother ramp-up; linear above keeps high values predictable.
  const t = effective / 100;
  const perceptual = t < 0.5 ? t * t * 2 : t;
  const panelAlpha = clamp01(1 - perceptual * 0.98);
  const sidebarAlpha = clamp01(1 - perceptual * 0.99);
  // Cards: lower min so max transparency reveals wallpaper cleanly through content areas.
  const cardAlpha = clamp01(Math.max(0.08, 1 - perceptual * 0.92));
  // Controls: remain readable and distinct, but visibly translucent at high transparency.
  const controlAlpha = clamp01(Math.max(0.18, 1 - perceptual * 0.82));
  // Headers: subtle scrim, not opaque.
  const headerAlpha = clamp01(Math.max(0.06, 1 - perceptual * 0.94));
  // Modals: keep stronger surface for transient high-information UI.
  const modalAlpha = clamp01(Math.max(0.45, 1 - perceptual * 0.55));
  const scrimAlpha = clamp01(boost / 100 + (effective > 40 ? 0.08 : 0));
  // Frosted blur must collapse toward 0 as transparency rises; otherwise the
  // sampled backdrop itself paints an opaque-looking veil over the wallpaper.
  // Only wallpaper-scoped CSS consumes --core-backdrop-blur; base chrome keeps
  // a fixed blur so inactive-wallpaper mode is unaffected when this is 0.
  const backdropBlurPx = Math.round(12 * (1 - perceptual) * 10) / 10;

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
    backdropBlurPx,
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
    "--core-backdrop-blur",
    `${tokens.backdropBlurPx}px`,
  );
  root.style.setProperty(
    "--core-content-overlay",
    `color-mix(in srgb, var(--surface) ${Math.round(tokens.panelAlpha * 100)}%, transparent)`,
  );
  root.style.setProperty(
    "--core-sidebar-overlay",
    `color-mix(in srgb, var(--surface) ${Math.round(tokens.sidebarAlpha * 100)}%, transparent)`,
  );
  // Nested surfaces must use these — opaque var(--surface) defeats panel translucency.
  root.style.setProperty(
    "--core-card-overlay",
    `color-mix(in srgb, var(--surface) ${Math.round(tokens.cardAlpha * 100)}%, transparent)`,
  );
  root.style.setProperty(
    "--core-muted-overlay",
    `color-mix(in srgb, var(--surface-muted) ${Math.round(tokens.cardAlpha * 100)}%, transparent)`,
  );
  root.style.setProperty(
    "--core-control-overlay",
    `color-mix(in srgb, var(--surface) ${Math.round(tokens.controlAlpha * 100)}%, transparent)`,
  );
  root.style.setProperty(
    "--core-header-overlay",
    `color-mix(in srgb, var(--surface) ${Math.round(tokens.headerAlpha * 100)}%, transparent)`,
  );
  root.style.setProperty(
    "--core-modal-overlay",
    `color-mix(in srgb, var(--surface) ${Math.round(tokens.modalAlpha * 100)}%, transparent)`,
  );
}

function clamp01(n: number): number {
  return Math.round(Math.min(1, Math.max(0, n)) * 10000) / 10000;
}
