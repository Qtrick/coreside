import { Download, ExternalLink, Play } from "lucide-react";
import { useState } from "react";
import { api } from "@/lib/tauri";
import { openExternalUrl } from "@/lib/open-url";
import type { VideoSearchResult } from "@/types/search";
import type { ImportMediaInput } from "@/types/media";

export function VideoResultCard({ result }: { result: VideoSearchResult }) {
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  const importVideo = async () => {
    if (busy) return;
    const ok = window.confirm(`Import "${result.title}" into your media library?`);
    if (!ok) return;
    setBusy(true);
    setStatus(null);
    try {
      await api.importMediaAsset({
        url: result.url,
        title: result.title,
        category: "video",
      } satisfies ImportMediaInput);
      setStatus("Imported");
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Import failed");
    } finally {
      setBusy(false);
    }
  };

  return (
    <article className="search-result-card video-result-card">
      <button
        type="button"
        className="video-result-thumb"
        onClick={() => void openExternalUrl(result.url)}
        aria-label={`Open ${result.title}`}
      >
        {result.thumbnailUrl ? (
          <img src={result.thumbnailUrl} alt="" loading="lazy" />
        ) : (
          <span className="video-result-thumb-fallback" aria-hidden>
            <Play size={20} />
          </span>
        )}
        {result.duration ? (
          <span className="video-result-duration">{result.duration}</span>
        ) : null}
      </button>
      <div className="video-result-body">
        <p className="search-result-title-text">{result.title}</p>
        {result.creator ? (
          <p className="muted search-result-domain">{result.creator}</p>
        ) : null}
        <div className="search-result-actions">
          <button
            type="button"
            className="btn btn-secondary btn-compact"
            onClick={() => void openExternalUrl(result.url)}
          >
            <ExternalLink size={12} aria-hidden />
            Open
          </button>
          <button
            type="button"
            className="btn btn-secondary btn-compact"
            disabled={busy}
            onClick={() => void importVideo()}
          >
            <Download size={12} aria-hidden />
            Import
          </button>
        </div>
        {status ? <p className="muted search-result-status">{status}</p> : null}
      </div>
    </article>
  );
}
