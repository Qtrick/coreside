import { createClient } from "https://esm.sh/@supabase/supabase-js@2.49.1";
import { readBodyTextBounded } from "../_shared/read-body.ts";
import {
  boundSearchResults,
  DEFAULT_RESULTS,
  MAX_SEARCH_BODY_BYTES,
  MAX_UPSTREAM_RESPONSE_BYTES,
  parseSearchBody,
  parseSearchReserveRpcResult,
  parseSearchSettleRpcResult,
  normalizeProviderResults,
  normalizeExaResults,
  normalizeFirecrawlResults,
  normalizeFirecrawlScrapeResult,
  validatePublicWebUrl,
  deduplicateAndRankResults,
  type SearchRequestBody,
} from "./search-lib.ts";

const SERVICE = "coreside-search-gateway";
const VERSION = "1";
const RATE_LIMIT_PER_MINUTE = 20;
const MAX_CONCURRENT = 3;
const IDEMPOTENCY_TTL_HOURS = 24;
const UPSTREAM_TIMEOUT_MS = 30_000;

function isAllowedOrigin(origin: string | null): boolean {
  if (!origin) return false;
  try {
    const parsed = new URL(origin);
    if (parsed.protocol === "tauri:") {
      return parsed.hostname === "localhost" || parsed.hostname === "";
    }
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return false;
    }
    return parsed.hostname === "localhost" || parsed.hostname === "127.0.0.1";
  } catch {
    return false;
  }
}

function corsHeaders(origin: string | null): HeadersInit {
  return {
    "Access-Control-Allow-Origin": isAllowedOrigin(origin)
      ? origin!
      : "http://localhost:1422",
    "Access-Control-Allow-Headers":
      "authorization, x-client-info, apikey, content-type",
    "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
  };
}

function json(
  body: unknown,
  status = 200,
  origin: string | null = null,
): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: {
      ...corsHeaders(origin),
      "Content-Type": "application/json",
    },
  });
}

function consumerError(
  message: string,
  status: number,
  origin: string | null,
): Response {
  return json({ error: message }, status, origin);
}

async function sha256(input: string): Promise<string> {
  const data = new TextEncoder().encode(input);
  const hash = await crypto.subtle.digest("SHA-256", data);
  return Array.from(new Uint8Array(hash))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

type AdminClient = ReturnType<typeof createClient>;

async function readUpstreamJsonBounded(
  response: Response,
  maxBytes: number,
): Promise<unknown | null> {
  const cl = response.headers.get("content-length");
  if (cl) {
    const n = Number(cl);
    if (Number.isFinite(n) && n > maxBytes) return null;
  }
  if (!response.body) return null;
  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let total = 0;
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    if (!value) continue;
    total += value.byteLength;
    if (total > maxBytes) {
      try {
        await reader.cancel();
      } catch {
        // ignore
      }
      return null;
    }
    chunks.push(value);
  }
  const merged = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    merged.set(chunk, offset);
    offset += chunk.byteLength;
  }
  try {
    return JSON.parse(new TextDecoder().decode(merged));
  } catch {
    return null;
  }
}

