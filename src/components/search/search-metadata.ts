import type { MessageSearchResults, SourceCitation } from "@/types/search";
import { MessageSearchResultsSchema, SourceCitationSchema } from "@/types/search";

function parseCitations(raw: unknown): SourceCitation[] {
  return Array.isArray(raw)
    ? raw
        .map((item) => SourceCitationSchema.safeParse(item))
        .filter((r) => r.success)
        .map((r) => r.data)
    : [];
}

function parseSearchResults(raw: unknown): MessageSearchResults | null {
  const parsed = MessageSearchResultsSchema.safeParse(raw);
  return parsed.success ? parsed.data : null;
}

/** Read citations and search results out of assistant message metadata. */
export function searchDataFromMetadata(
  metadata: Record<string, unknown> | null | undefined,
): { citations: SourceCitation[]; searchResults: MessageSearchResults | null } {
  if (!metadata) {
    return { citations: [], searchResults: null };
  }
  return {
    citations: parseCitations(metadata.citations),
    searchResults: parseSearchResults(metadata.searchResults),
  };
}
