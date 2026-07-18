import { Download, ExternalLink } from "lucide-react";
import { useState } from "react";
import { api } from "@/lib/tauri";
import { openExternalUrl } from "@/lib/open-url";
import type { ImageSearchResult } from "@/types/search";
import type { ImportMediaInput } from "@/types/media";

export function ImageResultCard({ result }: { result: ImageSearchResult }) {
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  const importImage = async () => {
    if (busy) return;
    const ok = window.confirm(`Import "${result.title}" into your media library?`);
    if (!ok) return;
    setBusy(true);
    setStatus(null);
    try {
      await api.importMediaAsset({
        url: result.imageUrl,
        title: result.title,
        category: "image",
        sourcePageUrl: result.pageUrl,
      } satisfies ImportMediaInput);
      setStatus("Imported");
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Import failed");
    } finally {
      setBusy(false);
    }
  };

  return (
    <article className="search-result-card image-result-card">
      <a
        href={result.imageUrl}
        className="image-result-thumb"
        onClick={(e) => {
          e.preventDefault();
          void openExternalUrl(result.pageUrl || result.imageUrl);
        }}
      >
        <img
          src={result.thumbnailUrl ?? result.imageUrl}
          alt={result.title}
          loading="lazy"
        />
      </a>
      <div className="image-result-body">
        <p className="search-result-title-text">{result.title}</p>
        {result.source ? (
          <p className="muted search-result-domain">{result.source}</p>
        ) : null}
        <div className="search-result-actions">
          <button
            type="button"
            className="btn btn-secondary btn-compact"
            onClick={() => void openExternalUrl(result.pageUrl)}
          >
            <ExternalLink size={12} aria-hidden />
            Open
          </button>
          <button
            type="button"
            className="btn btn-secondary btn-compact"
            disabled={busy}
            onClick={() => void importImage()}
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
