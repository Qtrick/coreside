import { createClient } from "https://esm.sh/@supabase/supabase-js@2.49.1";
import {
  buildStripeCheckoutSessionBody,
  expectedPriceCents,
  expectedStripeRecurringInterval,
  parseCheckoutBody,
  resolveBillingReturnUrl,
  resolveCheckoutPriceIdFromEnv,
} from "../billing-lib.ts";
import { readBodyTextBounded } from "../_shared/read-body.ts";

const SERVICE = "coreside-billing-checkout";
const VERSION = "1";

function isAllowedOrigin(origin: string | null): boolean {
  if (!origin) return false;
  try {
    const parsed = new URL(origin);
    if (parsed.protocol === "tauri:") {
      return parsed.hostname === "localhost" || parsed.hostname === "";
    }
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return false;
    }
    return parsed.hostname === "localhost" || parsed.hostname === "127.0.0.1";
  } catch {
    return false;
  }
}

function corsHeaders(origin: string | null): HeadersInit {
  return {
    "Access-Control-Allow-Origin": isAllowedOrigin(origin)
      ? origin!
      : "http://localhost:1422",
    "Access-Control-Allow-Headers":
      "authorization, x-client-info, apikey, content-type",
    "Access-Control-Allow-Methods": "POST, OPTIONS",
  };
}

function json(
  body: unknown,
  status = 200,
  origin: string | null = null,
): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: {
      ...corsHeaders(origin),
      "Content-Type": "application/json",
    },
  });
}

function defaultAppBaseUrl(): string {
  return (Deno.env.get("CORESIDE_APP_BASE_URL")?.trim() || "http://localhost:1422")
    .replace(/\/$/, "");
}

Deno.serve(async (req) => {
  const origin = req.headers.get("Origin");

  if (req.method === "OPTIONS") {
    return new Response(null, { status: 204, headers: corsHeaders(origin) });
  }

  if (req.method !== "POST") {
    return json({ error: "Method not allowed" }, 405, origin);
  }

  const authHeader = req.headers.get("Authorization");
  if (!authHeader?.startsWith("Bearer ")) {
    return json({ error: "Unauthorized" }, 401, origin);
  }

  const supabaseUrl = Deno.env.get("SUPABASE_URL");
  const supabaseAnonKey = Deno.env.get("SUPABASE_ANON_KEY");
  const serviceRoleKey = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY");
  if (!supabaseUrl || !supabaseAnonKey || !serviceRoleKey) {
    return json({ error: "Billing is not configured", stub: true }, 503, origin);
  }

  const userClient = createClient(supabaseUrl, supabaseAnonKey, {
    global: { headers: { Authorization: authHeader } },
  });
  const adminClient = createClient(supabaseUrl, serviceRoleKey);

  const {
    data: { user },
    error: userError,
  } = await userClient.auth.getUser();
  if (userError || !user) {
    return json({ error: "Unauthorized" }, 401, origin);
  }

  let body: ReturnType<typeof parseCheckoutBody>;
  try {
    const bodyRead = await readBodyTextBounded(req, 64 * 1024);
    if (!bodyRead.ok) {
      return json({ error: "Request body too large" }, 413, origin);
    }
    body = parseCheckoutBody(JSON.parse(bodyRead.text));
  } catch {
    return json({ error: "Invalid request body" }, 400, origin);
  }
  if (!body) {
    return json({ error: "Invalid plan" }, 400, origin);
  }

  const stripeSecret = Deno.env.get("STRIPE_SECRET_KEY")?.trim();
  if (!stripeSecret) {
    return json(
      {
        stub: true,
        service: SERVICE,
        version: VERSION,
        planId: body.planId,
        interval: body.interval,
        checkoutUrl: null,
        message:
          "Stripe checkout scaffolding. Set STRIPE_SECRET_KEY and STRIPE_PRICE_* env for plan+interval variants.",
      },
      503,
      origin,
    );
  }

  // Server resolves Price ID from allowlisted env — never from the desktop.
  const priceId = resolveCheckoutPriceIdFromEnv(body.planId, body.interval);
  if (!priceId) {
    return json(
      {
        error: "Plan interval is not configured for checkout",
        planId: body.planId,
        interval: body.interval,
        stub: true,
      },
      503,
      origin,
    );
  }

  const expectedCents = expectedPriceCents(body.planId, body.interval);
  const expectedRecurring = expectedStripeRecurringInterval(body.interval);
  if (expectedCents == null || expectedRecurring == null) {
    return json({ error: "Unknown plan interval" }, 400, origin);
  }

  // Verify live Stripe amount + recurring interval (fail closed on drift).
  // priceId is allowlisted `price_[A-Za-z0-9]+` — safe path segment.
  const priceVerify = await fetch(`https://api.stripe.com/v1/prices/${priceId}`, {
    headers: { Authorization: `Bearer ${stripeSecret}` },
  });
  if (!priceVerify.ok) {
    return json({ error: "Could not verify checkout price" }, 503, origin);
  }
  const priceJson = await priceVerify.json() as {
    unit_amount?: number;
    active?: boolean;
    currency?: string;
    type?: string;
    recurring?: { interval?: string } | null;
  };
  if (
    !priceJson.active ||
    priceJson.currency !== "usd" ||
    priceJson.type !== "recurring" ||
    priceJson.unit_amount !== expectedCents ||
    priceJson.recurring?.interval !== expectedRecurring
  ) {
    return json(
      {
        error: "Checkout price failed amount verification",
        planId: body.planId,
        interval: body.interval,
      },
      503,
      origin,
    );
  }

  const { data: existingCustomer } = await adminClient
    .from("billing_stripe_customers")
    .select("stripe_customer_id")
    .eq("user_id", user.id)
    .maybeSingle();

  const successUrl = resolveBillingReturnUrl(
    body.successDestination,
    defaultAppBaseUrl(),
  );
  const cancelUrl = resolveBillingReturnUrl(
    body.cancelDestination,
    defaultAppBaseUrl(),
  );
  if (!successUrl || !cancelUrl) {
    return json({ error: "Invalid billing return destination" }, 400, origin);
  }

  const sessionBody = buildStripeCheckoutSessionBody({
    userId: user.id,
    planId: body.planId,
    interval: body.interval,
    priceId,
    successUrl,
    cancelUrl,
    customerId: existingCustomer?.stripe_customer_id ?? undefined,
  });

  const stripeResponse = await fetch("https://api.stripe.com/v1/checkout/sessions", {
    method: "POST",
    headers: {
      Authorization: `Bearer ${stripeSecret}`,
      "Content-Type": "application/x-www-form-urlencoded",
    },
    body: sessionBody,
  });

  if (!stripeResponse.ok) {
    return json({ error: "Checkout session creation failed" }, 502, origin);
  }

  const session = await stripeResponse.json() as { id?: string; url?: string };
  if (!session.url) {
    return json({ error: "Checkout session missing redirect URL" }, 502, origin);
  }

  return json(
    {
      service: SERVICE,
      version: VERSION,
      planId: body.planId,
      checkoutUrl: session.url,
      sessionId: session.id ?? null,
    },
    200,
    origin,
  );
});
