import { createClient } from "https://esm.sh/@supabase/supabase-js@2.49.1";
import { parsePortalBody, buildStripePortalSessionBody } from "../billing-lib.ts";

const SERVICE = "coreside-billing-portal";
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

  let body: ReturnType<typeof parsePortalBody>;
  try {
    body = parsePortalBody(await req.json().catch(() => ({})));
  } catch {
    return json({ error: "Invalid request body" }, 400, origin);
  }

  const stripeSecret = Deno.env.get("STRIPE_SECRET_KEY")?.trim();
  if (!stripeSecret) {
    return json(
      {
        stub: true,
        service: SERVICE,
        version: VERSION,
        portalUrl: null,
        message:
          "Stripe portal scaffolding. Set STRIPE_SECRET_KEY to enable customer portal sessions.",
      },
      503,
      origin,
    );
  }

  const { data: customerRow } = await adminClient
    .from("billing_stripe_customers")
    .select("stripe_customer_id")
    .eq("user_id", user.id)
    .maybeSingle();

  if (!customerRow?.stripe_customer_id) {
    return json(
      {
        error: "No billing account",
        stub: true,
        message: "Subscribe to a paid plan before opening the billing portal.",
      },
      404,
      origin,
    );
  }

  const returnUrl =
    body.returnUrl?.trim() ||
    Deno.env.get("CORESIDE_BILLING_RETURN_URL")?.trim() ||
    "http://localhost:1422/settings";

  const portalBody = buildStripePortalSessionBody({
    customerId: customerRow.stripe_customer_id,
    returnUrl,
  });

  const stripeResponse = await fetch(
    "https://api.stripe.com/v1/billing_portal/sessions",
    {
      method: "POST",
      headers: {
        Authorization: `Bearer ${stripeSecret}`,
        "Content-Type": "application/x-www-form-urlencoded",
      },
      body: portalBody,
    },
  );

  if (!stripeResponse.ok) {
    return json({ error: "Portal session creation failed" }, 502, origin);
  }

  const session = await stripeResponse.json() as { id?: string; url?: string };
  if (!session.url) {
    return json({ error: "Portal session missing redirect URL" }, 502, origin);
  }

  return json(
    {
      service: SERVICE,
      version: VERSION,
      portalUrl: session.url,
      sessionId: session.id ?? null,
      returnUrl,
    },
    200,
    origin,
  );
});
