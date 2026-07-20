import { createClient } from "https://esm.sh/@supabase/supabase-js@2.49.1";

interface SearchRequestBody {
  query: string;
  idempotencyKey: string;
  numResults?: number;
}

const SERVICE = "coreside-search-gateway";
const VERSION = "1";
const MAX_RESULTS = 10;
const DEFAULT_RESULTS = 5;
const RATE_LIMIT_PER_MINUTE = 20;

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
    // Host equality only — never prefix-match (blocks localhost.evil.com).
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

function parseBody(raw: unknown): SearchRequestBody | null {
  if (!raw || typeof raw !== "object") return null;
  const body = raw as Record<string, unknown>;
  if (typeof body.query !== "string" || typeof body.idempotencyKey !== "string") {
    return null;
  }
  const query = body.query.trim();
  if (!query || body.idempotencyKey.trim() === "") return null;

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

async function sha256(input: string): Promise<string> {
  const data = new TextEncoder().encode(input);
  const hash = await crypto.subtle.digest("SHA-256", data);
  return Array.from(new Uint8Array(hash))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
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

  const exaKey = Deno.env.get("EXA_API_KEY")?.trim();
  if (!exaKey) {
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
  const adminClient = createClient(supabaseUrl, serviceRoleKey);

  const {
    data: { user },
    error: userError,
  } = await userClient.auth.getUser();
  if (userError || !user) {
    return consumerError("Unauthorized", 401, origin);
  }

  let body: SearchRequestBody | null;
  try {
    body = parseBody(await req.json());
  } catch {
    return consumerError("Invalid request body", 400, origin);
  }
  if (!body) {
    return consumerError("Invalid request body", 400, origin);
  }

  const { data: entitlement, error: entitlementError } = await userClient
    .from("ai_entitlements")
    .select(
      "hosted_search_enabled, allowance_amount, used_amount, hard_limit_enabled",
    )
    .eq("user_id", user.id)
    .maybeSingle();

  if (entitlementError || !entitlement) {
    return consumerError("Coreside Search is unavailable", 403, origin);
  }

  if (!entitlement.hosted_search_enabled) {
    return consumerError(
      "Coreside Search is not enabled for this account",
      403,
      origin,
    );
  }

  const usedAmount = Number(entitlement.used_amount);
  const allowanceAmount = Number(entitlement.allowance_amount);
  const hardLimitEnabled = Boolean(entitlement.hard_limit_enabled);

  if (hardLimitEnabled && usedAmount >= allowanceAmount) {
    return consumerError("Coreside Search allowance exceeded", 402, origin);
  }

  const oneMinuteAgo = new Date(Date.now() - 60_000).toISOString();
  const { count: recentCount, error: rateError } = await adminClient
    .from("hosted_search_usage")
    .select("id", { count: "exact", head: true })
    .eq("user_id", user.id)
    .gte("created_at", oneMinuteAgo);

  if (rateError) {
    return consumerError("Coreside Search is unavailable", 503, origin);
  }
  if ((recentCount ?? 0) >= RATE_LIMIT_PER_MINUTE) {
    return consumerError("Rate limit exceeded", 429, origin);
  }

  const numResults = body.numResults ?? DEFAULT_RESULTS;
  const cacheParams = { query: body.query, numResults };
  const fingerprint = await sha256(JSON.stringify(cacheParams));

  const { data: cached } = await adminClient
    .from("hosted_search_cache")
    .select("result_json, expires_at")
    .eq("fingerprint", fingerprint)
    .maybeSingle();

  if (cached && new Date(cached.expires_at).getTime() > Date.now()) {
    return json({ cached: true, results: cached.result_json }, 200, origin);
  }

  // Reserve shared allowance before Exa spend (optimistic lock).
  const { data: reserved } = await adminClient
    .from("ai_entitlements")
    .update({ used_amount: usedAmount + 1 })
    .eq("user_id", user.id)
    .eq("used_amount", usedAmount)
    .select("user_id")
    .maybeSingle();
  if (!reserved) {
    return consumerError("Coreside Search allowance exceeded", 402, origin);
  }

  const requestId = crypto.randomUUID();

  const upstream = await fetch("https://api.exa.ai/search", {
    method: "POST",
    headers: {
      "x-api-key": exaKey,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      query: body.query,
      numResults,
      type: "auto",
      contents: { highlights: { maxCharacters: 1000 } },
    }),
  });

  if (!upstream.ok) {
    await adminClient
      .from("ai_entitlements")
      .update({ used_amount: usedAmount })
      .eq("user_id", user.id)
      .eq("used_amount", usedAmount + 1);
    return consumerError("Coreside Search request failed", 502, origin);
  }

  const payload = await upstream.json();
  const results = Array.isArray(payload.results) ? payload.results.slice(0, numResults) : [];

  const cacheExpires = new Date(Date.now() + 60 * 60 * 1000).toISOString();
  await adminClient.from("hosted_search_cache").upsert({
    fingerprint,
    params_json: cacheParams,
    result_json: results,
    expires_at: cacheExpires,
    hit_count: 0,
  });

  await adminClient.from("hosted_search_usage").insert({
    user_id: user.id,
    request_id: requestId,
    request_type: "exa_search",
    query_fingerprint: fingerprint,
    provider_usage_json: { numResults },
    status: "completed",
  });

  return json({ cached: false, requestId, results }, 200, origin);
});
