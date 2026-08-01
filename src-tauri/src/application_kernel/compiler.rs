//! Deterministic Change Compiler — high-level intents → ordered AppOperations.

use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::runtime_v2::operations::{AppOperation, OperationTarget};
use crate::runtime_v2::packs::validate_component_type_allowed;

use super::errors::KernelError;
use super::COMPILER_VERSION;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "intent", rename_all = "camelCase")]
pub enum ChangeIntent {
    #[serde(rename_all = "camelCase")]
    AddSetting {
        application_id: String,
        setting_id: String,
        label: String,
        value_type: String,
        default: serde_json::Value,
    },
    #[serde(rename_all = "camelCase")]
    UpsertManifest {
        manifest: super::manifest::ApplicationManifest,
    },
    #[serde(rename_all = "camelCase")]
    UpsertDataModel {
        application_id: String,
        model: super::data::DataModelDefinition,
    },
    #[serde(rename_all = "camelCase")]
    InsertComponent {
        surface_id: String,
        parent_id: Option<String>,
        component_type: String,
        props: serde_json::Value,
        base_revision: Option<i64>,
    },
    #[serde(rename_all = "camelCase")]
    NavigateRoute {
        application_id: String,
        route_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledChange {
    pub compiler_version: String,
    pub operations: Vec<AppOperation>,
    pub summary: String,
    pub rollback_hint: String,
}

fn new_op(op_type: &str, target: OperationTarget, payload: serde_json::Value) -> AppOperation {
    AppOperation {
        id: format!("op-{}", Uuid::new_v4()),
        op_type: op_type.into(),
        target,
        base_revision: None,
        transaction_group: Some("compiled".into()),
        idempotency_key: Some(format!("ik-{}", Uuid::new_v4())),
        depends_on: None,
        payload,
        requires_approval: None,
        destructive: None,
        audience: None,
    }
}

pub fn compile(intent: ChangeIntent) -> Result<CompiledChange, KernelError> {
    match intent {
        ChangeIntent::AddSetting {
            application_id,
            setting_id,
            label,
            value_type,
            default,
        } => {
            crate::security::assert_not_protected(&setting_id).map_err(KernelError::Protected)?;
            let op = new_op(
                "setting.create",
                OperationTarget {
                    application_id: Some(application_id.clone()),
                    ..Default::default()
                },
                json!({
                    "id": setting_id,
                    "label": label,
                    "valueType": value_type,
                    "default": default,
                    "applicationId": application_id,
                }),
            );
            Ok(CompiledChange {
                compiler_version: COMPILER_VERSION.into(),
                operations: vec![op],
                summary: "Add setting".into(),
                rollback_hint: "setting.delete".into(),
            })
        }
        ChangeIntent::UpsertManifest { manifest } => {
            super::manifest::validate_manifest(&manifest).map_err(KernelError::Validation)?;
            let name = manifest.name.clone();
            let op = new_op(
                "manifest.upsert",
                OperationTarget {
                    application_id: Some(manifest.application_id.clone()),
                    ..Default::default()
                },
                serde_json::to_value(&manifest)
                    .map_err(|e| KernelError::Validation(e.to_string()))?,
            );
            Ok(CompiledChange {
                compiler_version: COMPILER_VERSION.into(),
                operations: vec![op],
                summary: format!("Upsert application {name}"),
                rollback_hint: "manifest.restore_last_known_good".into(),
            })
        }
        ChangeIntent::UpsertDataModel {
            application_id,
            model,
        } => {
            super::data::validate_model(&model).map_err(KernelError::Validation)?;
            let mid = model.model_id.clone();
            let op = new_op(
                "data.model_upsert",
                OperationTarget {
                    application_id: Some(application_id.clone()),
                    model_id: Some(mid.clone()),
                    ..Default::default()
                },
                json!({ "applicationId": application_id, "model": model }),
            );
            Ok(CompiledChange {
                compiler_version: COMPILER_VERSION.into(),
                operations: vec![op],
                summary: format!("Upsert data model {mid}"),
                rollback_hint: "restore previous model schema version".into(),
            })
        }
        ChangeIntent::InsertComponent {
            surface_id,
            parent_id,
            component_type,
            props,
            base_revision,
        } => {
            validate_component_type_allowed(&component_type)
                .map_err(KernelError::CapabilityUnavailable)?;
            let component_id = format!("c-{}", Uuid::new_v4());
            let mut op = new_op(
                "component.insert",
                OperationTarget {
                    surface_id: Some(surface_id),
                    parent_id,
                    component_id: Some(component_id.clone()),
                    ..Default::default()
                },
                json!({
                    "component": {
                        "id": component_id,
                        "type": component_type,
                        "props": props,
                    }
                }),
            );
            op.base_revision = base_revision;
            Ok(CompiledChange {
                compiler_version: COMPILER_VERSION.into(),
                operations: vec![op],
                summary: "Insert component".into(),
                rollback_hint: "component.remove".into(),
            })
        }
        ChangeIntent::NavigateRoute {
            application_id,
            route_id,
        } => {
            if route_id == "settings" || route_id == "recovery" {
                return Err(KernelError::Protected(
                    "cannot navigate to protected routes via generated apps".into(),
                ));
            }
            let op = new_op(
                "route.navigate",
                OperationTarget {
                    application_id: Some(application_id),
                    ..Default::default()
                },
                json!({ "routeId": route_id }),
            );
            Ok(CompiledChange {
                compiler_version: COMPILER_VERSION.into(),
                operations: vec![op],
                summary: "Navigate route".into(),
                rollback_hint: "route.navigate previous".into(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_setting() {
        let c = compile(ChangeIntent::AddSetting {
            application_id: "app-1".into(),
            setting_id: "setting.daily-goal".into(),
            label: "Daily goal".into(),
            value_type: "integer".into(),
            default: json!(3),
        })
        .unwrap();
        assert_eq!(c.operations.len(), 1);
        assert_eq!(c.operations[0].op_type, "setting.create");
    }

    #[test]
    fn rejects_unsupported_component() {
        let err = compile(ChangeIntent::InsertComponent {
            surface_id: "s1".into(),
            parent_id: None,
            component_type: "iframe".into(),
            props: json!({}),
            base_revision: None,
        })
        .unwrap_err();
        assert!(matches!(err, KernelError::CapabilityUnavailable(_)));
    }
}
