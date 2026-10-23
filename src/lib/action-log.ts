export type ActionLogMode = "off" | "always" | "intelligent";

/** Authoritative lifecycle statuses for Action Log steps. */
export type ActionEventStatus =
  | "pending"
  | "running"
  | "completed"
  | "succeeded"
  | "failed"
  | "cancelled"
  | "awaitingApproval"
  | "blocked";

export type AgentActionEntry = {
  id: string;
  label: string;
  eventType?: string;
  status: ActionEventStatus;
  stepId?: string;
  startedAt?: number;
  completedAt?: number;
  durationMs?: number;
};

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
  "automation_proposed",
  "automation_applied",
]);

export function normalizeActionStatus(raw: unknown): ActionEventStatus {
  const s = typeof raw === "string" ? raw.trim() : "";
  switch (s) {
    case "pending":
      return "pending";
    case "running":
      return "running";
    case "completed":
    case "succeeded":
      return "completed";
    case "failed":
      return "failed";
    case "cancelled":
      return "cancelled";
    case "awaitingApproval":
    case "awaiting_approval":
      return "awaitingApproval";
    case "blocked":
      return "blocked";
    default:
      return "completed";
  }
}

export function isTerminalActionStatus(status: ActionEventStatus): boolean {
  return (
    status === "completed" ||
    status === "succeeded" ||
    status === "failed" ||
    status === "cancelled" ||
    status === "blocked"
  );
}

export function isSuccessActionStatus(status: ActionEventStatus): boolean {
  return status === "completed" || status === "succeeded";
}

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
    label.includes("referenced @") ||
    label.includes("automation")
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

/** Upsert live/saved steps by stepId so start→terminal stays one row. */
export function upsertAgentAction(
  actions: AgentActionEntry[],
  entry: AgentActionEntry,
  limit = 12,
): AgentActionEntry[] {
  if (entry.stepId) {
    const idx = actions.findIndex((a) => a.stepId === entry.stepId);
    if (idx >= 0) {
      const next = [...actions];
      const prev = next[idx]!;
      next[idx] = {
        ...prev,
        ...entry,
        id: entry.id || prev.id,
        startedAt: entry.startedAt ?? prev.startedAt,
      };
      return next.slice(-limit);
    }
  }
  return [...actions, entry].slice(-limit);
}

export function actionEntryLabels(actions: AgentActionEntry[]): string[] {
  return actions.map((a) => a.label);
}

export function coerceAgentActionEntry(
  raw: string | AgentActionEntry,
  index = 0,
): AgentActionEntry {
  if (typeof raw === "string") {
    return {
      id: `act-legacy-${index}`,
      label: raw,
      status: "completed",
    };
  }
  return {
    ...raw,
    status: normalizeActionStatus(raw.status),
  };
}

export function formatActionDuration(ms: number | undefined | null): string | null {
  if (ms == null || !Number.isFinite(ms) || ms < 0) return null;
  if (ms < 1000) return `${Math.round(ms)}ms`;
  const seconds = ms / 1000;
  if (seconds < 60) {
    return seconds < 10 ? `${seconds.toFixed(1)}s` : `${Math.round(seconds)}s`;
  }
  const minutes = Math.floor(seconds / 60);
  const rem = Math.round(seconds % 60);
  return rem > 0 ? `${minutes}m ${rem}s` : `${minutes}m`;
}

export function totalActionDurationMs(
  events: Array<{ durationMs?: number; startedAt?: number; completedAt?: number }>,
): number | null {
  let sum = 0;
  let any = false;
  for (const e of events) {
    if (typeof e.durationMs === "number" && Number.isFinite(e.durationMs)) {
      sum += Math.max(0, e.durationMs);
      any = true;
      continue;
    }
    if (
      typeof e.startedAt === "number" &&
      typeof e.completedAt === "number" &&
      e.completedAt >= e.startedAt
    ) {
      sum += e.completedAt - e.startedAt;
      any = true;
    }
  }
  return any ? sum : null;
}

