/** Shared billing + Stripe webhook helpers (Edge Function scaffolding). */

export const BILLING_PLAN_IDS = new Set(["personal", "pro"]);
export const BILLING_INTERVALS = new Set(["monthly", "annual"]);

/** Logical in-app destinations only — never accept arbitrary client URLs. */
export const BILLING_DESTINATIONS = new Set(["settings", "billing"]);

/** Expected Stripe unit amounts (cents). Annual display ≠ monthly×12 intentionally. */
export const EXPECTED_PRICE_CENTS: Record<string, number> = {
  "personal:monthly": 1999,
  "personal:annual": 19188,
  "pro:monthly": 4999,
  "pro:annual": 49188,
};

/** Micro-USD (1 USD = 1_000_000). Included hosted AI credits per monthly benefit period. */
export const CREDIT_MICRO_USD_PER_PERIOD: Record<string, number> = {
  personal: 20_000_000,
  pro: 50_000_000,
};

export interface CheckoutBody {
  planId: string;
  interval: string;
  successDestination: string;
  cancelDestination: string;
}

export function parseCheckoutBody(raw: unknown): CheckoutBody | null {
  if (!raw || typeof raw !== "object") return null;
  const body = raw as Record<string, unknown>;
  if (typeof body.planId !== "string") return null;
  const planId = body.planId.trim().toLowerCase();
  if (!BILLING_PLAN_IDS.has(planId)) return null;

  if (typeof body.interval !== "string") return null;
  const interval = body.interval.trim().toLowerCase();
  if (!BILLING_INTERVALS.has(interval)) return null;

  // Reject legacy arbitrary URL fields — open-redirect surface.
  if (body.successUrl !== undefined || body.cancelUrl !== undefined) {
    return null;
  }
  // Desktop must never choose an arbitrary Stripe Price ID.
  if (body.priceId !== undefined || body.stripePriceId !== undefined) {
    return null;
  }

  const successDestination = normalizeBillingDestination(
    body.successDestination,
    "settings",
  );
  const cancelDestination = normalizeBillingDestination(
    body.cancelDestination,
    "settings",
  );
  if (!successDestination || !cancelDestination) return null;

  return { planId, interval, successDestination, cancelDestination };
}

export function checkoutVariantKey(planId: string, interval: string): string {
  return `${planId}:${interval}`;
}

/** Stripe Price IDs only — rejects path/query injection into `/v1/prices/{id}`. */
const SAFE_STRIPE_PRICE_ID = /^price_[A-Za-z0-9]+$/;

export function isSafeStripePriceId(value: string): boolean {
  return SAFE_STRIPE_PRICE_ID.test(value);
}

/**
 * Resolve Stripe Price ID from server env only.
 * Keys: STRIPE_PRICE_PERSONAL_MONTHLY, STRIPE_PRICE_PERSONAL_ANNUAL,
 *       STRIPE_PRICE_PRO_MONTHLY, STRIPE_PRICE_PRO_ANNUAL.
 */
export function resolveCheckoutPriceIdFromEnv(
  planId: string,
  interval: string,
  env: { get(key: string): string | undefined } = Deno.env,
): string | null {
  if (!BILLING_PLAN_IDS.has(planId) || !BILLING_INTERVALS.has(interval)) {
    return null;
  }
  const key = `STRIPE_PRICE_${planId.toUpperCase()}_${interval.toUpperCase()}`;
  const value = env.get(key)?.trim();
  if (!value || !isSafeStripePriceId(value)) return null;
  return value;
}

export function expectedPriceCents(planId: string, interval: string): number | null {
  const cents = EXPECTED_PRICE_CENTS[checkoutVariantKey(planId, interval)];
  return typeof cents === "number" ? cents : null;
}

/** Maps Coreside checkout interval → Stripe Price `recurring.interval`. */
export function expectedStripeRecurringInterval(
  interval: string,
): "month" | "year" | null {
  if (interval === "monthly") return "month";
  if (interval === "annual") return "year";
  return null;
}

/**
 * Map a Stripe Price ID back to an allowlisted plan via server env.
 * Prefer this over catalog columns so checkout + webhook share one source of truth.
 */
