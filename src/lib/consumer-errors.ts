/**
 * Normalize raw backend / schema errors into consumer-facing copy.
 * Developer Mode may still surface a redacted technical detail.
 */

const WALLPAPER_SCHEMA =
  /wallpaper\s*json\s*invalid|schemaVersion|missing field/i;
const WINDOW_ORCHESTRATOR =
  /window_orchestrator|failed to (set|get).*(size|position|monitor)/i;

export function consumerErrorMessage(
  error: unknown,
  fallback = "Something went wrong. Your previous state was kept.",
): { message: string; technical: string | null } {
  const raw =
    error instanceof Error
      ? error.message
      : typeof error === "string"
        ? error
        : fallback;
  const technical = raw.trim() || null;

  if (WALLPAPER_SCHEMA.test(raw)) {
    return {
      message:
        "Coreside could not apply that wallpaper. The previous appearance was restored.",
      technical,
    };
  }
  if (WINDOW_ORCHESTRATOR.test(raw)) {
    return {
      message:
        "Coreside could not adjust the window size. Your current layout was kept.",
      technical,
    };
  }
  // Strip common serde / path noise for consumers.
  if (/at line \d+ column \d+/i.test(raw) || /serde|Deserialize/i.test(raw)) {
    return {
      message: fallback,
      technical,
    };
  }
  return { message: raw || fallback, technical };
}