/** Compact card title from final step mix — never invents success. */
export function actionTimelineSummary(events: AgentActionEntry[]): {
  title: string;
  tone: "success" | "error" | "active" | "neutral";
} {
  if (events.length === 0) {
    return { title: "Working", tone: "active" };
  }
  const statuses = events.map((e) => normalizeActionStatus(e.status));
  const types = events.map((e) => (e.eventType ?? "").trim());
  if (statuses.some((s) => s === "running" || s === "pending")) {
    return { title: "Working on your request", tone: "active" };
  }
  // Automation proposals before generic awaitingApproval so the title is specific.
  if (types.some((t) => t === "automation_proposed" || t === "automation_scheduled")) {
    if (statuses.some((s) => s === "awaitingApproval")) {
      return { title: "Automation proposed", tone: "neutral" };
    }
    if (statuses.some((s) => s === "cancelled")) {
      return { title: "Automation discarded", tone: "neutral" };
    }
    return { title: "Automation scheduled", tone: "success" };
  }
  if (statuses.some((s) => s === "awaitingApproval")) {
    return { title: "Ready for your review", tone: "neutral" };
  }
  if (statuses.some((s) => s === "failed" || s === "blocked")) {
    return { title: "Needs attention", tone: "error" };
  }
  if (statuses.every((s) => s === "cancelled")) {
    return { title: "Stopped", tone: "neutral" };
  }
  // Proposal-only turns: do not claim the change was built or the automation ran.
  if (types.some((t) => t === "change_applied")) {
    return { title: "Built it", tone: "success" };
  }
  if (types.some((t) => t === "tool_change_proposed")) {
    return { title: "Change ready for review", tone: "neutral" };
  }
  if (types.some((t) => t.includes("search") || t === "tool_completed")) {
    return { title: "Finished researching", tone: "success" };
  }
  if (types.some((t) => t.includes("automation"))) {
    return { title: "Automation updated", tone: "success" };
  }
  return { title: "Done", tone: "success" };
}

/** Capability icon hint for timeline rows (CSS class suffix). */
/**
 * After the user decides a proposal, message metadata still has awaitingApproval
 * on change_proposed / automation_proposed rows. Correlate by event family so an
 * automation decision does not falsely resolve an unrelated kernel proposal.
 */
export function resolveActionEventsAfterProposal(
  events: AgentActionEntry[],
  kernelProposalStatus: string | undefined | null,
  automationChangeStatus?: string | null,
): AgentActionEntry[] {
  const kernel = (kernelProposalStatus ?? "").toLowerCase();
  const automation = (automationChangeStatus ?? "").toLowerCase();
  return events.map((e) => {
    if (normalizeActionStatus(e.status) !== "awaitingApproval") return e;
    const ty = (e.eventType ?? "").toLowerCase();
    const isAutomation = ty.includes("automation");
    const isKernelChange =
      ty.includes("change") ||
      ty === "tool_change_proposed" ||
      ty.includes("propos");

    if (isAutomation) {
      if (automation !== "applied" && automation !== "discarded" && automation !== "rejected") {
        return e;
      }
      return {
        ...e,
        status: (automation === "applied" ? "completed" : "cancelled") as ActionEventStatus,
        eventType:
          automation === "applied" ? "automation_scheduled" : e.eventType,
      };
    }

    if (!isKernelChange) return e;
    if (kernel !== "applied" && kernel !== "discarded" && kernel !== "rejected") {
      return e;
    }
    const nextStatus: ActionEventStatus =
      kernel === "applied" ? "completed" : "cancelled";
    return {
      ...e,
      status: nextStatus,
      eventType:
        kernel === "applied"
          ? e.eventType === "tool_change_proposed"
            ? "tool_change_proposed"
            : "change_applied"
          : e.eventType,
    };
  });
}

export function actionCapabilityKind(event: {
  eventType?: string;
  label?: string;
}): "search" | "crawl" | "tool" | "change" | "automation" | "context" | "generic" {
  const ty = (event.eventType ?? "").toLowerCase();
  const label = (event.label ?? "").toLowerCase();
  if (ty.includes("search") || label.includes("search") || label.includes("web")) {
    return "search";
  }
  if (ty.includes("crawl") || label.includes("fetch") || label.includes("page")) {
    return "crawl";
  }
  if (ty.includes("automation") || label.includes("automation")) {
    return "automation";
  }
  if (
    ty.includes("change") ||
    label.includes("propos") ||
    label.includes("appear") ||
    label.includes("appl")
  ) {
    return "change";
  }
  if (ty.includes("tool") || label.includes("tool")) {
    return "tool";
  }
  if (ty.includes("context") || ty.includes("project") || label.includes("project")) {
    return "context";
  }
  return "generic";
}