export function planIdForAllowlistedPriceId(
  priceId: string,
  env: { get(key: string): string | undefined } = Deno.env,
): string | null {
  if (!isSafeStripePriceId(priceId)) return null;
  for (const planId of BILLING_PLAN_IDS) {
    for (const interval of BILLING_INTERVALS) {
      if (resolveCheckoutPriceIdFromEnv(planId, interval, env) === priceId) {
        return planId;
      }
    }
  }
  return null;
}

export interface PortalBody {
  returnDestination: string;
}

export function parsePortalBody(raw: unknown): PortalBody | null {
  if (!raw || typeof raw !== "object") return { returnDestination: "settings" };
  const body = raw as Record<string, unknown>;
  if (body.returnUrl !== undefined) return null;
  const returnDestination = normalizeBillingDestination(
    body.returnDestination,
    "settings",
  );
  if (!returnDestination) return null;
  return { returnDestination };
}

export function normalizeBillingDestination(
  value: unknown,
  fallback: string,
): string | null {
  const raw =
    typeof value === "string" && value.trim()
      ? value.trim().toLowerCase()
      : fallback;
  if (!BILLING_DESTINATIONS.has(raw)) return null;
  return raw;
}

/** Server constructs Stripe return URLs from logical destinations + env base. */
export function resolveBillingReturnUrl(
  destination: string,
  appBaseUrl?: string | null,
): string | null {
  const dest = normalizeBillingDestination(destination, "");
  if (!dest) return null;
  const base = (appBaseUrl?.trim() || "http://localhost:1422").replace(/\/$/, "");
  try {
    const parsed = new URL(base);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return null;
    }
    if (
      parsed.hostname !== "localhost" &&
      parsed.hostname !== "127.0.0.1" &&
      parsed.protocol !== "https:"
    ) {
      return null;
    }
    return `${base}/${dest}`;
  } catch {
    return null;
  }
}

export type WebhookEventStatus =
  | "received"
  | "processing"
  | "processed"
  | "retryable_failed"
  | "permanent_failed";

export function isTerminalWebhookStatus(status: string): boolean {
  return status === "processed" || status === "permanent_failed";
}

export function isRetryableWebhookStatus(status: string): boolean {
  return status === "received" || status === "retryable_failed";
}

/** Stripe-Signature: t=timestamp,v1=hex,... */
export function parseStripeSignatureHeader(
  header: string | null,
): { timestamp: number; signatures: string[] } | null {
  if (!header) return null;
  let timestamp: number | null = null;
  const signatures: string[] = [];
  for (const part of header.split(",")) {
    const [key, value] = part.split("=", 2);
    if (!key || !value) continue;
    if (key.trim() === "t") {
      const parsed = Number(value.trim());
      if (Number.isFinite(parsed)) timestamp = parsed;
    } else if (key.trim() === "v1") {
      signatures.push(value.trim());
    }
  }
  if (timestamp === null || signatures.length === 0) return null;
  return { timestamp, signatures };
}

export async function computeStripeSignature(
  secret: string,
  timestamp: number,
  payload: string,
): Promise<string> {
  const key = await crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(secret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const signed = await crypto.subtle.sign(
    "HMAC",
    key,
    new TextEncoder().encode(`${timestamp}.${payload}`),
  );
  return Array.from(new Uint8Array(signed))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

function timingSafeEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i++) diff |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return diff === 0;
}

export async function verifyStripeWebhookSignature(
  payload: string,
  signatureHeader: string | null,
  webhookSecret: string,
  toleranceSec = 300,
  nowSec = Math.floor(Date.now() / 1000),
): Promise<{ ok: true } | { ok: false; reason: string }> {
  const parsed = parseStripeSignatureHeader(signatureHeader);
  if (!parsed) return { ok: false, reason: "missing_signature" };
  if (Math.abs(nowSec - parsed.timestamp) > toleranceSec) {
    return { ok: false, reason: "timestamp_out_of_tolerance" };
  }
  const expected = await computeStripeSignature(
    webhookSecret,
    parsed.timestamp,
    payload,
  );
  const matched = parsed.signatures.some((sig) => timingSafeEqual(sig, expected));
  if (!matched) return { ok: false, reason: "signature_mismatch" };
  return { ok: true };
}

