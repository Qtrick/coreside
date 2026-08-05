import { createClient } from "https://esm.sh/@supabase/supabase-js@2.49.1";
import { buildStripeCheckoutSessionBody, parseCheckoutBody } from "../billing-lib.ts";

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

function defaultBillingUrl(
  explicit: string | undefined,
  envKey: string,
): string {
  if (explicit) return explicit;
  const fromEnv = Deno.env.get(envKey)?.trim();
  if (fromEnv) return fromEnv;
  return "http://localhost:1422/settings";
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
    body = parseCheckoutBody(await req.json());
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
        checkoutUrl: null,
        message:
          "Stripe checkout scaffolding. Set STRIPE_SECRET_KEY and stripe_checkout_price_id on the plan catalog to enable.",
      },
      503,
      origin,
    );
  }

  const { data: planRow, error: planError } = await userClient
    .from("ai_plan_catalog")
    .select("plan_id, stripe_checkout_price_id")
    .eq("plan_id", body.planId)
    .maybeSingle();

  const priceId = planRow?.stripe_checkout_price_id?.trim();
  if (planError || !priceId) {
    return json(
      {
        error: "Plan is not available for checkout",
        planId: body.planId,
        stub: true,
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

  const sessionBody = buildStripeCheckoutSessionBody({
    userId: user.id,
    planId: body.planId,
    priceId,
    successUrl: defaultBillingUrl(body.successUrl, "CORESIDE_BILLING_SUCCESS_URL"),
    cancelUrl: defaultBillingUrl(body.cancelUrl, "CORESIDE_BILLING_CANCEL_URL"),
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
