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
  parseCheckoutBody,
  parsePortalBody,
  parseStripeEventPayload,
  parseStripeSignatureHeader,
  resolveEntitlementsForSubscription,
  shouldProcessStripeEventType,
  verifyStripeWebhookSignature,
} from "./billing-lib.ts";

describe("billing parseCheckoutBody", () => {
  it("accepts personal and pro plans", () => {
    expect(parseCheckoutBody({ planId: "personal" })?.planId).toBe("personal");
    expect(parseCheckoutBody({ planId: "PRO" })?.planId).toBe("pro");
  });

  it("rejects free and unknown plans", () => {
    expect(parseCheckoutBody({ planId: "free" })).toBeNull();
    expect(parseCheckoutBody({ planId: "enterprise" })).toBeNull();
  });
});

describe("billing parsePortalBody", () => {
  it("accepts optional returnUrl", () => {
    expect(parsePortalBody({ returnUrl: "https://example.com/settings" })).toEqual({
      returnUrl: "https://example.com/settings",
    });
    expect(parsePortalBody({})).toEqual({});
  });

  it("builds Stripe portal session body with customer + return_url", () => {
    const body = buildStripePortalSessionBody({
      customerId: "cus_123",
      returnUrl: "https://example.com/settings",
    });
    expect(body.get("customer")).toBe("cus_123");
    expect(body.get("return_url")).toBe("https://example.com/settings");
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

  it("extracts event id and type", () => {
    const payload = '{"id":"evt_abc","type":"customer.subscription.updated"}';
    expect(extractStripeEventId(payload)).toBe("evt_abc");
    expect(extractStripeEventType(payload)).toBe("customer.subscription.updated");
  });
});

describe("checkout session entitlement sync guardrails", () => {
  it("requires client_reference_id to match metadata user_id", () => {
    const event = parseStripeEventPayload(
      JSON.stringify({
        id: "evt_1",
        type: "checkout.session.completed",
        data: {
          object: {
            status: "complete",
            payment_status: "paid",
            customer: "cus_1",
            subscription: "sub_1",
            client_reference_id: "user-a",
            metadata: {
              user_id: "user-b",
              plan_id: "personal",
              price_id: "price_personal",
            },
            line_items: {
              data: [{ price: { id: "price_personal" } }],
            },
          },
        },
      }),
    );
    expect(extractCheckoutSessionSync(event!)).toBeNull();
  });

  it("accepts a server-bound checkout session payload", () => {
    const event = parseStripeEventPayload(
      JSON.stringify({
        id: "evt_2",
        type: "checkout.session.completed",
        data: {
          object: {
            status: "complete",
            payment_status: "paid",
            customer: "cus_1",
            subscription: "sub_1",
            client_reference_id: "user-a",
            metadata: {
              user_id: "user-a",
              plan_id: "pro",
              price_id: "price_pro",
            },
            line_items: {
              data: [{ price: { id: "price_pro" } }],
            },
          },
        },
      }),
    );
    expect(extractCheckoutSessionSync(event!)).toEqual({
      userId: "user-a",
      planId: "pro",
      stripeCustomerId: "cus_1",
      stripeSubscriptionId: "sub_1",
      stripePriceId: "price_pro",
    });
  });
});

describe("stripe entitlement sync helpers", () => {
  const personalPlan = {
    planId: "personal",
    hostedAiEnabled: true,
    hostedSearchEnabled: false,
    allowanceAmount: 500,
    hardLimitEnabled: true,
  };

  it("extracts checkout.session.completed metadata", () => {
    const event = parseStripeEventPayload(
      JSON.stringify({
        id: "evt_checkout",
        type: "checkout.session.completed",
        data: {
          object: {
            status: "complete",
            payment_status: "paid",
            customer: "cus_123",
            subscription: "sub_456",
            client_reference_id: "user-uuid",
            metadata: {
              user_id: "user-uuid",
              plan_id: "personal",
              price_id: "price_personal",
            },
            line_items: {
              data: [{ price: { id: "price_personal" } }],
            },
          },
        },
      }),
    );
    expect(extractCheckoutSessionSync(event!)).toEqual({
      userId: "user-uuid",
      planId: "personal",
      stripeCustomerId: "cus_123",
      stripeSubscriptionId: "sub_456",
      stripePriceId: "price_personal",
    });
  });

  it("rejects checkout without server metadata", () => {
    const event = parseStripeEventPayload(
      JSON.stringify({
        id: "evt_checkout",
        type: "checkout.session.completed",
        data: { object: { customer: "cus_123" } },
      }),
    );
    expect(extractCheckoutSessionSync(event!)).toBeNull();
  });

  it("extracts subscription.updated payload", () => {
    const event = parseStripeEventPayload(
      JSON.stringify({
        id: "evt_sub",
        type: "customer.subscription.updated",
        data: {
          object: {
            id: "sub_456",
            customer: "cus_123",
            status: "active",
            current_period_end: 1_700_000_000,
            items: { data: [{ price: { id: "price_personal" } }] },
          },
        },
      }),
    );
    const sync = extractSubscriptionSync(event!);
    expect(sync?.stripeSubscriptionId).toBe("sub_456");
    expect(sync?.status).toBe("active");
    expect(sync?.priceId).toBe("price_personal");
  });

  it("downgrades canceled subscriptions to free entitlements", () => {
    expect(
      resolveEntitlementsForSubscription("canceled", personalPlan),
    ).toEqual(FREE_PLAN_ENTITLEMENTS);
    expect(
      resolveEntitlementsForSubscription("active", personalPlan),
    ).toEqual(personalPlan);
  });

  it("filters webhook event types for entitlement sync", () => {
    expect(shouldProcessStripeEventType("checkout.session.completed")).toBe(true);
    expect(shouldProcessStripeEventType("customer.subscription.deleted")).toBe(true);
    expect(shouldProcessStripeEventType("invoice.paid")).toBe(false);
  });

  it("entitlement sync payload never resets used_amount", () => {
    const payload = entitlementSyncPayload({
      planId: "pro",
      hostedAiEnabled: true,
      hostedSearchEnabled: true,
      allowanceAmount: 2000,
      hardLimitEnabled: true,
    });
    expect(payload).toEqual({
      plan_id: "pro",
      hosted_ai_enabled: true,
      hosted_search_enabled: true,
      allowance_amount: 2000,
      hard_limit_enabled: true,
    });
    expect(payload).not.toHaveProperty("used_amount");
  });
});
