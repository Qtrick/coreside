# Application Packages (`.coreside-app`)

**Product:** Coreside  
**Code:** `application_kernel/packages.rs`

## Format (v1)

Primary export is a **ZIP** archive containing `manifest.json` (UTF-8 JSON package document). Legacy single-file JSON `.coreside-app` remains accepted on import.

| Field | Notes |
| --- | --- |
| `format` | `"coreside-app"` |
| `packageVersion` | `"1"` |
| `trustState` | e.g. `local`, `untrusted`, `signed` |
| `manifest` | Schema v1 `ApplicationManifest` |
| `dataModels`, `records`, `tests` | Optional arrays |
| `mediaRefs` | Path strings (validated) |
| `signature` | Optional; future signed-package extension point |

## Validation

- Max ~20 MB
- ZIP: must include `manifest.json`; rejects path traversal and executable-looking entries
- Legacy JSON: UTF-8 JSON document
- Rejects ELF / PE magic (`\x7fELF`, `MZ`)
- Rejects credential-shaped content and path traversal / executable markers
- Validates manifest + declared permissions

## Import / export

- `export_package` — policy-checked; trust_state `local`; caps records at 500
- `package_to_bytes` — ZIP with `manifest.json`
- `preview_package` — name, permissions, capabilities, warnings for unsigned/untrusted
- `import_package` / `validate_package_bytes` — accepts ZIP or legacy JSON; requires `approve`; optional `remintIds`
