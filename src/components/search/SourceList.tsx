import { ExternalLink } from "lucide-react";
import type { SourceCitation } from "@/types/search";
import { openExternalUrl } from "@/lib/open-url";

export function SourceList({ citations }: { citations: SourceCitation[] }) {
  if (citations.length === 0) return null;

  return (
    <section className="source-list" aria-label="Sources">
      <h4 className="source-list-heading">Sources</h4>
      <ol className="source-list-items">
        {citations.map((cite, index) => (
          <li key={cite.id || `${cite.url}-${index}`}>
            <button
              type="button"
              className="source-list-link"
              onClick={() => void openExternalUrl(cite.url)}
            >
              <span className="source-list-title">{cite.title}</span>
              {cite.displayDomain ? (
                <span className="source-list-domain muted">{cite.displayDomain}</span>
              ) : null}
              <ExternalLink size={12} aria-hidden className="source-list-icon" />
            </button>
            {cite.snippet ? (
              <p className="source-list-snippet muted">{cite.snippet}</p>
            ) : null}
          </li>
        ))}
      </ol>
    </section>
  );
}
