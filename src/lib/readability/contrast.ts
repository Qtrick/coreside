/** sRGB channel 0–1 → linear for relative luminance (WCAG 2.x). */
function channelToLinear(channel: number): number {
  const c = channel / 255;
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

export type RgbColor = { r: number; g: number; b: number };

/** Parse `#rgb` / `#rrggbb` (optional leading `#`). Returns null if invalid. */
export function parseHexColor(hex: string): RgbColor | null {
  const raw = hex.trim().replace(/^#/, "");
  if (!/^[0-9a-fA-F]{3}$|^[0-9a-fA-F]{6}$/.test(raw)) return null;
  const full =
    raw.length === 3
      ? raw
          .split("")
          .map((ch) => ch + ch)
          .join("")
      : raw;
  return {
    r: Number.parseInt(full.slice(0, 2), 16),
    g: Number.parseInt(full.slice(2, 4), 16),
    b: Number.parseInt(full.slice(4, 6), 16),
  };
}

function toRgb(color: string | RgbColor): RgbColor | null {
  if (typeof color === "string") return parseHexColor(color);
  if (
    Number.isFinite(color.r) &&
    Number.isFinite(color.g) &&
    Number.isFinite(color.b)
  ) {
    return color;
  }
  return null;
}

function clampByte(n: number): number {
  return Math.max(0, Math.min(255, Math.round(n)));
}

export function rgbToHex(rgb: RgbColor): string {
  const r = clampByte(rgb.r).toString(16).padStart(2, "0");
  const g = clampByte(rgb.g).toString(16).padStart(2, "0");
  const b = clampByte(rgb.b).toString(16).padStart(2, "0");
  return `#${r}${g}${b}`;
}

function mixRgb(from: RgbColor, to: RgbColor, t: number): RgbColor {
  const u = Math.max(0, Math.min(1, t));
  return {
    r: from.r + (to.r - from.r) * u,
    g: from.g + (to.g - from.g) * u,
    b: from.b + (to.b - from.b) * u,
  };
}

/** Relative luminance per WCAG 2.x (0 = black, 1 = white). */
export function relativeLuminance(color: string | RgbColor): number {
  const rgb = toRgb(color);
  if (!rgb) return 0;
  const r = channelToLinear(rgb.r);
  const g = channelToLinear(rgb.g);
  const b = channelToLinear(rgb.b);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/**
 * Contrast ratio between two colors (WCAG 2.x), range 1–21.
 * Order does not matter; always returns (lighter + 0.05) / (darker + 0.05).
 */
export function contrastRatio(
  foreground: string | RgbColor,
  background: string | RgbColor,
): number {
  const l1 = relativeLuminance(foreground);
  const l2 = relativeLuminance(background);
  const lighter = Math.max(l1, l2);
  const darker = Math.min(l1, l2);
  return (lighter + 0.05) / (darker + 0.05);
}

/**
 * Ensure a foreground color stays readable on a background.
 * Dark backgrounds → lighten toward white; light backgrounds → darken toward black.
 * Preserves hue when possible; returns the original string when already readable or unparsable.
 */
export function ensureReadableForeground(
  foreground: string,
  background: string,
  minRatio = 4.5,
): string {
  const fg = parseHexColor(foreground);
  const bg = parseHexColor(background);
  if (!fg || !bg) return foreground;

  if (contrastRatio(fg, bg) >= minRatio) {
    return rgbToHex(fg);
  }

  // Move toward white on dark surfaces, toward black on light surfaces.
  const target: RgbColor =
    relativeLuminance(bg) > 0.45
      ? { r: 0, g: 0, b: 0 }
      : { r: 255, g: 255, b: 255 };

  // Aim slightly above the minimum so 8-bit rounding still clears the bar.
  const targetRatio = minRatio * 1.02;

  let lo = 0;
  let hi = 1;
  let best = mixRgb(fg, target, 1);

  for (let i = 0; i < 18; i += 1) {
    const mid = (lo + hi) / 2;
    const candidate = mixRgb(fg, target, mid);
    if (contrastRatio(candidate, bg) >= targetRatio) {
      best = candidate;
      hi = mid;
    } else {
      lo = mid;
    }
  }

  let hex = rgbToHex(best);
  // If rounding still undershoots, take one more step toward the target.
  if (contrastRatio(hex, bg) < minRatio) {
    hex = rgbToHex(mixRgb(parseHexColor(hex) ?? best, target, 0.08));
  }
  return hex;
}
