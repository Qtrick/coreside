import { describe, expect, it } from "vitest";
import {
  extractAssistantText,
  parseBody,
  appendBoundedAssistantText,
  parseReserveRpcResult,
} from "./gateway-lib.ts";

describe("ai-gateway parseBody", () => {
  it("accepts a minimal valid chat body", () => {
    const body = parseBody({
      messages: [{ role: "user", content: "hi" }],
      idempotencyKey: "key-1",
      stream: false,
    });
    expect(body).toEqual({
      messages: [{ role: "user", content: "hi" }],
      profile: undefined,
      idempotencyKey: "key-1",
      maxOutputTokens: undefined,
      stream: false,
    });
  });

  it("rejects empty messages", () => {
    expect(parseBody({ messages: [], idempotencyKey: "k" })).toBeNull();
  });

  it("defaults stream to true when omitted", () => {
    const body = parseBody({
      messages: [{ role: "user", content: "hi" }],
      idempotencyKey: "k",
    });
    expect(body?.stream).toBe(true);
  });
});

describe("ai-gateway SSE delta extraction", () => {
  it("extracts OpenAI chat.completion.chunk delta text", () => {
    const text = extractAssistantText({
      choices: [{ delta: { content: "hello" } }],
    });
    expect(text).toBe("hello");
  });

  it("returns null for empty delta", () => {
    expect(extractAssistantText({ choices: [{ delta: { content: "" } }] })).toBeNull();
  });
});

describe("ai-gateway bounded assistant text", () => {
  it("appends delta within byte limit", () => {
    expect(appendBoundedAssistantText("hello", " world")).toBe("hello world");
  });

  it("truncates combined text to max UTF-8 bytes", () => {
    const delta = "x".repeat(20_000);
    const result = appendBoundedAssistantText("", delta, 1024);
    expect(new TextEncoder().encode(result).length).toBeLessThanOrEqual(1024);
    expect(result.length).toBeGreaterThan(0);
  });
});

describe("ai-gateway reserve RPC parsing", () => {
  it("maps reserved outcome", () => {
    expect(
      parseReserveRpcResult({ outcome: "reserved", request_id: "req-1" }),
    ).toEqual({ kind: "reserved", requestId: "req-1" });
  });

  it("maps idempotent completed outcome", () => {
    expect(
      parseReserveRpcResult({
        outcome: "idempotent",
        request_id: "req-2",
        status: "completed",
        result_reference: "hello",
      }),
    ).toEqual({
      kind: "idempotent",
      requestId: "req-2",
      status: "completed",
      resultReference: "hello",
    });
  });

  it("maps conflict and denied outcomes", () => {
    expect(
      parseReserveRpcResult({ outcome: "conflict", request_id: "req-3" }),
    ).toEqual({ kind: "conflict", requestId: "req-3" });
    expect(
      parseReserveRpcResult({ outcome: "denied", reason: "allowance_exceeded" }),
    ).toEqual({ kind: "denied", reason: "allowance_exceeded" });
  });
});
