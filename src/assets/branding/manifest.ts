/**
 * Protected Coreside brand asset manifest.
 * Runtime AI must never mutate these paths or mappings.
 */

export const BRAND_MANIFEST = {
  productName: "Coreside",
  protected: true,
  inApp: {
    light: "/branding/coreside-mark-black-transparent.png",
    dark: "/branding/coreside-mark-white-transparent.png",
  },
  dock: {
    /** Dark-background tile (white mark) — Classic artwork */
    dark: "coreside-dock-dark.png",
    /** Light-background tile (black mark) — Classic artwork */
    light: "coreside-dock-light.png",
    /** Manual Split tile (generated from protected Split source) */
    split: "coreside-dock-split.png",
  },
  note:
    "Follow macOS clears the temporary Dock override so the packaged application icon is authoritative. Adaptive Icon & Widget Style needs Assets.car (blocked without Xcode/Icon Composer). Manual Classic/Split tiles are temporary AppKit overrides while Coreside is running.",
} as const;

export type BrandAppearance = "light" | "dark";

export function inAppLogoFor(appearance: BrandAppearance): string {
  return appearance === "dark"
    ? BRAND_MANIFEST.inApp.dark
    : BRAND_MANIFEST.inApp.light;
}
