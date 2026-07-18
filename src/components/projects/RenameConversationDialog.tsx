import { useEffect, useId, useState } from "react";
import { X } from "lucide-react";
import { ModalPortal } from "@/components/ui/ModalPortal";

type RenameConversationDialogProps = {
  open: boolean;
  title: string;
  busy?: boolean;
  error?: string | null;
  onClose: () => void;
  onRename: (title: string) => Promise<void>;
};

export function RenameConversationDialog({
  open,
  title: initialTitle,
  busy,
  error,
  onClose,
  onRename,
}: RenameConversationDialogProps) {
  const titleId = useId();
  const [title, setTitle] = useState(initialTitle);

  useEffect(() => {
    if (open) setTitle(initialTitle);
  }, [open, initialTitle]);

  if (!open) return null;

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
            <h2 id={titleId}>Rename chat</h2>
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
        <form
          className="project-form"
          onSubmit={(event) => {
            event.preventDefault();
            void onRename(title.trim());
          }}
        >
          <label>
            Title
            <input
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              disabled={busy}
              autoFocus
            />
          </label>
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
              type="submit"
              className="btn btn-primary"
              disabled={busy || !title.trim()}
            >
              {busy ? "Saving…" : "Save"}
            </button>
          </div>
        </form>
      </div>
    </div>
    </ModalPortal>
  );
}
