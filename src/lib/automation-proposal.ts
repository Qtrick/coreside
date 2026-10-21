/**
 * Chat → automation proposal contract.
 *
 * Mirrors settings/kernel proposals: the agent proposes; the user confirms;
 * Rust upsert_automation + scheduler own persistence and execution.
 * Never auto-schedule from the model without confirmation.
 */

export type AutomationProposalTrigger =
  | { type: "interval"; intervalMinutes: number; timezone?: string | null }
  | { type: "daily"; time: string; timezone?: string | null }
  | { type: "weekly"; weekday: number; time: string; timezone?: string | null };

export type AutomationProposalAction =
  | {
      type: "cycle_workspace_backgrounds";
      workspaceId: string;
      presetIds: string[];
    }
  | {
      type: "set_workspace_background";
      workspaceId: string;
      presetId: string;
    }
  | {
      type: "set_tool_state_value";
      toolId: string;
      path: string;
      value: unknown;
    };

export type AutomationChangeProposal = {
  name: string;
  trigger: AutomationProposalTrigger;
  action: AutomationProposalAction;
  requiresAi?: boolean;
  applicationId?: string | null;
  changeSummary?: string | null;
  /** When true, chat proposals must not use ai_prompt (executor deferred). */
  scheduled?: boolean;
};

export type AutomationChangeStatus =
  | "none"
  | "pending"
  | "applied"
  | "discarded"
  | "failed";

const SAFE_ACTION_TYPES = new Set([
  "cycle_workspace_backgrounds",
  "set_workspace_background",
  "set_tool_state_value",
]);

function asRecord(raw: unknown): Record<string, unknown> | null {
  return raw && typeof raw === "object" && !Array.isArray(raw)
    ? (raw as Record<string, unknown>)
    : null;
}

function parseTrigger(raw: unknown): AutomationProposalTrigger | null {
  const t = asRecord(raw);
  if (!t || typeof t.type !== "string") return null;
  if (t.type === "interval") {
    const minutes = Number(t.intervalMinutes ?? t.interval_minutes);
    if (!Number.isFinite(minutes) || minutes < 5) return null;
    return {
      type: "interval",
      intervalMinutes: Math.floor(minutes),
      timezone: typeof t.timezone === "string" ? t.timezone : null,
    };
  }
  if (t.type === "daily") {
    const time = typeof t.time === "string" ? t.time.trim() : "";
    if (!/^\d{1,2}:\d{2}$/.test(time)) return null;
    return {
      type: "daily",
      time,
      timezone: typeof t.timezone === "string" ? t.timezone : null,
    };
  }
  if (t.type === "weekly") {
    const time = typeof t.time === "string" ? t.time.trim() : "";
    const weekday = Number(t.weekday);
    if (!/^\d{1,2}:\d{2}$/.test(time) || !Number.isFinite(weekday)) return null;
    const day = Math.floor(weekday);
    // 0 = Monday … 6 = Sunday (matches Rust AutomationTrigger::Weekly).
    if (day < 0 || day > 6) return null;
    return {
      type: "weekly",
      weekday: day,
      time,
      timezone: typeof t.timezone === "string" ? t.timezone : null,
    };
  }
  return null;
}

function parseAction(raw: unknown): AutomationProposalAction | null {
  const a = asRecord(raw);
  if (!a || typeof a.type !== "string") return null;
  if (!SAFE_ACTION_TYPES.has(a.type)) return null;
  if (a.type === "cycle_workspace_backgrounds") {
    const workspaceId = String(a.workspaceId ?? a.workspace_id ?? "").trim();
    const presetIds = Array.isArray(a.presetIds)
      ? a.presetIds.map(String).map((id) => id.trim()).filter(Boolean)
      : Array.isArray(a.preset_ids)
        ? a.preset_ids.map(String).map((id) => id.trim()).filter(Boolean)
        : [];
    // Rust validate_automation requires at least two presets for rotation.
    if (!workspaceId || presetIds.length < 2) return null;
    return { type: "cycle_workspace_backgrounds", workspaceId, presetIds };
  }
  if (a.type === "set_workspace_background") {
    const workspaceId = String(a.workspaceId ?? a.workspace_id ?? "").trim();
    const presetId = String(a.presetId ?? a.preset_id ?? "").trim();
    if (!workspaceId || !presetId) return null;
    return { type: "set_workspace_background", workspaceId, presetId };
  }
  if (a.type === "set_tool_state_value") {
    const toolId = String(a.toolId ?? a.tool_id ?? "").trim();
    const path = String(a.path ?? "").trim();
    if (!toolId || !path || path.includes("..") || path.startsWith("core.")) {
      return null;
    }
    return {
      type: "set_tool_state_value",
      toolId,
      path,
      value: a.value ?? null,
    };
  }
  return null;
}

