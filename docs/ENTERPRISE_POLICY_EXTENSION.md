# Enterprise Policy Extension

**Product:** Coreside  
**Code:** `application_kernel/policy.rs`  
**Table:** `policy_overrides`

Consumer default: **allow** when no override row exists.

## Evaluation

`evaluate_policy(db, PolicyAction, ChangeRequest)` looks up `decision` by `policy_key`:

| `PolicyAction` | Key |
| --- | --- |
| `ApplyOperations` | `apply_operations` |
| `ExportPackage` | `export_package` |
| `WebSearch` | `web_search` |
| `ImportPackage` | `import_package` |

`disable_export` on export actions maps to `deny`.

Gateway behavior: `deny` aborts apply; `require_user_approval` forces the proposal path with risk classification.

## Decision vocabulary

Trusted setters (`set_policy_override`) accept only:

`allow`, `deny`, `require_user_approval`, `require_admin_approval`, `require_security_review`, `restrict_scope`, `force_provider`, `force_local_only`, `disable_export`, `require_retention`, `require_audit`

Enterprise meanings (admin approval, retention, audit) are vocabulary + messaging today — no remote policy server is implemented. Consumer installs typically leave the table empty.
