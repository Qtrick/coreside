import { describe, expect, it } from "vitest";
import {
  deserializeHostedAuthSession,
  isSessionExpired,
  parseHostedAuthSession,
  serializeHostedAuthSession,
} from "./auth-session";
import { getSupabaseConfig } from "./supabase-config";

describe("hosted auth session", () => {
  it("parses a valid session", () => {
    const session = parseHostedAuthSession({
      accessToken: "tok",
      refreshToken: "ref",
      expiresAt: Date.now() + 60_000,
      userId: "user-1",
    });
    expect(session?.userId).toBe("user-1");
    expect(isSessionExpired(session!, Date.now())).toBe(false);
  });

  it("rejects incomplete session", () => {
    expect(parseHostedAuthSession({ accessToken: "tok" })).toBeNull();
  });

  it("round-trips serialize/deserialize", () => {
    const original = {
      accessToken: "tok",
      refreshToken: "ref",
      expiresAt: 1_700_000_000_000,
      userId: "u",
    };
    const raw = serializeHostedAuthSession(original);
    expect(deserializeHostedAuthSession(raw)).toEqual(original);
  });

  it("detects expiry", () => {
    expect(
      isSessionExpired(
        {
          accessToken: "t",
          refreshToken: "r",
          expiresAt: 100,
          userId: "u",
        },
        100,
      ),
    ).toBe(true);
  });
});

describe("supabase config", () => {
  it("returns null when publishable env is unset", () => {
    // Vite test env typically has empty VITE_SUPABASE_* unless set.
    const config = getSupabaseConfig();
    if (!import.meta.env.VITE_SUPABASE_URL) {
      expect(config).toBeNull();
    }
  });
});
