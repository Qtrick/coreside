/**
 * Product capability gate for manual Dock icon selection (Classic Dark / Light / Split).
 *
 * CURRENT PRODUCT: false — Coreside follows macOS via the packaged adaptive icon.
 * The manual subsystem remains in source for intentional future reactivation.
 *
 * This is a compile-time / source-level product flag. It is not user-configurable,
 * not a Base Setting, not an env var, and not remotely toggleable.
 *
 * To re-enable later: set to true, mount ManualDockIconSelector in Appearance, and
 * ensure Rust `MANUAL_DOCK_ICON_SELECTION_ENABLED` matches.
 */
export const MANUAL_DOCK_ICON_SELECTION_ENABLED = false;

/** Effective runtime preference while the product capability is disabled. */
export function effectiveDockIconForProduct(
  config: { authority: string },
): "follow_macos" | "manual" {
  if (!MANUAL_DOCK_ICON_SELECTION_ENABLED) return "follow_macos";
  return config.authority === "manual" ? "manual" : "follow_macos";
}
