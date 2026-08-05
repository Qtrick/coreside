-- Hosted AI RLS expectations for Coreside.
-- Run after migration with a local Supabase instance and pgTAP optional.

-- pgTAP (optional):
-- SELECT plan(6);
-- SELECT has_table('public', 'profiles');
-- SELECT has_table('public', 'ai_entitlements');
-- SELECT has_table('public', 'ai_usage_ledger');
-- SELECT policies_are('public', 'profiles', ARRAY['profiles_select_own', 'profiles_update_own']);
-- SELECT policies_are('public', 'ai_entitlements', ARRAY['ai_entitlements_select_own']);
-- SELECT * FROM finish();

DO $$
BEGIN
  ASSERT (
    SELECT relrowsecurity
    FROM pg_class
    WHERE oid = 'public.profiles'::regclass
  ), 'profiles must have RLS enabled';

  ASSERT (
    SELECT relrowsecurity
    FROM pg_class
    WHERE oid = 'public.ai_entitlements'::regclass
  ), 'ai_entitlements must have RLS enabled';

  ASSERT (
    SELECT relrowsecurity
    FROM pg_class
    WHERE oid = 'public.ai_usage_ledger'::regclass
  ), 'ai_usage_ledger must have RLS enabled';

  ASSERT (
    SELECT relrowsecurity
    FROM pg_class
    WHERE oid = 'public.ai_request_idempotency'::regclass
  ), 'ai_request_idempotency must have RLS enabled';

  ASSERT (
    SELECT relrowsecurity
    FROM pg_class
    WHERE oid = 'public.hosted_search_usage'::regclass
  ), 'hosted_search_usage must have RLS enabled';

  ASSERT (
    SELECT relrowsecurity
    FROM pg_class
    WHERE oid = 'public.hosted_search_cache'::regclass
  ), 'hosted_search_cache must have RLS enabled';

  ASSERT NOT EXISTS (
    SELECT 1
    FROM pg_policies
    WHERE schemaname = 'public'
      AND tablename = 'ai_entitlements'
      AND cmd IN ('INSERT', 'UPDATE', 'DELETE')
      AND roles::text LIKE '%authenticated%'
  ), 'authenticated users must not mutate ai_entitlements';

  ASSERT NOT EXISTS (
    SELECT 1
    FROM pg_policies
    WHERE schemaname = 'public'
      AND tablename = 'ai_usage_ledger'
      AND cmd = 'INSERT'
      AND roles::text LIKE '%authenticated%'
  ), 'authenticated users must not insert ai_usage_ledger rows';

  ASSERT NOT EXISTS (
    SELECT 1
    FROM pg_policies
    WHERE schemaname = 'public'
      AND tablename = 'hosted_search_usage'
      AND cmd = 'INSERT'
      AND roles::text LIKE '%authenticated%'
  ), 'authenticated users must not insert hosted_search_usage rows';

  ASSERT NOT EXISTS (
    SELECT 1
    FROM pg_policies
    WHERE schemaname = 'public'
      AND tablename = 'ai_plan_catalog'
      AND cmd IN ('INSERT', 'UPDATE', 'DELETE')
      AND roles::text LIKE '%authenticated%'
  ), 'authenticated users must not mutate ai_plan_catalog';

  ASSERT (
    SELECT relrowsecurity
    FROM pg_class
    WHERE oid = 'public.billing_stripe_customers'::regclass
  ), 'billing_stripe_customers must have RLS enabled';

  ASSERT (
    SELECT relrowsecurity
    FROM pg_class
    WHERE oid = 'public.billing_subscriptions'::regclass
  ), 'billing_subscriptions must have RLS enabled';

  ASSERT NOT EXISTS (
    SELECT 1
    FROM pg_policies
    WHERE schemaname = 'public'
      AND tablename = 'billing_stripe_customers'
      AND cmd IN ('INSERT', 'UPDATE', 'DELETE')
      AND roles::text LIKE '%authenticated%'
  ), 'authenticated users must not mutate billing_stripe_customers';

  ASSERT NOT EXISTS (
    SELECT 1
    FROM pg_policies
    WHERE schemaname = 'public'
      AND tablename = 'billing_subscriptions'
      AND cmd IN ('INSERT', 'UPDATE', 'DELETE')
      AND roles::text LIKE '%authenticated%'
  ), 'authenticated users must not mutate billing_subscriptions';

  ASSERT NOT EXISTS (
    SELECT 1
    FROM pg_policies
    WHERE schemaname = 'public'
      AND tablename = 'billing_stripe_webhook_events'
      AND roles::text LIKE '%authenticated%'
  ), 'billing_stripe_webhook_events must not be readable by authenticated users';

  ASSERT NOT EXISTS (
    SELECT 1
    FROM information_schema.routine_privileges
    WHERE routine_schema = 'public'
      AND routine_name IN (
        'reserve_hosted_ai_request',
        'settle_hosted_ai_request',
        'fail_hosted_ai_request',
        'reserve_hosted_search_request',
        'settle_hosted_search_request',
        'fail_hosted_search_request'
      )
      AND grantee IN ('PUBLIC', 'anon', 'authenticated')
  ), 'hosted AI/search billing RPCs must be service_role only';

  ASSERT (
    SELECT relrowsecurity
    FROM pg_class
    WHERE oid = 'public.hosted_search_idempotency'::regclass
  ), 'hosted_search_idempotency must have RLS enabled';

  ASSERT EXISTS (
    SELECT 1
    FROM information_schema.columns
    WHERE table_schema = 'public'
      AND table_name = 'billing_stripe_webhook_events'
      AND column_name = 'status'
  ), 'billing_stripe_webhook_events must track processing status';

  RAISE NOTICE 'hosted_ai_rls expectations passed';
END;
$$;

-- Manual verification checklist (requires test users + JWT):
-- 1. User A can SELECT/UPDATE own profile only.
-- 2. User A can SELECT own entitlements but cannot UPDATE allowance_amount.
-- 3. User A can SELECT own usage rows but cannot INSERT into usage tables.
-- 4. User A cannot read User B profile, entitlements, or usage.
-- 5. hosted_search_cache has no authenticated policies (edge/service only).
