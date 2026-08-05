-- P0.6/P0.8/P0.9: atomic rate/concurrency, hosted search RPCs, webhook state machine.
-- Extends reservation RPCs; does not rewrite unrelated foundation tables.

-- ── Idempotency created_at for rate windows ──────────────────────────────────

ALTER TABLE public.ai_request_idempotency
  ADD COLUMN IF NOT EXISTS created_at timestamptz NOT NULL DEFAULT now();

CREATE INDEX IF NOT EXISTS ai_request_idempotency_user_created_idx
  ON public.ai_request_idempotency (user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS ai_request_idempotency_user_status_idx
  ON public.ai_request_idempotency (user_id, status);

-- ── Hosted search: tenant-scoped cache + idempotency ─────────────────────────

ALTER TABLE public.hosted_search_cache
  ADD COLUMN IF NOT EXISTS user_id uuid REFERENCES auth.users (id) ON DELETE CASCADE;

-- Drop global fingerprint PK so cache cannot serve cross-tenant hits.
DO $$
BEGIN
  IF EXISTS (
    SELECT 1 FROM pg_constraint
    WHERE conname = 'hosted_search_cache_pkey'
      AND conrelid = 'public.hosted_search_cache'::regclass
  ) THEN
    ALTER TABLE public.hosted_search_cache DROP CONSTRAINT hosted_search_cache_pkey;
  END IF;
END;
$$;

-- Remove unscoped rows (no tenant) before enforcing composite identity.
DELETE FROM public.hosted_search_cache WHERE user_id IS NULL;

ALTER TABLE public.hosted_search_cache
  ALTER COLUMN user_id SET NOT NULL;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
    WHERE conname = 'hosted_search_cache_user_fingerprint_pkey'
      AND conrelid = 'public.hosted_search_cache'::regclass
  ) THEN
    ALTER TABLE public.hosted_search_cache
      ADD CONSTRAINT hosted_search_cache_user_fingerprint_pkey
      PRIMARY KEY (user_id, fingerprint);
  END IF;
END;
$$;

CREATE TABLE IF NOT EXISTS public.hosted_search_idempotency (
  user_id uuid NOT NULL REFERENCES auth.users (id) ON DELETE CASCADE,
  idempotency_key text NOT NULL,
  request_id text NOT NULL,
  status text NOT NULL,
  result_reference jsonb,
  expires_at timestamptz NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (user_id, idempotency_key)
);

CREATE INDEX IF NOT EXISTS hosted_search_idempotency_user_created_idx
  ON public.hosted_search_idempotency (user_id, created_at DESC);

ALTER TABLE public.hosted_search_idempotency ENABLE ROW LEVEL SECURITY;

CREATE POLICY hosted_search_idempotency_select_own
  ON public.hosted_search_idempotency
  FOR SELECT
  TO authenticated
  USING (auth.uid() = user_id);

-- ── Stripe webhook state machine + subscription event ordering ───────────────

ALTER TABLE public.billing_stripe_webhook_events
  ADD COLUMN IF NOT EXISTS status text NOT NULL DEFAULT 'received',
  ADD COLUMN IF NOT EXISTS attempt_count integer NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS last_error text,
  ADD COLUMN IF NOT EXISTS created_at timestamptz NOT NULL DEFAULT now(),
  ADD COLUMN IF NOT EXISTS updated_at timestamptz NOT NULL DEFAULT now();

-- processed_at was NOT NULL DEFAULT now() — make nullable so "received" is not "done".
ALTER TABLE public.billing_stripe_webhook_events
  ALTER COLUMN processed_at DROP NOT NULL;

ALTER TABLE public.billing_stripe_webhook_events
  ALTER COLUMN processed_at DROP DEFAULT;

UPDATE public.billing_stripe_webhook_events
SET status = 'processed'
WHERE status = 'received'
  AND processed_at IS NOT NULL;

ALTER TABLE public.billing_subscriptions
  ADD COLUMN IF NOT EXISTS last_event_created_at timestamptz;

