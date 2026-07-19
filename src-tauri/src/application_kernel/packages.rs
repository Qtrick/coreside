//! `.coreside-app` package export / import / malicious rejection.
//! v1 primary format is ZIP (`manifest.json` inside). Legacy single JSON still accepted.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::{now_rfc3339, Database, DbError, DbResult};
use rusqlite::params;

use super::errors::KernelError;
use super::manifest::{get_manifest, upsert_manifest, validate_manifest, ApplicationManifest};
use super::permissions::{grant_permission, validate_declared_permissions};
use super::policy::{evaluate_policy, PolicyAction};
use super::ChangeRequest;

pub const PACKAGE_FORMAT: &str = "coreside-app";
pub const PACKAGE_VERSION: &str = "1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPackage {
    pub format: String,
    pub package_version: String,
    pub trust_state: String,
    pub manifest: ApplicationManifest,
    #[serde(default)]
    pub data_models: Vec<Value>,
    #[serde(default)]
    pub records: Vec<Value>,
    #[serde(default)]
    pub tests: Vec<Value>,
    #[serde(default)]
    pub media_refs: Vec<String>,
    /// Future signed-package extension point
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackagePreview {
    pub name: String,
    pub application_id: String,
    pub permissions: Vec<String>,
    pub capabilities: Vec<String>,
    pub record_count: usize,
    pub trust_state: String,
    pub warnings: Vec<String>,
}

pub fn export_package(db: &Database, application_id: &str) -> Result<AppPackage, KernelError> {
    let policy = evaluate_policy(
        db,
        PolicyAction::ExportPackage,
        &ChangeRequest {
            conversation_id: None,
            project_id: None,
            turn_id: None,
            summary: "export".into(),
            operations: vec![],
            silent: true,
            source_type: "user".into(),
            provider: None,
            model: None,
            require_approval: false,
            approval_granted: true,
        },
    )
    .map_err(KernelError::Db)?;
    if policy.decision == "deny" {
        return Err(KernelError::PolicyDenied(policy.message));
    }

    let rec = get_manifest(db, application_id).map_err(KernelError::Db)?;
    reject_credential_shaped(&serde_json::to_string(&rec.manifest).unwrap_or_default())?;

    let mut models = Vec::new();
    {
        let mut stmt = db
            .conn()
            .prepare(
                "SELECT definition_json FROM generated_data_models WHERE application_id = ?1",
            )
            .map_err(|e| KernelError::Db(DbError::Sqlite(e)))?;
        let rows = stmt
            .query_map([application_id], |row| row.get::<_, String>(0))
            .map_err(|e| KernelError::Db(DbError::Sqlite(e)))?;
        for r in rows.flatten() {
            if let Ok(v) = serde_json::from_str(&r) {
                models.push(v);
            }
        }
    }

    let mut records = Vec::new();
    {
        let mut stmt = db
            .conn()
            .prepare(
                "SELECT model_id, data_json FROM generated_data_records WHERE application_id = ?1 LIMIT 500",
            )
            .map_err(|e| KernelError::Db(DbError::Sqlite(e)))?;
        let rows = stmt
            .query_map([application_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| KernelError::Db(DbError::Sqlite(e)))?;
        for (model_id, data_s) in rows.flatten() {
            if let Ok(data) = serde_json::from_str::<Value>(&data_s) {
                records.push(json!({ "modelId": model_id, "data": data }));
            }
        }
    }

    let mut tests = Vec::new();
    {
        let mut stmt = db
            .conn()
            .prepare("SELECT definition_json FROM generated_tests WHERE application_id = ?1")
            .map_err(|e| KernelError::Db(DbError::Sqlite(e)))?;
        let rows = stmt
            .query_map([application_id], |row| row.get::<_, String>(0))
            .map_err(|e| KernelError::Db(DbError::Sqlite(e)))?;
        for r in rows.flatten() {
            if let Ok(v) = serde_json::from_str(&r) {
                tests.push(v);
            }
        }
    }

    Ok(AppPackage {
        format: PACKAGE_FORMAT.into(),
        package_version: PACKAGE_VERSION.into(),
        trust_state: "local".into(),
        manifest: rec.manifest,
        data_models: models,
        records,
        tests,
        media_refs: vec![],
        signature: None,
    })
}

pub fn package_to_bytes(pkg: &AppPackage) -> Result<Vec<u8>, KernelError> {
    let json = serde_json::to_vec_pretty(pkg).map_err(|e| KernelError::Validation(e.to_string()))?;
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(&mut cursor);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("manifest.json", opts)
            .map_err(|e| KernelError::PackageInvalid(e.to_string()))?;
        zip.write_all(&json)
            .map_err(|e| KernelError::PackageInvalid(e.to_string()))?;
        zip.finish()
            .map_err(|e| KernelError::PackageInvalid(e.to_string()))?;
    }
    Ok(cursor.into_inner())
}

