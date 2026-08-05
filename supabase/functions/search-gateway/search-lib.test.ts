import { describe, expect, it } from "vitest";
import {
  boundSearchResults,
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
