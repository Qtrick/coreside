import { useEffect, useId, useState } from "react";
import { X } from "lucide-react";
import { ModalPortal } from "@/components/ui/ModalPortal";

type DeleteConversationDialogProps = {
  open: boolean;
  title: string;
  busy?: boolean;
  error?: string | null;
  onClose: () => void;
  onDelete: () => Promise<void>;
};

/**
 * In-app confirm for chat delete.
 * ponytail: Tauri WKWebView often no-ops or auto-dismisses window.confirm.
 */
export function DeleteConversationDialog({
  open,
  title,
  busy,
  error,
  onClose,
  onDelete,
}: DeleteConversationDialogProps) {
  const titleId = useId();
  // Keep the title while optimistic store removal clears the chat row.
  const [displayTitle, setDisplayTitle] = useState(title);
  useEffect(() => {
    if (open && title) setDisplayTitle(title);
  }, [open, title]);

  if (!open) return null;

  return (
    <ModalPortal onClose={() => !busy && onClose()}>
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
              <h2 id={titleId}>Delete chat?</h2>
              <p>
                Delete <strong>{displayTitle || "this chat"}</strong>? This
                cannot be undone.
              </p>
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
              disabled={busy}
              onClick={() => void onDelete()}
              data-testid="confirm-delete-conversation"
            >
              {busy ? "Deleting…" : "Delete"}
            </button>
          </div>
        </div>
      </div>
    </ModalPortal>
  );
}
