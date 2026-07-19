# Application Permissions

**Product:** Coreside  
**Code:** `application_kernel/permissions.rs`

Declared on manifests; **granted/revoked by trusted code only**.

## Allowed

`local_data.read`, `local_data.write`, `media.read`, `media.create`, `media.export`, `project_context.read`, `tool_reference.read`, `automation.propose`, `export.prepare`, `microphone.request`, `clipboard.read`, `clipboard.write`, `file.open_user_selected`, `file.save_user_selected`, `external_link.open`, `web_search.request`

## Forbidden

`unrestricted.filesystem`, `unrestricted.network`, `unrestricted.shell`, `unrestricted.tauri`, `credential.read`, `credential.write`, `protected_settings.write` — plus any `unrestricted.*`

Unknown permission strings are also rejected.

## Grant / revoke

- `grant_permission` / `revoke_permission` — trusted API (e.g. package import with user approval)
- Agent `permission.grant` ops are rejected at the kernel gateway
- Forbidden permissions in op payloads fail closed
- Data writes call `assert_can_write_data` → requires granted `local_data.write`

Grants persist in `application_permissions` (`status`: granted / revoked).
