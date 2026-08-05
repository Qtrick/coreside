import { describe, expect, it } from "vitest";
import {
  computeStripeSignature,
  buildStripePortalSessionBody,
  entitlementSyncPayload,
  extractCheckoutSessionSync,
  extractStripeEventId,
  extractStripeEventType,
  extractSubscriptionSync,
  FREE_PLAN_ENTITLEMENTS,
  isRetryableWebhookStatus,
  isTerminalWebhookStatus,
  parseCheckoutBody,
  parsePortalBody,
  parseStripeEventPayload,
  parseStripeSignatureHeader,
  resolveBillingReturnUrl,
  resolveEntitlementsForSubscription,
  shouldProcessStripeEventType,
  verifyStripeWebhookSignature,
} from "./billing-lib.ts";

describe("billing parseCheckoutBody", () => {
  it("accepts personal and pro plans with monthly/annual intervals", () => {
    expect(parseCheckoutBody({ planId: "personal", interval: "monthly" })).toEqual({
      planId: "personal",
      interval: "monthly",
      successDestination: "settings",
      cancelDestination: "settings",
    });
    expect(parseCheckoutBody({ planId: "PRO", interval: "annual" })?.planId).toBe("pro");
    expect(parseCheckoutBody({ planId: "pro", interval: "ANNUAL" })?.interval).toBe(
      "annual",
    );
  });

  it("rejects free, unknown plans, and missing interval", () => {
    expect(parseCheckoutBody({ planId: "free", interval: "monthly" })).toBeNull();
    expect(parseCheckoutBody({ planId: "enterprise", interval: "monthly" })).toBeNull();
    expect(parseCheckoutBody({ planId: "personal" })).toBeNull();
    expect(parseCheckoutBody({ planId: "personal", interval: "weekly" })).toBeNull();
  });

  it("rejects arbitrary success/cancel URLs and client price IDs", () => {
    expect(
      parseCheckoutBody({
        planId: "personal",
        interval: "monthly",
        successUrl: "https://evil.example/phish",
        cancelUrl: "https://evil.example/phish",
      }),
    ).toBeNull();
    expect(
      parseCheckoutBody({
        planId: "personal",
        interval: "monthly",
        priceId: "price_attacker",
      }),
    ).toBeNull();
  });

  it("rejects unknown destinations", () => {
    expect(
      parseCheckoutBody({
        planId: "personal",
        interval: "monthly",
        successDestination: "javascript:alert(1)",
      }),
    ).toBeNull();
  });

  it("encodes exact annual and monthly catalog cents", async () => {
    const { EXPECTED_PRICE_CENTS, CREDIT_MICRO_USD_PER_PERIOD } = await import(
      "./billing-lib.ts"
    );
    expect(EXPECTED_PRICE_CENTS["personal:monthly"]).toBe(1999);
    expect(EXPECTED_PRICE_CENTS["personal:annual"]).toBe(19188);
    expect(EXPECTED_PRICE_CENTS["pro:monthly"]).toBe(4999);
    expect(EXPECTED_PRICE_CENTS["pro:annual"]).toBe(49188);
    expect(CREDIT_MICRO_USD_PER_PERIOD.personal).toBe(20_000_000);
    expect(CREDIT_MICRO_USD_PER_PERIOD.pro).toBe(50_000_000);
    // Superseded annual figures must not appear as active truth.
    expect(Object.values(EXPECTED_PRICE_CENTS)).not.toContain(1699);
    expect(Object.values(EXPECTED_PRICE_CENTS)).not.toContain(4199);
  });

  it("resolves env price IDs only for allowlisted plan+interval", async () => {
    const {
      resolveCheckoutPriceIdFromEnv,
      expectedStripeRecurringInterval,
      isSafeStripePriceId,
    } = await import("./billing-lib.ts");
    const env = {
      get(key: string) {
        const map: Record<string, string> = {
          STRIPE_PRICE_PERSONAL_MONTHLY: "price_abc123",
          STRIPE_PRICE_PRO_ANNUAL: "price_evil/../customers",
        };
        return map[key];
      },
    };
    expect(resolveCheckoutPriceIdFromEnv("personal", "monthly", env)).toBe(
      "price_abc123",
    );
    expect(resolveCheckoutPriceIdFromEnv("free", "monthly", env)).toBeNull();
    expect(resolveCheckoutPriceIdFromEnv("pro", "annual", env)).toBeNull();
    expect(isSafeStripePriceId("price_abc123")).toBe(true);
    expect(isSafeStripePriceId("price_evil/../customers")).toBe(false);
    expect(isSafeStripePriceId("price_has_underscore")).toBe(false);
    expect(expectedStripeRecurringInterval("monthly")).toBe("month");
    expect(expectedStripeRecurringInterval("annual")).toBe("year");
    expect(expectedStripeRecurringInterval("weekly")).toBeNull();
  });

  it("maps allowlisted env price IDs back to plan ids", async () => {
    const { planIdForAllowlistedPriceId } = await import("./billing-lib.ts");
    const env = {
      get(key: string) {
        const map: Record<string, string> = {
          STRIPE_PRICE_PERSONAL_MONTHLY: "price_personalM1",
          STRIPE_PRICE_PRO_ANNUAL: "price_proA1",
        };
        return map[key];
      },
    };
    expect(planIdForAllowlistedPriceId("price_personalM1", env)).toBe("personal");
    expect(planIdForAllowlistedPriceId("price_proA1", env)).toBe("pro");
    expect(planIdForAllowlistedPriceId("price_unknown", env)).toBeNull();
  });
});

