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
    /** Dark-background tile (white mark) — default for OS Light / preference `dark` */
    dark: "coreside-dock-dark.png",
    /** Light-background tile (black mark) — default for OS Dark / preference `light` */
    light: "coreside-dock-light.png",
  },
  note:
    "In-app logos follow Coreside Appearance. Dock icon defaults to Auto (OS appearance); users can lock Dark or Light tile in Settings.",
} as const;

export type BrandAppearance = "light" | "dark";

export function inAppLogoFor(appearance: BrandAppearance): string {
  return appearance === "dark"
    ? BRAND_MANIFEST.inApp.dark
    : BRAND_MANIFEST.inApp.light;
}
