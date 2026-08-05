-- RC3.9: monetary hosted AI credit ledger (micro-USD) + plan interval price mapping.
-- Credits are included non-transferable benefits, not cash. Refresh monthly for annual too.
-- Do not use floating-point money.
--
-- IMPORTANT: reserve_hosted_ai_request / settle_hosted_ai_request still debit
-- allowance by +1 request. Do NOT flip default_allowance_amount / allowance_unit
-- to micro_usd until those RPCs settle micro-USD against ai_credit_ledger.
-- monthly_credit_micro_usd is the forward-looking catalog target only.

-- ── Catalog: interval price IDs + micro-USD credit targets (no RPC rewrite) ───

ALTER TABLE public.ai_plan_catalog
  ADD COLUMN IF NOT EXISTS stripe_price_monthly_id text,
  ADD COLUMN IF NOT EXISTS stripe_price_annual_id text,
  ADD COLUMN IF NOT EXISTS monthly_credit_micro_usd bigint NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS credit_currency text NOT NULL DEFAULT 'USD';

UPDATE public.ai_plan_catalog
SET
  monthly_credit_micro_usd = 20000000,
  credit_currency = 'USD',
  hosted_ai_enabled = true,
  hosted_search_enabled = false,
  hard_limit_enabled = true
WHERE plan_id = 'personal';

UPDATE public.ai_plan_catalog
SET
  monthly_credit_micro_usd = 50000000,
  credit_currency = 'USD',
  hosted_ai_enabled = true,
  hosted_search_enabled = true,
  hard_limit_enabled = true
WHERE plan_id = 'pro';

-- Free/beta remain request-bounded via existing default_allowance_amount.
UPDATE public.ai_plan_catalog
SET
  monthly_credit_micro_usd = COALESCE(monthly_credit_micro_usd, 0),
  credit_currency = COALESCE(NULLIF(credit_currency, ''), 'USD')
WHERE plan_id IN ('free', 'beta');

-- ── Immutable monetary ledger ────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS public.ai_credit_ledger (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id uuid NOT NULL REFERENCES auth.users (id) ON DELETE CASCADE,
  subscription_id text,
  benefit_period_start timestamptz,
  benefit_period_end timestamptz,
  plan_id text,
  entry_type text NOT NULL
    CHECK (entry_type IN (
      'benefit_grant',
      'reservation',
      'settlement',
      'reservation_release',
      'expiration',
      'refund',
      'support_adjustment',
      'reconciliation',
      'provider_correction'
    )),
  request_id text,
  turn_id text,
  attempt_id text,
  route text,
  provider text,
  model text,
  usage_source text,
  pricing_version text,
  amount_micro_usd bigint NOT NULL,
  reserved_micro_usd bigint,
  settled_micro_usd bigint,
  currency text NOT NULL DEFAULT 'USD',
  stripe_invoice_id text,
  stripe_subscription_id text,
  idempotency_key text NOT NULL,
  metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (user_id, idempotency_key)
);

CREATE INDEX IF NOT EXISTS ai_credit_ledger_user_created_idx
  ON public.ai_credit_ledger (user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS ai_credit_ledger_request_idx
  ON public.ai_credit_ledger (request_id)
  WHERE request_id IS NOT NULL;

ALTER TABLE public.ai_credit_ledger ENABLE ROW LEVEL SECURITY;

CREATE POLICY ai_credit_ledger_select_own
  ON public.ai_credit_ledger
  FOR SELECT
  TO authenticated
  USING (auth.uid() = user_id);

-- No authenticated INSERT/UPDATE/DELETE — service role only.

-- ── Webhook processing leases (idempotent claim) ─────────────────────────────

ALTER TABLE public.billing_stripe_webhook_events
  ADD COLUMN IF NOT EXISTS status text NOT NULL DEFAULT 'received',
  ADD COLUMN IF NOT EXISTS lease_token text,
  ADD COLUMN IF NOT EXISTS lease_expires_at timestamptz,
  ADD COLUMN IF NOT EXISTS attempt_count integer NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS last_error text,
  ADD COLUMN IF NOT EXISTS stripe_created_at timestamptz;

CREATE INDEX IF NOT EXISTS billing_webhook_events_status_idx
  ON public.billing_stripe_webhook_events (status, lease_expires_at);

COMMENT ON TABLE public.ai_credit_ledger IS
  'Server-authoritative micro-USD hosted AI credit ledger. BYOK/Local never debit.';
