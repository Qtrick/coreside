export interface SearchRequestBody {
  query: string;
  idempotencyKey: string;
  numResults?: number;
  intent?: "lookup" | "news" | "deep_research" | "known_url" | "domain_search";
  url?: string;
}

export const MAX_SEARCH_BODY_BYTES = 16 * 1024;
export const MAX_QUERY_CHARS = 512;
export const MAX_RESULTS = 10;
export const DEFAULT_RESULTS = 5;
export const MAX_UPSTREAM_RESPONSE_BYTES = 512 * 1024;

const VALID_INTENTS = new Set([
  "lookup",
  "news",
  "deep_research",
  "known_url",
  "domain_search",
]);

export function parseSearchBody(raw: unknown): SearchRequestBody | null {
  if (!raw || typeof raw !== "object") return null;
  const body = raw as Record<string, unknown>;
  if (typeof body.query !== "string" || typeof body.idempotencyKey !== "string") {
    return null;
  }
  const query = body.query.trim();
  if (!query || query.length > MAX_QUERY_CHARS || body.idempotencyKey.trim() === "") {
    return null;
  }

  let numResults = DEFAULT_RESULTS;
  if (body.numResults !== undefined) {
    if (typeof body.numResults !== "number" || body.numResults < 1) return null;
    numResults = Math.min(Math.floor(body.numResults), MAX_RESULTS);
  }

  let intent: SearchRequestBody["intent"] = undefined;
  if (typeof body.intent === "string" && VALID_INTENTS.has(body.intent.toLowerCase())) {
    intent = body.intent.toLowerCase() as SearchRequestBody["intent"];
  }

  let targetUrl: string | undefined = undefined;
  if (typeof body.url === "string" && body.url.trim().length > 0 && body.url.trim().length <= 2048) {
    try {
      const parsed = new URL(body.url.trim());
      if (parsed.protocol === "http:" || parsed.protocol === "https:") {
        targetUrl = parsed.href;
      }
    } catch {
      // ignore invalid optional url
    }
  }

  return {
    query,
    idempotencyKey: body.idempotencyKey.trim(),
    numResults,
    ...(intent ? { intent } : {}),
    ...(targetUrl ? { url: targetUrl } : {}),
  };
}

export type SearchReserveRpcResult =
  | { kind: "reserved"; requestId: string }
  | {
      kind: "idempotent";
      requestId: string;
      status: string;
      resultReference: unknown;
    }
  | { kind: "conflict"; requestId: string }
  | { kind: "denied"; reason: string }
  | { kind: "error"; reason: string };

export function parseSearchReserveRpcResult(data: unknown): SearchReserveRpcResult {
  if (!data || typeof data !== "object") {
    return { kind: "error", reason: "invalid_rpc_response" };
  }
  const row = data as Record<string, unknown>;
  const outcome = typeof row.outcome === "string" ? row.outcome : "";
  const requestId = typeof row.request_id === "string" ? row.request_id : "";
  switch (outcome) {
    case "reserved":
      return requestId
        ? { kind: "reserved", requestId }
        : { kind: "error", reason: "missing_request_id" };
    case "idempotent":
      return requestId
        ? {
            kind: "idempotent",
            requestId,
            status: typeof row.status === "string" ? row.status : "completed",
            resultReference: row.result_reference ?? null,
          }
        : { kind: "error", reason: "missing_request_id" };
    case "conflict":
      return requestId
        ? { kind: "conflict", requestId }
        : { kind: "error", reason: "missing_request_id" };
    case "denied":
      return {
        kind: "denied",
        reason: typeof row.reason === "string" ? row.reason : "denied",
      };
    default:
      return {
        kind: "error",
        reason: typeof row.reason === "string" ? row.reason : "unknown_outcome",
      };
  }
}

export function parseSearchSettleRpcResult(data: unknown): boolean {
  if (!data || typeof data !== "object") return false;
  const outcome = (data as Record<string, unknown>).outcome;
  return outcome === "settled" || outcome === "idempotent";
}

export function parseSearchFailRpcResult(data: unknown): boolean {
  if (!data || typeof data !== "object") return false;
  const outcome = (data as Record<string, unknown>).outcome;
  return outcome === "released" || outcome === "idempotent";
}

/** Bound Exa result array size and strip oversized fields. */
export function boundSearchResults(
  results: unknown,
  maxResults: number,
): unknown[] {
  if (!Array.isArray(results)) return [];
  return results.slice(0, maxResults).map((item) => {
    if (!item || typeof item !== "object") return item;
    const row = item as Record<string, unknown>;
    const out: Record<string, unknown> = {};
    for (const [key, value] of Object.entries(row)) {
      if (typeof value === "string" && value.length > 4000) {
        out[key] = value.slice(0, 4000);
      } else {
        out[key] = value;
      }
    }
    return out;
  });
}

