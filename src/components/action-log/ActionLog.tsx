import { useMemo, useState } from "react";
import type { ChatMessage } from "@/types/messages";
import { useAppStore } from "@/stores/app-store";
import { shouldShowActionLog } from "@/lib/action-log";

type ActionEventView = {
  id?: string;
  label: string;
  status?: string;
  eventType?: string;
};

function eventsFromMessage(message: ChatMessage): ActionEventView[] {
  const meta = message.metadata;
  if (!meta || typeof meta !== "object") return [];
  const raw = (meta as Record<string, unknown>).actionEvents;
  if (!Array.isArray(raw)) return [];
  const out: ActionEventView[] = [];
  for (const [index, item] of raw.entries()) {
    if (!item || typeof item !== "object") continue;
    const row = item as Record<string, unknown>;
    const label = typeof row.label === "string" ? row.label : null;
    if (!label) continue;
    out.push({
      id: typeof row.id === "string" ? row.id : `evt-${index}`,
      label,
      status: typeof row.status === "string" ? row.status : "completed",
      eventType:
        typeof row.eventType === "string" ? row.eventType : undefined,
    });
  }
  return out;
}

export function ActionLog({ message }: { message: ChatMessage }) {
  const actionLogMode = useAppStore((s) => s.actionLogMode);
  const events = useMemo(() => eventsFromMessage(message), [message]);
  const [open, setOpen] = useState(false);

  if (
    !shouldShowActionLog(actionLogMode, events) ||
    message.role !== "assistant"
  ) {
    return null;
  }

  return (
    <div className="action-log">
      <button
        type="button"
        className="action-log-toggle"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
      >
        Actions · {events.length}
      </button>
      {open ? (
        <ul className="action-log-list">
          {events.map((event) => (
            <li key={event.id ?? event.label}>
              <span className="action-log-dot" aria-hidden />
              {event.label}
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}
