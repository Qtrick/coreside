import { describe, expect, it } from "vitest";
import {
  BoundedSseParser,
  extractAssistantText,
  parseBody,
  appendBoundedAssistantText,
  parseFailRpcResult,
  parseReserveRpcResult,
  parseSettleRpcResult,
  resolveUpstreamChatUrl,
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

describe("ai-gateway settle/fail RPC outcome checks", () => {
  it("accepts settled and idempotent settle outcomes", () => {
    expect(parseSettleRpcResult({ outcome: "settled" })).toBe(true);
    expect(parseSettleRpcResult({ outcome: "idempotent" })).toBe(true);
    expect(parseSettleRpcResult({ outcome: "error", reason: "invalid_state" })).toBe(
      false,
    );
    expect(parseSettleRpcResult(null)).toBe(false);
  });

  it("accepts released and idempotent fail outcomes", () => {
    expect(parseFailRpcResult({ outcome: "released" })).toBe(true);
    expect(parseFailRpcResult({ outcome: "idempotent" })).toBe(true);
    expect(parseFailRpcResult({ outcome: "error" })).toBe(false);
  });
});

describe("ai-gateway upstream URL registry", () => {
  it("resolves openai and openrouter registry URLs", () => {
    expect(resolveUpstreamChatUrl({ provider: "openai" })).toEqual({
      ok: true,
      url: "https://api.openai.com/v1/chat/completions",
    });
    expect(resolveUpstreamChatUrl({ provider: "openrouter" })).toEqual({
      ok: true,
      url: "https://openrouter.ai/api/v1/chat/completions",
    });
  });

  it("fail-closes unknown providers and unapproved base URLs", () => {
    expect(resolveUpstreamChatUrl({ provider: "evil" }).ok).toBe(false);
    expect(
      resolveUpstreamChatUrl({
        baseUrl: "https://evil.example/v1",
      }).ok,
    ).toBe(false);
  });

  it("allows explicit base URL only with development switch", () => {
    expect(
      resolveUpstreamChatUrl({
        baseUrl: "http://127.0.0.1:8080/v1",
        allowDevBaseUrl: "1",
      }),
    ).toEqual({
      ok: true,
      url: "http://127.0.0.1:8080/v1/chat/completions",
    });
  });

  it("rejects non-loopback explicit base URLs even with development switch", () => {
    expect(
      resolveUpstreamChatUrl({
        baseUrl: "https://evil.example/v1",
        allowDevBaseUrl: "1",
      }).ok,
    ).toBe(false);
    expect(
      resolveUpstreamChatUrl({
        baseUrl: "https://169.254.169.254/",
        allowDevBaseUrl: "1",
      }).ok,
    ).toBe(false);
  });
});

describe("BoundedSseParser fragmented fixtures", () => {
  it("reassembles deltas split across chunk boundaries", () => {
    const parser = new BoundedSseParser();
    const first = parser.feed('data: {"choices":[{"delta":{"content":"hel');
    expect(first.ok).toBe(true);
    if (first.ok) {
      expect(first.forwardFrames).toEqual([]);
    }
    const mid = parser.feed('lo"}}]}\n\n');
    expect(mid.ok).toBe(true);
    if (mid.ok) {
      expect(mid.text).toBe("hello");
      expect(mid.forwardFrames).toHaveLength(1);
      expect(mid.forwardFrames[0]).toContain('"hello"');
    }
    const done = parser.feed("data: [DONE]\n\n");
    expect(done).toEqual({
      ok: true,
      done: true,
      text: "hello",
      forwardFrames: [],
    });
    expect(parser.upstreamTerminalSeen).toBe(true);
    expect(parser.finish()).toEqual({
      ok: true,
      done: true,
      text: "hello",
      forwardFrames: [],
    });
  });

  it("handles CRLF and SSE comments", () => {
    const parser = new BoundedSseParser();
    parser.feed(": OPENROUTER PROCESSING\r\n");
    parser.feed('data: {"choices":[{"delta":{"content":"a"}}]}\r\n\r\n');
    const done = parser.feed("data: [DONE]\r\n\r\n");
    expect(done.ok && done.done).toBe(true);
    if (done.ok) {
      expect(done.forwardFrames).toEqual([]);
    }
    expect(parser.assistantText).toBe("a");
  });

  it("rejects EOF without [DONE] even with text", () => {
    const parser = new BoundedSseParser();
    parser.feed('data: {"choices":[{"delta":{"content":"partial"}}]}\n\n');
    expect(parser.finish()).toEqual({ ok: false, reason: "eof_without_done" });
  });

  it("fails closed on midstream error events", () => {
    const parser = new BoundedSseParser();
    const result = parser.feed(
      'data: {"error":{"message":"upstream failed"}}\n\n',
    );
    expect(result).toEqual({ ok: false, reason: "upstream_error_event" });
  });

  it("bounds event count", () => {
    const parser = new BoundedSseParser({ maxEvents: 2 });
    parser.feed('data: {"choices":[{"delta":{"content":"a"}}]}\n\n');
    parser.feed('data: {"choices":[{"delta":{"content":"b"}}]}\n\n');
    expect(parser.feed('data: {"choices":[{"delta":{"content":"c"}}]}\n\n')).toEqual(
      { ok: false, reason: "event_count_exceeded" },
    );
  });

  it("decodes UTF-8 split across chunk boundaries", () => {
    const parser = new BoundedSseParser();
    const emoji = "😀";
    const bytes = new TextEncoder().encode(
      `data: {"choices":[{"delta":{"content":"${emoji}"}}]}\n\n`,
    );
    const split = Math.max(1, Math.floor(bytes.length / 2));
    const first = parser.feed(bytes.slice(0, split));
    const second = parser.feed(bytes.slice(split));
    expect(first.ok).toBe(true);
    expect(second.ok).toBe(true);
    if (second.ok) expect(second.text).toBe(emoji);
    const done = parser.feed(new TextEncoder().encode("data: [DONE]\n\n"));
    expect(done.ok && done.done).toBe(true);
    expect(parser.assistantText).toBe(emoji);
  });

  it("rejects invalid UTF-8 byte sequences", () => {
    const parser = new BoundedSseParser();
    const invalid = new Uint8Array([0xff, 0xfe, 0x80]);
    expect(parser.feed(invalid)).toEqual({ ok: false, reason: "invalid_utf8" });
  });

  it("fails closed on malformed JSON before assistant output", () => {
    const parser = new BoundedSseParser();
    expect(parser.feed("data: {not-json}\n\n")).toEqual({
      ok: false,
      reason: "malformed_json",
    });
  });

  it("fails closed on malformed JSON after assistant output", () => {
    const parser = new BoundedSseParser();
    parser.feed('data: {"choices":[{"delta":{"content":"ok"}}]}\n\n');
    expect(parser.feed("data: not-json\n\n")).toEqual({
      ok: false,
      reason: "malformed_json",
    });
  });

  it("rejects trailing events in the same chunk after upstream terminal", () => {
    const parser = new BoundedSseParser();
    const result = parser.feed(
      'data: [DONE]\n\ndata: {"choices":[{"delta":{"content":"late"}}]}\n\n',
    );
    expect(result).toEqual({ ok: false, reason: "data_after_terminal" });
  });

  it("rejects duplicate terminal events", () => {
    const parser = new BoundedSseParser();
    parser.feed("data: [DONE]\n\n");
    expect(parser.feed("data: [DONE]\n\n")).toEqual({
      ok: false,
      reason: "data_after_terminal",
    });
  });

  it("rejects output after upstream terminal", () => {
    const parser = new BoundedSseParser();
    parser.feed('data: {"choices":[{"delta":{"content":"x"}}]}\n\n');
    parser.feed("data: [DONE]\n\n");
    expect(
      parser.feed('data: {"choices":[{"delta":{"content":"late"}}]}\n\n'),
    ).toEqual({ ok: false, reason: "data_after_terminal" });
  });

  it("detects upstream done without client-forwardable [DONE] frame", () => {
    const parser = new BoundedSseParser();
    const result = parser.feed("data: [DONE]\n\n");
    expect(result.ok && result.done).toBe(true);
    if (result.ok) {
      expect(result.forwardFrames).toEqual([]);
      expect(result.forwardFrames.some((f) => f.includes("[DONE]"))).toBe(
        false,
      );
    }
    expect(parser.upstreamTerminalSeen).toBe(true);
  });

  it("tracks raw bytes from Uint8Array chunks without re-encoding", () => {
    const parser = new BoundedSseParser({ maxRawBytes: 10 });
    const chunk = new Uint8Array(11);
    expect(parser.feed(chunk)).toEqual({ ok: false, reason: "raw_bytes_exceeded" });
  });

  it("splits CRLF across chunk boundaries", () => {
    const parser = new BoundedSseParser();
    parser.feed('data: {"choices":[{"delta":{"content":"a"}}]}\r');
    const result = parser.feed('\n\r\ndata: [DONE]\r\n\r\n');
    expect(result.ok && result.done).toBe(true);
    expect(parser.assistantText).toBe("a");
  });
});

describe("BoundedSseParser settlement ordering", () => {
  it("requires upstream terminal before finish succeeds (settle gate)", () => {
    const parser = new BoundedSseParser();
    parser.feed('data: {"choices":[{"delta":{"content":"billable"}}]}\n\n');
    expect(parser.finish()).toEqual({ ok: false, reason: "eof_without_done" });
    expect(parser.completed).toBe(false);

    const terminal = parser.feed("data: [DONE]\n\n");
    expect(terminal.ok && terminal.done).toBe(true);
    expect(parser.completed).toBe(true);
    expect(parser.finish().ok).toBe(true);
  });
});
