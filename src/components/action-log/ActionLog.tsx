import { useId, useMemo, useState } from "react";
import type { ChatMessage } from "@/types/messages";
import { useAppStore } from "@/stores/app-store";
import {
  actionCapabilityKind,
  actionTimelineSummary,
  formatActionDuration,
  isSuccessActionStatus,
  isTerminalActionStatus,
  normalizeActionStatus,
  resolveActionEventsAfterProposal,
  shouldShowActionLog,
  totalActionDurationMs,
  type AgentActionEntry,
  type ActionEventStatus,
} from "@/lib/action-log";
import { humanizeAgentActionLabel } from "@/lib/application-proposal-status";

function eventsFromMessage(message: ChatMessage): AgentActionEntry[] {
  const meta = message.metadata;
  if (!meta || typeof meta !== "object") return [];
  const metaObj = meta as Record<string, unknown>;
  const raw = metaObj.actionEvents;
  if (!Array.isArray(raw)) return [];
  const out: AgentActionEntry[] = [];
  for (const [index, item] of raw.entries()) {
    if (!item || typeof item !== "object") continue;
    const row = item as Record<string, unknown>;
    const label = typeof row.label === "string" ? row.label : null;
    if (!label) continue;
    out.push({
      id: typeof row.id === "string" ? row.id : `evt-${index}`,
      label,
      status: normalizeActionStatus(row.status),
      eventType:
        typeof row.eventType === "string" ? row.eventType : undefined,
      stepId: typeof row.stepId === "string" ? row.stepId : undefined,
      startedAt: typeof row.startedAt === "number" ? row.startedAt : undefined,
      completedAt:
        typeof row.completedAt === "number" ? row.completedAt : undefined,
      durationMs:
        typeof row.durationMs === "number" ? row.durationMs : undefined,
    });
  }
  const proposalStatus =
    typeof metaObj.kernelProposalStatus === "string"
      ? metaObj.kernelProposalStatus
      : typeof (metaObj.runtimeV2 as { status?: unknown } | undefined)?.status ===
          "string"
        ? (metaObj.runtimeV2 as { status: string }).status
        : null;
  return resolveActionEventsAfterProposal(out, proposalStatus);
}

function statusGlyph(status: ActionEventStatus): {
  className: string;
  label: string;
} {
  if (status === "running" || status === "pending") {
    return { className: "is-running", label: "In progress" };
  }
  if (status === "awaitingApproval") {
    return { className: "is-awaiting", label: "Awaiting approval" };
  }
  if (status === "failed" || status === "blocked") {
    return { className: "is-failed", label: "Failed" };
  }
  if (status === "cancelled") {
    return { className: "is-cancelled", label: "Cancelled" };
  }
  return { className: "is-success", label: "Succeeded" };
}

export function ActionTimelineCard({
  events,
  live = false,
  defaultOpen,
}: {
  events: AgentActionEntry[];
  live?: boolean;
  defaultOpen?: boolean;
}) {
  const summary = actionTimelineSummary(events);
  const totalMs = totalActionDurationMs(events);
  const totalLabel = formatActionDuration(totalMs);
  const allTerminal = events.every((e) =>
    isTerminalActionStatus(normalizeActionStatus(e.status)),
  );
  const [open, setOpen] = useState(
    defaultOpen ?? (live || events.length <= 4),
  );
  const stepsId = useId();

  if (events.length === 0) return null;

  return (
    <div
      className={`action-timeline-card tone-${summary.tone}${live ? " is-live" : ""}`}
    >
      {/* Live status must not wrap the expand control (interactive + status conflict). */}
      {live ? (
        <span className="sr-only" role="status" aria-live="polite" aria-atomic="true">
          {summary.title}
        </span>
      ) : null}
      <button
        type="button"
        className="action-timeline-header"
        aria-expanded={open}
        aria-controls={stepsId}
        onClick={() => setOpen((v) => !v)}
      >
        <span
          className={`action-timeline-summary-icon ${
            summary.tone === "success"
              ? "is-success"
              : summary.tone === "error"
                ? "is-failed"
                : summary.tone === "active"
                  ? "is-running"
                  : "is-awaiting"
          }`}
          aria-hidden
        />
        <span className="action-timeline-title">{summary.title}</span>
        {totalLabel && allTerminal ? (
          <span className="action-timeline-elapsed">{totalLabel}</span>
        ) : null}
        <span className="action-timeline-chevron" aria-hidden>
          {open ? "▾" : "▸"}
        </span>
      </button>
      {open ? (
        <ol id={stepsId} className="action-timeline-steps">
          {events.map((event) => {
            const status = normalizeActionStatus(event.status);
            const glyph = statusGlyph(status);
            const kind = actionCapabilityKind(event);
            const stepDuration = formatActionDuration(event.durationMs);
            const stepKey = event.stepId || event.id;
            return (
              <li
                key={stepKey}
                className={`action-timeline-step ${glyph.className}`}
              >
                <span
                  className={`action-timeline-kind kind-${kind}`}
                  aria-hidden
                />
                <span className="action-timeline-label">
                  {humanizeAgentActionLabel(event.label)}
                </span>
                {stepDuration && isSuccessActionStatus(status) ? (
                  <span className="action-timeline-step-time">{stepDuration}</span>
                ) : null}
                <span
                  className={`action-timeline-status ${glyph.className}`}
                  title={glyph.label}
                  aria-label={glyph.label}
                />
              </li>
            );
          })}
        </ol>
      ) : (
        <p id={stepsId} className="action-timeline-collapsed-count">
          {events.length} step{events.length === 1 ? "" : "s"}
        </p>
      )}
    </div>
  );
}

export function ActionLog({ message }: { message: ChatMessage }) {
  const actionLogMode = useAppStore((s) => s.actionLogMode);
  const events = useMemo(() => eventsFromMessage(message), [message]);

  if (
    !shouldShowActionLog(actionLogMode, events) ||
    message.role !== "assistant"
  ) {
    return null;
  }

  return (
    <div className="action-log">
      <ActionTimelineCard events={events} defaultOpen={false} />
    </div>
  );
}
