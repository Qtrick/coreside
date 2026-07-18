import { useCallback, useEffect, useState } from "react";
import { ExternalLink, History, Trash2 } from "lucide-react";
import { api } from "@/lib/tauri";
import { openExternalUrl } from "@/lib/open-url";
import type { SearchSessionDetail, SearchSessionSummary } from "@/types/search";

function formatWhen(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

function typeLabel(searchType: string): string {
  switch (searchType) {
    case "web":
      return "Web";
    case "image":
      return "Images";
    case "video":
      return "Videos";
    default:
      return searchType;
  }
}

export function SearchHistoryPanel() {
  const [sessions, setSessions] = useState<SearchSessionSummary[]>([]);
  const [selected, setSelected] = useState<SearchSessionDetail | null>(null);
  const [filter, setFilter] = useState<"all" | "web" | "image" | "video">("all");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      const rows = await api.listSearchSessions({
        searchType: filter === "all" ? null : filter,
        limit: 50,
      });
      setSessions(rows);
      setSelected((prev) => {
        if (!prev) return null;
        return rows.some((r) => r.id === prev.session.id) ? prev : null;
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load search history");
    } finally {
      setBusy(false);
    }
  }, [filter]);

  useEffect(() => {
    void reload();
  }, [reload]);

  const openSession = async (id: string) => {
    setBusy(true);
    setError(null);
    try {
      const detail = await api.getSearchSession(id);
      setSelected(detail);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to open session");
    } finally {
      setBusy(false);
    }
  };

  const clearAll = async () => {
    const ok = window.confirm(
      "Clear all local search history? This cannot be undone.",
    );
    if (!ok) return;
    setBusy(true);
    try {
      await api.clearSearchHistory();
      setSelected(null);
      await reload();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to clear history");
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="search-history-panel" aria-labelledby="search-history-heading">
      <div className="search-history-header">
        <div>
          <h4 id="search-history-heading" className="settings-subheading">
            <History size={16} aria-hidden /> Search history
          </h4>
          <p className="muted search-history-lead">
            Locally stored queries and results. Credentials are never saved here.
          </p>
        </div>
        <button
          type="button"
          className="btn btn-danger btn-compact"
          onClick={() => void clearAll()}
          disabled={busy || sessions.length === 0}
        >
          <Trash2 size={14} aria-hidden />
          Clear all
        </button>
      </div>

      <div className="theme-options" role="group" aria-label="Search history filter">
        {(["all", "web", "image", "video"] as const).map((id) => (
          <button
            key={id}
            type="button"
            className="btn btn-secondary btn-compact"
            aria-pressed={filter === id}
            onClick={() => setFilter(id)}
            disabled={busy}
          >
            {id === "all" ? "All" : typeLabel(id)}
          </button>
        ))}
      </div>

      {error ? <p className="provider-error">{error}</p> : null}

      <div className="search-history-body">
        <ul className="search-history-list" aria-label="Past searches">
          {sessions.length === 0 ? (
            <li className="muted search-history-empty">No search history yet.</li>
          ) : (
            sessions.map((session) => (
              <li key={session.id}>
                <button
                  type="button"
                  className={`search-history-item${
                    selected?.session.id === session.id ? " selected" : ""
                  }`}
                  onClick={() => void openSession(session.id)}
                  disabled={busy}
                >
                  <span className="search-history-query">{session.query}</span>
                  <span className="search-history-meta muted">
                    {typeLabel(session.searchType)} · {session.resultCount} results ·{" "}
                    {formatWhen(session.createdAt)}
                  </span>
                </button>
              </li>
            ))
          )}
        </ul>

        {selected ? (
          <div className="search-history-detail" aria-label="Search session detail">
            <header>
              <h5>{selected.session.query}</h5>
              <p className="muted">
                {typeLabel(selected.session.searchType)} · {selected.session.provider} ·{" "}
                {formatWhen(selected.session.createdAt)}
              </p>
            </header>
            <ul className="search-history-results">
              {selected.results.map((result) => (
                <li key={result.id} className="search-history-result">
                  {result.thumbnailUrl ? (
                    <img
                      src={result.thumbnailUrl}
                      alt=""
                      className="search-history-thumb"
                      loading="lazy"
                    />
                  ) : null}
                  <div>
                    <p className="search-result-title-text">
                      {result.title?.trim() || result.url || "Result"}
                    </p>
                    {result.displayDomain || result.snippet ? (
                      <p className="muted search-result-domain">
                        {[result.displayDomain, result.snippet]
                          .filter(Boolean)
                          .join(" · ")}
                      </p>
                    ) : null}
                    {result.url ? (
                      <button
                        type="button"
                        className="btn btn-secondary btn-compact"
                        onClick={() => void openExternalUrl(result.url!)}
                      >
                        <ExternalLink size={12} aria-hidden />
                        Open source
                      </button>
                    ) : null}
                  </div>
                </li>
              ))}
            </ul>
          </div>
        ) : null}
      </div>
    </section>
  );
}
