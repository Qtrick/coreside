import { useState } from "react";
import type { MessageSearchResults, SourceCitation } from "@/types/search";
import { SourceList } from "@/components/search/SourceList";
import { WebResultCard } from "@/components/search/WebResultCard";
import { ImageResultCard } from "@/components/search/ImageResultCard";
import { VideoResultCard } from "@/components/search/VideoResultCard";
import { api } from "@/lib/tauri";
import type { ImportMediaInput } from "@/types/media";

function PendingMediaImports({
  items,
}: {
  items: NonNullable<MessageSearchResults["pendingMediaImports"]>;
}) {
  const [busyUrl, setBusyUrl] = useState<string | null>(null);
  const [done, setDone] = useState<Record<string, string>>({});

  if (items.length === 0) return null;

  return (
    <section className="search-results-section" aria-label="Media import approvals">
      <h4 className="search-results-heading">Approve media imports</h4>
      <ul className="pending-media-import-list">
        {items.map((item) => {
          const key = item.url;
          const status = done[key];
          return (
            <li key={key} className="pending-media-import-item">
              <div>
                <p className="search-result-title-text">
                  {item.title?.trim() || item.url}
                </p>
                <p className="muted search-result-domain">
                  {item.category ?? "media"} · License not provided by source
                </p>
                {status ? <p className="muted">{status}</p> : null}
              </div>
              <button
                type="button"
                className="btn btn-primary btn-compact"
                disabled={busyUrl === key || Boolean(status)}
                onClick={() => {
                  void (async () => {
                    const ok = window.confirm(
                      `Import this media into your Coreside Media Library?\n\n${item.url}`,
                    );
                    if (!ok) return;
                    setBusyUrl(key);
                    try {
                      await api.importMediaAsset({
                        url: item.url,
                        title: item.title ?? undefined,
                        category: item.category ?? undefined,
                        sourcePageUrl: item.sourcePageUrl ?? undefined,
                        creator: item.creator ?? undefined,
                        license: item.license ?? undefined,
                      } satisfies ImportMediaInput);
                      setDone((prev) => ({ ...prev, [key]: "Imported" }));
                    } catch (err) {
                      setDone((prev) => ({
                        ...prev,
                        [key]:
                          err instanceof Error ? err.message : "Import failed",
                      }));
                    } finally {
                      setBusyUrl(null);
                    }
                  })();
                }}
              >
                {busyUrl === key ? "Importing…" : "Approve import"}
              </button>
            </li>
          );
        })}
      </ul>
    </section>
  );
}

export function SearchResults({
  citations,
  searchResults,
}: {
  citations?: SourceCitation[];
  searchResults?: MessageSearchResults | null;
}) {
  const web = searchResults?.web?.results ?? [];
  const images = searchResults?.images?.results ?? [];
  const videos = searchResults?.videos?.results ?? [];
  const pending = searchResults?.pendingMediaImports ?? [];
  const hasCards =
    web.length > 0 ||
    images.length > 0 ||
    videos.length > 0 ||
    pending.length > 0;
  const citeList = citations ?? [];

  if (!hasCards && citeList.length === 0) return null;

  return (
    <div className="message-search-results">
      {web.length > 0 ? (
        <section className="search-results-section" aria-label="Web results">
          <h4 className="search-results-heading">Web results</h4>
          <div className="search-results-grid web-results-grid">
            {web.map((result) => (
              <WebResultCard key={result.id} result={result} />
            ))}
          </div>
        </section>
      ) : null}

      {images.length > 0 ? (
        <section className="search-results-section" aria-label="Image results">
          <h4 className="search-results-heading">Images</h4>
          <div className="search-results-grid image-results-grid">
            {images.map((result) => (
              <ImageResultCard key={result.id} result={result} />
            ))}
          </div>
        </section>
      ) : null}

      {videos.length > 0 ? (
        <section className="search-results-section" aria-label="Video results">
          <h4 className="search-results-heading">Videos</h4>
          <div className="search-results-grid video-results-grid">
            {videos.map((result) => (
              <VideoResultCard key={result.id} result={result} />
            ))}
          </div>
        </section>
      ) : null}

      <PendingMediaImports items={pending} />

      {citeList.length > 0 ? <SourceList citations={citeList} /> : null}
    </div>
  );
}
