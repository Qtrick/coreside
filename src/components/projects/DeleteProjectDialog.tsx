import { useEffect, useId, useState } from "react";
import { X } from "lucide-react";
import type { DeleteProjectMode } from "@/types/project";
import { ModalPortal } from "@/components/ui/ModalPortal";

type DeleteProjectDialogProps = {
  open: boolean;
  projectName: string;
  chatCount: number;
  busy?: boolean;
  error?: string | null;
  onClose: () => void;
  onDelete: (mode: DeleteProjectMode) => Promise<void>;
};

export function DeleteProjectDialog({
  open,
  projectName,
  chatCount,
  busy,
  error,
  onClose,
  onDelete,
}: DeleteProjectDialogProps) {
  const titleId = useId();
  const [mode, setMode] = useState<DeleteProjectMode>("keepChats");
  const [confirmDeleteChats, setConfirmDeleteChats] = useState(false);

  useEffect(() => {
    if (!open) return;
    setMode("keepChats");
    setConfirmDeleteChats(false);
  }, [open]);

  if (!open) return null;

  const canSubmit =
    mode === "keepChats" || (mode === "deleteChats" && confirmDeleteChats);

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
            <h2 id={titleId}>Delete {projectName}?</h2>
            <p>
              This project has {chatCount} chat{chatCount === 1 ? "" : "s"}.
              Choose what happens to them.
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
        <div className="project-form">
          <fieldset className="radio-group">
            <legend className="visually-hidden">Delete mode</legend>
            <label className="radio-card">
              <input
                type="radio"
                name="delete-mode"
                checked={mode === "keepChats"}
                onChange={() => setMode("keepChats")}
                disabled={busy}
              />
              <span>
                <strong>Keep chats</strong>
                <span className="muted">
                  Unassign chats from this project (default)
                </span>
              </span>
            </label>
            <label className="radio-card destructive">
              <input
                type="radio"
                name="delete-mode"
                checked={mode === "deleteChats"}
                onChange={() => setMode("deleteChats")}
                disabled={busy}
              />
              <span>
                <strong>Delete chats too</strong>
                <span className="muted">
                  Permanently delete all chats in this project
                </span>
              </span>
            </label>
          </fieldset>
          {mode === "deleteChats" ? (
            <label className="confirm-checkbox">
              <input
                type="checkbox"
                checked={confirmDeleteChats}
                onChange={(e) => setConfirmDeleteChats(e.target.checked)}
                disabled={busy}
              />
              I understand this will permanently delete {chatCount} chat
              {chatCount === 1 ? "" : "s"}.
            </label>
          ) : null}
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
              className="btn btn-danger"
              disabled={busy || !canSubmit}
              onClick={() => void onDelete(mode)}
            >
              {busy ? "Deleting…" : "Delete project"}
            </button>
          </div>
        </div>
      </div>
    </div>
    </ModalPortal>
  );
}
