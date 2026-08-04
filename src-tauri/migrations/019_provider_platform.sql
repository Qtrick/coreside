-- RC3.5: Provider platform metadata on connections (secrets stay in keychain).
-- Existing rows backfill safe defaults for previously supported remote BYOK providers.

ALTER TABLE provider_connections ADD COLUMN provider_descriptor_id TEXT;
ALTER TABLE provider_connections ADD COLUMN protocol_family TEXT;
ALTER TABLE provider_connections ADD COLUMN auth_mode TEXT;
ALTER TABLE provider_connections ADD COLUMN endpoint_class TEXT;
ALTER TABLE provider_connections ADD COLUMN api_version TEXT;
ALTER TABLE provider_connections ADD COLUMN region TEXT;
ALTER TABLE provider_connections ADD COLUMN deployment TEXT;
ALTER TABLE provider_connections ADD COLUMN organization_id TEXT;
ALTER TABLE provider_connections ADD COLUMN project_id TEXT;
ALTER TABLE provider_connections ADD COLUMN capability_profile_json TEXT;
ALTER TABLE provider_connections ADD COLUMN capability_checked_at TEXT;
ALTER TABLE provider_connections ADD COLUMN model_catalog_checked_at TEXT;
ALTER TABLE provider_connections ADD COLUMN provider_preset_version TEXT;
ALTER TABLE provider_connections ADD COLUMN enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1));

UPDATE provider_connections
SET
  provider_descriptor_id = COALESCE(provider_descriptor_id, provider),
  protocol_family = COALESCE(
    protocol_family,
    CASE provider
      WHEN 'anthropic' THEN 'anthropic_messages'
      WHEN 'gemini' THEN 'gemini_generate_content'
      ELSE 'openai_chat_completions'
    END
  ),
  auth_mode = COALESCE(auth_mode, 'api_key_bearer'),
  endpoint_class = COALESCE(
    endpoint_class,
    CASE
      WHEN provider IN ('compatible') THEN 'user_configured_remote_compatible'
      ELSE 'fixed_trusted_remote'
    END
  ),
  provider_preset_version = COALESCE(provider_preset_version, '1'),
  enabled = COALESCE(enabled, 1)
WHERE provider_descriptor_id IS NULL
   OR protocol_family IS NULL
   OR auth_mode IS NULL
   OR endpoint_class IS NULL;
