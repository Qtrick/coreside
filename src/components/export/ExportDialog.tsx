import { useEffect, useState } from "react";
import { api } from "@/lib/tauri";
import { ModalPortal } from "@/components/ui/ModalPortal";

type ExportFormat = {
  id: string;
  label: string;
  extension: string;
  available: boolean;
  description: string;
};

export function ExportDialog({
  toolId,
  toolName,
  onClose,
}: {
  toolId: string;
  toolName: string;
  onClose: () => void;
}) {
  const [formats, setFormats] = useState<ExportFormat[]>([]);
  const [selected, setSelected] = useState<string>("coreside-tool");
  const [path, setPath] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void api.listExportFormats(toolId).then((rows) => {
      setFormats(rows);
      const first = rows.find((r) => r.available);
      if (first) setSelected(first.id);
    });
  }, [toolId]);

  const active = formats.find((f) => f.id === selected);

  const exportNow = async () => {
    if (!active?.available) return;
    const defaultName = `${toolName || "tool"}`.toLowerCase().replace(/\s+/g, "-");
    const destination =
      path.trim() ||
      `${defaultName}.${active.extension}`;
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      const result = await api.exportTool({
        toolId,
        format: active.id,
        destinationPath: destination,
      });
      setMessage(`${result.message}: ${result.path}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Export failed");
    } finally {
      setBusy(false);
    }
  };

  return (
    <ModalPortal onClose={onClose}>
      <div className="provider-modal-root" role="presentation">
      <button
        type="button"
        className="provider-modal-backdrop"
        aria-label="Dismiss"
        onClick={onClose}
      />
      <div className="provider-modal" role="dialog" aria-modal="true" aria-label="Export tool">
        <header className="provider-modal-header">
          <div>
            <h2>Export</h2>
            <p>Choose a compatible format. Secrets are never included.</p>
          </div>
          <button type="button" className="btn-icon" onClick={onClose} aria-label="Close">
            ×
          </button>
        </header>
        <ul className="provider-picker-list">
          {formats.map((format) => (
            <li key={format.id}>
              <button
                type="button"
                className={`provider-picker-item${selected === format.id ? " active" : ""}`}
                disabled={!format.available}
                onClick={() => setSelected(format.id)}
              >
                <strong>{format.label}</strong>
                <span className="muted">
                  {format.available
                    ? format.description
                    : `${format.description} (unavailable)`}
                </span>
              </button>
            </li>
          ))}
        </ul>
        <label className="field" style={{ marginTop: "1rem" }}>
          <span>Save as</span>
          <input
            value={path}
            onChange={(e) => setPath(e.target.value)}
            placeholder={
              active
                ? `${(toolName || "tool").toLowerCase().replace(/\s+/g, "-")}.${active.extension}`
                : "filename"
            }
            autoComplete="off"
          />
        </label>
        {message ? <p className="muted" role="status">{message}</p> : null}
        {error ? (
          <p className="provider-error" role="alert">
            {error}
          </p>
        ) : null}
        <div className="button-row provider-form-actions">
          <button type="button" className="btn btn-secondary" onClick={onClose}>
            Cancel
          </button>
          <button
            type="button"
            className="btn btn-primary"
            disabled={busy || !active?.available}
            onClick={() => void exportNow()}
          >
            {busy ? "Exporting…" : "Export"}
          </button>
        </div>
      </div>
    </div>
    </ModalPortal>
  );
}
