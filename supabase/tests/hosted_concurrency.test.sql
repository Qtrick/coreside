-- Concurrency / rate-limit RPC shape checks for hosted AI + search.
-- Full reserve race exercises need a live migrated DB with test users; this
-- file asserts the DB-authoritative surfaces exist (P0.6).

DO $$
BEGIN
  ASSERT EXISTS (
    SELECT 1
    FROM pg_proc p
    JOIN pg_namespace n ON n.oid = p.pronamespace
    WHERE n.nspname = 'public'
      AND p.proname = 'reserve_hosted_ai_request'
      AND p.pronargs = 6
  ), 'reserve_hosted_ai_request must accept rate + concurrency params';

  ASSERT EXISTS (
    SELECT 1
    FROM pg_proc p
    JOIN pg_namespace n ON n.oid = p.pronamespace
    WHERE n.nspname = 'public'
      AND p.proname = 'reserve_hosted_search_request'
      AND p.pronargs = 6
  ), 'reserve_hosted_search_request must accept rate + concurrency params';

  ASSERT NOT EXISTS (
    SELECT 1
    FROM pg_proc p
    JOIN pg_namespace n ON n.oid = p.pronamespace
    WHERE n.nspname = 'public'
      AND p.proname = 'reserve_hosted_ai_request'
      AND p.pronargs = 4
  ), 'legacy 4-arg reserve_hosted_ai_request must be removed';

  ASSERT EXISTS (
    SELECT 1
    FROM information_schema.columns
    WHERE table_schema = 'public'
      AND table_name = 'ai_request_idempotency'
      AND column_name = 'created_at'
  ), 'ai_request_idempotency.created_at required for rate windows';

  ASSERT EXISTS (
    SELECT 1
    FROM information_schema.columns
    WHERE table_schema = 'public'
      AND table_name = 'hosted_search_cache'
      AND column_name = 'user_id'
  ), 'hosted_search_cache must be tenant-scoped';

  ASSERT EXISTS (
    SELECT 1
    FROM information_schema.columns
    WHERE table_schema = 'public'
      AND table_name = 'billing_stripe_webhook_events'
      AND column_name = 'status'
  ), 'webhook events must expose status for state machine';

  ASSERT EXISTS (
    SELECT 1
    FROM information_schema.columns
    WHERE table_schema = 'public'
      AND table_name = 'billing_subscriptions'
      AND column_name = 'last_event_created_at'
  ), 'subscriptions must track last_event_created_at for out-of-order protection';

  RAISE NOTICE 'hosted_concurrency expectations passed';
END;
$$;
