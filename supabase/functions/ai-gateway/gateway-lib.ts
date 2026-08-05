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
                : row.result_reference === null
                  ? null
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