export function extractStripeEventId(payload: string): string | null {
  try {
    const parsed = JSON.parse(payload) as { id?: unknown };
    return typeof parsed.id === "string" && parsed.id.startsWith("evt_")
      ? parsed.id
      : null;
  } catch {
    return null;
  }
}

export function extractStripeEventType(payload: string): string | null {
  try {
    const parsed = JSON.parse(payload) as { type?: unknown };
    return typeof parsed.type === "string" ? parsed.type : null;
  } catch {
    return null;
  }
}

/** Stripe event.created (unix seconds) for out-of-order protection. */
export function extractStripeEventCreated(payload: string): number | null {
  try {
    const parsed = JSON.parse(payload) as { created?: unknown };
    return typeof parsed.created === "number" && Number.isFinite(parsed.created)
      ? parsed.created
      : null;
  } catch {
    return null;
  }
}

export async function sha256Hex(payload: string): Promise<string> {
  const digest = await crypto.subtle.digest(
    "SHA-256",
    new TextEncoder().encode(payload),
  );
  return Array.from(new Uint8Array(digest))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

export interface StripeEvent {
  id: string;
  type: string;
  data?: { object?: Record<string, unknown> };
}

export interface PlanEntitlementDefaults {
  planId: string;
  hostedAiEnabled: boolean;
  hostedSearchEnabled: boolean;
  allowanceAmount: number;
  hardLimitEnabled: boolean;
}

export const FREE_PLAN_ENTITLEMENTS: PlanEntitlementDefaults = {
  planId: "free",
  hostedAiEnabled: false,
  hostedSearchEnabled: false,
  allowanceAmount: 0,
  hardLimitEnabled: true,
};

export function parseStripeEventPayload(payload: string): StripeEvent | null {
  try {
    const parsed = JSON.parse(payload) as StripeEvent;
    if (typeof parsed.id !== "string" || typeof parsed.type !== "string") {
      return null;
    }
    return parsed;
  } catch {
    return null;
  }
}

function readMetadataString(
  metadata: unknown,
  key: string,
): string | null {
  if (!metadata || typeof metadata !== "object") return null;
  const value = (metadata as Record<string, unknown>)[key];
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

/** Checkout session metadata must include server-set user_id + plan_id. */
export function extractCheckoutSessionSync(
  event: StripeEvent,
): {
  userId: string;
  planId: string;
  stripeCustomerId: string;
  stripeSubscriptionId: string | null;
  stripePriceId: string | null;
} | null {
  if (event.type !== "checkout.session.completed") return null;
  const session = event.data?.object;
  if (!session || typeof session !== "object") return null;

  const sessionStatus =
    typeof session.status === "string" ? session.status : "";
  const paymentStatus =
    typeof session.payment_status === "string" ? session.payment_status : "";
  if (sessionStatus !== "complete") return null;
  if (paymentStatus !== "paid" && paymentStatus !== "no_payment_required") {
    return null;
  }

  const userId = readMetadataString(session.metadata, "user_id");
  const planId = readMetadataString(session.metadata, "plan_id")?.toLowerCase() ??
    null;
  const clientReferenceId =
    typeof session.client_reference_id === "string"
      ? session.client_reference_id.trim()
      : null;
  const stripeCustomerId =
    typeof session.customer === "string" ? session.customer : null;
  const stripeSubscriptionId =
    typeof session.subscription === "string" ? session.subscription : null;

  if (
    !userId ||
    !planId ||
    !stripeCustomerId ||
    !BILLING_PLAN_IDS.has(planId) ||
    clientReferenceId !== userId
  ) {
    return null;
  }

  let stripePriceId: string | null = null;
  const lineItems = session.line_items;
  if (lineItems && typeof lineItems === "object") {
    const data = (lineItems as { data?: unknown }).data;
    if (Array.isArray(data) && data.length > 0) {
      const first = data[0];
      if (first && typeof first === "object") {
        const price = (first as { price?: unknown }).price;
        if (price && typeof price === "object") {
          const id = (price as { id?: unknown }).id;
          if (typeof id === "string") stripePriceId = id;
        } else if (typeof price === "string") {
          stripePriceId = price;
        }
      }
    }
  }
  if (!stripePriceId) {
    stripePriceId = readMetadataString(session.metadata, "price_id");
  }

  return {
    userId,
    planId,
    stripeCustomerId,
    stripeSubscriptionId,
    stripePriceId,
  };
}

export function extractSubscriptionSync(
  event: StripeEvent,
): {
  stripeSubscriptionId: string;
  stripeCustomerId: string;
  status: string;
  currentPeriodEnd: string | null;
  priceId: string | null;
} | null {
  const allowed = new Set([
    "customer.subscription.updated",
    "customer.subscription.deleted",
  ]);
  if (!allowed.has(event.type)) return null;

  const subscription = event.data?.object;
  if (!subscription || typeof subscription !== "object") return null;

  const stripeSubscriptionId =
    typeof subscription.id === "string" ? subscription.id : null;
  const stripeCustomerId =
    typeof subscription.customer === "string" ? subscription.customer : null;
  const status =
    typeof subscription.status === "string" ? subscription.status : "inactive";

  let currentPeriodEnd: string | null = null;
  if (typeof subscription.current_period_end === "number") {
    currentPeriodEnd = new Date(subscription.current_period_end * 1000).toISOString();
  }

  let priceId: string | null = null;
  const items = subscription.items;
  if (items && typeof items === "object") {
    const data = (items as { data?: unknown }).data;
    if (Array.isArray(data) && data.length > 0) {
      const first = data[0];
      if (first && typeof first === "object") {
        const price = (first as { price?: unknown }).price;
        if (price && typeof price === "object") {
          const id = (price as { id?: unknown }).id;
          if (typeof id === "string") priceId = id;
        } else if (typeof price === "string") {
          priceId = price;
        }
      }
    }
  }

  if (!stripeSubscriptionId || !stripeCustomerId) return null;
  return {
    stripeSubscriptionId,
    stripeCustomerId,
    status: event.type === "customer.subscription.deleted" ? "canceled" : status,
    currentPeriodEnd,
    priceId,
  };
}

export function resolveEntitlementsForSubscription(
  status: string,
  planDefaults: PlanEntitlementDefaults | null,
): PlanEntitlementDefaults {
  const active = new Set(["active", "trialing"]);
  if (active.has(status) && planDefaults) return planDefaults;
  return FREE_PLAN_ENTITLEMENTS;
}

export function shouldProcessStripeEventType(eventType: string): boolean {
  return (
    eventType === "checkout.session.completed" ||
    eventType === "customer.subscription.updated" ||
    eventType === "customer.subscription.deleted"
  );
}

/** Server-bound Stripe Checkout session body (subscription mode). */
export interface CheckoutSessionParams {
  userId: string;
  planId: string;
  interval?: string;
  priceId: string;
  successUrl: string;
  cancelUrl: string;
  customerId?: string;
}

export function buildStripeCheckoutSessionBody(
  params: CheckoutSessionParams,
): URLSearchParams {
  const body = new URLSearchParams();
  body.set("mode", "subscription");
  body.set("client_reference_id", params.userId);
  body.set("line_items[0][price]", params.priceId);
  body.set("line_items[0][quantity]", "1");
  body.set("metadata[user_id]", params.userId);
  body.set("metadata[plan_id]", params.planId);
  body.set("metadata[price_id]", params.priceId);
  if (params.interval) {
    body.set("metadata[interval]", params.interval);
  }
  body.set("success_url", params.successUrl);
  body.set("cancel_url", params.cancelUrl);
  if (params.customerId) {
    body.set("customer", params.customerId);
  }
  return body;
}

/** Server-bound Stripe billing portal session body. */
export interface PortalSessionParams {
  customerId: string;
  returnUrl: string;
}

export function buildStripePortalSessionBody(
  params: PortalSessionParams,
): URLSearchParams {
  const body = new URLSearchParams();
  body.set("customer", params.customerId);
  body.set("return_url", params.returnUrl);
  return body;
}

/** Entitlement fields applied by webhook sync — never includes used_amount. */
export function entitlementSyncPayload(
  entitlements: PlanEntitlementDefaults,
): Record<string, unknown> {
  return {
    plan_id: entitlements.planId,
    hosted_ai_enabled: entitlements.hostedAiEnabled,
    hosted_search_enabled: entitlements.hostedSearchEnabled,
    allowance_amount: entitlements.allowanceAmount,
    hard_limit_enabled: entitlements.hardLimitEnabled,
  };
}
