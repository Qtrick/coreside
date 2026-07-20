import { createClient } from "https://esm.sh/@supabase/supabase-js@2.49.1";

type AiProfile = "fast" | "balanced" | "best";

interface ChatMessage {
  role: string;
  content: string;
}

interface AiRequestBody {
  messages: ChatMessage[];
  profile?: AiProfile;
  idempotencyKey: string;
  maxOutputTokens?: number;
  /** Desktop prefers false (JSON). Streaming remains available for future UI. */
  stream?: boolean;
}

const SERVICE = "coreside-ai-gateway";
const VERSION = "1";
const MAX_OUTPUT_TOKENS = 4096;
const RATE_LIMIT_PER_MINUTE = 20;
const IDEMPOTENCY_TTL_HOURS = 24;

const PROFILE_MAX_TOKENS: Record<AiProfile, number> = {
  fast: 1024,
  balanced: 2048,
  best: 4096,
};

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

function parseBody(raw: unknown): AiRequestBody | null {
  if (!raw || typeof raw !== "object") return null;
  const body = raw as Record<string, unknown>;
  if (!Array.isArray(body.messages) || typeof body.idempotencyKey !== "string") {
    return null;
  }
  const allowedRoles = new Set(["system", "user", "assistant"]);
  const messages = body.messages.filter(
    (m): m is ChatMessage =>
      !!m &&
      typeof m === "object" &&
      typeof (m as ChatMessage).role === "string" &&
      allowedRoles.has((m as ChatMessage).role) &&
      typeof (m as ChatMessage).content === "string",
  );
  if (messages.length === 0 || body.idempotencyKey.trim() === "") return null;

  const profile = body.profile;
  if (
    profile !== undefined &&
    profile !== "fast" &&
    profile !== "balanced" &&
    profile !== "best"
  ) {
    return null;
  }

  let maxOutputTokens: number | undefined;
  if (body.maxOutputTokens !== undefined) {
    if (typeof body.maxOutputTokens !== "number" || body.maxOutputTokens < 1) {
      return null;
    }
    maxOutputTokens = Math.min(
      Math.floor(body.maxOutputTokens),
      MAX_OUTPUT_TOKENS,
    );
  }

  const stream = body.stream === undefined ? true : body.stream === true;

  return {
    messages,
    profile,
    idempotencyKey: body.idempotencyKey.trim(),
    maxOutputTokens,
    stream,
  };
}

function resolveMaxTokens(profile: AiProfile | undefined, override?: number): number {
  const profileCap = PROFILE_MAX_TOKENS[profile ?? "balanced"];
  if (override === undefined) return profileCap;
  return Math.min(override, profileCap, MAX_OUTPUT_TOKENS);
}

function upstreamChatUrl(): string {
  const explicit = Deno.env.get("CORESIDE_AI_BASE_URL")?.trim();
  if (explicit) return `${explicit.replace(/\/$/, "")}/chat/completions`;
  const provider = (Deno.env.get("CORESIDE_AI_PROVIDER") ?? "openai").toLowerCase();
  if (provider === "openrouter") {
    return "https://openrouter.ai/api/v1/chat/completions";
  }
  return "https://api.openai.com/v1/chat/completions";
}

function extractAssistantText(payload: unknown): string | null {
  if (!payload || typeof payload !== "object") return null;
  const content = (payload as { choices?: Array<{ message?: { content?: unknown } }> })
    .choices?.[0]?.message?.content;
  return typeof content === "string" && content.trim() ? content : null;
}

/** Optimistic-lock reserve before upstream so concurrent requests cannot overspend. */
async function tryReserveEntitlement(
  adminClient: ReturnType<typeof createClient>,
  userId: string,
  usedAmount: number,
  hardLimitEnabled: boolean,
  allowanceAmount: number,
): Promise<boolean> {
  if (hardLimitEnabled && usedAmount >= allowanceAmount) return false;
  const { data } = await adminClient
    .from("ai_entitlements")
    .update({ used_amount: usedAmount + 1 })
    .eq("user_id", userId)
    .eq("used_amount", usedAmount)
    .select("user_id")
    .maybeSingle();
  return !!data;
}

async function releaseEntitlement(
  adminClient: ReturnType<typeof createClient>,
  userId: string,
  reservedFrom: number,
): Promise<void> {
  await adminClient
    .from("ai_entitlements")
    .update({ used_amount: reservedFrom })
    .eq("user_id", userId)
    .eq("used_amount", reservedFrom + 1);
}

