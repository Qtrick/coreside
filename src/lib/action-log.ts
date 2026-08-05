export type ActionLogMode = "off" | "always" | "intelligent";

export function parseActionLogMode(raw: unknown): ActionLogMode {
  if (typeof raw !== "string") {
    if (raw === true) return "always";
    return "off";
  }
  const t = raw.trim().toLowerCase();
  if (t === "always" || t === "on" || t === "true" || t === "1" || t === "yes") {
    return "always";
  }
  if (t === "intelligent" || t === "auto" || t === "smart") {
    return "intelligent";
  }
  return "off";
}

const SUBSTANTIVE_TYPES = new Set([
  "tool_started",
  "tool_completed",
  "tool_failed",
  "tool_reference_resolved",
  "change_applied",
  "change_proposed",
  "tool_change_proposed",
  "search_started",
  "search_completed",
  "crawl_started",
  "crawl_completed",
  "project_context_loaded",
]);

export function isSubstantiveActionEvent(event: {
  eventType?: string;
  label?: string;
}): boolean {
  const ty = (event.eventType ?? "").trim();
  if (SUBSTANTIVE_TYPES.has(ty)) return true;
  const label = (event.label ?? "").toLowerCase();
  return (
    label.includes("search") ||
    label.includes("crawl") ||
    label.includes("tool") ||
    label.includes("propos") ||
    label.includes("appear") ||
    label.includes("import") ||
    label.includes("fetch") ||
    label.includes("project") ||
    label.includes("referenced @")
  );
}

/** Whether the Action Log UI should appear for this mode + event set. */
export function shouldShowActionLog(
  mode: ActionLogMode,
  events: Array<{ eventType?: string; label?: string }>,
): boolean {
  if (mode === "off" || events.length === 0) return false;
  if (mode === "always") return true;
  return events.some(isSubstantiveActionEvent);
}