describe("billing parsePortalBody", () => {
  it("defaults to settings and rejects arbitrary returnUrl", () => {
    expect(parsePortalBody({})).toEqual({ returnDestination: "settings" });
    expect(
      parsePortalBody({ returnUrl: "https://evil.example/settings" }),
    ).toBeNull();
    expect(parsePortalBody({ returnDestination: "billing" })).toEqual({
      returnDestination: "billing",
    });
  });

  it("builds server-controlled return URLs", () => {
    expect(resolveBillingReturnUrl("settings", "http://localhost:1422")).toBe(
      "http://localhost:1422/settings",
    );
    expect(resolveBillingReturnUrl("https://evil.example", "http://localhost:1422")).toBeNull();
  });

  it("builds Stripe portal session body with customer + return_url", () => {
    const body = buildStripePortalSessionBody({
      customerId: "cus_123",
      returnUrl: "http://localhost:1422/settings",
    });
    expect(body.get("customer")).toBe("cus_123");
    expect(body.get("return_url")).toBe("http://localhost:1422/settings");
  });
});

describe("stripe webhook signature", () => {
  it("parses Stripe-Signature header", () => {
    const parsed = parseStripeSignatureHeader("t=1234567890,v1=abc123");
    expect(parsed).toEqual({ timestamp: 1234567890, signatures: ["abc123"] });
  });

  it("verifies a valid signature", async () => {
    const secret = "whsec_test_secret";
    const payload = '{"id":"evt_123","type":"checkout.session.completed"}';
    const timestamp = 1_700_000_000;
    const sig = await computeStripeSignature(secret, timestamp, payload);
    const result = await verifyStripeWebhookSignature(
      payload,
      `t=${timestamp},v1=${sig}`,
      secret,
      300,
      timestamp,
    );
    expect(result).toEqual({ ok: true });
  });

  it("rejects tampered payload", async () => {
    const secret = "whsec_test_secret";
    const payload = '{"id":"evt_123","type":"checkout.session.completed"}';
    const timestamp = 1_700_000_000;
    const sig = await computeStripeSignature(secret, timestamp, payload);
    const result = await verifyStripeWebhookSignature(
      payload + " ",
      `t=${timestamp},v1=${sig}`,
      secret,
      300,
      timestamp,
    );
    expect(result.ok).toBe(false);
  });
});

describe("stripe event helpers", () => {
  it("extracts event id and type", () => {
    const payload = JSON.stringify({
      id: "evt_123",
      type: "checkout.session.completed",
      data: { object: {} },
    });
    expect(extractStripeEventId(payload)).toBe("evt_123");
    expect(extractStripeEventType(payload)).toBe("checkout.session.completed");
    expect(shouldProcessStripeEventType("checkout.session.completed")).toBe(true);
    expect(shouldProcessStripeEventType("invoice.paid")).toBe(false);
  });

  it("extracts checkout sync fields", () => {
    const event = parseStripeEventPayload(
      JSON.stringify({
        id: "evt_1",
        type: "checkout.session.completed",
        data: {
          object: {
            id: "cs_1",
            customer: "cus_1",
            subscription: "sub_1",
            client_reference_id: "user-1",
            status: "complete",
            payment_status: "paid",
            metadata: { user_id: "user-1", plan_id: "pro", price_id: "price_pro" },
            line_items: { data: [{ price: { id: "price_pro" } }] },
          },
        },
      }),
    );
    const sync = extractCheckoutSessionSync(event!);
    expect(sync?.userId).toBe("user-1");
    expect(sync?.planId).toBe("pro");
  });

  it("extracts subscription sync fields", () => {
    const event = parseStripeEventPayload(
      JSON.stringify({
        id: "evt_2",
        type: "customer.subscription.updated",
        data: {
          object: {
            id: "sub_1",
            customer: "cus_1",
            status: "active",
            current_period_end: 1_700_000_000,
            items: { data: [{ price: { id: "price_pro" } }] },
          },
        },
      }),
    );
    const sync = extractSubscriptionSync(event!);
    expect(sync?.status).toBe("active");
    expect(sync?.priceId).toBe("price_pro");
  });

  it("maps inactive subscription to free entitlements", () => {
    expect(
      resolveEntitlementsForSubscription("canceled", {
        planId: "pro",
        hostedAiEnabled: true,
        hostedSearchEnabled: true,
        allowanceAmount: 100,
        hardLimitEnabled: true,
      }),
    ).toEqual(FREE_PLAN_ENTITLEMENTS);
  });

  it("builds entitlement sync payload", () => {
    const payload = entitlementSyncPayload(FREE_PLAN_ENTITLEMENTS);
    expect(payload.plan_id).toBe("free");
  });
});

describe("stripe webhook status helpers", () => {
  it("classifies terminal and retryable statuses", () => {
    expect(isTerminalWebhookStatus("processed")).toBe(true);
    expect(isTerminalWebhookStatus("permanent_failed")).toBe(true);
    expect(isRetryableWebhookStatus("received")).toBe(true);
    expect(isRetryableWebhookStatus("retryable_failed")).toBe(true);
    expect(isRetryableWebhookStatus("processed")).toBe(false);
  });
});
