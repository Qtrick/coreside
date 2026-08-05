import { createClient } from "https://esm.sh/@supabase/supabase-js@2.49.1";
import {
  type AiProfile,
  type AiRequestBody,
  appendBoundedAssistantText,
  boundAssistantText,
  extractAssistantText,
  parseBody,
  parseReserveRpcResult,
} from "./gateway-lib.ts";

const SERVICE = "coreside-ai-gateway";
const VERSION = "1";
const MAX_OUTPUT_TOKENS = 4096;
const MAX_BODY_BYTES = 256 * 1024;
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

type AdminClient = ReturnType<typeof createClient>;

async function reserveHostedRequest(
  adminClient: AdminClient,
  userId: string,
  requestId: string,
  idempotencyKey: string,
) {
  const { data, error } = await adminClient.rpc("reserve_hosted_ai_request", {
    p_user_id: userId,
    p_request_id: requestId,
    p_idempotency_key: idempotencyKey,
    p_ttl_hours: IDEMPOTENCY_TTL_HOURS,
  });
  if (error) {
    return parseReserveRpcResult({ outcome: "error", reason: "rpc_error" });
  }
  return parseReserveRpcResult(data);
}

async function settleHostedRequest(
  adminClient: AdminClient,
  userId: string,
  requestId: string,
  idempotencyKey: string,
  profile: AiProfile,
  maxTokens: number,
  resultText: string,
  streamed: boolean,
): Promise<boolean> {
  const { error } = await adminClient.rpc("settle_hosted_ai_request", {
    p_user_id: userId,
    p_request_id: requestId,
    p_idempotency_key: idempotencyKey,
    p_profile: profile,
    p_result_reference: boundAssistantText(resultText),
    p_provider_usage: { streamed, max_tokens: maxTokens },
  });
  return !error;
}

async function failHostedRequest(
  adminClient: AdminClient,
  userId: string,
  requestId: string,
  idempotencyKey: string,
  failureReason: string,
): Promise<boolean> {
  const { error } = await adminClient.rpc("fail_hosted_ai_request", {
    p_user_id: userId,
    p_request_id: requestId,
    p_idempotency_key: idempotencyKey,
    p_failure_reason: failureReason,
  });
  return !error;
}

async function settleHostedRequestWithRetry(
  adminClient: AdminClient,
  userId: string,
  requestId: string,
  idempotencyKey: string,
  profile: AiProfile,
  maxTokens: number,
  resultText: string,
  streamed: boolean,
): Promise<boolean> {
  if (
    await settleHostedRequest(
      adminClient,
      userId,
      requestId,
      idempotencyKey,
      profile,
      maxTokens,
      resultText,
      streamed,
    )
  ) {
    return true;
  }
  return settleHostedRequest(
    adminClient,
    userId,
    requestId,
    idempotencyKey,
    profile,
    maxTokens,
    resultText,
    streamed,
  );
}

function deniedStatus(reason: string): number {
  if (reason === "allowance_exceeded") return 402;
  return 403;
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
    const raw = await req.text();
    if (new TextEncoder().encode(raw).length > MAX_BODY_BYTES) {
      return consumerError("Request body too large", 413, origin);
    }
    body = parseBody(raw ? JSON.parse(raw) : null);
  } catch {
    return consumerError("Invalid request body", 400, origin);
  }
  if (!body) {
    return consumerError("Invalid request body", 400, origin);
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

  const requestId = crypto.randomUUID();
  const reserve = await reserveHostedRequest(
    adminClient,
    user.id,
    requestId,
    body.idempotencyKey,
  );

  if (reserve.kind === "idempotent" && reserve.status === "completed") {
    const text = reserve.resultReference ?? "";
    return json(
      {
        idempotent: true,
        requestId: reserve.requestId,
        status: "completed",
        text,
        resultReference: reserve.resultReference,
      },
      200,
      origin,
    );
  }
  if (reserve.kind === "conflict") {
    return consumerError("Coreside AI request is already in progress", 409, origin);
  }
  if (reserve.kind === "denied") {
    const message =
      reserve.reason === "allowance_exceeded"
        ? "Coreside AI allowance exceeded"
        : reserve.reason === "hosted_disabled"
          ? "Coreside AI is not enabled for this account"
          : "Coreside AI is unavailable";
    return consumerError(message, deniedStatus(reserve.reason), origin);
  }
  if (reserve.kind !== "reserved") {
    return consumerError("Coreside AI is unavailable", 503, origin);
  }

  const profile = body.profile ?? "balanced";
  const maxTokens = resolveMaxTokens(profile, body.maxOutputTokens);
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
    await failHostedRequest(
      adminClient,
      user.id,
      requestId,
      body.idempotencyKey,
      "upstream_error",
    );
    return consumerError("Coreside AI request failed", 502, origin);
  }

  if (!wantStream) {
    let payload: unknown;
    try {
      payload = await upstream.json();
    } catch {
      await failHostedRequest(
        adminClient,
        user.id,
        requestId,
        body.idempotencyKey,
        "parse_error",
      );
      return consumerError("Coreside AI request failed", 502, origin);
    }
    const text = extractAssistantText(payload);
    if (!text) {
      await failHostedRequest(
        adminClient,
        user.id,
        requestId,
        body.idempotencyKey,
        "empty_response",
      );
      return consumerError("Coreside AI request failed", 502, origin);
    }
    await settleHostedRequestWithRetry(
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
  type TerminalState = "open" | "settled" | "failed";
  let terminal: TerminalState = "open";

  const failIfOpen = async (reason: string) => {
    if (terminal !== "open") return;
    if (
      await failHostedRequest(
        adminClient,
        user.id,
        requestId,
        body.idempotencyKey,
        reason,
      )
    ) {
      terminal = "failed";
    }
  };

  const settleIfOpen = async () => {
    if (terminal !== "open") return;
    if (
      await settleHostedRequestWithRetry(
        adminClient,
        user.id,
        requestId,
        body.idempotencyKey,
        profile,
        maxTokens,
        assistantText || "[streamed]",
        true,
      )
    ) {
      terminal = "settled";
    }
  };

  if (req.signal.aborted) {
    await failIfOpen("client_aborted");
    return consumerError("Coreside AI request failed", 499, origin);
  }
  req.signal.addEventListener("abort", () => {
    void failIfOpen("client_aborted");
  });

  const stream = new ReadableStream({
    async start(controller) {
      const encoder = new TextEncoder();
      try {
        while (true) {
          if (req.signal.aborted) {
            throw new Error("client_aborted");
          }
          const { done, value } = await reader.read();
          if (done) break;
          const chunk = decoder.decode(value, { stream: true });
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
              if (typeof delta === "string") {
                assistantText = appendBoundedAssistantText(assistantText, delta);
              }
            } catch {
              // ignore malformed SSE lines
            }
          }
          controller.enqueue(encoder.encode(chunk));
        }
        controller.close();
        await settleIfOpen();
      } catch {
        try {
          controller.error(new Error("stream_failed"));
        } catch {
          // Client may already have disconnected.
        }
        await failIfOpen(
          req.signal.aborted ? "client_aborted" : "stream_error",
        );
      } finally {
        try {
          reader.releaseLock();
        } catch {
          // Reader may already be released.
        }
      }
    },
    async cancel() {
      await failIfOpen("client_disconnected");
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
