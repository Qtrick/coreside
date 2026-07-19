# Enterprise Connector Architecture

**Product:** Coreside  
**Status:** Architecture only — **no connector implementations shipped**

Consumer Coreside has no live CRM/ERP/email/calendar connectors. This document records extension boundaries so future enterprise connectors can share the Application Kernel without replacing the consumer runtime.

## Principles

1. Connectors are **trusted adapters** owned by Rust — never agent-installed packages or CDN scripts.
2. Generated apps request capability via **declared permissions** + policy; they never hold org credentials.
3. Secrets stay in OS-secure storage / future enterprise key management — never in manifests, packages, or SQLite tool JSON.
4. Mutations still enter through `application_kernel::apply_change` (or narrower trusted commands).
5. Project/application isolation remains enforced in DB queries.

## Extension points (existing hooks)

| Hook | Role |
| --- | --- |
| `policy::evaluate_policy` | Local `policy_overrides` → future org policy gateway |
| `permissions` allowlist | Expand only via trusted releases |
| Package `signature` field | Future org-signed `.coreside-app` verification |
| Manifest `organizationId` / `ownership` | Optional scoping fields already on schema v1 |
| Protected id `core.enterprise_policy_hook` | Reserved protected resource |

## Non-goals (current phase)

Do not invent fake connector modules, mock Salesforce/Slack adapters, or claim sync pipelines exist. When a connector is added, document it under its own file and keep this architecture note as the boundary contract.