fn reject_credential_shaped(s: &str) -> Result<(), KernelError> {
    let lower = s.to_lowercase();
    if lower.contains("api_key")
        || lower.contains("apikey")
        || lower.contains("authorization")
        || lower.contains("keychain")
        || lower.contains("\"sk-")
    {
        return Err(KernelError::PackageInvalid(
            "credential-shaped content rejected".into(),
        ));
    }
    Ok(())
}

fn reject_malicious_paths(s: &str) -> Result<(), KernelError> {
    if s.contains("..") || s.contains("file://") || s.contains("/users/") || s.contains("\\\\") {
        return Err(KernelError::PackageInvalid("path traversal rejected".into()));
    }
    let lower = s.to_lowercase();
    if lower.contains("<script")
        || lower.contains("javascript:")
        || lower.contains(".exe")
        || lower.contains("#!/")
    {
        return Err(KernelError::PackageInvalid("executable content rejected".into()));
    }
    Ok(())
}

fn parse_package_json(bytes: &[u8]) -> Result<AppPackage, KernelError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| KernelError::PackageInvalid("package must be UTF-8 JSON".into()))?;
    reject_credential_shaped(text)?;
    reject_malicious_paths(text)?;
    let pkg: AppPackage =
        serde_json::from_slice(bytes).map_err(|e| KernelError::PackageInvalid(e.to_string()))?;
    if pkg.format != PACKAGE_FORMAT {
        return Err(KernelError::PackageInvalid("unknown package format".into()));
    }
    validate_manifest(&pkg.manifest).map_err(KernelError::PackageInvalid)?;
    validate_declared_permissions(&pkg.manifest.permissions).map_err(KernelError::PackageInvalid)?;
    for media in &pkg.media_refs {
        reject_malicious_paths(media)?;
    }
    for model in &pkg.data_models {
        if let Ok(s) = serde_json::to_string(model) {
            reject_credential_shaped(&s)?;
        }
    }
    for record in &pkg.records {
        if let Ok(s) = serde_json::to_string(record) {
            reject_credential_shaped(&s)?;
        }
    }
    Ok(pkg)
}

fn validate_zip_package(bytes: &[u8]) -> Result<AppPackage, KernelError> {
    use std::io::Read;
    let cursor = std::io::Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| KernelError::PackageInvalid(e.to_string()))?;
    if archive.len() > 64 {
        return Err(KernelError::PackageInvalid("too many archive entries".into()));
    }
    let mut manifest_bytes = None;
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| KernelError::PackageInvalid(e.to_string()))?;
        let name = file.name().to_string();
        if name.contains("..") || name.starts_with('/') || name.contains('\\') {
            return Err(KernelError::PackageInvalid("path traversal rejected".into()));
        }
        let lower = name.to_lowercase();
        if lower.ends_with(".exe")
            || lower.ends_with(".sh")
            || lower.ends_with(".js")
            || lower.ends_with(".wasm")
            || lower.ends_with(".dylib")
        {
            return Err(KernelError::PackageInvalid("executable content rejected".into()));
        }
        if name == "manifest.json" || name.ends_with("/manifest.json") {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)
                .map_err(|e| KernelError::PackageInvalid(e.to_string()))?;
            if buf.len() > 5_000_000 {
                return Err(KernelError::PackageInvalid("manifest too large".into()));
            }
            manifest_bytes = Some(buf);
        }
    }
    let buf = manifest_bytes
        .ok_or_else(|| KernelError::PackageInvalid("manifest.json missing".into()))?;
    parse_package_json(&buf)
}

pub fn validate_package_bytes(bytes: &[u8]) -> Result<AppPackage, KernelError> {
    if bytes.len() > 20_000_000 {
        return Err(KernelError::PackageInvalid("package too large".into()));
    }
    // Reject non-ZIP executables
    if bytes.starts_with(b"\x7fELF") || bytes.starts_with(b"MZ") {
        return Err(KernelError::PackageInvalid("executable packages rejected".into()));
    }
    if bytes.starts_with(b"PK") {
        return validate_zip_package(bytes);
    }
    // Legacy JSON .coreside-app still accepted
    parse_package_json(bytes)
}

pub fn preview_package(pkg: &AppPackage) -> PackagePreview {
    let mut warnings = Vec::new();
    if pkg.trust_state != "signed" {
        warnings.push("Package is untrusted until validated and approved.".into());
    }
    if pkg.signature.is_none() {
        warnings.push("No organization signature present (consumer package).".into());
    }
    PackagePreview {
        name: pkg.manifest.name.clone(),
        application_id: pkg.manifest.application_id.clone(),
        permissions: pkg.manifest.permissions.clone(),
        capabilities: pkg.manifest.capabilities.clone(),
        record_count: pkg.records.len(),
        trust_state: pkg.trust_state.clone(),
        warnings,
    }
}

