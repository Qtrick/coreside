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
    /**
     * Split mark source (1254×1254, no alpha). Protected reference for Icon Composer /
     * adaptive packaging. Not a temporary NSApplication.applicationIconImage override —
     * runtime PNG overrides do not follow macOS Icon & Widget Style.
     */
    splitSource: "coreside-dock-split-source.png",
  },
  note:
    "In-app logos follow Coreside Appearance. Settings Dock tiles are Classic Dark/Light (or Auto appearance). Split source artwork is a packaging/Icon Composer reference — not a runtime Dock preference. Icon & Widget Style follows macOS only when the packaged adaptive icon is authoritative; temporary PNG overrides cannot provide it.",
} as const;

export type BrandAppearance = "light" | "dark";

export function inAppLogoFor(appearance: BrandAppearance): string {
  return appearance === "dark"
    ? BRAND_MANIFEST.inApp.dark
    : BRAND_MANIFEST.inApp.light;
}
