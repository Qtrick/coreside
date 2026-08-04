# Provider Endpoint Security (RC3.5)

Enforced in `src-tauri/src/ai/platform/endpoint_policy.rs` before connection save.

## Rules

1. URL userinfo (`user:pass@`) is rejected.
2. Remote / custom / hosted classes require HTTPS.
3. Loopback Local AI may use HTTP only when the host is loopback.
4. RFC1918 / unique-local addresses require `private_lan_local`, not silent loopback classification.
5. Link-local and cloud metadata destinations (`169.254.169.254`, `metadata.google.internal`) are blocked.
6. Fixed trusted providers reject endpoint overrides unless the connection is reclassified as custom.

## Redirects

Credential-bearing HTTP clients must not auto-follow redirects to new origins. Product policy: disable automatic redirects and validate each hop (implementation hardening continues in Phase 4 shared protocol runtime).

## Secrets

Endpoint URLs and classification live in SQLite. API keys and tokens stay in the OS keychain. Authless Local AI stores no secret.
