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
  if (typeof body.url === "string") {
    const trimmedUrl = body.url.trim();
    if (trimmedUrl.length > 0) {
      const check = validatePublicWebUrl(trimmedUrl);
      if (!check.valid || !check.url) {
        return null;
      }
      targetUrl = check.url.href;
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

function parseIpv4Octets(str: string): number[] | null {
  const parts = str.split(".");
  if (parts.length !== 4) return null;
  const octets: number[] = [];
  for (const p of parts) {
    let num: number;
    if (/^0x[0-9a-f]+$/i.test(p)) {
      num = parseInt(p, 16);
    } else if (/^0[0-7]+$/.test(p)) {
      num = parseInt(p, 8);
    } else if (/^\d+$/.test(p)) {
      num = parseInt(p, 10);
    } else {
      return null;
    }
    if (Number.isNaN(num) || num < 0 || num > 255) return null;
    octets.push(num);
  }
  return octets;
}

function isPrivateIpv4Octets(octets: number[]): boolean {
  const [a, b, c] = octets;
  if (a === 0) return true; // 0.0.0.0/8 Current network
  if (a === 10) return true; // 10.0.0.0/8 Private RFC 1918
  if (a === 100 && b >= 64 && b <= 127) return true; // 100.64.0.0/10 Carrier-grade NAT
  if (a === 127) return true; // 127.0.0.0/8 Loopback
  if (a === 169 && b === 254) return true; // 169.254.0.0/16 Link-local / Cloud metadata (AWS/GCP/Azure)
  if (a === 172 && b >= 16 && b <= 31) return true; // 172.16.0.0/12 Private RFC 1918
  if (a === 192 && b === 0 && (c === 0 || c === 2)) return true; // 192.0.0.0/24, 192.0.2.0/24 (TEST-NET-1)
  if (a === 192 && b === 168) return true; // 192.168.0.0/16 Private RFC 1918
  if (a === 198 && (b === 18 || b === 19)) return true; // 198.18.0.0/15 Benchmarking
  if (a === 198 && b === 51 && c === 100) return true; // 198.51.100.0/24 (TEST-NET-2)
  if (a === 203 && b === 0 && c === 113) return true; // 203.0.113.0/24 (TEST-NET-3)
  if (a >= 224 && a <= 239) return true; // 224.0.0.0/4 Multicast
  if (a >= 240) return true; // 240.0.0.0/4 Reserved & 255.255.255.255 Broadcast
  return false;
}

export function isPrivateOrLocalHost(host: string): boolean {
  const normalized = host.toLowerCase().replace(/^\[|\]$/g, "");
  if (!normalized) return true;

  // Named localhost and internal cloud metadata
  if (
    normalized === "localhost" ||
    normalized === "metadata.google.internal" ||
    normalized === "instance-data" ||
    normalized === "ip6-localhost" ||
    normalized === "ip6-loopback" ||
    normalized.endsWith(".localhost") ||
    normalized.endsWith(".local") ||
    normalized.endsWith(".internal") ||
    normalized.endsWith(".arpa")
  ) {
    return true;
  }

  // Pure integer / decimal representation (e.g. 2130706433 -> 127.0.0.1)
  if (/^\d+$/.test(normalized)) {
    const intVal = Number(normalized);
    if (!Number.isSafeInteger(intVal) || intVal < 0 || intVal > 0xffffffff) return true;
    const octets = [
      (intVal >>> 24) & 255,
      (intVal >>> 16) & 255,
      (intVal >>> 8) & 255,
      intVal & 255,
    ];
    return isPrivateIpv4Octets(octets);
  }

  // Hexadecimal representation (e.g. 0x7f000001 -> 127.0.0.1)
  if (/^0x[0-9a-f]+$/i.test(normalized)) {
    const intVal = parseInt(normalized, 16);
    if (Number.isNaN(intVal) || intVal < 0 || intVal > 0xffffffff) return true;
    const octets = [
      (intVal >>> 24) & 255,
      (intVal >>> 16) & 255,
      (intVal >>> 8) & 255,
      intVal & 255,
    ];
    return isPrivateIpv4Octets(octets);
  }

  // Dotted IPv4 (standard decimal, octal, or hex parts)
  const octets = parseIpv4Octets(normalized);
  if (octets) {
    return isPrivateIpv4Octets(octets);
  }

  // IPv6 address checks
  if (normalized.includes(":")) {
    if (
      normalized === "::1" ||
      normalized === "::" ||
      normalized === "0:0:0:0:0:0:0:1" ||
      normalized === "0:0:0:0:0:0:0:0"
    ) {
      return true;
    }
    // Unique local addresses fc00::/7 (starts with fc or fd)
    if (normalized.startsWith("fc") || normalized.startsWith("fd")) {
      return true;
    }
    // Link-local fe80::/10 (starts with fe8, fe9, fea, feb)
    if (/^fe[89ab]/i.test(normalized)) {
      return true;
    }
    // Multicast ff00::/8
    if (normalized.startsWith("ff")) {
      return true;
    }
    // IPv4-mapped IPv6 (e.g. ::ffff:127.0.0.1 or normalized ::ffff:7f00:1 or 0:0:0:0:0:ffff:...)
    if (normalized.includes("ffff:")) {
      const idx = normalized.indexOf("ffff:");
      const rest = normalized.slice(idx + 5);
      if (rest.includes(".")) {
        const oct = parseIpv4Octets(rest);
        if (oct) return isPrivateIpv4Octets(oct);
      }
      const hexes = rest.split(":");
      if (hexes.length === 2) {
        const n1 = parseInt(hexes[0], 16);
        const n2 = parseInt(hexes[1], 16);
        if (!Number.isNaN(n1) && !Number.isNaN(n2)) {
          return isPrivateIpv4Octets([
            (n1 >>> 8) & 255,
            n1 & 255,
            (n2 >>> 8) & 255,
            n2 & 255,
          ]);
        }
      } else if (hexes.length === 1) {
        const n = parseInt(hexes[0], 16);
        if (!Number.isNaN(n)) {
          return isPrivateIpv4Octets([0, 0, (n >>> 8) & 255, n & 255]);
        }
      }
      return true;
    }
    return false;
  }

  return false;
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

export function validatePublicWebUrl(raw: string): { valid: boolean; url?: URL; reason?: string } {
  if (typeof raw !== "string") return { valid: false, reason: "url_not_string" };
  const trimmed = raw.trim();
  if (!trimmed || trimmed.length > 2048) {
    return { valid: false, reason: "url_invalid_length" };
  }

  let parsed: URL;
  try {
    parsed = new URL(trimmed);
  } catch {
    return { valid: false, reason: "url_parse_failed" };
  }

  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
    return { valid: false, reason: "unsupported_protocol" };
  }

  if (parsed.username || parsed.password) {
    return { valid: false, reason: "credentials_not_allowed" };
  }

  if (parsed.port && parsed.port !== "80" && parsed.port !== "443") {
    return { valid: false, reason: "non_standard_port" };
  }

  if (isPrivateOrLocalHost(parsed.hostname)) {
    return { valid: false, reason: "private_or_reserved_host" };
  }

  return { valid: true, url: parsed };
}

export function isValidPublicUrl(url: URL): boolean {
  return validatePublicWebUrl(url.href).valid;
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