CREATE TRIGGER billing_stripe_webhook_events_set_updated_at
  BEFORE UPDATE ON public.billing_stripe_webhook_events
  FOR EACH ROW
  EXECUTE FUNCTION public.set_updated_at();

-- ── Atomic AI reserve with rate + concurrency ────────────────────────────────

CREATE OR REPLACE FUNCTION public.reserve_hosted_ai_request(
  p_user_id uuid,
  p_request_id text,
  p_idempotency_key text,
  p_ttl_hours integer DEFAULT 24,
  p_rate_limit_per_minute integer DEFAULT 20,
  p_max_concurrent integer DEFAULT 3
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
  v_rate_count integer;
  v_concurrent integer;
BEGIN
  IF p_request_id IS NULL OR btrim(p_request_id) = '' THEN
    RETURN jsonb_build_object('outcome', 'error', 'reason', 'missing_request_id');
  END IF;
  IF p_idempotency_key IS NULL OR btrim(p_idempotency_key) = '' THEN
    RETURN jsonb_build_object('outcome', 'error', 'reason', 'missing_idempotency_key');
  END IF;

  PERFORM pg_advisory_xact_lock(
    hashtextextended(p_user_id::text || ':' || p_idempotency_key, 0)
  );
  PERFORM pg_advisory_xact_lock(
    hashtextextended('hosted-ai-rate:' || p_user_id::text, 0)
  );

  -- Expire stale in-flight rows and release allowance.
  WITH expired AS (
    UPDATE public.ai_request_idempotency
    SET
      status = 'expired',
      result_reference = 'expired'
    WHERE user_id = p_user_id
      AND status IN ('reserved', 'processing')
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
    ELSIF prior.status IN ('reserved', 'processing') THEN
      RETURN jsonb_build_object(
        'outcome', 'conflict',
        'status', prior.status,
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

  SELECT count(*)::integer INTO v_rate_count
  FROM public.ai_request_idempotency
  WHERE user_id = p_user_id
    AND created_at >= now() - interval '1 minute'
    AND status IN (
      'reserved', 'processing', 'completed', 'reconciliation_required'
    );

  IF v_rate_count >= GREATEST(1, p_rate_limit_per_minute) THEN
    RETURN jsonb_build_object('outcome', 'denied', 'reason', 'rate_limited');
  END IF;

  SELECT count(*)::integer INTO v_concurrent
  FROM public.ai_request_idempotency
  WHERE user_id = p_user_id
    AND status IN ('reserved', 'processing')
    AND expires_at > now();

  IF v_concurrent >= GREATEST(1, p_max_concurrent) THEN
    RETURN jsonb_build_object('outcome', 'denied', 'reason', 'concurrency_limited');
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
    expires_at,
    created_at
  ) VALUES (
    p_user_id,
    p_idempotency_key,
    p_request_id,
    'reserved',
    NULL,
    v_expires,
    now()
  )
  ON CONFLICT (user_id, idempotency_key) DO UPDATE SET
    request_id = EXCLUDED.request_id,
    status = 'reserved',
    result_reference = NULL,
    expires_at = EXCLUDED.expires_at,
    created_at = now();

  -- Move to processing immediately so concurrent caps see in-flight work.
  UPDATE public.ai_request_idempotency
  SET status = 'processing'
  WHERE user_id = p_user_id
    AND idempotency_key = p_idempotency_key;

  RETURN jsonb_build_object(
    'outcome', 'reserved',
    'request_id', p_request_id,
    'status', 'processing',
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

  IF prior.status NOT IN ('reserved', 'processing') OR prior.request_id <> p_request_id THEN
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

  IF prior.status NOT IN ('reserved', 'processing', 'reconciliation_required') THEN
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

-- ── Hosted search reserve / settle / fail ────────────────────────────────────

CREATE OR REPLACE FUNCTION public.reserve_hosted_search_request(
  p_user_id uuid,
  p_request_id text,
  p_idempotency_key text,
  p_ttl_hours integer DEFAULT 24,
  p_rate_limit_per_minute integer DEFAULT 20,
  p_max_concurrent integer DEFAULT 3
)
RETURNS jsonb
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
DECLARE
  ent public.ai_entitlements%ROWTYPE;
  prior public.hosted_search_idempotency%ROWTYPE;
  v_expires timestamptz;
  v_expired_count integer;
  v_rate_count integer;
  v_concurrent integer;
BEGIN
  IF p_request_id IS NULL OR btrim(p_request_id) = '' THEN
    RETURN jsonb_build_object('outcome', 'error', 'reason', 'missing_request_id');
  END IF;
  IF p_idempotency_key IS NULL OR btrim(p_idempotency_key) = '' THEN
    RETURN jsonb_build_object('outcome', 'error', 'reason', 'missing_idempotency_key');
  END IF;

  PERFORM pg_advisory_xact_lock(
    hashtextextended(p_user_id::text || ':search:' || p_idempotency_key, 0)
  );
  PERFORM pg_advisory_xact_lock(
    hashtextextended('hosted-search-rate:' || p_user_id::text, 0)
  );

  WITH expired AS (
    UPDATE public.hosted_search_idempotency
    SET status = 'expired'
    WHERE user_id = p_user_id
      AND status IN ('reserved', 'processing')
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
  FROM public.hosted_search_idempotency
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
    ELSIF prior.status IN ('reserved', 'processing') THEN
      RETURN jsonb_build_object(
        'outcome', 'conflict',
        'status', prior.status,
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

  IF NOT ent.hosted_search_enabled THEN
    RETURN jsonb_build_object('outcome', 'denied', 'reason', 'hosted_disabled');
  END IF;

  IF ent.hard_limit_enabled AND ent.used_amount >= ent.allowance_amount THEN
    RETURN jsonb_build_object('outcome', 'denied', 'reason', 'allowance_exceeded');
  END IF;

  SELECT count(*)::integer INTO v_rate_count
  FROM public.hosted_search_idempotency
  WHERE user_id = p_user_id
    AND created_at >= now() - interval '1 minute'
    AND status IN (
      'reserved', 'processing', 'completed', 'reconciliation_required'
    );

  IF v_rate_count >= GREATEST(1, p_rate_limit_per_minute) THEN
    RETURN jsonb_build_object('outcome', 'denied', 'reason', 'rate_limited');
  END IF;

  SELECT count(*)::integer INTO v_concurrent
  FROM public.hosted_search_idempotency
  WHERE user_id = p_user_id
    AND status IN ('reserved', 'processing')
    AND expires_at > now();

  IF v_concurrent >= GREATEST(1, p_max_concurrent) THEN
    RETURN jsonb_build_object('outcome', 'denied', 'reason', 'concurrency_limited');
  END IF;

  UPDATE public.ai_entitlements
  SET used_amount = used_amount + 1
  WHERE user_id = p_user_id;

  v_expires := now() + make_interval(hours => p_ttl_hours);

  INSERT INTO public.hosted_search_idempotency (
    user_id,
    idempotency_key,
    request_id,
    status,
    result_reference,
    expires_at,
    created_at
  ) VALUES (
    p_user_id,
    p_idempotency_key,
    p_request_id,
    'reserved',
    NULL,
    v_expires,
    now()
  )
  ON CONFLICT (user_id, idempotency_key) DO UPDATE SET
    request_id = EXCLUDED.request_id,
    status = 'reserved',
    result_reference = NULL,
    expires_at = EXCLUDED.expires_at,
    created_at = now();

  UPDATE public.hosted_search_idempotency
  SET status = 'processing'
  WHERE user_id = p_user_id
    AND idempotency_key = p_idempotency_key;

  RETURN jsonb_build_object(
    'outcome', 'reserved',
    'request_id', p_request_id,
    'status', 'processing'
  );
END;
$$;

CREATE OR REPLACE FUNCTION public.settle_hosted_search_request(
  p_user_id uuid,
  p_request_id text,
  p_idempotency_key text,
  p_result_reference jsonb DEFAULT NULL,
  p_provider_usage jsonb DEFAULT NULL
)
RETURNS jsonb
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
DECLARE
  prior public.hosted_search_idempotency%ROWTYPE;
BEGIN
  SELECT *
  INTO prior
  FROM public.hosted_search_idempotency
  WHERE user_id = p_user_id
    AND idempotency_key = p_idempotency_key
  FOR UPDATE;

  IF NOT FOUND THEN
    RETURN jsonb_build_object('outcome', 'error', 'reason', 'unknown_idempotency_key');
  END IF;

  IF prior.status = 'completed' THEN
    RETURN jsonb_build_object(
      'outcome', 'idempotent',
      'request_id', prior.request_id,
      'result_reference', prior.result_reference
    );
  END IF;

  IF prior.status NOT IN ('reserved', 'processing') OR prior.request_id <> p_request_id THEN
    RETURN jsonb_build_object('outcome', 'error', 'reason', 'invalid_state');
  END IF;

  UPDATE public.hosted_search_idempotency
  SET
    status = 'completed',
    result_reference = p_result_reference
  WHERE user_id = p_user_id
    AND idempotency_key = p_idempotency_key;

  INSERT INTO public.hosted_search_usage (
    user_id,
    request_id,
    request_type,
    query_fingerprint,
    provider_usage_json,
    status
  ) VALUES (
    p_user_id,
    p_request_id,
    'exa_search',
    coalesce(p_provider_usage ->> 'fingerprint', NULL),
    p_provider_usage,
    'completed'
  )
  ON CONFLICT (user_id, request_id) DO NOTHING;

  RETURN jsonb_build_object('outcome', 'settled', 'request_id', p_request_id);
END;
$$;

CREATE OR REPLACE FUNCTION public.fail_hosted_search_request(
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
  prior public.hosted_search_idempotency%ROWTYPE;
BEGIN
  SELECT *
  INTO prior
  FROM public.hosted_search_idempotency
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

  IF prior.status NOT IN ('reserved', 'processing', 'reconciliation_required') THEN
    RETURN jsonb_build_object('outcome', 'error', 'reason', 'invalid_state');
  END IF;

  IF prior.request_id <> p_request_id THEN
    RETURN jsonb_build_object('outcome', 'error', 'reason', 'request_id_mismatch');
  END IF;

  UPDATE public.ai_entitlements
  SET used_amount = GREATEST(0, used_amount - 1)
  WHERE user_id = p_user_id;

  UPDATE public.hosted_search_idempotency
  SET
    status = 'failed',
    result_reference = jsonb_build_object('error', left(coalesce(p_failure_reason, 'upstream_error'), 256))
  WHERE user_id = p_user_id
    AND idempotency_key = p_idempotency_key;

  RETURN jsonb_build_object('outcome', 'released', 'request_id', p_request_id);
END;
$$;

REVOKE ALL ON FUNCTION public.reserve_hosted_ai_request(uuid, text, text, integer, integer, integer) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.settle_hosted_ai_request(uuid, text, text, text, text, jsonb) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.fail_hosted_ai_request(uuid, text, text, text) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.reserve_hosted_search_request(uuid, text, text, integer, integer, integer) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.settle_hosted_search_request(uuid, text, text, jsonb, jsonb) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.fail_hosted_search_request(uuid, text, text, text) FROM PUBLIC, anon, authenticated;

-- Drop prior 4-arg reserve signature if present so only the hardened form remains.
DROP FUNCTION IF EXISTS public.reserve_hosted_ai_request(uuid, text, text, integer);

GRANT EXECUTE ON FUNCTION public.reserve_hosted_ai_request(uuid, text, text, integer, integer, integer) TO service_role;
GRANT EXECUTE ON FUNCTION public.settle_hosted_ai_request(uuid, text, text, text, text, jsonb) TO service_role;
GRANT EXECUTE ON FUNCTION public.fail_hosted_ai_request(uuid, text, text, text) TO service_role;
GRANT EXECUTE ON FUNCTION public.reserve_hosted_search_request(uuid, text, text, integer, integer, integer) TO service_role;
GRANT EXECUTE ON FUNCTION public.settle_hosted_search_request(uuid, text, text, jsonb, jsonb) TO service_role;
GRANT EXECUTE ON FUNCTION public.fail_hosted_search_request(uuid, text, text, text) TO service_role;
