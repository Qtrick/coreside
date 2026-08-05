/** Canonical wallpaper / appearance hex: `#RRGGBB` (lowercase). */

const HEX6 = /^#[0-9a-fA-F]{6}$/;
const HEX3 = /^#[0-9a-fA-F]{3}$/;

/** True when already canonical `#RRGGBB`. */
export function isCanonicalHexColor(value: string): boolean {
  return HEX6.test(value.trim());
}

/**
 * Normalize `#RGB` / `#RRGGBB` → lowercase `#RRGGBB`.
 * Returns null when invalid (aligned with Rust wallpaper / settings hex rules).
 */
export function normalizeCanonicalHex(raw: string): string | null {
  const s = raw.trim();
  if (HEX6.test(s)) return s.toLowerCase();
  if (HEX3.test(s)) {
    const chars = s.slice(1).split("");
    return `#${chars[0]}${chars[0]}${chars[1]}${chars[1]}${chars[2]}${chars[2]}`.toLowerCase();
  }
  return null;
}
