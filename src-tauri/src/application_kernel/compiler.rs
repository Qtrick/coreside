//! Deterministic Change Compiler — high-level intents → ordered AppOperations.

use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::runtime_v2::operations::{AppOperation, OperationTarget};
use crate::runtime_v2::packs::validate_component_type_allowed;

use super::errors::KernelError;
use super::COMPILER_VERSION;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    /// Evolve an existing model schema (same op as upsert; semantic marker for plans).
    #[serde(rename_all = "camelCase")]
    MigrateDataModel {
        application_id: String,
        model: super::data::DataModelDefinition,
        #[serde(default)]
        migration_strategy: Option<String>,
        #[serde(default)]
        require_approval: Option<bool>,
    },
    #[serde(rename_all = "camelCase")]
    InsertComponent {
        surface_id: String,
        parent_id: Option<String>,
        component_type: String,
        props: serde_json::Value,
        base_revision: Option<i64>,
        #[serde(default)]
        component_id: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    UpdateComponent {
        surface_id: String,
        component_id: String,
        #[serde(default)]
        component_type: Option<String>,
        #[serde(default)]
        props: Option<serde_json::Value>,
        base_revision: Option<i64>,
    },
    #[serde(rename_all = "camelCase")]
    RemoveComponent {
        surface_id: String,
        component_id: String,
        base_revision: Option<i64>,
    },
    #[serde(rename_all = "camelCase")]
    NavigateRoute {
        application_id: String,
        route_id: String,
    },
    /// High-level surface creation — compiles to `surface.create`.
    #[serde(rename_all = "camelCase")]
    CreateSurface {
        tool: crate::ai::ToolDefinition,
        #[serde(default)]
        change_summary: Option<String>,
    },
    /// Evolve an existing surface — compiles to `surface.create` with action update.
    #[serde(rename_all = "camelCase")]
    UpdateSurface {
        tool_id: String,
        tool: crate::ai::ToolDefinition,
        #[serde(default)]
        change_summary: Option<String>,
        #[serde(default)]
        base_revision: Option<i64>,
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
        ChangeIntent::MigrateDataModel {
            application_id,
            model,
            migration_strategy,
            require_approval,
        } => {
            super::data::validate_model(&model).map_err(KernelError::Validation)?;
            let mid = model.model_id.clone();
            let strategy = migration_strategy.unwrap_or_else(|| "add_optional_fields".into());
            // Destructive strategies require explicit approval at apply time.
            let destructive = matches!(
                strategy.as_str(),
                "drop_fields" | "rewrite_required" | "destructive"
            );
            if destructive && require_approval != Some(true) {
                return Err(KernelError::Validation(
                    "destructive data model migration requires requireApproval: true".into(),
                ));
            }
            let mut op = new_op(
                "data.model_upsert",
                OperationTarget {
                    application_id: Some(application_id.clone()),
                    model_id: Some(mid.clone()),
                    ..Default::default()
                },
                json!({
                    "applicationId": application_id,
                    "model": model,
                    "migrationStrategy": strategy,
                }),
            );
            op.requires_approval = if destructive { Some(true) } else { None };
            op.destructive = if destructive { Some(true) } else { None };
            Ok(CompiledChange {
                compiler_version: COMPILER_VERSION.into(),
                operations: vec![op],
                summary: format!("Migrate data model {mid}"),
                rollback_hint: "restore previous model schema version".into(),
            })
        }
        ChangeIntent::InsertComponent {
            surface_id,
            parent_id,
            component_type,
            props,
            base_revision,
            component_id,
        } => {
            validate_component_type_allowed(&component_type)
                .map_err(KernelError::CapabilityUnavailable)?;
            let component_id = component_id
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| format!("c-{}", Uuid::new_v4()));
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
        ChangeIntent::UpdateComponent {
            surface_id,
            component_id,
            component_type,
            props,
            base_revision,
        } => {
            if let Some(ref ct) = component_type {
                validate_component_type_allowed(ct).map_err(KernelError::CapabilityUnavailable)?;
            }
            let mut payload = json!({});
            if let Some(p) = props {
                payload["props"] = p;
            }
            if let Some(ct) = component_type {
                payload["type"] = json!(ct);
            }
            let mut op = new_op(
                "component.update_props",
                OperationTarget {
                    surface_id: Some(surface_id),
                    component_id: Some(component_id),
                    ..Default::default()
                },
                payload,
            );
            op.base_revision = base_revision;
            Ok(CompiledChange {
                compiler_version: COMPILER_VERSION.into(),
                operations: vec![op],
                summary: "Update component".into(),
                rollback_hint: "component.update_props previous".into(),
            })
        }
        ChangeIntent::RemoveComponent {
            surface_id,
            component_id,
            base_revision,
        } => {
            let mut op = new_op(
                "component.remove",
                OperationTarget {
                    surface_id: Some(surface_id),
                    component_id: Some(component_id),
                    ..Default::default()
                },
                json!({}),
            );
            op.base_revision = base_revision;
            Ok(CompiledChange {
                compiler_version: COMPILER_VERSION.into(),
                operations: vec![op],
                summary: "Remove component".into(),
                rollback_hint: "component.insert previous".into(),
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
        ChangeIntent::CreateSurface {
            tool,
            change_summary,
        } => {
            crate::security::assert_not_protected(&tool.id).map_err(KernelError::Protected)?;
            for c in &tool.components {
                validate_component_type_allowed(&c.component_type)
                    .map_err(KernelError::CapabilityUnavailable)?;
            }
            let name = tool.name.clone();
            let op = new_op(
                "surface.create",
                OperationTarget {
                    surface_id: Some(format!("surf-{}", tool.id)),
                    tool_id: Some(tool.id.clone()),
                    surface_type: Some("tool".into()),
                    placement: Some("tool_canvas".into()),
                    ..Default::default()
                },
                json!({
                    "action": "create",
                    "tool": tool,
                    "changeSummary": change_summary.unwrap_or_else(|| format!("Create {name}")),
                }),
            );
            Ok(CompiledChange {
                compiler_version: COMPILER_VERSION.into(),
                operations: vec![op],
                summary: format!("Create surface {name}"),
                rollback_hint: "surface.delete".into(),
            })
        }
        ChangeIntent::UpdateSurface {
            tool_id,
            tool,
            change_summary,
            base_revision,
        } => {
            crate::security::assert_not_protected(&tool_id).map_err(KernelError::Protected)?;
            crate::security::assert_not_protected(&tool.id).map_err(KernelError::Protected)?;
            for c in &tool.components {
                validate_component_type_allowed(&c.component_type)
                    .map_err(KernelError::CapabilityUnavailable)?;
            }
            let name = tool.name.clone();
            let mut op = new_op(
                "tool.full_replace",
                OperationTarget {
                    surface_id: Some(format!("surf-{tool_id}")),
                    tool_id: Some(tool_id.clone()),
                    surface_type: Some("tool".into()),
                    placement: Some("tool_canvas".into()),
                    ..Default::default()
                },
                json!({
                    "action": "update",
                    "tool": tool,
                    "targetToolId": tool_id,
                    "changeSummary": change_summary.unwrap_or_else(|| format!("Update {name}")),
                }),
            );
            op.base_revision = base_revision;
            Ok(CompiledChange {
                compiler_version: COMPILER_VERSION.into(),
                operations: vec![op],
                summary: format!("Update surface {name}"),
                rollback_hint: "surface.restore previous revision".into(),
            })
        }
    }
}

/// Compile an ordered list of intents into one CompiledChange.
pub fn compile_all(intents: &[ChangeIntent]) -> Result<CompiledChange, KernelError> {
    if intents.is_empty() {
        return Err(KernelError::Validation(
            "compile_all requires at least one intent".into(),
        ));
    }
    let mut operations = Vec::new();
    let mut summaries = Vec::new();
    let mut rollback_hints = Vec::new();
    for intent in intents {
        let compiled = compile(intent.clone())?;
        operations.extend(compiled.operations);
        summaries.push(compiled.summary);
        rollback_hints.push(compiled.rollback_hint);
    }
    Ok(CompiledChange {
        compiler_version: COMPILER_VERSION.into(),
        operations,
        summary: summaries.join("; "),
        rollback_hint: rollback_hints.join("; "),
    })
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
            component_id: None,
        })
        .unwrap_err();
        assert!(matches!(err, KernelError::CapabilityUnavailable(_)));
    }

    #[test]
    fn update_surface_compiles_to_update_action() {
        let tool = crate::ai::ToolDefinition {
            id: "tool-task-tracker".into(),
            name: "Task Tracker".into(),
            description: "Track tasks".into(),
            layout: json!({"type": "dashboard"}),
            components: vec![crate::ai::ToolComponent {
                id: "heading".into(),
                component_type: "heading".into(),
                value_key: None,
                props: Some(json!({"text": "Task Tracker", "level": 1})),
                children: None,
                ..Default::default()
            }],
            ..Default::default()
        };
        let c = compile(ChangeIntent::UpdateSurface {
            tool_id: "tool-task-tracker".into(),
            tool,
            change_summary: Some("Add due dates".into()),
            base_revision: Some(3),
        })
        .unwrap();
        assert_eq!(c.operations[0].op_type, "tool.full_replace");
        assert_eq!(c.operations[0].payload["action"], "update");
        assert_eq!(c.operations[0].base_revision, Some(3));
    }

    #[test]
    fn migrate_destructive_requires_approval() {
        let model = super::super::data::DataModelDefinition {
            model_id: "tasks".into(),
            display_name: "Tasks".into(),
            schema_version: 2,
            fields: vec![super::super::data::DataField {
                field_id: "title".into(),
                field_type: "text".into(),
                required: true,
                default: None,
                enum_values: None,
            }],
        };
        let err = compile(ChangeIntent::MigrateDataModel {
            application_id: "tool-task-tracker".into(),
            model,
            migration_strategy: Some("drop_fields".into()),
            require_approval: None,
        })
        .unwrap_err();
        assert!(matches!(err, KernelError::Validation(_)));
    }

    #[test]
    fn compile_all_orders_operations() {
        let tool = crate::ai::ToolDefinition {
            id: "tool-notes".into(),
            name: "Notes".into(),
            description: "".into(),
            layout: json!({"type": "stack"}),
            components: vec![crate::ai::ToolComponent {
                id: "h".into(),
                component_type: "heading".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let compiled = compile_all(&[
            ChangeIntent::CreateSurface {
                tool,
                change_summary: None,
            },
            ChangeIntent::UpsertDataModel {
                application_id: "tool-notes".into(),
                model: super::super::data::DataModelDefinition {
                    model_id: "notes".into(),
                    display_name: "Notes".into(),
                    schema_version: 1,
                    fields: vec![super::super::data::DataField {
                        field_id: "title".into(),
                        field_type: "text".into(),
                        required: true,
                        default: None,
                        enum_values: None,
                    }],
                },
            },
        ])
        .unwrap();
        assert_eq!(compiled.operations.len(), 2);
        assert_eq!(compiled.operations[0].op_type, "surface.create");
        assert_eq!(compiled.operations[1].op_type, "data.model_upsert");
    }

    #[test]
    fn create_surface_compiles_to_surface_create() {
        let tool = crate::ai::ToolDefinition {
            id: "tool-task-tracker".into(),
            name: "Task Tracker".into(),
            description: "Track tasks".into(),
            layout: json!({"type": "dashboard"}),
            components: vec![crate::ai::ToolComponent {
                id: "heading".into(),
                component_type: "heading".into(),
                value_key: None,
                props: Some(json!({"text": "Task Tracker", "level": 1})),
                children: None,
                ..Default::default()
            }],
            ..Default::default()
        };
        let c = compile(ChangeIntent::CreateSurface {
            tool: tool.clone(),
            change_summary: Some("Create Task Tracker".into()),
        })
        .unwrap();
        assert_eq!(c.operations.len(), 1);
        assert_eq!(c.operations[0].op_type, "surface.create");
        assert_eq!(
            c.operations[0].target.surface_id.as_deref(),
            Some("surf-tool-task-tracker")
        );
        assert_eq!(
            c.operations[0].target.tool_id.as_deref(),
            Some("tool-task-tracker")
        );
        assert_eq!(c.operations[0].payload["action"], "create");
        assert_eq!(c.operations[0].payload["tool"]["id"], "tool-task-tracker");
        assert_eq!(
            c.operations[0].payload["changeSummary"],
            "Create Task Tracker"
        );
    }

    #[test]
    fn create_surface_defaults_change_summary_from_tool_name() {
        let tool = crate::ai::ToolDefinition {
            id: "tool-notes".into(),
            name: "Notes".into(),
            description: "".into(),
            layout: json!({"type": "stack"}),
            components: vec![],
            ..Default::default()
        };
        let c = compile(ChangeIntent::CreateSurface {
            tool,
            change_summary: None,
        })
        .unwrap();
        assert_eq!(c.operations[0].payload["changeSummary"], "Create Notes");
        assert_eq!(c.summary, "Create surface Notes");
        assert_eq!(c.rollback_hint, "surface.delete");
    }

    #[test]
    fn create_surface_rejects_protected_tool_id() {
        let tool = crate::ai::ToolDefinition {
            id: "core.settings".into(),
            name: "Settings".into(),
            description: "".into(),
            layout: json!({"type": "stack"}),
            components: vec![],
            ..Default::default()
        };
        let err = compile(ChangeIntent::CreateSurface {
            tool,
            change_summary: None,
        })
        .unwrap_err();
        assert!(matches!(err, KernelError::Protected(_)));
    }

    #[test]
    fn create_surface_rejects_disallowed_component() {
        let tool = crate::ai::ToolDefinition {
            id: "tool-bad".into(),
            name: "Bad".into(),
            description: "".into(),
            layout: json!({"type": "stack"}),
            components: vec![crate::ai::ToolComponent {
                id: "evil".into(),
                component_type: "iframe".into(),
                value_key: None,
                props: Some(json!({})),
                children: None,
                ..Default::default()
            }],
            ..Default::default()
        };
        let err = compile(ChangeIntent::CreateSurface {
            tool,
            change_summary: None,
        })
        .unwrap_err();
        assert!(matches!(err, KernelError::CapabilityUnavailable(_)));
    }
}
