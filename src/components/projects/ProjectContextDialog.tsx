import { useEffect, useId, useState } from "react";
import { X, Search, RefreshCw, MessageSquare } from "lucide-react";
import type { Project, ProjectContextHit } from "@/types/project";
import { ModalPortal } from "@/components/ui/ModalPortal";
import { api } from "@/lib/tauri/api";

type ProjectContextDialogProps = {
  open: boolean;
  project: Project;
  onClose: () => void;
  onOpenChat?: (chatId: string) => void;
};

export function ProjectContextDialog({
  open,
  project,
  onClose,
  onOpenChat,
}: ProjectContextDialogProps) {
  const titleId = useId();
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<ProjectContextHit[]>([]);
  const [loading, setLoading] = useState(false);
  const [rebuilding, setRebuilding] = useState(false);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setQuery("");
    setResults([]);
    setError(null);
    setStatusMessage(null);
  }, [open, project.id]);

  if (!open) return null;

  const handleSearch = async (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    const q = query.trim();
    if (!q) {
      setResults([]);
      return;
    }

    setLoading(true);
    setError(null);
    setStatusMessage(null);
    try {
      const hits = await api.searchProjectContext(project.id, q, 15);
      setResults(hits);
      if (hits.length === 0) {
        setStatusMessage("No matching context found in this project.");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleRebuildIndex = async () => {
    setRebuilding(true);
    setError(null);
    setStatusMessage(null);
    try {
      const indexedCount = await api.rebuildProjectIndex(project.id);
      setStatusMessage(`Successfully re-indexed ${indexedCount} messages in project.`);
      if (query.trim()) {
        await handleSearch();
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setRebuilding(false);
    }
  };

  return (
    <ModalPortal>
      <div
        className="modal-backdrop"
        role="presentation"
        onClick={() => onClose()}
      >
        <div
          className="modal-dialog"
          role="dialog"
          aria-modal="true"
          aria-labelledby={titleId}
          onClick={(e) => e.stopPropagation()}
          style={{ maxWidth: "640px", width: "90%" }}
        >
          <div className="modal-header">
            <h2 id={titleId} className="modal-title">
              Manage Project Context
            </h2>
            <button
              type="button"
              className="icon-btn"
              onClick={onClose}
              aria-label="Close"
            >
              <X size={18} />
            </button>
          </div>

          <div className="modal-body" style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>
            <p className="muted" style={{ margin: 0 }}>
              Search and inspect conversation context for <strong>{project.name}</strong>.
              Coreside uses local full-text retrieval to augment AI answers with project context.
            </p>

            <form onSubmit={handleSearch} style={{ display: "flex", gap: "0.5rem" }}>
              <div style={{ position: "relative", flex: 1 }}>
                <input
                  type="search"
                  className="input"
                  style={{ width: "100%", paddingLeft: "2rem" }}
                  placeholder="Search project messages and notes..."
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  autoFocus
                />
                <Search
                  size={14}
                  style={{
                    position: "absolute",
                    left: "0.75rem",
                    top: "50%",
                    transform: "translateY(-50%)",
                    opacity: 0.5,
                  }}
                />
              </div>
              <button
                type="submit"
                className="btn btn-primary"
                disabled={loading || !query.trim()}
              >
                {loading ? "Searching..." : "Search"}
              </button>
              <button
                type="button"
                className="btn btn-secondary"
                onClick={handleRebuildIndex}
                disabled={rebuilding}
                title="Rebuild local full-text search index for this project"
              >
                <RefreshCw size={14} className={rebuilding ? "spin" : ""} />
                {rebuilding ? "Indexing..." : "Rebuild Index"}
              </button>
            </form>

            {error && <div className="callout callout-danger">{error}</div>}
            {statusMessage && <div className="callout callout-info">{statusMessage}</div>}

            <div
              className="context-hit-list"
              style={{
                maxHeight: "320px",
                overflowY: "auto",
                display: "flex",
                flexDirection: "column",
                gap: "0.5rem",
              }}
            >
              {results.length > 0 ? (
                results.map((hit) => (
                  <div
                    key={hit.messageId}
                    className="card"
                    style={{
                      padding: "0.75rem",
                      cursor: onOpenChat ? "pointer" : "default",
                      display: "flex",
                      flexDirection: "column",
                      gap: "0.25rem",
                      border: "1px solid var(--border)",
                      borderRadius: "var(--radius-sm)",
                    }}
                    onClick={() => {
                      if (onOpenChat) {
                        onClose();
                        onOpenChat(hit.conversationId);
                      }
                    }}
                  >
                    <div
                      style={{
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "space-between",
                        fontSize: "0.8rem",
                      }}
                    >
                      <span
                        style={{
                          display: "inline-flex",
                          alignItems: "center",
                          gap: "0.25rem",
                          fontWeight: 600,
                        }}
                      >
                        <MessageSquare size={12} />
                        {hit.conversationTitle}
                      </span>
                      <span className="badge badge-secondary" style={{ textTransform: "capitalize" }}>
                        {hit.role}
                      </span>
                    </div>
                    <p
                      style={{
                        margin: 0,
                        fontSize: "0.85rem",
                        color: "var(--text-secondary)",
                        whiteSpace: "pre-wrap",
                      }}
                    >
                      {hit.snippet}
                    </p>
                  </div>
                ))
              ) : !loading && !statusMessage && (
                <div style={{ textAlign: "center", padding: "2rem 1rem", color: "var(--text-secondary)" }}>
                  <Search size={28} style={{ opacity: 0.3, marginBottom: "0.5rem" }} />
                  <p style={{ margin: 0, fontSize: "0.9rem" }}>
                    Type a query above to search context indexed from this project's chats.
                  </p>
                </div>
              )}
            </div>
          </div>

          <div className="modal-footer">
            <button type="button" className="btn btn-secondary" onClick={onClose}>
              Close
            </button>
          </div>
        </div>
      </div>
    </ModalPortal>
  );
}
