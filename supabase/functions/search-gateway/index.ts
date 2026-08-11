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

  // LINKUP_API_KEY is the canonical deployed secret name. Keep the alternate
  // name as a non-breaking fallback while existing environments transition.
  const linkupKey =
    Deno.env.get("LINKUP_API_KEY")?.trim() ||
    Deno.env.get("LINKUP_SECRET_KEY")?.trim();
  if (!linkupKey) {
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
      provider: "linkup",
      depth: "fast",
      outputType: "searchResults",
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

  let upstream: Response;
  try {
    upstream = await fetch("https://api.linkup.so/v1/search", {
      method: "POST",
      headers: {
        Authorization: `Bearer ${linkupKey}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        q: body.query,
        depth: "fast",
        outputType: "searchResults",
        maxResults: numResults,
      }),
      signal: upstreamAbort.signal,
    });
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

  if (!upstream.ok) {
    await adminClient.rpc("fail_hosted_search_request", {
      p_user_id: user.id,
      p_request_id: requestId,
      p_idempotency_key: body.idempotencyKey,
      p_failure_reason: "upstream_error",
    });
    return consumerError("Coreside Search request failed", 502, origin);
  }

  const payload = await readUpstreamJsonBounded(
    upstream,
    MAX_UPSTREAM_RESPONSE_BYTES,
  );
  if (!payload || typeof payload !== "object") {
    await adminClient.rpc("fail_hosted_search_request", {
      p_user_id: user.id,
      p_request_id: requestId,
      p_idempotency_key: body.idempotencyKey,
      p_failure_reason: "upstream_response_too_large",
    });
    return consumerError("Coreside Search request failed", 502, origin);
  }

  const results = normalizeProviderResults(payload, numResults);

  const cacheExpires = new Date(Date.now() + 60 * 60 * 1000).toISOString();
  await adminClient.from("hosted_search_cache").upsert({
    user_id: user.id,
    fingerprint,
    params_json: { provider: "linkup", depth: "fast", outputType: "searchResults", numResults },
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
      p_provider_usage: { fingerprint, numResults, cache_hit: false },
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
