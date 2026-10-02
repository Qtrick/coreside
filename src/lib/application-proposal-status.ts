/** Consumer-facing labels for application generation and change proposals. */

export const LIVE_APPLICATION_GENERATION_STEPS = [
  "Generating",
  "Draft ready",
  "Review changes",
  "Applying",
  "Testing",
  "Ready",
] as const;

export type LiveApplicationGenerationIndex = 0 | 1 | 2 | 3 | 4 | 5;

export const LIVE_APPLICATION_EVOLUTION_STEPS = [
  "Understanding your app",
  "Preparing changes",
  "Review changes",
  "Preserving your data",
  "Applying",
  "Testing",
  "Ready",
] as const;

export const LIVE_APPLICATION_REPAIR_STEPS = [
  "Checking",
  "Repairing",
  "Rechecking",
  "Ready",
] as const;

const SQL_OR_STACK =
  /(\bSELECT\b|\bINSERT\b|\bUPDATE\b|\bDELETE\b|\bFROM\b|\bsqlite\b|\.rs:\d+|stack trace|at \/|KernelError|rusqlite)/i;

/** Strip technical details unsuitable for consumer UI. */
export function sanitizeConsumerError(raw: string | null | undefined): string {
  if (!raw?.trim()) {
    return "Something went wrong. You can try again or ask Coreside to adjust the change.";
  }
  const trimmed = raw.trim();
  if (SQL_OR_STACK.test(trimmed) || trimmed.length > 280) {
    return "This change could not be completed. Try applying again or ask for a smaller update.";
  }
  if (trimmed.toLowerCase().includes("tampered")) {
    return "This preview no longer matches the saved proposal. Ask Coreside to regenerate the change.";
  }
  return trimmed;
}

export function computeLiveApplicationGenerationIndex(
  actions: string[],
  streaming: boolean,
): LiveApplicationGenerationIndex {
  const combined = actions.join(" ").toLowerCase();

  if (
    !streaming &&
    actions.length > 0 &&
    (combined.includes("ready") ||
      combined.includes("finished") ||
      combined.includes("complete") ||
      combined.includes("verified") ||
      combined.includes("test pass"))
  ) {
    return 5;
  }

  if (
    combined.includes("test") ||
    combined.includes("verif") ||
    combined.includes("declarative")
  ) {
    return 4;
  }

  if (
    streaming &&
    (/\bapplying\b/.test(combined) ||
      combined.includes("commit") ||
      (combined.includes("patch") && !combined.includes("dispatch")) ||
      /\bbuilding\b/.test(combined))
  ) {
    return 3;
  }

  if (
    combined.includes("preview") ||
    combined.includes("proposal") ||
    combined.includes("review") ||
    combined.includes("component") ||
    combined.includes("surface") ||
    combined.includes("layout")
  ) {
    return 2;
  }

  if (actions.length > 0 || streaming) {
    return 1;
  }

  return 0;
}

export type ProposalStatusTone = "neutral" | "active" | "success" | "warning" | "error";

export function proposalStatusPresentation(status: string): {
  label: string;
  tone: ProposalStatusTone;
  resolved: boolean;
} {
  const normalized = status.toLowerCase();

  switch (normalized) {
    case "pending":
      return { label: "Review changes", tone: "active", resolved: false };
    case "applying":
      return { label: "Applying", tone: "active", resolved: false };
    case "testing":
    case "verifying":
      return { label: "Testing", tone: "active", resolved: false };
    case "applied":
      return { label: "Ready", tone: "success", resolved: true };
    case "discarded":
    case "rejected":
      return { label: "Discarded", tone: "neutral", resolved: true };
    case "failed":
      return { label: "Needs attention", tone: "error", resolved: true };
    case "stale":
    case "expired":
      return { label: "Needs attention", tone: "warning", resolved: true };
    case "repairing":
      return { label: "Repairing", tone: "active", resolved: false };
    case "repair_exhausted":
      return { label: "Needs attention", tone: "error", resolved: true };
    default:
      if (normalized.includes("fail")) {
        return { label: "Needs attention", tone: "error", resolved: true };
      }
      if (normalized.includes("repair")) {
        return { label: "Repairing", tone: "active", resolved: false };
      }
      return { label: "Needs attention", tone: "warning", resolved: true };
  }
}

export function humanizeAgentActionLabel(raw: string): string {
  const lower = raw.toLowerCase();
  if (raw.startsWith("operation.") || lower.includes("execute_sql")) {
    return "Refining application components…";
  }
  if (lower.includes("repair")) {
    return "Repairing your application…";
  }
  if (lower.includes("proposal") || lower.includes("approval")) {
    return "Preparing changes for your review…";
  }
  if (lower.includes("preview")) {
    return "Drafting application preview…";
  }
  return raw;
}

export function deriveEvolutionCopy(
  operations: unknown[],
  impactSummary: string,
  summary: string,
  preservationSummary?: string,
): {
  headline: string;
  whatWillChange: string;
  whatWillBePreserved: string;
  isEvolution: boolean;
} {
  const ops = operations.filter(
    (op): op is Record<string, unknown> => !!op && typeof op === "object",
  );
  const types = ops.map((op) => String(op.type ?? op.opType ?? "")).filter(Boolean);

  const isCreate = types.some((t) => t.includes("create") || t.includes("full_replace"));
  const isIncremental = types.some((t) =>
    /component\.(update|insert|move|remove)|state\.patch|surface\.update/.test(t),
  );
  const isEvolution = !isCreate && (isIncremental || types.length > 0);

  const whatWillChange =
    impactSummary.trim() ||
    summary.trim() ||
    (isEvolution
      ? "Coreside will update parts of your application based on this conversation."
      : "Coreside will add or update an application from this conversation.");

  let whatWillBePreserved =
    preservationSummary?.trim() ||
    "";

  if (!whatWillBePreserved) {
    if (isCreate && types.every((t) => t.includes("create"))) {
      whatWillBePreserved =
        "Your other applications, chats, and settings stay as they are.";
    } else if (isEvolution) {
      whatWillBePreserved =
        "Existing records, layout focus, and in-progress edits are preserved where possible.";
    } else {
      whatWillBePreserved =
        "Unrelated applications and workspace settings are not changed.";
    }
  }

  return {
    headline: isEvolution ? "Proposed changes" : "New application preview",
    whatWillChange,
    whatWillBePreserved,
    isEvolution,
  };
}

export function consumerRiskHint(risk: string): string | null {
  const r = risk.toLowerCase();
  if (r === "strong") return "Review carefully before applying.";
  if (r === "lightweight") return "Small, targeted update.";
  return null;
}
