import { useEffect, useId, useMemo, useState } from "react";
import { X } from "lucide-react";
import type { Conversation } from "@/types/messages";
import { ModalPortal } from "@/components/ui/ModalPortal";

type AddChatsToProjectDialogProps = {
  open: boolean;
  projectName: string;
  conversations: Conversation[];
  assignedIds: Set<string>;
  busy?: boolean;
  error?: string | null;
  onClose: () => void;
  onAssign: (conversationIds: string[]) => Promise<void>;
};

export function AddChatsToProjectDialog({
  open,
  projectName,
  conversations,
  assignedIds,
  busy,
  error,
  onClose,
  onAssign,
}: AddChatsToProjectDialogProps) {
  const titleId = useId();
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<Set<string>>(new Set());

  useEffect(() => {
    if (!open) return;
    setQuery("");
    setSelected(new Set());
  }, [open]);

  const candidates = useMemo(() => {
    const q = query.trim().toLowerCase();
    return conversations
      .filter((c) => !assignedIds.has(c.id))
      .filter((c) => !q || c.title.toLowerCase().includes(q))
      .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
  }, [conversations, assignedIds, query]);

  const movingCount = useMemo(
    () =>
      [...selected].filter((id) => {
        const chat = conversations.find((c) => c.id === id);
        return chat?.projectId;
      }).length,
    [selected, conversations],
  );

  if (!open) return null;

  const toggle = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const submit = async () => {
    if (selected.size === 0) return;
    await onAssign([...selected]);
  };

  return (
    <ModalPortal>
      <div className="provider-modal-root" role="presentation">
      <button
        type="button"
        className="provider-modal-backdrop"
        aria-label="Close"
        onClick={() => !busy && onClose()}
      />
      <div
        className="provider-modal project-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
      >
        <div className="provider-modal-header">
          <div>
            <h2 id={titleId}>Add chats to {projectName}</h2>
            <p>Select one or more chats to include in this project.</p>
          </div>
          <button
            type="button"
            className="btn-icon"
            onClick={onClose}
            disabled={busy}
            aria-label="Close"
          >
            <X size={16} />
          </button>
        </div>
        <div className="project-form">
          <label>
            Search
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Filter chats"
              disabled={busy}
            />
          </label>
          {movingCount > 0 ? (
            <p className="form-warning" role="status">
              {movingCount} selected chat{movingCount === 1 ? "" : "s"} will be
              moved from another project.
            </p>
          ) : null}
          <div className="chat-picker-list" role="listbox" aria-multiselectable>
            {candidates.length === 0 ? (
              <p className="muted">No chats available to add.</p>
            ) : (
              candidates.map((chat) => (
                <label key={chat.id} className="chat-picker-item">
                  <input
                    type="checkbox"
                    checked={selected.has(chat.id)}
                    onChange={() => toggle(chat.id)}
                    disabled={busy}
                  />
                  <span>
                    <strong>{chat.title}</strong>
                    {chat.projectId ? (
                      <span className="chat-picker-meta">In another project</span>
                    ) : null}
                  </span>
                </label>
              ))
            )}
          </div>
          {error ? (
            <p className="form-error" role="alert">
              {error}
            </p>
          ) : null}
          <div className="dialog-actions">
            <button
              type="button"
              className="btn btn-secondary"
              onClick={onClose}
              disabled={busy}
            >
              Cancel
            </button>
            <button
              type="button"
              className="btn btn-primary"
              disabled={busy || selected.size === 0}
              onClick={() => void submit()}
            >
              {busy ? "Adding…" : `Add ${selected.size || ""} chat${selected.size === 1 ? "" : "s"}`}
            </button>
          </div>
        </div>
      </div>
    </div>
    </ModalPortal>
  );
}
