import { describe, expect, it } from "vitest";
import { readBodyTextBounded } from "./read-body.ts";

function requestWithBody(
  body: string,
  headers?: Record<string, string>,
): Request {
  return new Request("http://localhost/test", {
    method: "POST",
    headers: { "Content-Type": "application/json", ...headers },
    body,
  });
}

describe("readBodyTextBounded", () => {
  it("rejects oversized Content-Length before reading", async () => {
    const req = requestWithBody("{}", { "Content-Length": "999999" });
    const result = await readBodyTextBounded(req, 100);
    expect(result).toEqual({
      ok: false,
      status: 413,
      error: "Request body too large",
    });
  });

  it("accepts a body within the byte cap", async () => {
    const req = requestWithBody('{"ok":true}');
    const result = await readBodyTextBounded(req, 1024);
    expect(result).toEqual({ ok: true, text: '{"ok":true}' });
  });

  it("caps cumulative stream bytes without Content-Length", async () => {
    const big = "x".repeat(200);
    const req = requestWithBody(big);
    // Strip Content-Length so only the cumulative cap applies.
    const headers = new Headers(req.headers);
    headers.delete("content-length");
    const stripped = new Request(req.url, {
      method: "POST",
      headers,
      body: big,
    });
    const result = await readBodyTextBounded(stripped, 50);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.status).toBe(413);
  });
});
