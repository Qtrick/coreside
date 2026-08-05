import { createClient } from "https://esm.sh/@supabase/supabase-js@2.49.1";
import {
  entitlementSyncPayload,
  extractCheckoutSessionSync,
  extractStripeEventId,
  extractStripeEventType,
  extractSubscriptionSync,
  FREE_PLAN_ENTITLEMENTS,
  type PlanEntitlementDefaults,
  parseStripeEventPayload,
  resolveEntitlementsForSubscription,
  sha256Hex,
  shouldProcessStripeEventType,
  verifyStripeWebhookSignature,
} from "../billing-lib.ts";

const SERVICE = "coreside-stripe-webhook";
const VERSION = "1";

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

type AdminClient = ReturnType<typeof createClient>;

async function loadPlanDefaults(
  adminClient: AdminClient,
  planId: string,
): Promise<PlanEntitlementDefaults | null> {
  const { data, error } = await adminClient
    .from("ai_plan_catalog")
    .select(
      "plan_id, hosted_ai_enabled, hosted_search_enabled, default_allowance_amount, hard_limit_enabled",
    )
    .eq("plan_id", planId)
    .maybeSingle();
  if (error || !data) return null;
  return {
    planId: data.plan_id,
    hostedAiEnabled: Boolean(data.hosted_ai_enabled),
    hostedSearchEnabled: Boolean(data.hosted_search_enabled),
    allowanceAmount: Number(data.default_allowance_amount),
    hardLimitEnabled: Boolean(data.hard_limit_enabled),
  };
}

async function loadPlanByPriceId(
  adminClient: AdminClient,
  priceId: string,
): Promise<PlanEntitlementDefaults | null> {
  const { data, error } = await adminClient
    .from("ai_plan_catalog")
    .select(
      "plan_id, hosted_ai_enabled, hosted_search_enabled, default_allowance_amount, hard_limit_enabled",
    )
    .eq("stripe_checkout_price_id", priceId)
    .maybeSingle();
  if (error || !data) return null;
  return {
    planId: data.plan_id,
    hostedAiEnabled: Boolean(data.hosted_ai_enabled),
    hostedSearchEnabled: Boolean(data.hosted_search_enabled),
    allowanceAmount: Number(data.default_allowance_amount),
    hardLimitEnabled: Boolean(data.hard_limit_enabled),
  };
}

async function applyEntitlements(
  adminClient: AdminClient,
  userId: string,
  entitlements: PlanEntitlementDefaults,
): Promise<void> {
  const fields = entitlementSyncPayload(entitlements);
  const { data: existing } = await adminClient
    .from("ai_entitlements")
    .select("user_id")
    .eq("user_id", userId)
    .maybeSingle();

  if (existing) {
    await adminClient.from("ai_entitlements").update(fields).eq("user_id", userId);
    return;
  }

  await adminClient.from("ai_entitlements").insert({
    user_id: userId,
    ...fields,
  });
}

async function resolveUserIdByCustomer(
  adminClient: AdminClient,
  stripeCustomerId: string,
): Promise<string | null> {
  const { data } = await adminClient
    .from("billing_stripe_customers")
    .select("user_id")
    .eq("stripe_customer_id", stripeCustomerId)
    .maybeSingle();
  return data?.user_id ?? null;
}

async function syncCheckoutCompleted(
  adminClient: AdminClient,
  payload: string,
): Promise<{ synced: boolean; reason?: string }> {
  const event = parseStripeEventPayload(payload);
  if (!event) return { synced: false, reason: "invalid_event" };
  const checkout = extractCheckoutSessionSync(event);
  if (!checkout) return { synced: false, reason: "missing_checkout_metadata" };

  const planDefaults = await loadPlanDefaults(adminClient, checkout.planId);
  if (!planDefaults) return { synced: false, reason: "unknown_plan" };

  if (!checkout.stripePriceId) {
    return { synced: false, reason: "missing_price_id" };
  }
  const planFromPrice = await loadPlanByPriceId(
    adminClient,
    checkout.stripePriceId,
  );
  if (!planFromPrice || planFromPrice.planId !== checkout.planId) {
    return { synced: false, reason: "price_plan_mismatch" };
  }

  await adminClient.from("billing_stripe_customers").upsert({
    user_id: checkout.userId,
    stripe_customer_id: checkout.stripeCustomerId,
  });

  if (checkout.stripeSubscriptionId) {
    await adminClient.from("billing_subscriptions").upsert({
      user_id: checkout.userId,
      stripe_subscription_id: checkout.stripeSubscriptionId,
      stripe_customer_id: checkout.stripeCustomerId,
      plan_id: checkout.planId,
      status: "active",
    });
  }

  await applyEntitlements(adminClient, checkout.userId, planDefaults);
  return { synced: true };
}

