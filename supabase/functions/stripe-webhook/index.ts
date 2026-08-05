import { createClient } from "https://esm.sh/@supabase/supabase-js@2.49.1";
import { readBodyTextBounded } from "../_shared/read-body.ts";
import {
  entitlementSyncPayload,
  extractCheckoutSessionSync,
  extractStripeEventCreated,
  extractStripeEventId,
  extractStripeEventType,
  extractSubscriptionSync,
  isRetryableWebhookStatus,
  isTerminalWebhookStatus,
  type PlanEntitlementDefaults,
  parseStripeEventPayload,
  resolveEntitlementsForSubscription,
  sha256Hex,
  shouldProcessStripeEventType,
  verifyStripeWebhookSignature,
} from "../billing-lib.ts";

const SERVICE = "coreside-stripe-webhook";
const VERSION = "1";
const MAX_BODY_BYTES = 256 * 1024;

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
  eventCreated: number | null,
): Promise<{ synced: boolean; reason?: string; permanent?: boolean }> {
  const event = parseStripeEventPayload(payload);
  if (!event) return { synced: false, reason: "invalid_event", permanent: true };
  const checkout = extractCheckoutSessionSync(event);
  if (!checkout) {
    return { synced: false, reason: "missing_checkout_metadata", permanent: true };
  }

  const planDefaults = await loadPlanDefaults(adminClient, checkout.planId);
  if (!planDefaults) {
    return { synced: false, reason: "unknown_plan", permanent: true };
  }

  if (!checkout.stripePriceId) {
    return { synced: false, reason: "missing_price_id", permanent: true };
  }
  const planFromPrice = await loadPlanByPriceId(
    adminClient,
    checkout.stripePriceId,
  );
  if (!planFromPrice || planFromPrice.planId !== checkout.planId) {
    return { synced: false, reason: "unknown_price", permanent: true };
  }

  await adminClient.from("billing_stripe_customers").upsert({
    user_id: checkout.userId,
    stripe_customer_id: checkout.stripeCustomerId,
  });

  if (checkout.stripeSubscriptionId) {
    const { data: existingSub } = await adminClient
      .from("billing_subscriptions")
      .select("last_event_created_at")
      .eq("stripe_subscription_id", checkout.stripeSubscriptionId)
      .maybeSingle();
    if (
      eventCreated !== null &&
      existingSub?.last_event_created_at &&
      new Date(existingSub.last_event_created_at).getTime() / 1000 > eventCreated
    ) {
      return { synced: true, reason: "stale_event_ignored" };
    }

    await adminClient.from("billing_subscriptions").upsert({
      user_id: checkout.userId,
      stripe_subscription_id: checkout.stripeSubscriptionId,
      stripe_customer_id: checkout.stripeCustomerId,
      plan_id: checkout.planId,
      status: "active",
      last_event_created_at:
        eventCreated !== null
          ? new Date(eventCreated * 1000).toISOString()
          : null,
    });
  }

  await applyEntitlements(adminClient, checkout.userId, planDefaults);
  return { synced: true };
}

async function syncSubscriptionEvent(
  adminClient: AdminClient,
  payload: string,
  eventCreated: number | null,
): Promise<{ synced: boolean; reason?: string; permanent?: boolean }> {
  const event = parseStripeEventPayload(payload);
  if (!event) return { synced: false, reason: "invalid_event", permanent: true };
  const subscription = extractSubscriptionSync(event);
  if (!subscription) {
    return { synced: false, reason: "invalid_subscription", permanent: true };
  }

  let userId = await resolveUserIdByCustomer(
    adminClient,
    subscription.stripeCustomerId,
  );

  if (!userId) {
    const { data: existing } = await adminClient
      .from("billing_subscriptions")
      .select("user_id, plan_id, last_event_created_at")
      .eq("stripe_subscription_id", subscription.stripeSubscriptionId)
      .maybeSingle();
    userId = existing?.user_id ?? null;
  }

  if (!userId) {
    return { synced: false, reason: "unknown_customer", permanent: false };
  }

  const { data: existingSub } = await adminClient
    .from("billing_subscriptions")
    .select("last_event_created_at, plan_id")
    .eq("stripe_subscription_id", subscription.stripeSubscriptionId)
    .maybeSingle();

  if (
    eventCreated !== null &&
    existingSub?.last_event_created_at &&
    new Date(existingSub.last_event_created_at).getTime() / 1000 > eventCreated
  ) {
    return { synced: true, reason: "stale_event_ignored" };
  }

  const active = new Set(["active", "trialing"]);
  let planDefaults: PlanEntitlementDefaults | null = null;

  if (subscription.priceId) {
    planDefaults = await loadPlanByPriceId(adminClient, subscription.priceId);
    if (!planDefaults && active.has(subscription.status)) {
      // Unknown price while active → fail closed (do not invent entitlements).
      return { synced: false, reason: "unknown_price", permanent: true };
    }
  } else if (active.has(subscription.status)) {
    return { synced: false, reason: "missing_price_id", permanent: true };
  }

  // Unknown price while active already failed above. Do not revive a paid plan
  // from a prior row when the current event has no mapped price.
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
    last_event_created_at:
      eventCreated !== null
        ? new Date(eventCreated * 1000).toISOString()
        : null,
  });

  await applyEntitlements(adminClient, userId, entitlements);
  return { synced: true };
}

