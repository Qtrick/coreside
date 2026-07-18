import { ExternalLink } from "lucide-react";
import type { WebSearchResult } from "@/types/search";
import { openExternalUrl } from "@/lib/open-url";

export function WebResultCard({ result }: { result: WebSearchResult }) {
  return (
    <article className="search-result-card web-result-card">
      <div className="search-result-card-header">
        {result.displayDomain ? (
          <span className="search-result-domain muted">{result.displayDomain}</span>
        ) : null}
        {result.age ? <span className="search-result-age muted">{result.age}</span> : null}
      </div>
      <button
        type="button"
        className="search-result-title"
        onClick={() => void openExternalUrl(result.url)}
      >
        {result.title}
        <ExternalLink size={12} aria-hidden />
      </button>
      {result.snippet ? (
        <p className="search-result-snippet muted">{result.snippet}</p>
      ) : null}
    </article>
  );
}