async function syncSubscriptionEvent(
  adminClient: AdminClient,
  payload: string,
): Promise<{ synced: boolean; reason?: string }> {
  const event = parseStripeEventPayload(payload);
  if (!event) return { synced: false, reason: "invalid_event" };
  const subscription = extractSubscriptionSync(event);
  if (!subscription) return { synced: false, reason: "invalid_subscription" };

  let userId = await resolveUserIdByCustomer(
    adminClient,
    subscription.stripeCustomerId,
  );

  if (!userId) {
    const { data: existing } = await adminClient
      .from("billing_subscriptions")
      .select("user_id, plan_id")
      .eq("stripe_subscription_id", subscription.stripeSubscriptionId)
      .maybeSingle();
    userId = existing?.user_id ?? null;
  }

  if (!userId) return { synced: false, reason: "unknown_customer" };

  let planDefaults: PlanEntitlementDefaults | null = null;
  if (subscription.priceId) {
    planDefaults = await loadPlanByPriceId(adminClient, subscription.priceId);
  }
  if (!planDefaults) {
    const { data: subRow } = await adminClient
      .from("billing_subscriptions")
      .select("plan_id")
      .eq("stripe_subscription_id", subscription.stripeSubscriptionId)
      .maybeSingle();
    if (subRow?.plan_id) {
      planDefaults = await loadPlanDefaults(adminClient, subRow.plan_id);
    }
  }

  const entitlements = resolveEntitlementsForSubscription(
    subscription.status,
    planDefaults,
  );

  await adminClient.from("billing_subscriptions").upsert({
    user_id: userId,
    stripe_subscription_id: subscription.stripeSubscriptionId,
    stripe_customer_id: subscription.stripeCustomerId,
    plan_id: entitlements.planId,
    status: subscription.status,
    current_period_end: subscription.currentPeriodEnd,
  });

  await applyEntitlements(adminClient, userId, entitlements);
  return { synced: true };
}

async function dispatchEntitlementSync(
  adminClient: AdminClient,
  eventType: string,
  payload: string,
): Promise<{ synced: boolean; reason?: string }> {
  if (eventType === "checkout.session.completed") {
    return syncCheckoutCompleted(adminClient, payload);
  }
  if (
    eventType === "customer.subscription.updated" ||
    eventType === "customer.subscription.deleted"
  ) {
    return syncSubscriptionEvent(adminClient, payload);
  }
  return { synced: false, reason: "ignored_event_type" };
}

Deno.serve(async (req) => {
  if (req.method !== "POST") {
    return json({ error: "Method not allowed" }, 405);
  }

  const webhookSecret = Deno.env.get("STRIPE_WEBHOOK_SECRET")?.trim();
  const supabaseUrl = Deno.env.get("SUPABASE_URL");
  const serviceRoleKey = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY");

  if (!webhookSecret || !supabaseUrl || !serviceRoleKey) {
    return json({ error: "Webhook is not configured", stub: true }, 503);
  }

  const payload = await req.text();
  const signatureHeader = req.headers.get("Stripe-Signature");
  const verified = await verifyStripeWebhookSignature(
    payload,
    signatureHeader,
    webhookSecret,
  );
  if (!verified.ok) {
    return json({ error: "Invalid signature", reason: verified.reason }, 400);
  }

  const eventId = extractStripeEventId(payload);
  const eventType = extractStripeEventType(payload);
  if (!eventId || !eventType) {
    return json({ error: "Invalid event payload" }, 400);
  }

  const adminClient = createClient(supabaseUrl, serviceRoleKey);
  const payloadHash = await sha256Hex(payload);

  const { error: insertError } = await adminClient
    .from("billing_stripe_webhook_events")
    .insert({
      event_id: eventId,
      event_type: eventType,
      payload_sha256: payloadHash,
    });

  if (insertError) {
    if (insertError.code === "23505") {
      const { data: existing, error: lookupError } = await adminClient
        .from("billing_stripe_webhook_events")
        .select("payload_sha256")
        .eq("event_id", eventId)
        .maybeSingle();
      if (lookupError || !existing) {
        return json({ error: "Failed to verify webhook idempotency" }, 500);
      }
      if (existing.payload_sha256 !== payloadHash) {
        return json({ error: "Event payload mismatch" }, 409);
      }
      return json({
        ok: true,
        service: SERVICE,
        version: VERSION,
        eventId,
        duplicate: true,
      });
    }
    return json({ error: "Failed to record webhook event" }, 500);
  }

  if (!shouldProcessStripeEventType(eventType)) {
    return json({
      ok: true,
      service: SERVICE,
      version: VERSION,
      eventId,
      eventType,
      entitlementSync: { synced: false, reason: "ignored_event_type" },
    });
  }

  const entitlementSync = await dispatchEntitlementSync(
    adminClient,
    eventType,
    payload,
  );

  return json({
    ok: true,
    service: SERVICE,
    version: VERSION,
    eventId,
    eventType,
    entitlementSync,
  });
});