function isPrivateOrLocalHost(host: string): boolean {
  const normalized = host.toLowerCase().replace(/^\[|\]$/g, "");
  if (
    normalized === "localhost" ||
    normalized === "::1" ||
    normalized === "0.0.0.0" ||
    normalized.endsWith(".localhost") ||
    normalized.endsWith(".local") ||
    normalized.endsWith(".internal") ||
    normalized.startsWith("fc") ||
    normalized.startsWith("fd") ||
    normalized.startsWith("fe80:") ||
    normalized.startsWith("::ffff:") ||
    /^\d+$/.test(normalized) ||
    /^0x[0-9a-f]+$/i.test(normalized)
  ) return true;
  const ipv4 = normalized.split(".").map(Number);
  if (ipv4.length !== 4 || ipv4.some((part) => !Number.isInteger(part) || part < 0 || part > 255)) {
    return false;
  }
  return (
    ipv4[0] === 0 || ipv4[0] === 10 || ipv4[0] === 127 ||
    (ipv4[0] === 100 && ipv4[1] >= 64 && ipv4[1] <= 127) ||
    (ipv4[0] === 169 && ipv4[1] === 254) ||
    (ipv4[0] === 172 && ipv4[1] >= 16 && ipv4[1] <= 31) ||
    (ipv4[0] === 192 && ipv4[1] === 168)
  );
}

export function sanitizePromptInjection(text: string): string {
  if (!text) return "";
  let clean = text;
  const injectionPatterns = [
    /ignore\s+(all\s+)?(previous|prior)\s+instructions/gi,
    /disregard\s+(all\s+)?(previous|prior)\s+instructions/gi,
    /you\s+are\s+now\s+(a|an|in)\s+/gi,
    /<\s*\/?\s*system\s*>/gi,
    /\[\s*\/?\s*system\s*\]/gi,
    /<\s*\/?\s*assistant\s*>/gi,
    /\[\s*\/?\s*instruction\s*\]/gi,
    /override\s+(all\s+)?system\s+prompts?/gi,
    /new\s+system\s+prompt:/gi,
  ];
  for (const pattern of injectionPatterns) {
    clean = clean.replace(pattern, "[untrusted-reference-neutralized]");
  }
  return clean;
}

function isValidPublicUrl(url: URL): boolean {
  if (url.protocol !== "https:" && url.protocol !== "http:") return false;
  if (url.username || url.password) return false;
  if (url.port && url.port !== "80" && url.port !== "443") return false;
  if (isPrivateOrLocalHost(url.hostname)) return false;
  return true;
}

/** Convert an upstream Linkup result into Coreside's stable source shape. Provider
 * URLs remain untrusted and must at least be public HTTP(S) before desktop use. */
export function normalizeProviderResults(payload: unknown, maxResults: number): unknown[] {
  const rows = payload && typeof payload === "object" && Array.isArray((payload as Record<string, unknown>).results)
    ? (payload as Record<string, unknown>).results as unknown[]
    : [];
  const seen = new Set<string>();
  const normalized: Record<string, unknown>[] = [];
  for (const row of rows) {
    if (!row || typeof row !== "object") continue;
    const source = row as Record<string, unknown>;
    if (source.type !== undefined && source.type !== "text") continue;
    if (typeof source.url !== "string") continue;
    let url: URL;
    try { url = new URL(source.url); } catch { continue; }
    if (!isValidPublicUrl(url)) continue;
    if (seen.has(url.href)) continue;
    seen.add(url.href);
    const rawTitle = typeof source.name === "string" && source.name.trim() ? source.name.slice(0, 400) : url.href;
    const rawSnippet = typeof source.content === "string" ? source.content.slice(0, 4000) : "";
    normalized.push({
      title: sanitizePromptInjection(rawTitle),
      url: url.href,
      snippet: sanitizePromptInjection(rawSnippet),
      date: typeof source.date === "string" ? source.date.slice(0, 80) : null,
      rank: normalized.length + 1,
      provider: "linkup",
    });
    if (normalized.length >= maxResults) break;
  }
  return normalized;
}

