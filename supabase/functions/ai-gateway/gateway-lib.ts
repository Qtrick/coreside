export type AiProfile = "fast" | "balanced" | "best";

export interface ChatMessage {
  role: string;
  content: string;
}

export interface AiRequestBody {
  messages: ChatMessage[];
  profile?: AiProfile;
  idempotencyKey: string;
  maxOutputTokens?: number;
  stream?: boolean;
}

const MAX_OUTPUT_TOKENS = 4096;
/** Matches ai_request_idempotency.result_reference cap in Postgres. */
export const MAX_ASSISTANT_TEXT_BYTES = 16 * 1024;

export const SSE_DEFAULT_LIMITS = {
  maxRawBytes: 2 * 1024 * 1024,
  maxLineBytes: 256 * 1024,
  maxEvents: 50_000,
} as const;

export type SseLimits = {
  maxRawBytes: number;
  maxLineBytes: number;
  maxEvents: number;
};

export type SseFeedResult =
  | { ok: true; done: boolean; text: string }
  | { ok: false; reason: string };

/**
 * Stateful SSE parser: retains partial lines across chunks, honors LF/CRLF,
 * blank-line event boundaries, comments, multiline data, [DONE], and midstream
 * OpenRouter/OpenAI error events. Settlement must wait for terminal success.
 */
export class BoundedSseParser {
  private carry = "";
  private rawBytes = 0;
  private eventCount = 0;
  private pendingData: string[] = [];
  private text = "";
  private sawDone = false;
  private fatal: string | null = null;
  private readonly limits: SseLimits;

  constructor(limits: Partial<SseLimits> = {}) {
    this.limits = { ...SSE_DEFAULT_LIMITS, ...limits };
  }

  get assistantText(): string {
    return this.text;
  }

  get completed(): boolean {
    return this.sawDone && this.fatal === null;
  }

  feed(chunk: string | Uint8Array): SseFeedResult {
    if (this.fatal) return { ok: false, reason: this.fatal };
    if (this.sawDone) return { ok: true, done: true, text: this.text };

    const asText =
      typeof chunk === "string" ? chunk : new TextDecoder().decode(chunk);
    const byteLen = new TextEncoder().encode(asText).length;
    this.rawBytes += byteLen;
    if (this.rawBytes > this.limits.maxRawBytes) {
      this.fatal = "raw_bytes_exceeded";
      return { ok: false, reason: this.fatal };
    }

    this.carry += asText;
    while (true) {
      const nl = this.carry.indexOf("\n");
      if (nl < 0) break;
      let line = this.carry.slice(0, nl);
      this.carry = this.carry.slice(nl + 1);
      if (line.endsWith("\r")) line = line.slice(0, -1);
      if (new TextEncoder().encode(line).length > this.limits.maxLineBytes) {
        this.fatal = "line_bytes_exceeded";
        return { ok: false, reason: this.fatal };
      }
      const result = this.handleLine(line);
      if (!result.ok || result.done) return result;
    }

    if (new TextEncoder().encode(this.carry).length > this.limits.maxLineBytes) {
      this.fatal = "line_bytes_exceeded";
      return { ok: false, reason: this.fatal };
    }
    return { ok: true, done: false, text: this.text };
  }

  /** Flush remainder and require a terminal [DONE] before success. */
  finish(): SseFeedResult {
    if (this.fatal) return { ok: false, reason: this.fatal };
    if (this.carry.length > 0) {
      let line = this.carry;
      this.carry = "";
      if (line.endsWith("\r")) line = line.slice(0, -1);
      const result = this.handleLine(line);
      if (!result.ok) return result;
    }
    if (this.pendingData.length > 0) {
      const result = this.dispatchEvent();
      if (!result.ok) return result;
    }
    if (!this.sawDone) {
      return { ok: false, reason: "eof_without_done" };
    }
    return { ok: true, done: true, text: this.text };
  }

  private handleLine(line: string): SseFeedResult {
    if (line === "") {
      if (this.pendingData.length === 0) {
        return { ok: true, done: this.sawDone, text: this.text };
      }
      return this.dispatchEvent();
    }
    // SSE comments (e.g. `: OPENROUTER PROCESSING`)
    if (line.startsWith(":")) {
      return { ok: true, done: false, text: this.text };
    }
    if (line.startsWith("data:")) {
      const raw = line.slice(5);
      this.pendingData.push(raw.startsWith(" ") ? raw.slice(1) : raw);
      return { ok: true, done: false, text: this.text };
    }
    // Ignore event:/id:/retry: and other fields.
    return { ok: true, done: false, text: this.text };
  }

  private dispatchEvent(): SseFeedResult {
    this.eventCount += 1;
    if (this.eventCount > this.limits.maxEvents) {
      this.fatal = "event_count_exceeded";
      this.pendingData = [];
      return { ok: false, reason: this.fatal };
    }
    const data = this.pendingData.join("\n");
    this.pendingData = [];
    if (data === "[DONE]") {
      this.sawDone = true;
      return { ok: true, done: true, text: this.text };
    }
    try {
      const parsed = JSON.parse(data) as {
        error?: unknown;
        choices?: Array<{ delta?: { content?: unknown } }>;
      };
      if (parsed && typeof parsed === "object" && parsed.error != null) {
        this.fatal = "upstream_error_event";
        return { ok: false, reason: this.fatal };
      }
      const delta = parsed.choices?.[0]?.delta?.content;
      if (typeof delta === "string" && delta) {
        this.text = appendBoundedAssistantText(this.text, delta);
      }
    } catch {
      // ignore malformed SSE data lines
    }
    return { ok: true, done: this.sawDone, text: this.text };
  }
}