/** Parse message.metadata.automationChange into a confirmable proposal. */
export function parseAutomationChange(
  raw: unknown,
): AutomationChangeProposal | null {
  const row = asRecord(raw);
  if (!row) return null;
  const name = typeof row.name === "string" ? row.name.trim() : "";
  if (!name) return null;
  const trigger = parseTrigger(row.trigger);
  const action = parseAction(row.action);
  if (!trigger || !action) return null;
  if (row.requiresAi === true || row.requires_ai === true) {
    // Chat path refuses AI-backed automations until executor support lands.
    return null;
  }
  return {
    name,
    trigger,
    action,
    requiresAi: false,
    applicationId:
      typeof row.applicationId === "string"
        ? row.applicationId
        : typeof row.application_id === "string"
          ? row.application_id
          : null,
    changeSummary:
      typeof row.changeSummary === "string"
        ? row.changeSummary
        : typeof row.change_summary === "string"
          ? row.change_summary
          : null,
    scheduled: false,
  };
}

export function automationChangeStatusFromMetadata(
  meta: Record<string, unknown> | null | undefined,
): AutomationChangeStatus {
  const raw = meta?.automationChangeStatus;
  if (raw === "pending" || raw === "applied" || raw === "discarded" || raw === "failed") {
    return raw;
  }
  if (meta?.automationChange) return "pending";
  return "none";
}

export function describeAutomationTrigger(trigger: AutomationProposalTrigger): string {
  if (trigger.type === "interval") {
    return `Every ${trigger.intervalMinutes} minutes`;
  }
  if (trigger.type === "daily") {
    return `Daily at ${trigger.time}`;
  }
  const days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
  const day = days[trigger.weekday] ?? `day ${trigger.weekday}`;
  return `Weekly on ${day} at ${trigger.time}`;
}

export function describeAutomationAction(action: AutomationProposalAction): string {
  if (action.type === "cycle_workspace_backgrounds") {
    return `Rotate ${action.presetIds.length} wallpaper${action.presetIds.length === 1 ? "" : "s"}`;
  }
  if (action.type === "set_workspace_background") {
    return "Set workspace wallpaper";
  }
  return `Update ${action.toolId} (${action.path})`;
}

/** Shape accepted by api.upsertAutomation / Rust UpsertAutomationInput. */
export function toUpsertAutomationInput(
  proposal: AutomationChangeProposal,
  options?: { sourceMessageId?: string },
): Record<string, unknown> {
  const trigger =
    proposal.trigger.type === "interval"
      ? {
          type: "interval",
          intervalMinutes: proposal.trigger.intervalMinutes,
          timezone: proposal.trigger.timezone ?? undefined,
        }
      : proposal.trigger.type === "daily"
        ? {
            type: "daily",
            time: proposal.trigger.time,
            timezone: proposal.trigger.timezone ?? undefined,
          }
        : {
            type: "weekly",
            weekday: proposal.trigger.weekday,
            time: proposal.trigger.time,
            timezone: proposal.trigger.timezone ?? undefined,
          };

  const action =
    proposal.action.type === "cycle_workspace_backgrounds"
      ? {
          type: "cycle_workspace_backgrounds",
          workspaceId: proposal.action.workspaceId,
          presetIds: proposal.action.presetIds,
        }
      : proposal.action.type === "set_workspace_background"
        ? {
            type: "set_workspace_background",
            workspaceId: proposal.action.workspaceId,
            presetId: proposal.action.presetId,
          }
        : {
            type: "set_tool_state_value",
            toolId: proposal.action.toolId,
            path: proposal.action.path,
            value: proposal.action.value,
          };

  // Stable id per source message so re-confirm after reload upserts, not duplicates.
  const sourceMessageId = options?.sourceMessageId?.trim();
  return {
    ...(sourceMessageId ? { id: `auto-chat-${sourceMessageId}` } : {}),
    name: proposal.name,
    enabled: true,
    trigger,
    action,
    requiresAi: false,
    applicationId: proposal.applicationId ?? undefined,
  };
}
