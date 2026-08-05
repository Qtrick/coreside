-- Coreside hosted AI: billing scaffolding + atomic reservation RPC stubs.
-- Plans are server-authoritative; clients never write entitlements or subscriptions.

-- ── Plan catalog: Free / Personal / Pro ───────────────────────────────────────

INSERT INTO public.ai_plan_catalog (
  plan_id,
  display_name,
  hosted_ai_enabled,
  hosted_search_enabled,
  default_allowance_amount,
  allowance_unit,
  hard_limit_enabled
) VALUES
  ('personal', 'Personal', true, false, 500, 'requests', true),
  ('pro', 'Pro', true, true, 2000, 'requests', true)
ON CONFLICT (plan_id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  hosted_ai_enabled = EXCLUDED.hosted_ai_enabled,
  hosted_search_enabled = EXCLUDED.hosted_search_enabled,
  default_allowance_amount = EXCLUDED.default_allowance_amount,
  allowance_unit = EXCLUDED.allowance_unit,
  hard_limit_enabled = EXCLUDED.hard_limit_enabled;

ALTER TABLE public.ai_plan_catalog
  ADD COLUMN IF NOT EXISTS stripe_checkout_price_id text,
  ADD COLUMN IF NOT EXISTS stripe_portal_enabled boolean NOT NULL DEFAULT false;

UPDATE public.ai_plan_catalog
SET stripe_portal_enabled = true
WHERE plan_id IN ('personal', 'pro');

-- ── Stripe billing tables (service-role writes only) ─────────────────────────

CREATE TABLE IF NOT EXISTS public.billing_stripe_customers (
  user_id uuid PRIMARY KEY REFERENCES auth.users (id) ON DELETE CASCADE,
  stripe_customer_id text NOT NULL UNIQUE,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS public.billing_subscriptions (
  user_id uuid PRIMARY KEY REFERENCES auth.users (id) ON DELETE CASCADE,
  stripe_subscription_id text UNIQUE,
  stripe_customer_id text,
  plan_id text REFERENCES public.ai_plan_catalog (plan_id),
  status text NOT NULL DEFAULT 'inactive',
  current_period_end timestamptz,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS public.billing_stripe_webhook_events (
  event_id text PRIMARY KEY,
  event_type text NOT NULL,
  payload_sha256 text NOT NULL,
  processed_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS billing_subscriptions_stripe_sub_idx
  ON public.billing_subscriptions (stripe_subscription_id);

CREATE TRIGGER billing_stripe_customers_set_updated_at
  BEFORE UPDATE ON public.billing_stripe_customers
  FOR EACH ROW
  EXECUTE FUNCTION public.set_updated_at();

CREATE TRIGGER billing_subscriptions_set_updated_at
  BEFORE UPDATE ON public.billing_subscriptions
  FOR EACH ROW
  EXECUTE FUNCTION public.set_updated_at();

ALTER TABLE public.billing_stripe_customers ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.billing_subscriptions ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.billing_stripe_webhook_events ENABLE ROW LEVEL SECURITY;

CREATE POLICY billing_subscriptions_select_own
  ON public.billing_subscriptions
  FOR SELECT
  TO authenticated
  USING (auth.uid() = user_id);

-- No authenticated INSERT/UPDATE/DELETE on billing tables or webhook idempotency.

-- ── Atomic reservation RPC stubs (service_role only) ─────────────────────────

CREATE OR REPLACE FUNCTION public.reserve_hosted_ai_request(
  p_user_id uuid,
  p_request_id text,
  p_idempotency_key text,
  p_ttl_hours integer DEFAULT 24
)
RETURNS jsonb
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
DECLARE
  ent public.ai_entitlements%ROWTYPE;
  prior public.ai_request_idempotency%ROWTYPE;
  v_expires timestamptz;
  v_expired_count integer;
BEGIN
  IF p_request_id IS NULL OR btrim(p_request_id) = '' THEN
    RETURN jsonb_build_object('outcome', 'error', 'reason', 'missing_request_id');
  END IF;
  IF p_idempotency_key IS NULL OR btrim(p_idempotency_key) = '' THEN
    RETURN jsonb_build_object('outcome', 'error', 'reason', 'missing_idempotency_key');
  END IF;

  -- Serialize concurrent reserves for the same user + idempotency key.
  PERFORM pg_advisory_xact_lock(
    hashtextextended(p_user_id::text || ':' || p_idempotency_key, 0)
  );

  -- Release allowance for expired in-flight requests before a new reserve.
  WITH expired AS (
    UPDATE public.ai_request_idempotency
    SET
      status = 'failed',
      result_reference = 'expired'
    WHERE user_id = p_user_id
      AND status = 'processing'
      AND expires_at <= now()
    RETURNING 1
  )
  SELECT count(*)::integer INTO v_expired_count FROM expired;

  IF v_expired_count > 0 THEN
    UPDATE public.ai_entitlements
    SET used_amount = GREATEST(0, used_amount - v_expired_count)
    WHERE user_id = p_user_id;
  END IF;

  SELECT *
  INTO prior
  FROM public.ai_request_idempotency
  WHERE user_id = p_user_id
    AND idempotency_key = p_idempotency_key
  FOR UPDATE;

  IF FOUND AND prior.expires_at > now() THEN
    IF prior.status = 'completed' THEN
      RETURN jsonb_build_object(
        'outcome', 'idempotent',
        'request_id', prior.request_id,
        'status', 'completed',
        'result_reference', prior.result_reference
      );
    ELSIF prior.status = 'processing' THEN
      RETURN jsonb_build_object(
        'outcome', 'conflict',
        'status', 'processing',
        'request_id', prior.request_id
      );
    END IF;
  END IF;

  SELECT *
  INTO ent
  FROM public.ai_entitlements
  WHERE user_id = p_user_id
  FOR UPDATE;

  IF NOT FOUND THEN
    RETURN jsonb_build_object('outcome', 'denied', 'reason', 'no_entitlement');
  END IF;

  IF NOT ent.hosted_ai_enabled THEN
    RETURN jsonb_build_object('outcome', 'denied', 'reason', 'hosted_disabled');
  END IF;

  IF ent.hard_limit_enabled AND ent.used_amount >= ent.allowance_amount THEN
    RETURN jsonb_build_object('outcome', 'denied', 'reason', 'allowance_exceeded');
  END IF;

  UPDATE public.ai_entitlements
  SET used_amount = used_amount + 1
  WHERE user_id = p_user_id;

  v_expires := now() + make_interval(hours => p_ttl_hours);

  INSERT INTO public.ai_request_idempotency (
    user_id,
    idempotency_key,
    request_id,
    status,
    result_reference,
    expires_at
  ) VALUES (
    p_user_id,
    p_idempotency_key,
    p_request_id,
    'processing',
    NULL,
    v_expires
  )
  ON CONFLICT (user_id, idempotency_key) DO UPDATE SET
    request_id = EXCLUDED.request_id,
    status = 'processing',
    result_reference = NULL,
    expires_at = EXCLUDED.expires_at;

  RETURN jsonb_build_object(
    'outcome', 'reserved',
    'request_id', p_request_id,
    'used_amount', ent.used_amount + 1
  );
END;
$$;

CREATE OR REPLACE FUNCTION public.settle_hosted_ai_request(
  p_user_id uuid,
  p_request_id text,
  p_idempotency_key text,
  p_profile text DEFAULT NULL,
  p_result_reference text DEFAULT NULL,
  p_provider_usage jsonb DEFAULT NULL
)
RETURNS jsonb
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
DECLARE
  prior public.ai_request_idempotency%ROWTYPE;
BEGIN
  SELECT *
  INTO prior
  FROM public.ai_request_idempotency
  WHERE user_id = p_user_id
    AND idempotency_key = p_idempotency_key
  FOR UPDATE;

  IF NOT FOUND THEN
    RETURN jsonb_build_object('outcome', 'error', 'reason', 'unknown_idempotency_key');
  END IF;

  IF prior.status = 'completed' THEN
    RETURN jsonb_build_object('outcome', 'idempotent', 'request_id', prior.request_id);
  END IF;

  IF prior.status <> 'processing' OR prior.request_id <> p_request_id THEN
    RETURN jsonb_build_object('outcome', 'error', 'reason', 'invalid_state');
  END IF;

  UPDATE public.ai_request_idempotency
  SET
    status = 'completed',
    result_reference = left(coalesce(p_result_reference, ''), 16384)
  WHERE user_id = p_user_id
    AND idempotency_key = p_idempotency_key;

  INSERT INTO public.ai_usage_ledger (
    user_id,
    request_id,
    request_type,
    profile,
    provider_usage_json,
    estimated_cost,
    actual_cost,
    status
  ) VALUES (
    p_user_id,
    p_request_id,
    'chat_completion',
    p_profile,
    p_provider_usage,
    NULL,
    NULL,
    'completed'
  )
  ON CONFLICT (user_id, request_id) DO NOTHING;

  RETURN jsonb_build_object('outcome', 'settled', 'request_id', p_request_id);
END;
$$;

CREATE OR REPLACE FUNCTION public.fail_hosted_ai_request(
  p_user_id uuid,
  p_request_id text,
  p_idempotency_key text,
  p_failure_reason text DEFAULT 'upstream_error'
)
RETURNS jsonb
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
DECLARE
  prior public.ai_request_idempotency%ROWTYPE;
BEGIN
  SELECT *
  INTO prior
  FROM public.ai_request_idempotency
  WHERE user_id = p_user_id
    AND idempotency_key = p_idempotency_key
  FOR UPDATE;

  IF NOT FOUND THEN
    RETURN jsonb_build_object('outcome', 'error', 'reason', 'unknown_idempotency_key');
  END IF;

  IF prior.status = 'failed' THEN
    RETURN jsonb_build_object('outcome', 'idempotent', 'request_id', prior.request_id);
  END IF;

  IF prior.status = 'completed' THEN
    RETURN jsonb_build_object('outcome', 'error', 'reason', 'already_completed');
  END IF;

  IF prior.status <> 'processing' THEN
    RETURN jsonb_build_object('outcome', 'error', 'reason', 'invalid_state');
  END IF;

  IF prior.request_id <> p_request_id THEN
    RETURN jsonb_build_object('outcome', 'error', 'reason', 'request_id_mismatch');
  END IF;

  UPDATE public.ai_entitlements
  SET used_amount = GREATEST(0, used_amount - 1)
  WHERE user_id = p_user_id;

  UPDATE public.ai_request_idempotency
  SET
    status = 'failed',
    result_reference = left(coalesce(p_failure_reason, 'upstream_error'), 256)
  WHERE user_id = p_user_id
    AND idempotency_key = p_idempotency_key;

  RETURN jsonb_build_object('outcome', 'released', 'request_id', p_request_id);
END;
$$;

REVOKE ALL ON FUNCTION public.reserve_hosted_ai_request(uuid, text, text, integer) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.settle_hosted_ai_request(uuid, text, text, text, text, jsonb) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.fail_hosted_ai_request(uuid, text, text, text) FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION public.reserve_hosted_ai_request(uuid, text, text, integer) TO service_role;
GRANT EXECUTE ON FUNCTION public.settle_hosted_ai_request(uuid, text, text, text, text, jsonb) TO service_role;
GRANT EXECUTE ON FUNCTION public.fail_hosted_ai_request(uuid, text, text, text) TO service_role;