export function appendBoundedAssistantText(
  current: string,
  delta: string,
  maxBytes: number = MAX_ASSISTANT_TEXT_BYTES,
): string {
  if (!delta) return current;
  const encoder = new TextEncoder();
  const combined = current + delta;
  const bytes = encoder.encode(combined);
  if (bytes.length <= maxBytes) return combined;
  return new TextDecoder().decode(bytes.slice(0, maxBytes));
}

/** Bound stored assistant text to the Postgres result_reference UTF-8 byte cap. */
export function boundAssistantText(
  text: string,
  maxBytes: number = MAX_ASSISTANT_TEXT_BYTES,
): string {
  return appendBoundedAssistantText("", text, maxBytes);
}

export function parseBody(raw: unknown): AiRequestBody | null {
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

export function extractAssistantText(payload: unknown): string | null {
  if (!payload || typeof payload !== "object") return null;
  const record = payload as {
    choices?: Array<{ message?: { content?: unknown }; delta?: { content?: unknown } }>;
  };
  const content =
    record.choices?.[0]?.message?.content ?? record.choices?.[0]?.delta?.content;
  return typeof content === "string" && content.trim() ? content : null;
}

/** Parsed result from `reserve_hosted_ai_request` (service_role RPC). */
export type ReserveRpcResult =
  | { kind: "reserved"; requestId: string }
  | {
      kind: "idempotent";
      requestId: string;
      status: string;
      resultReference: string | null;
    }
  | { kind: "conflict"; requestId: string }
  | { kind: "denied"; reason: string }
  | { kind: "error"; reason: string };

export function parseReserveRpcResult(data: unknown): ReserveRpcResult {
  if (!data || typeof data !== "object") {
    return { kind: "error", reason: "invalid_rpc_response" };
  }
  const row = data as Record<string, unknown>;
  const outcome = typeof row.outcome === "string" ? row.outcome : "";
  const requestId =
    typeof row.request_id === "string" ? row.request_id : "";
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
            resultReference:
              typeof row.result_reference === "string"
                ? row.result_reference
                : null,
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

/** Transport OK is not enough — PostgREST may return jsonb error outcomes. */
export function parseSettleRpcResult(data: unknown): boolean {
  if (!data || typeof data !== "object") return false;
  const outcome = (data as Record<string, unknown>).outcome;
  return outcome === "settled" || outcome === "idempotent";
}

export function parseFailRpcResult(data: unknown): boolean {
  if (!data || typeof data !== "object") return false;
  const outcome = (data as Record<string, unknown>).outcome;
  return outcome === "released" || outcome === "idempotent";
}

/** Exact HTTPS chat-completions endpoints allowed in production. */
export const UPSTREAM_CHAT_REGISTRY: Record<string, string> = {
  openai: "https://api.openai.com/v1/chat/completions",
  openrouter: "https://openrouter.ai/api/v1/chat/completions",
};

export type UpstreamUrlResult =
  | { ok: true; url: string }
  | { ok: false; reason: string };

/**
 * Resolve upstream chat URL from a server-side provider registry.
 * Production fail-closed on unknown origins; explicit base URL override
 * requires CORESIDE_AI_ALLOW_DEV_BASE_URL=1 (development-only switch).
 */
export function resolveUpstreamChatUrl(env: {
  provider?: string | null;
  baseUrl?: string | null;
  allowDevBaseUrl?: string | null;
}): UpstreamUrlResult {
  const explicit = env.baseUrl?.trim();
  if (explicit) {
    const allow =
      env.allowDevBaseUrl?.trim() === "1" ||
      env.allowDevBaseUrl?.trim().toLowerCase() === "true";
    if (!allow) {
      return { ok: false, reason: "dev_base_url_disabled" };
    }
    try {
      const parsed = new URL(explicit);
      if (parsed.protocol !== "https:" && parsed.protocol !== "http:") {
        return { ok: false, reason: "invalid_dev_base_url" };
      }
      const host = parsed.hostname.toLowerCase();
      // Dev override is loopback-only — never SSRF into cloud metadata / RFC1918.
      if (host !== "localhost" && host !== "127.0.0.1" && host !== "[::1]" && host !== "::1") {
        return { ok: false, reason: "invalid_dev_base_url" };
      }
      return {
        ok: true,
        url: `${explicit.replace(/\/$/, "")}/chat/completions`,
      };
    } catch {
      return { ok: false, reason: "invalid_dev_base_url" };
    }
  }

  const provider = (env.provider ?? "openai").trim().toLowerCase();
  const url = UPSTREAM_CHAT_REGISTRY[provider];
  if (!url) {
    return { ok: false, reason: "unknown_provider" };
  }
  return { ok: true, url };
}