Deno.serve(async (req) => {
  const origin = req.headers.get("Origin");

  if (req.method === "OPTIONS") {
    return new Response(null, { status: 204, headers: corsHeaders(origin) });
  }

  if (req.method === "GET") {
    const authHeader = req.headers.get("Authorization");
    if (!authHeader?.startsWith("Bearer ")) {
      return consumerError("Unauthorized", 401, origin);
    }
    const supabaseUrl = Deno.env.get("SUPABASE_URL");
    const supabaseAnonKey = Deno.env.get("SUPABASE_ANON_KEY");
    if (!supabaseUrl || !supabaseAnonKey) {
      return consumerError("Coreside Search is not configured", 503, origin);
    }
    const userClient = createClient(supabaseUrl, supabaseAnonKey, {
      global: { headers: { Authorization: authHeader } },
    });
    const {
      data: { user },
      error: userError,
    } = await userClient.auth.getUser();
    if (userError || !user) {
      return consumerError("Unauthorized", 401, origin);
    }
    return json({ ok: true, service: SERVICE, version: VERSION }, 200, origin);
  }

  if (req.method !== "POST") {
    return consumerError("Method not allowed", 405, origin);
  }

  const authHeader = req.headers.get("Authorization");
  if (!authHeader?.startsWith("Bearer ")) {
    return consumerError("Unauthorized", 401, origin);
  }

  // Supported upstream providers: Linkup, Exa, and Firecrawl.
  const linkupKey =
    Deno.env.get("LINKUP_API_KEY")?.trim() ||
    Deno.env.get("LINKUP_SECRET_KEY")?.trim();
  const firecrawlKey =
    Deno.env.get("FIRECRAWL_API_KEY")?.trim() ||
    Deno.env.get("FIRECRAWL_SECRET_KEY")?.trim();
  const exaKey =
    Deno.env.get("EXA_API_KEY")?.trim() ||
    Deno.env.get("EXA_SECRET_KEY")?.trim();
  if (!linkupKey && !firecrawlKey && !exaKey) {
    return consumerError("Coreside Search is not configured", 503, origin);
  }

  const supabaseUrl = Deno.env.get("SUPABASE_URL");
  const supabaseAnonKey = Deno.env.get("SUPABASE_ANON_KEY");
  const serviceRoleKey = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY");
  if (!supabaseUrl || !supabaseAnonKey || !serviceRoleKey) {
    return consumerError("Coreside Search is not configured", 503, origin);
  }

  const userClient = createClient(supabaseUrl, supabaseAnonKey, {
    global: { headers: { Authorization: authHeader } },
  });
  const adminClient: AdminClient = createClient(supabaseUrl, serviceRoleKey);

  const {
    data: { user },
    error: userError,
  } = await userClient.auth.getUser();
  if (userError || !user) {
    return consumerError("Unauthorized", 401, origin);
  }

  const bodyRead = await readBodyTextBounded(req, MAX_SEARCH_BODY_BYTES);
  if (!bodyRead.ok) {
    return consumerError(bodyRead.error, bodyRead.status, origin);
  }

  let body: SearchRequestBody | null;
  try {
    body = parseSearchBody(bodyRead.text ? JSON.parse(bodyRead.text) : null);
  } catch {
    return consumerError("Invalid request body", 400, origin);
  }
  if (!body) {
    return consumerError("Invalid request body", 400, origin);
  }

  const requestId = crypto.randomUUID();
  const { data: reserveData, error: reserveError } = await adminClient.rpc(
    "reserve_hosted_search_request",
    {
      p_user_id: user.id,
      p_request_id: requestId,
      p_idempotency_key: body.idempotencyKey,
      p_ttl_hours: IDEMPOTENCY_TTL_HOURS,
      p_rate_limit_per_minute: RATE_LIMIT_PER_MINUTE,
      p_max_concurrent: MAX_CONCURRENT,
    },
  );
  const reserve = reserveError
    ? parseSearchReserveRpcResult({ outcome: "error", reason: "rpc_error" })
    : parseSearchReserveRpcResult(reserveData);

  if (reserve.kind === "idempotent" && reserve.status === "completed") {
    const cachedResults = Array.isArray(reserve.resultReference)
      ? reserve.resultReference
      : [];
    return json(
      {
        requestId: reserve.requestId,
        results: cachedResults,
        status: "completed",
        idempotent: true,
      },
      200,
      origin,
    );
  }
  if (reserve.kind === "conflict") {
    return consumerError("Coreside Search request is already in progress", 409, origin);
  }
  if (reserve.kind === "denied") {
    const status =
      reserve.reason === "allowance_exceeded"
        ? 402
        : reserve.reason === "rate_limited" ||
            reserve.reason === "concurrency_limited"
          ? 429
          : 403;
    const message =
      reserve.reason === "allowance_exceeded"
        ? "Coreside Search allowance exceeded"
        : reserve.reason === "hosted_disabled"
          ? "Coreside Search is not enabled for this account"
          : reserve.reason === "rate_limited"
            ? "Rate limit exceeded"
            : "Coreside Search is unavailable";
    return consumerError(message, status, origin);
  }
  if (reserve.kind !== "reserved") {
    return consumerError("Coreside Search is unavailable", 503, origin);
  }

  const numResults = body.numResults ?? DEFAULT_RESULTS;
  // Fingerprint is per-user cache key material — never log raw query.
  const fingerprint = await sha256(
    JSON.stringify({
      intent: body.intent ?? "lookup",
      url: body.url ?? null,
      query: body.query,
      numResults,
    }),
  );

  const { data: cached } = await adminClient
    .from("hosted_search_cache")
    .select("result_json, expires_at")
    .eq("user_id", user.id)
    .eq("fingerprint", fingerprint)
    .maybeSingle();

  if (cached && new Date(cached.expires_at).getTime() > Date.now()) {
    const results = boundSearchResults(cached.result_json, numResults);
    const { data: settleData, error: settleError } = await adminClient.rpc(
      "settle_hosted_search_request",
      {
        p_user_id: user.id,
        p_request_id: requestId,
        p_idempotency_key: body.idempotencyKey,
        p_result_reference: results,
        p_provider_usage: { fingerprint, numResults, cache_hit: true },
      },
    );
    if (settleError || !parseSearchSettleRpcResult(settleData)) {
      await adminClient.rpc("fail_hosted_search_request", {
        p_user_id: user.id,
        p_request_id: requestId,
        p_idempotency_key: body.idempotencyKey,
        p_failure_reason: "settle_failed",
      });
      return consumerError("Coreside Search is unavailable", 503, origin);
    }
    return json({ requestId, results, status: "completed" }, 200, origin);
  }

  if (req.signal.aborted) {
    await adminClient.rpc("fail_hosted_search_request", {
      p_user_id: user.id,
      p_request_id: requestId,
      p_idempotency_key: body.idempotencyKey,
      p_failure_reason: "client_aborted",
    });
    return consumerError("Coreside Search request failed", 499, origin);
  }

  const upstreamAbort = new AbortController();
  const onAbort = () => upstreamAbort.abort();
  req.signal.addEventListener("abort", onAbort);
  const timeout = setTimeout(() => upstreamAbort.abort(), UPSTREAM_TIMEOUT_MS);

  let results: unknown[] = [];
  let usedProvider = "unknown";
  let hadUpstreamError = false;

  try {
    const isDirectUrl = body.intent === "known_url" ||
      body.query.startsWith("http://") ||
      body.query.startsWith("https://") ||
      Boolean(body.url);

    // 1. Direct URL: Firecrawl v2 scrape takes top priority
    if (isDirectUrl && firecrawlKey) {
      const candidateUrl = body.url || body.query;
      const urlCheck = validatePublicWebUrl(candidateUrl);
      if (!urlCheck.valid || !urlCheck.url) {
        await adminClient.rpc("fail_hosted_search_request", {
          p_user_id: user.id,
          p_request_id: requestId,
          p_idempotency_key: body.idempotencyKey,
          p_failure_reason: "invalid_public_url",
        });
        return consumerError("Invalid or private URL destination", 400, origin);
      }
      const targetUrl = urlCheck.url.href;
      try {
        const scrapeResp = await fetch("https://api.firecrawl.dev/v2/scrape", {
          method: "POST",
          headers: {
            Authorization: `Bearer ${firecrawlKey}`,
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            url: targetUrl,
            formats: ["markdown"],
            onlyMainContent: true,
          }),
          signal: upstreamAbort.signal,
        });
        if (scrapeResp.ok) {
          const payload = await readUpstreamJsonBounded(scrapeResp, MAX_UPSTREAM_RESPONSE_BYTES);
          if (payload && typeof payload === "object") {
            const parsed = normalizeFirecrawlScrapeResult(payload, targetUrl);
            if (parsed.length > 0) {
              results = parsed;
              usedProvider = "firecrawl";
            }
          }
        }
      } catch {
        // Fall back to discovery search
      }
    }

    // 2. Deep research intent: Parallel discovery (Linkup + Exa) when both keys available
    if (results.length === 0 && body.intent === "deep_research" && (linkupKey || exaKey)) {
      const promises: Promise<unknown[]>[] = [];

      if (linkupKey) {
        promises.push(
          (async () => {
            try {
              const resp = await fetch("https://api.linkup.so/v1/search", {
                method: "POST",
                headers: {
                  Authorization: `Bearer ${linkupKey}`,
                  "Content-Type": "application/json",
                },
                body: JSON.stringify({
                  q: body.query,
                  depth: "standard",
                  outputType: "searchResults",
                  maxResults: numResults,
                }),
                signal: upstreamAbort.signal,
              });
              if (resp.ok) {
                const payload = await readUpstreamJsonBounded(resp, MAX_UPSTREAM_RESPONSE_BYTES);
                return normalizeProviderResults(payload, numResults);
              }
            } catch {
              hadUpstreamError = true;
            }
            return [];
          })(),
        );
      }

      if (exaKey) {
        promises.push(
          (async () => {
            try {
              const resp = await fetch("https://api.exa.ai/search", {
                method: "POST",
                headers: {
                  "x-api-key": exaKey,
                  "Content-Type": "application/json",
                },
                body: JSON.stringify({
                  query: body.query,
                  numResults,
                  type: "neural",
                  contents: {
                    highlights: { numSentences: 3 },
                  },
                }),
                signal: upstreamAbort.signal,
              });
              if (resp.ok) {
                const payload = await readUpstreamJsonBounded(resp, MAX_UPSTREAM_RESPONSE_BYTES);
                return normalizeExaResults(payload, numResults);
              }
            } catch {
              hadUpstreamError = true;
            }
            return [];
          })(),
        );
      }

      const settled = await Promise.allSettled(promises);
      const merged: unknown[] = [];
      for (const res of settled) {
        if (res.status === "fulfilled" && Array.isArray(res.value)) {
          merged.push(...res.value);
        }
      }

      if (merged.length > 0) {
        results = deduplicateAndRankResults(merged, body.query, numResults);
        usedProvider = "hybrid";
      }
    }

    // 3. Linkup discovery (standard / fast)
    if (results.length === 0 && linkupKey) {
      try {
        const linkupResp = await fetch("https://api.linkup.so/v1/search", {
          method: "POST",
          headers: {
            Authorization: `Bearer ${linkupKey}`,
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            q: body.query,
            depth: body.intent === "news" ? "fast" : "standard",
            outputType: "searchResults",
            maxResults: numResults,
          }),
          signal: upstreamAbort.signal,
        });
        if (linkupResp.ok) {
          const payload = await readUpstreamJsonBounded(linkupResp, MAX_UPSTREAM_RESPONSE_BYTES);
          if (payload && typeof payload === "object") {
            results = normalizeProviderResults(payload, numResults);
            if (results.length > 0) {
              usedProvider = "linkup";
            }
          }
        } else {
          hadUpstreamError = true;
        }
      } catch {
        hadUpstreamError = true;
      }
    }

    // 4. Exa discovery (neural/semantic discovery fallback or primary)
    if (results.length === 0 && exaKey) {
      try {
        const exaResp = await fetch("https://api.exa.ai/search", {
          method: "POST",
          headers: {
            "x-api-key": exaKey,
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            query: body.query,
            numResults,
            type: body.intent === "deep_research" ? "neural" : "auto",
            contents: {
              highlights: { numSentences: 3 },
            },
          }),
          signal: upstreamAbort.signal,
        });
        if (exaResp.ok) {
          const payload = await readUpstreamJsonBounded(exaResp, MAX_UPSTREAM_RESPONSE_BYTES);
          if (payload && typeof payload === "object") {
            results = normalizeExaResults(payload, numResults);
            if (results.length > 0) {
              usedProvider = "exa";
            }
          }
        } else {
          hadUpstreamError = true;
        }
      } catch {
        hadUpstreamError = true;
      }
    }

    // 5. Firecrawl v2 search fallback
    if (results.length === 0 && firecrawlKey) {
      try {
        const firecrawlResp = await fetch("https://api.firecrawl.dev/v2/search", {
          method: "POST",
          headers: {
            Authorization: `Bearer ${firecrawlKey}`,
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            query: body.query,
            limit: numResults,
            scrapeOptions: { formats: ["markdown"], onlyMainContent: true },
          }),
          signal: upstreamAbort.signal,
        });
        if (firecrawlResp.ok) {
          const payload = await readUpstreamJsonBounded(firecrawlResp, MAX_UPSTREAM_RESPONSE_BYTES);
          if (payload && typeof payload === "object") {
            results = normalizeFirecrawlResults(payload, numResults);
            if (results.length > 0) {
              usedProvider = "firecrawl";
            }
          }
        } else {
          hadUpstreamError = true;
        }
      } catch {
        hadUpstreamError = true;
      }
    }
  } catch {
    clearTimeout(timeout);
    req.signal.removeEventListener("abort", onAbort);
    await adminClient.rpc("fail_hosted_search_request", {
      p_user_id: user.id,
      p_request_id: requestId,
      p_idempotency_key: body.idempotencyKey,
      p_failure_reason: req.signal.aborted ? "client_aborted" : "upstream_timeout",
    });
    return consumerError("Coreside Search request failed", 502, origin);
  } finally {
    clearTimeout(timeout);
    req.signal.removeEventListener("abort", onAbort);
  }

  if (results.length === 0 && hadUpstreamError && !req.signal.aborted) {
    await adminClient.rpc("fail_hosted_search_request", {
      p_user_id: user.id,
      p_request_id: requestId,
      p_idempotency_key: body.idempotencyKey,
      p_failure_reason: "upstream_error",
    });
    return consumerError("Coreside Search request failed", 502, origin);
  }

  const cacheExpires = new Date(Date.now() + 60 * 60 * 1000).toISOString();
  await adminClient.from("hosted_search_cache").upsert({
    user_id: user.id,
    fingerprint,
    params_json: { provider: usedProvider, query: body.query, numResults },
    result_json: results,
    expires_at: cacheExpires,
    hit_count: 0,
  });

  const { data: settleData, error: settleError } = await adminClient.rpc(
    "settle_hosted_search_request",
    {
      p_user_id: user.id,
      p_request_id: requestId,
      p_idempotency_key: body.idempotencyKey,
      p_result_reference: results,
      p_provider_usage: { fingerprint, numResults, provider: usedProvider, cache_hit: false },
    },
  );
  if (settleError || !parseSearchSettleRpcResult(settleData)) {
    // Allowance already consumed; mark reconciliation if settle fails after spend.
    await adminClient
      .from("hosted_search_idempotency")
      .update({ status: "reconciliation_required" })
      .eq("user_id", user.id)
      .eq("idempotency_key", body.idempotencyKey);
    return consumerError("Coreside Search is unavailable", 503, origin);
  }

  return json({ requestId, results, status: "completed" }, 200, origin);
});
