import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "@/lib/tauri";
import { useAppStore } from "@/stores/app-store";

type QueueRow = {
  id: string;
  status: string;
  preview: string;
};

const POLL_MS = 2000;
const PREVIEW_MAX = 80;

function previewFromPrompt(prompt: unknown): string {
  if (prompt && typeof prompt === "object" && "content" in prompt) {
    const content = (prompt as { content: unknown }).content;
    if (typeof content === "string") {
      const trimmed = content.trim();
      if (!trimmed) return "(empty message)";
      return trimmed.length > PREVIEW_MAX
        ? `${trimmed.slice(0, PREVIEW_MAX)}…`
        : trimmed;
    }
  }
  return "(no preview)";
}

function parseRows(raw: Record<string, unknown>[]): QueueRow[] {
  const rows: QueueRow[] = [];
  for (const row of raw) {
    if (typeof row.id !== "string" || !row.id) continue;
    rows.push({
      id: row.id,
      status: typeof row.status === "string" ? row.status : "unknown",
      preview: previewFromPrompt(row.prompt),
    });
  }
  return rows;
}

function queueSummary(items: QueueRow[]): string {
  const queuedCount = items.filter((item) => item.status === "queued").length;
  const activeCount = items.filter((item) => item.status === "active").length;
  const parts: string[] = [];
  if (queuedCount > 0) parts.push(`${queuedCount} queued`);
  if (activeCount > 0) parts.push(`${activeCount} active`);
  if (parts.length === 0) parts.push(`${items.length}`);
  return parts.join(" · ");
}

export function ConversationQueue({
  conversationId,
}: {
  conversationId: string;
}) {
  const sending = useAppStore(
    (s) => s.sending && s.sendingConversationId === conversationId,
  );
  const [items, setItems] = useState<QueueRow[]>([]);
  const [cancellingId, setCancellingId] = useState<string | null>(null);
  const conversationIdRef = useRef(conversationId);
  conversationIdRef.current = conversationId;

  const refresh = useCallback(async () => {
    const id = conversationId;
    try {
      const raw = await api.listAgentQueue(id);
      // Drop stale responses after conversation switch.
      if (conversationIdRef.current !== id) return;
      setItems(parseRows(raw));
    } catch {
      // ponytail: keep last good snapshot; queue list is best-effort UI
    }
  }, [conversationId]);

  useEffect(() => {
    setItems([]);
    setCancellingId(null);
    void refresh();
  }, [conversationId, refresh]);

  useEffect(() => {
    void refresh();
  }, [sending, refresh]);

  // Lightweight poll while the conversation has queue work or a send in flight.
  useEffect(() => {
    if (!sending && items.length === 0) return;
    const timer = window.setInterval(() => {
      void refresh();
    }, POLL_MS);
    return () => window.clearInterval(timer);
  }, [sending, items.length, refresh]);

  const cancelItem = async (itemId: string) => {
    setCancellingId(itemId);
    try {
      await api.cancelQueueItem(itemId);
    } catch {
      // Refresh below reconciles status (e.g. item already active).
    } finally {
      if (conversationIdRef.current === conversationId) {
        await refresh();
        setCancellingId((current) => (current === itemId ? null : current));
      }
    }
  };

  if (items.length === 0) return null;

  return (
    <div className="conversation-queue" aria-label="Conversation queue">
      <p className="conversation-queue-summary">
        <strong>{queueSummary(items)}</strong>
        <span className="muted"> in queue</span>
      </p>
      <ul className="conversation-queue-list">
        {items.map((item) => {
          // Backend cancel is queued-only; active turns must finish or be stopped elsewhere.
          const canCancel = item.status === "queued";
          return (
            <li key={item.id} className="conversation-queue-item">
              <div className="conversation-queue-body">
                <span className="conversation-queue-preview" title={item.preview}>
                  {item.preview}
                </span>
                <span className="muted conversation-queue-status">
                  {item.status}
                </span>
              </div>
              {canCancel ? (
                <button
                  type="button"
                  className="btn btn-secondary btn-compact"
                  aria-label={`Cancel queued message: ${item.preview}`}
                  disabled={cancellingId === item.id}
                  onClick={() => void cancelItem(item.id)}
                >
                  Cancel
                </button>
              ) : null}
            </li>
          );
        })}
      </ul>
    </div>
  );
}