async function recordSuccess(
  adminClient: ReturnType<typeof createClient>,
  userId: string,
  requestId: string,
  idempotencyKey: string,
  profile: AiProfile,
  maxTokens: number,
  resultText: string,
  streamed: boolean,
): Promise<void> {
  await adminClient.from("ai_usage_ledger").insert({
    user_id: userId,
    request_id: requestId,
    request_type: "chat_completion",
    profile,
    provider_usage_json: { streamed, max_tokens: maxTokens },
    estimated_cost: null,
    actual_cost: null,
    status: "completed",
  });
  await adminClient
    .from("ai_request_idempotency")
    .update({
      status: "completed",
      result_reference: resultText.slice(0, 16_384),
    })
    .eq("user_id", userId)
    .eq("idempotency_key", idempotencyKey);
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
      return consumerError("Coreside AI is not configured", 503, origin);
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

  const providerKey = Deno.env.get("CORESIDE_AI_PROVIDER_API_KEY")?.trim();
  const defaultModel = Deno.env.get("CORESIDE_AI_DEFAULT_MODEL")?.trim();
  if (!providerKey || !defaultModel) {
    return consumerError("Coreside AI is not configured", 503, origin);
  }

  const supabaseUrl = Deno.env.get("SUPABASE_URL");
  const supabaseAnonKey = Deno.env.get("SUPABASE_ANON_KEY");
  const serviceRoleKey = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY");
  if (!supabaseUrl || !supabaseAnonKey || !serviceRoleKey) {
    return consumerError("Coreside AI is not configured", 503, origin);
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

  let body: AiRequestBody | null;
  try {
    body = parseBody(await req.json());
  } catch {
    return consumerError("Invalid request body", 400, origin);
  }
  if (!body) {
    return consumerError("Invalid request body", 400, origin);
  }

  const { data: prior } = await adminClient
    .from("ai_request_idempotency")
    .select("request_id, status, result_reference, expires_at")
    .eq("user_id", user.id)
    .eq("idempotency_key", body.idempotencyKey)
    .maybeSingle();

  if (prior && new Date(prior.expires_at).getTime() > Date.now()) {
    if (prior.status === "completed") {
      const text =
        typeof prior.result_reference === "string" ? prior.result_reference : "";
      return json(
        {
          idempotent: true,
          requestId: prior.request_id,
          status: "completed",
          text,
          resultReference: prior.result_reference,
        },
        200,
        origin,
      );
    }
    if (prior.status === "processing") {
      return consumerError("Coreside AI request is already in progress", 409, origin);
    }
    // failed within TTL: fall through and retry
  }

  const { data: entitlement, error: entitlementError } = await userClient
    .from("ai_entitlements")
    .select(
      "hosted_ai_enabled, allowance_amount, used_amount, hard_limit_enabled",
    )
    .eq("user_id", user.id)
    .maybeSingle();

  if (entitlementError || !entitlement) {
    return consumerError("Coreside AI is unavailable", 403, origin);
  }

  if (!entitlement.hosted_ai_enabled) {
    return consumerError("Coreside AI is not enabled for this account", 403, origin);
  }

  const usedAmount = Number(entitlement.used_amount);
  const allowanceAmount = Number(entitlement.allowance_amount);
  const hardLimitEnabled = Boolean(entitlement.hard_limit_enabled);

  if (hardLimitEnabled && usedAmount >= allowanceAmount) {
    return consumerError("Coreside AI allowance exceeded", 402, origin);
  }

  const oneMinuteAgo = new Date(Date.now() - 60_000).toISOString();
  const { count: recentCount, error: rateError } = await adminClient
    .from("ai_usage_ledger")
    .select("id", { count: "exact", head: true })
    .eq("user_id", user.id)
    .gte("created_at", oneMinuteAgo);

  if (rateError) {
    return consumerError("Coreside AI is unavailable", 503, origin);
  }
  if ((recentCount ?? 0) >= RATE_LIMIT_PER_MINUTE) {
    return consumerError("Rate limit exceeded", 429, origin);
  }

  const reserved = await tryReserveEntitlement(
    adminClient,
    user.id,
    usedAmount,
    hardLimitEnabled,
    allowanceAmount,
  );
  if (!reserved) {
    return consumerError("Coreside AI allowance exceeded", 402, origin);
  }

  const requestId = crypto.randomUUID();
  const profile = body.profile ?? "balanced";
  const maxTokens = resolveMaxTokens(profile, body.maxOutputTokens);
  const expiresAt = new Date(
    Date.now() + IDEMPOTENCY_TTL_HOURS * 60 * 60 * 1000,
  ).toISOString();

  await adminClient.from("ai_request_idempotency").upsert({
    user_id: user.id,
    idempotency_key: body.idempotencyKey,
    request_id: requestId,
    status: "processing",
    result_reference: null,
    expires_at: expiresAt,
  });

  const wantStream = body.stream !== false;
  const upstream = await fetch(upstreamChatUrl(), {
    method: "POST",
    headers: {
      Authorization: `Bearer ${providerKey}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      model: defaultModel,
      messages: body.messages,
      max_tokens: maxTokens,
      stream: wantStream,
    }),
  });

  if (!upstream.ok || !upstream.body) {
    await releaseEntitlement(adminClient, user.id, usedAmount);
    await adminClient
      .from("ai_request_idempotency")
      .update({ status: "failed", result_reference: "upstream_error" })
      .eq("user_id", user.id)
      .eq("idempotency_key", body.idempotencyKey);

    return consumerError("Coreside AI request failed", 502, origin);
  }

  if (!wantStream) {
    let payload: unknown;
    try {
      payload = await upstream.json();
    } catch {
      await releaseEntitlement(adminClient, user.id, usedAmount);
      await adminClient
        .from("ai_request_idempotency")
        .update({ status: "failed", result_reference: "parse_error" })
        .eq("user_id", user.id)
        .eq("idempotency_key", body.idempotencyKey);
      return consumerError("Coreside AI request failed", 502, origin);
    }
    const text = extractAssistantText(payload);
    if (!text) {
      await releaseEntitlement(adminClient, user.id, usedAmount);
      await adminClient
        .from("ai_request_idempotency")
        .update({ status: "failed", result_reference: "empty_response" })
        .eq("user_id", user.id)
        .eq("idempotency_key", body.idempotencyKey);
      return consumerError("Coreside AI request failed", 502, origin);
    }
    await recordSuccess(
      adminClient,
      user.id,
      requestId,
      body.idempotencyKey,
      profile,
      maxTokens,
      text,
      false,
    );
    return json(
      { requestId, text, status: "completed" },
      200,
      origin,
    );
  }

  const decoder = new TextDecoder();
  const reader = upstream.body.getReader();
  let assistantText = "";

  const stream = new ReadableStream({
    async start(controller) {
      const encoder = new TextEncoder();
      try {
        while (true) {
          const { done, value } = await reader.read();
          if (done) break;
          const chunk = decoder.decode(value, { stream: true });
          // Extract delta content from SSE so ledger/idempotency store text, not raw frames.
          for (const line of chunk.split("\n")) {
            const trimmed = line.trim();
            if (!trimmed.startsWith("data:")) continue;
            const data = trimmed.slice(5).trim();
            if (!data || data === "[DONE]") continue;
            try {
              const parsed = JSON.parse(data) as {
                choices?: Array<{ delta?: { content?: unknown } }>;
              };
              const delta = parsed.choices?.[0]?.delta?.content;
              if (typeof delta === "string") assistantText += delta;
            } catch {
              // ignore malformed SSE lines
            }
          }
          controller.enqueue(encoder.encode(chunk));
        }
        controller.close();

        await recordSuccess(
          adminClient,
          user.id,
          requestId,
          body.idempotencyKey,
          profile,
          maxTokens,
          assistantText || "[streamed]",
          true,
        );
      } catch {
        controller.error(new Error("stream_failed"));
        await releaseEntitlement(adminClient, user.id, usedAmount);
        await adminClient
          .from("ai_request_idempotency")
          .update({ status: "failed", result_reference: "stream_error" })
          .eq("user_id", user.id)
          .eq("idempotency_key", body.idempotencyKey);
      }
    },
  });

  return new Response(stream, {
    status: 200,
    headers: {
      ...corsHeaders(origin),
      "Content-Type": "text/event-stream",
      "Cache-Control": "no-cache",
      Connection: "keep-alive",
      "X-Coreside-Request-Id": requestId,
    },
  });
});