pub fn import_package(
    db: &mut Database,
    bytes: &[u8],
    approve: bool,
    remint_ids: bool,
) -> Result<ApplicationManifest, KernelError> {
    let mut pkg = validate_package_bytes(bytes)?;
    let policy = evaluate_policy(
        db,
        PolicyAction::ImportPackage,
        &ChangeRequest {
            conversation_id: None,
            project_id: None,
            turn_id: None,
            summary: "import".into(),
            operations: vec![],
            silent: true,
            source_type: "user".into(),
            provider: None,
            model: None,
            require_approval: false,
            approval_granted: approve,
        },
    )
    .map_err(KernelError::Db)?;
    if policy.decision == "deny" {
        return Err(KernelError::PolicyDenied(policy.message));
    }
    let preview = preview_package(&pkg);
    if !approve {
        return Err(KernelError::Validation(format!(
            "approval required: {}",
            preview.name
        )));
    }
    if remint_ids {
        pkg.manifest.application_id = format!("app-{}", Uuid::new_v4());
        pkg.manifest.instance_id = format!("instance-{}", Uuid::new_v4());
    }
    upsert_manifest(db, pkg.manifest.clone()).map_err(KernelError::Db)?;
    for perm in &pkg.manifest.permissions {
        let _ = grant_permission(
            db,
            &pkg.manifest.application_id,
            perm,
            json!({ "scope": "application" }),
            "package_import",
        );
    }
    for model in &pkg.data_models {
        if let Ok(def) = serde_json::from_value::<super::data::DataModelDefinition>(model.clone()) {
            let _ = super::data::upsert_model(db, &pkg.manifest.application_id, def);
        }
    }
    db.conn()
        .execute(
            "INSERT INTO application_packages (
                id, application_id, package_version, direction, status, filename, trust_state, created_at
             ) VALUES (?1,?2,?3,'import','installed',NULL,?4,?5)",
            params![
                format!("pkg-{}", Uuid::new_v4()),
                pkg.manifest.application_id,
                pkg.package_version,
                pkg.trust_state,
                now_rfc3339()
            ],
        )
        .map_err(|e| KernelError::Db(DbError::Sqlite(e)))?;
    Ok(pkg.manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application_kernel::manifest::{ApplicationManifest, ManifestSurfaceRef};

    fn sample_pkg() -> AppPackage {
        AppPackage {
            format: PACKAGE_FORMAT.into(),
            package_version: PACKAGE_VERSION.into(),
            trust_state: "untrusted".into(),
            manifest: ApplicationManifest {
                schema_version: "1".into(),
                application_id: "app-pkg".into(),
                instance_id: "inst-pkg".into(),
                name: "Packaged App".into(),
                description: String::new(),
                version: 1,
                surfaces: vec![ManifestSurfaceRef {
                    surface_id: "s1".into(),
                    placement: "tool_canvas".into(),
                    definition_ref: None,
                }],
                routes: vec![],
                data_models: vec![],
                settings: vec![],
                capabilities: vec!["coreside.core".into()],
                permissions: vec!["local_data.read".into()],
                events: vec![],
                tests: vec![],
                search_keywords: vec![],
                tags: vec![],
                agent_description: String::new(),
                project_id: None,
                conversation_id: None,
                organization_id: None,
                ownership: None,
            },
            data_models: vec![],
            records: vec![],
            tests: vec![],
            media_refs: vec![],
            signature: None,
        }
    }

    #[test]
    fn roundtrip_zip() {
        let bytes = package_to_bytes(&sample_pkg()).unwrap();
        assert!(bytes.starts_with(b"PK"));
        let pkg = validate_package_bytes(&bytes).unwrap();
        assert_eq!(pkg.manifest.name, "Packaged App");
    }

    #[test]
    fn accepts_legacy_json() {
        let json = serde_json::to_vec(&sample_pkg()).unwrap();
        let pkg = validate_package_bytes(&json).unwrap();
        assert_eq!(pkg.manifest.name, "Packaged App");
    }

    #[test]
    fn rejects_malformed_zip() {
        assert!(validate_package_bytes(b"PK\x03\x04malicious").is_err());
    }

    #[test]
    fn rejects_path_traversal() {
        let mut pkg = sample_pkg();
        pkg.media_refs.push("../etc/passwd".into());
        let bytes = package_to_bytes(&pkg).unwrap();
        assert!(validate_package_bytes(&bytes).is_err());
    }

    #[test]
    fn rejects_api_key() {
        let mut pkg = sample_pkg();
        pkg.manifest.description = "api_key=secret".into();
        let bytes = package_to_bytes(&pkg).unwrap();
        assert!(validate_package_bytes(&bytes).is_err());
    }
}
