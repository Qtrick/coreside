import { describe, expect, it } from "vitest";
import {
  boundSearchResults,
  normalizeProviderResults,
  parseSearchBody,
  parseSearchFailRpcResult,
  parseSearchReserveRpcResult,
  parseSearchSettleRpcResult,
} from "./search-lib.ts";

describe("search-gateway parseSearchBody", () => {
  it("requires query and idempotencyKey", () => {
    expect(parseSearchBody({ query: "cats", idempotencyKey: "k1" })).toEqual({
      query: "cats",
      idempotencyKey: "k1",
      numResults: 5,
    });
    expect(parseSearchBody({ query: "cats" })).toBeNull();
  });

  it("rejects oversized queries", () => {
    expect(
      parseSearchBody({ query: "x".repeat(600), idempotencyKey: "k" }),
    ).toBeNull();
  });
});

describe("search-gateway Linkup normalization", () => {
  it("keeps public text sources and rejects private or non-web URLs", () => {
    const results = normalizeProviderResults({
      results: [
        { type: "text", name: "Public", url: "https://example.com/a", content: "ok" },
        { type: "text", url: "http://172.16.0.1/private" },
        { type: "text", url: "http://100.64.0.1/private" },
        { type: "text", url: "http://169.254.169.254/latest" },
        { type: "text", url: "http://[::ffff:127.0.0.1]/private" },
        { type: "text", url: "http://[fe80::1]/private" },
        { type: "text", url: "file:///etc/passwd" },
        { type: "image", url: "https://example.com/image" },
      ],
    }, 5);
    expect(results).toEqual([expect.objectContaining({ url: "https://example.com/a", provider: "linkup" })]);
  });
});

describe("search-gateway RPC parsing", () => {
  it("maps reserve outcomes", () => {
    expect(
      parseSearchReserveRpcResult({ outcome: "reserved", request_id: "r1" }),
    ).toEqual({ kind: "reserved", requestId: "r1" });
    expect(
      parseSearchSettleRpcResult({ outcome: "settled" }),
    ).toBe(true);
    expect(parseSearchFailRpcResult({ outcome: "released" })).toBe(true);
  });
});

describe("search-gateway result bounds", () => {
  it("slices and truncates long strings", () => {
    const results = boundSearchResults(
      [{ title: "a".repeat(5000), url: "https://example.com" }, { title: "b" }],
      1,
    );
    expect(results).toHaveLength(1);
    expect((results[0] as { title: string }).title.length).toBe(4000);
  });
});