/** Convert an upstream Exa result into Coreside's stable source shape. */
export function normalizeExaResults(payload: unknown, maxResults: number): unknown[] {
  if (!payload || typeof payload !== "object") return [];
  const body = payload as Record<string, unknown>;
  const rows = Array.isArray(body.results) ? body.results : [];
  const seen = new Set<string>();
  const normalized: Record<string, unknown>[] = [];
  for (const row of rows) {
    if (!row || typeof row !== "object") continue;
    const source = row as Record<string, unknown>;
    if (typeof source.url !== "string") continue;
    let url: URL;
    try { url = new URL(source.url); } catch { continue; }
    if (!isValidPublicUrl(url)) continue;
    if (seen.has(url.href)) continue;
    seen.add(url.href);

    const rawTitle = typeof source.title === "string" && source.title.trim()
      ? source.title.slice(0, 400)
      : url.href;

    let rawSnippet = "";
    if (Array.isArray(source.highlights) && source.highlights.length > 0) {
      rawSnippet = source.highlights.filter((h) => typeof h === "string").join(" ").slice(0, 4000);
    } else if (typeof source.text === "string" && source.text.trim()) {
      rawSnippet = source.text.slice(0, 4000);
    }

    normalized.push({
      title: sanitizePromptInjection(rawTitle),
      url: url.href,
      snippet: sanitizePromptInjection(rawSnippet),
      date: typeof source.publishedDate === "string" ? source.publishedDate.slice(0, 80) : null,
      rank: normalized.length + 1,
      provider: "exa",
    });
    if (normalized.length >= maxResults) break;
  }
  return normalized;
}

/** Convert an upstream Firecrawl search result into Coreside's stable source shape. */
export function normalizeFirecrawlResults(payload: unknown, maxResults: number): unknown[] {
  if (!payload || typeof payload !== "object") return [];
  const body = payload as Record<string, unknown>;
  const data = Array.isArray(body.data) ? body.data : [];
  const seen = new Set<string>();
  const normalized: Record<string, unknown>[] = [];
  for (const item of data) {
    if (!item || typeof item !== "object") continue;
    const row = item as Record<string, unknown>;
    const rawUrl = typeof row.url === "string" ? row.url : "";
    if (!rawUrl) continue;
    let url: URL;
    try { url = new URL(rawUrl); } catch { continue; }
    if (!isValidPublicUrl(url)) continue;
    if (seen.has(url.href)) continue;
    seen.add(url.href);

    const meta = row.metadata && typeof row.metadata === "object" ? (row.metadata as Record<string, unknown>) : {};
    const rawTitle = typeof row.title === "string" && row.title.trim()
      ? row.title
      : typeof meta.title === "string" && meta.title.trim()
        ? meta.title
        : url.href;
    const rawSnippet = typeof row.markdown === "string" && row.markdown.trim()
      ? row.markdown.slice(0, 4000)
      : typeof row.description === "string" && row.description.trim()
        ? row.description.slice(0, 4000)
        : typeof meta.description === "string" && meta.description.trim()
          ? meta.description.slice(0, 4000)
          : "";

    normalized.push({
      title: sanitizePromptInjection(rawTitle.slice(0, 400)),
      url: url.href,
      snippet: sanitizePromptInjection(rawSnippet),
      date: null,
      rank: normalized.length + 1,
      provider: "firecrawl",
    });
    if (normalized.length >= maxResults) break;
  }
  return normalized;
}

/** Convert a Firecrawl scrape result into Coreside's stable source shape. */
export function normalizeFirecrawlScrapeResult(payload: unknown, fallbackUrl: string): unknown[] {
  if (!payload || typeof payload !== "object") return [];
  const body = payload as Record<string, unknown>;
  const data = body.data && typeof body.data === "object" ? (body.data as Record<string, unknown>) : null;
  if (!data) return [];

  const meta = data.metadata && typeof data.metadata === "object" ? (data.metadata as Record<string, unknown>) : {};
  const targetUrl = typeof meta.sourceURL === "string" && meta.sourceURL.trim() ? meta.sourceURL.trim() : fallbackUrl;
  let url: URL;
  try { url = new URL(targetUrl); } catch { return []; }
  if (!isValidPublicUrl(url)) return [];

  const rawTitle = typeof meta.title === "string" && meta.title.trim()
    ? meta.title
    : typeof meta.ogTitle === "string" && meta.ogTitle.trim()
      ? meta.ogTitle
      : url.href;
  const rawContent = typeof data.markdown === "string" && data.markdown.trim()
    ? data.markdown.slice(0, 8000)
    : typeof meta.description === "string" && meta.description.trim()
      ? meta.description.slice(0, 4000)
      : "";

  return [
    {
      title: sanitizePromptInjection(rawTitle.slice(0, 400)),
      url: url.href,
      snippet: sanitizePromptInjection(rawContent.slice(0, 4000)),
      content: sanitizePromptInjection(rawContent),
      date: null,
      rank: 1,
      provider: "firecrawl",
    },
  ];
}
