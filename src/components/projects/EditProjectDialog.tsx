import { useEffect, useId, useState } from "react";
import { X } from "lucide-react";
import { ModalPortal } from "@/components/ui/ModalPortal";
import {
  PROJECT_ICON_KEYS,
  PROJECT_ICON_LABELS,
  type Project,
  type ProjectIconKey,
  type UpdateProjectInput,
} from "@/types/project";

type EditProjectDialogProps = {
  open: boolean;
  project: Project | null;
  busy?: boolean;
  error?: string | null;
  onClose: () => void;
  onSave: (projectId: string, input: UpdateProjectInput) => Promise<void | unknown>;
};

export function EditProjectDialog({
  open,
  project,
  busy,
  error,
  onClose,
  onSave,
}: EditProjectDialogProps) {
  const titleId = useId();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [instructions, setInstructions] = useState("");
  const [iconKey, setIconKey] = useState<ProjectIconKey>("folder");
  const [localError, setLocalError] = useState<string | null>(null);

  useEffect(() => {
    if (!open || !project) return;
    setName(project.name);
    setDescription(project.description ?? "");
    setInstructions(project.instructions ?? "");
    setIconKey((project.iconKey as ProjectIconKey) || "folder");
    setLocalError(null);
  }, [open, project]);

  if (!open || !project) return null;

  const submit = async () => {
    const trimmed = name.trim();
    if (!trimmed) {
      setLocalError("Project name is required");
      return;
    }
    setLocalError(null);
    await onSave(project.id, {
      name: trimmed,
      description: description.trim() || null,
      instructions: instructions.trim() || null,
      iconKey,
    });
  };

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
            <h2 id={titleId}>Edit project</h2>
            <p>Update name, description, instructions, or icon.</p>
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
            void submit();
          }}
        >
          <label>
            Name
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              disabled={busy}
            />
          </label>
          <label>
            Description
            <textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              rows={2}
              disabled={busy}
            />
          </label>
          <label>
            Instructions
            <textarea
              value={instructions}
              onChange={(e) => setInstructions(e.target.value)}
              rows={5}
              disabled={busy}
            />
          </label>
          <label>
            Icon
            <select
              value={iconKey}
              onChange={(e) => setIconKey(e.target.value as ProjectIconKey)}
              disabled={busy}
            >
              {PROJECT_ICON_KEYS.map((key) => (
                <option key={key} value={key}>
                  {PROJECT_ICON_LABELS[key]}
                </option>
              ))}
            </select>
          </label>
          {localError || error ? (
            <p className="form-error" role="alert">
              {localError || error}
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
            <button type="submit" className="btn btn-primary" disabled={busy}>
              {busy ? "Saving…" : "Save changes"}
            </button>
          </div>
        </form>
      </div>
    </div>
    </ModalPortal>
  );
}