async function dispatchEntitlementSync(
  adminClient: AdminClient,
  eventType: string,
  payload: string,
  eventCreated: number | null,
): Promise<{ synced: boolean; reason?: string; permanent?: boolean }> {
  if (eventType === "checkout.session.completed") {
    return syncCheckoutCompleted(adminClient, payload, eventCreated);
  }
  if (
    eventType === "customer.subscription.updated" ||
    eventType === "customer.subscription.deleted"
  ) {
    return syncSubscriptionEvent(adminClient, payload, eventCreated);
  }
  return { synced: false, reason: "ignored_event_type", permanent: true };
}

async function markWebhookStatus(
  adminClient: AdminClient,
  eventId: string,
  status: string,
  lastError?: string | null,
): Promise<void> {
  const patch: Record<string, unknown> = {
    status,
    last_error: lastError ?? null,
  };
  if (status === "processed") {
    patch.processed_at = new Date().toISOString();
  }
  await adminClient
    .from("billing_stripe_webhook_events")
    .update(patch)
    .eq("event_id", eventId);
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

  const bodyRead = await readBodyTextBounded(req, MAX_BODY_BYTES);
  if (!bodyRead.ok) {
    return json({ error: bodyRead.error }, bodyRead.status);
  }
  const payload = bodyRead.text;

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
  const eventCreated = extractStripeEventCreated(payload);
  if (!eventId || !eventType) {
    return json({ error: "Invalid event payload" }, 400);
  }

  const adminClient = createClient(supabaseUrl, serviceRoleKey);
  const payloadHash = await sha256Hex(payload);

  const { data: existing } = await adminClient
    .from("billing_stripe_webhook_events")
    .select("event_id, payload_sha256, status")
    .eq("event_id", eventId)
    .maybeSingle();

  if (existing) {
    if (existing.payload_sha256 !== payloadHash) {
      return json({ error: "Event payload mismatch" }, 409);
    }
    if (isTerminalWebhookStatus(existing.status)) {
      return json({
        ok: true,
        service: SERVICE,
        version: VERSION,
        eventId,
        duplicate: true,
        status: existing.status,
      });
    }
    if (existing.status === "processing") {
      return json({
        ok: true,
        service: SERVICE,
        version: VERSION,
        eventId,
        duplicate: true,
        status: "processing",
      });
    }
    if (!isRetryableWebhookStatus(existing.status)) {
      return json({ error: "Webhook event not retryable", status: existing.status }, 409);
    }
  } else {
    const { error: insertError } = await adminClient
      .from("billing_stripe_webhook_events")
      .insert({
        event_id: eventId,
        event_type: eventType,
        payload_sha256: payloadHash,
        status: "received",
        processed_at: null,
        attempt_count: 0,
      });

    if (insertError) {
      if (insertError.code === "23505") {
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
  }

  // Claim for processing — do not mark processed before entitlement sync.
  const { data: claimed, error: claimError } = await adminClient
    .from("billing_stripe_webhook_events")
    .update({ status: "processing" })
    .eq("event_id", eventId)
    .in("status", ["received", "retryable_failed"])
    .select("event_id, attempt_count")
    .maybeSingle();

  if (claimError || !claimed) {
    return json({
      ok: true,
      service: SERVICE,
      version: VERSION,
      eventId,
      duplicate: true,
      status: "processing",
    });
  }

  await adminClient
    .from("billing_stripe_webhook_events")
    .update({ attempt_count: (claimed.attempt_count ?? 0) + 1 })
    .eq("event_id", eventId);

  if (!shouldProcessStripeEventType(eventType)) {
    await markWebhookStatus(adminClient, eventId, "processed", null);
    return json({
      ok: true,
      service: SERVICE,
      version: VERSION,
      eventId,
      eventType,
      entitlementSync: { synced: false, reason: "ignored_event_type" },
    });
  }

  let entitlementSync: {
    synced: boolean;
    reason?: string;
    permanent?: boolean;
  };
  try {
    entitlementSync = await dispatchEntitlementSync(
      adminClient,
      eventType,
      payload,
      eventCreated,
    );
  } catch {
    await markWebhookStatus(
      adminClient,
      eventId,
      "retryable_failed",
      "sync_exception",
    );
    return json({ error: "Entitlement sync failed", eventId }, 500);
  }

  if (!entitlementSync.synced) {
    const permanent = entitlementSync.permanent === true;
    await markWebhookStatus(
      adminClient,
      eventId,
      permanent ? "permanent_failed" : "retryable_failed",
      entitlementSync.reason ?? "sync_failed",
    );
    if (permanent) {
      return json({
        ok: false,
        service: SERVICE,
        version: VERSION,
        eventId,
        eventType,
        entitlementSync,
      }, 422);
    }
    return json({ error: "Entitlement sync failed", eventId, entitlementSync }, 500);
  }

  await markWebhookStatus(adminClient, eventId, "processed", null);

  return json({
    ok: true,
    service: SERVICE,
    version: VERSION,
    eventId,
    eventType,
    entitlementSync,
  });
});
