-- Migration 035: Bind apply idempotency outcomes to a request body hash.
--
-- Same idempotency key with a different operation payload must conflict
-- (Vendo-style), not silently replay the prior commit result.

ALTER TABLE apply_idempotency_outcomes
  ADD COLUMN request_hash TEXT NOT NULL DEFAULT '';

-- Pre-035 rows cannot be rebound to a verified body hash. Keeping them with
-- an empty hash would let any payload reuse the key and silently replay the
-- prior commit. Drop them so retries re-apply under the hash-bound contract.
DELETE FROM apply_idempotency_outcomes WHERE request_hash = '';
