export interface SearchRequestBody {
  query: string;
  idempotencyKey: string;
  numResults?: number;
}

export const MAX_SEARCH_BODY_BYTES = 16 * 1024;
export const MAX_QUERY_CHARS = 512;
export const MAX_RESULTS = 10;
export const DEFAULT_RESULTS = 5;
export const MAX_UPSTREAM_RESPONSE_BYTES = 512 * 1024;

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

  return {
    query,
    idempotencyKey: body.idempotencyKey.trim(),
    numResults,
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
