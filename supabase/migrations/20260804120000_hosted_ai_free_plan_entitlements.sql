-- Coreside hosted AI: Free plan catalog + production-safe new-user entitlements.
-- Replaces beta bootstrap (auto 50-request grant) with disabled Free defaults.
-- Provider API keys remain server-side only (Edge Function secrets); never in this schema.

-- ── Plan catalog (entitlement scaffolding) ───────────────────────────────────

CREATE TABLE IF NOT EXISTS public.ai_plan_catalog (
  plan_id text PRIMARY KEY,
  display_name text NOT NULL,
  hosted_ai_enabled boolean NOT NULL DEFAULT false,
  hosted_search_enabled boolean NOT NULL DEFAULT false,
  default_allowance_amount numeric NOT NULL DEFAULT 0,
  allowance_unit text NOT NULL DEFAULT 'requests',
  hard_limit_enabled boolean NOT NULL DEFAULT true,
  created_at timestamptz NOT NULL DEFAULT now()
);

INSERT INTO public.ai_plan_catalog (
  plan_id,
  display_name,
  hosted_ai_enabled,
  hosted_search_enabled,
  default_allowance_amount
) VALUES
  ('free', 'Free', false, false, 0),
  ('beta', 'Beta', true, false, 50)
ON CONFLICT (plan_id) DO NOTHING;

ALTER TABLE public.ai_plan_catalog ENABLE ROW LEVEL SECURITY;

CREATE POLICY ai_plan_catalog_select_authenticated
  ON public.ai_plan_catalog
  FOR SELECT
  TO authenticated
  USING (true);

-- ── New-user bootstrap: Free plan, no auto allowance ─────────────────────────

CREATE OR REPLACE FUNCTION public.handle_new_user()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
DECLARE
  plan_row public.ai_plan_catalog%ROWTYPE;
BEGIN
  INSERT INTO public.profiles (user_id, display_name)
  VALUES (
    NEW.id,
    COALESCE(NEW.raw_user_meta_data ->> 'display_name', NEW.email)
  );

  SELECT *
  INTO plan_row
  FROM public.ai_plan_catalog
  WHERE plan_id = 'free';

  IF NOT FOUND THEN
    -- Fallback if catalog row missing (should not happen after migration).
    INSERT INTO public.ai_entitlements (
      user_id,
      plan_id,
      hosted_ai_enabled,
      hosted_search_enabled,
      allowance_amount
    ) VALUES (
      NEW.id,
      'free',
      false,
      false,
      0
    );
  ELSE
    INSERT INTO public.ai_entitlements (
      user_id,
      plan_id,
      hosted_ai_enabled,
      hosted_search_enabled,
      allowance_amount,
      allowance_unit,
      hard_limit_enabled
    ) VALUES (
      NEW.id,
      plan_row.plan_id,
      plan_row.hosted_ai_enabled,
      plan_row.hosted_search_enabled,
      plan_row.default_allowance_amount,
      plan_row.allowance_unit,
      plan_row.hard_limit_enabled
    );
  END IF;

  RETURN NEW;
END;
$$;
