-- Coreside hosted AI foundation (profiles, entitlements, usage, search cache).
-- ponytail: beta bootstrap grants 50 requests; production should start disabled.

CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- ── Tables ───────────────────────────────────────────────────────────────────

CREATE TABLE public.profiles (
  user_id uuid PRIMARY KEY REFERENCES auth.users (id) ON DELETE CASCADE,
  display_name text,
  account_status text NOT NULL DEFAULT 'active',
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE public.ai_entitlements (
  user_id uuid PRIMARY KEY REFERENCES auth.users (id) ON DELETE CASCADE,
  plan_id text,
  hosted_ai_enabled boolean NOT NULL DEFAULT false,
  hosted_search_enabled boolean NOT NULL DEFAULT false,
  allowance_amount numeric NOT NULL DEFAULT 0,
  allowance_unit text NOT NULL DEFAULT 'requests',
  used_amount numeric NOT NULL DEFAULT 0,
  reset_at timestamptz,
  trial_ends_at timestamptz,
  hard_limit_enabled boolean NOT NULL DEFAULT true,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE public.ai_usage_ledger (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id uuid NOT NULL REFERENCES auth.users (id) ON DELETE CASCADE,
  request_id text NOT NULL,
  request_type text NOT NULL,
  profile text,
  provider_usage_json jsonb,
  actual_cost numeric,
  estimated_cost numeric,
  status text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (user_id, request_id)
);

CREATE TABLE public.ai_request_idempotency (
  user_id uuid NOT NULL REFERENCES auth.users (id) ON DELETE CASCADE,
  idempotency_key text NOT NULL,
  request_id text NOT NULL,
  status text NOT NULL,
  result_reference text,
  expires_at timestamptz NOT NULL,
  PRIMARY KEY (user_id, idempotency_key)
);

CREATE TABLE public.hosted_search_usage (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id uuid NOT NULL REFERENCES auth.users (id) ON DELETE CASCADE,
  request_id text NOT NULL,
  request_type text NOT NULL,
  query_fingerprint text,
  provider_usage_json jsonb,
  actual_cost numeric,
  estimated_cost numeric,
  status text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (user_id, request_id)
);

CREATE TABLE public.hosted_search_cache (
  fingerprint text PRIMARY KEY,
  params_json jsonb NOT NULL,
  result_json jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  expires_at timestamptz NOT NULL,
  hit_count integer NOT NULL DEFAULT 0
);

-- ── Indexes ──────────────────────────────────────────────────────────────────

CREATE INDEX ai_usage_ledger_user_created_idx
  ON public.ai_usage_ledger (user_id, created_at DESC);

CREATE INDEX hosted_search_usage_user_created_idx
  ON public.hosted_search_usage (user_id, created_at DESC);

CREATE INDEX ai_request_idempotency_expires_idx
  ON public.ai_request_idempotency (expires_at);

CREATE INDEX hosted_search_cache_expires_idx
  ON public.hosted_search_cache (expires_at);

-- ── Timestamps ───────────────────────────────────────────────────────────────

CREATE OR REPLACE FUNCTION public.set_updated_at()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
  NEW.updated_at = now();
  RETURN NEW;
END;
$$;

CREATE TRIGGER profiles_set_updated_at
  BEFORE UPDATE ON public.profiles
  FOR EACH ROW
  EXECUTE FUNCTION public.set_updated_at();

CREATE TRIGGER ai_entitlements_set_updated_at
  BEFORE UPDATE ON public.ai_entitlements
  FOR EACH ROW
  EXECUTE FUNCTION public.set_updated_at();

-- ── Auth bootstrap ───────────────────────────────────────────────────────────

CREATE OR REPLACE FUNCTION public.handle_new_user()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
BEGIN
  INSERT INTO public.profiles (user_id, display_name)
  VALUES (
    NEW.id,
    COALESCE(NEW.raw_user_meta_data ->> 'display_name', NEW.email)
  );

  -- Beta bootstrap: small allowance for launch testing.
  -- Production should start with hosted_ai_enabled = false, allowance_amount = 0.
  INSERT INTO public.ai_entitlements (
    user_id,
    plan_id,
    hosted_ai_enabled,
    hosted_search_enabled,
    allowance_amount
  ) VALUES (
    NEW.id,
    'beta',
    true,
    false,
    50
  );

  RETURN NEW;
END;
$$;

CREATE TRIGGER on_auth_user_created
  AFTER INSERT ON auth.users
  FOR EACH ROW
  EXECUTE FUNCTION public.handle_new_user();

-- ── Row level security ───────────────────────────────────────────────────────

ALTER TABLE public.profiles ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.ai_entitlements ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.ai_usage_ledger ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.ai_request_idempotency ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.hosted_search_usage ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.hosted_search_cache ENABLE ROW LEVEL SECURITY;

CREATE POLICY profiles_select_own
  ON public.profiles
  FOR SELECT
  TO authenticated
  USING (auth.uid() = user_id);

CREATE POLICY profiles_update_own
  ON public.profiles
  FOR UPDATE
  TO authenticated
  USING (auth.uid() = user_id)
  WITH CHECK (auth.uid() = user_id);

CREATE POLICY ai_entitlements_select_own
  ON public.ai_entitlements
  FOR SELECT
  TO authenticated
  USING (auth.uid() = user_id);

CREATE POLICY ai_usage_ledger_select_own
  ON public.ai_usage_ledger
  FOR SELECT
  TO authenticated
  USING (auth.uid() = user_id);

CREATE POLICY ai_request_idempotency_select_own
  ON public.ai_request_idempotency
  FOR SELECT
  TO authenticated
  USING (auth.uid() = user_id);

CREATE POLICY hosted_search_usage_select_own
  ON public.hosted_search_usage
  FOR SELECT
  TO authenticated
  USING (auth.uid() = user_id);

-- hosted_search_cache: no authenticated policies (edge/service role only).
