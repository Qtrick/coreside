import { useEffect, useId, useRef, useState } from "react";
import { X } from "lucide-react";
import type { ImportMediaInput } from "@/types/media";
import { ModalPortal } from "@/components/ui/ModalPortal";

type MediaImportDialogProps = {
  open: boolean;
  busy?: boolean;
  error?: string | null;
  projectId?: string | null;
  onClose: () => void;
  onImport: (input: ImportMediaInput) => Promise<void>;
};

export function MediaImportDialog({
  open,
  busy,
  error,
  projectId,
  onClose,
  onImport,
}: MediaImportDialogProps) {
  const titleId = useId();
  const urlRef = useRef<HTMLInputElement>(null);
  const [url, setUrl] = useState("");
  const [title, setTitle] = useState("");
  const [sourcePageUrl, setSourcePageUrl] = useState("");
  const [creator, setCreator] = useState("");
  const [license, setLicense] = useState("");
  const [attribution, setAttribution] = useState("");
  const [localError, setLocalError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setUrl("");
    setTitle("");
    setSourcePageUrl("");
    setCreator("");
    setLicense("");
    setAttribution("");
    setLocalError(null);
    requestAnimationFrame(() => urlRef.current?.focus());
  }, [open]);

  if (!open) return null;

  const submit = async () => {
    const trimmed = url.trim();
    if (!trimmed) {
      setLocalError("Media URL is required");
      return;
    }
    if (!/^https?:\/\//i.test(trimmed)) {
      setLocalError("Only http(s) URLs are supported for import");
      return;
    }
    setLocalError(null);
    await onImport({
      url: trimmed,
      title: title.trim() || null,
      projectId: projectId ?? null,
      sourcePageUrl: sourcePageUrl.trim() || null,
      creator: creator.trim() || null,
      license: license.trim() || null,
      attribution: attribution.trim() || null,
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
        className="provider-modal project-dialog media-import-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
      >
        <div className="provider-modal-header">
          <div>
            <h2 id={titleId}>Import media</h2>
            <p>
              Downloads are validated locally. Remote URLs are never stored as
              persistent wallpaper sources.
            </p>
          </div>
          <button
            type="button"
            className="icon-btn"
            aria-label="Close"
            disabled={busy}
            onClick={onClose}
          >
            <X size={18} />
          </button>
        </div>

        <div className="provider-modal-body">
          <label className="field">
            <span>Media URL</span>
            <input
              ref={urlRef}
              type="url"
              value={url}
              placeholder="https://…"
              disabled={busy}
              onChange={(e) => setUrl(e.target.value)}
            />
          </label>
          <label className="field">
            <span>Title</span>
            <input
              type="text"
              value={title}
              placeholder="Optional display title"
              disabled={busy}
              onChange={(e) => setTitle(e.target.value)}
            />
          </label>
          <label className="field">
            <span>Source page</span>
            <input
              type="url"
              value={sourcePageUrl}
              placeholder="https://…"
              disabled={busy}
              onChange={(e) => setSourcePageUrl(e.target.value)}
            />
          </label>
          <div className="media-import-grid">
            <label className="field">
              <span>Creator</span>
              <input
                type="text"
                value={creator}
                disabled={busy}
                onChange={(e) => setCreator(e.target.value)}
              />
            </label>
            <label className="field">
              <span>License</span>
              <input
                type="text"
                value={license}
                disabled={busy}
                onChange={(e) => setLicense(e.target.value)}
              />
            </label>
          </div>
          <label className="field">
            <span>Attribution</span>
            <input
              type="text"
              value={attribution}
              disabled={busy}
              onChange={(e) => setAttribution(e.target.value)}
            />
          </label>
          {(localError || error) && (
            <p className="form-error" role="alert">
              {localError || error}
            </p>
          )}
        </div>

        <div className="provider-modal-footer">
          <button
            type="button"
            className="btn btn-secondary"
            disabled={busy}
            onClick={onClose}
          >
            Cancel
          </button>
          <button
            type="button"
            className="btn btn-primary"
            disabled={busy}
            onClick={() => void submit()}
          >
            {busy ? "Importing…" : "Import"}
          </button>
        </div>
      </div>
    </div>
    </ModalPortal>
  );
}
