//! Authoritative surface mount readiness (renderer live ≠ SQLite row exists).
//!
//! Durable operations may proceed when a surface exists in SQLite.
//! Renderer-targeted live updates consult this registry to know whether a
//! concrete window is currently mounted and eligible to display them.
//!
//! Window identity is always taken from the Tauri window label at IPC time —
//! never from a model- or renderer-supplied spoofable field.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{Database, DbError, DbResult};

/// Mounts older than this are treated as stale (crash / missed unmount).
pub const MOUNT_STALE_TTL: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceMountRegistration {
    pub surface_id: String,
    pub conversation_id: Option<String>,
    pub application_id: Option<String>,
    pub window_label: String,
    pub renderer_instance_id: String,
    pub definition_revision: i64,
    pub state_revision: i64,
    pub generation: u64,
    #[serde(skip)]
    pub ready_at: Option<Instant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RendererReadiness {
    Ready {
        window_label: String,
        renderer_instance_id: String,
        generation: u64,
    },
    NotMounted,
    Stale {
        reason: String,
    },
}

/// How an operation meets a live renderer.
///
/// Durable construction must not wait for a window. A live visual update must
/// not treat a SQLite row as a mounted renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationDelivery {
    /// Persist even when nothing is mounted (creates, data, manifests).
    DurableOnly,
    /// Persist now. A mounted renderer reconciles the result when it appears.
    RendererOptional,
    /// Do not apply until a fresh mount exists for the target surface.
    RendererRequired,
}

/// Multiple renderer instances may mount one surface (inline + tool window,
/// or two copies in one window). Readiness is "any fresh instance", not
/// last-register-wins. The same `renderer_instance_id` replaces only itself.
#[derive(Debug, Default)]
pub struct MountRegistry {
    /// Keyed by surface_id → renderer_instance_id.
    by_surface: HashMap<String, HashMap<String, SurfaceMountRegistration>>,
    generation: u64,
}

/// Model-visible and internal operation types, classified without trusting
/// a caller-supplied window or renderer id.
pub fn operation_delivery(op_type: &str) -> OperationDelivery {
    match op_type {
        // Window navigation is a live view. A route row is not a mounted renderer.
        "route.navigate" => OperationDelivery::RendererRequired,
        "surface.create" | "chat.inline_surface_create" | "tool.full_replace" | "tool.create" => {
            OperationDelivery::DurableOnly
        }
        t if t.starts_with("data.")
            || t.starts_with("manifest.")
            || t.starts_with("state.")
            || t.starts_with("surface.")
            || t.starts_with("component.")
            || t.starts_with("subscription.")
            || t.starts_with("chat.")
            || t == "event.dispatch"
            || t == "interactive.action"
            || t == "layout.update"
            || t == "wallpaper.apply" =>
        {
            OperationDelivery::RendererOptional
        }
        _ => OperationDelivery::DurableOnly,
    }
}

impl MountRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a mount after Rust validates surface ownership.
    /// `window_label` must come from the Tauri window, not the payload.
    pub fn register(
        &mut self,
        db: &Database,
        window_label: &str,
        surface_id: &str,
        renderer_instance_id: &str,
        claimed_application_id: Option<&str>,
        claimed_conversation_id: Option<&str>,
        definition_revision: i64,
        state_revision: i64,
    ) -> DbResult<SurfaceMountRegistration> {
        let window_label = normalize_window_label(window_label);
        let surface = crate::runtime_v2::surfaces::get_surface(db, surface_id)?;
        if surface.archived || surface.lifecycle_state == "archived" {
            return Err(DbError::Invalid(format!(
                "surface {surface_id} is archived and cannot be mounted"
            )));
        }

        let authoritative_app = crate::application_kernel::manifest::authoritative_application_id(
            db,
            surface.tool_id.as_deref(),
        );
        if let Some(claimed) = claimed_application_id
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            match authoritative_app.as_deref() {
                Some(auth) if auth == claimed => {}
                Some(auth) => {
                    return Err(DbError::Invalid(format!(
                        "application identity mismatch: claimed '{claimed}' but surface belongs to '{auth}'"
                    )));
                }
                None => {
                    return Err(DbError::Invalid(format!(
                        "application identity mismatch: claimed '{claimed}' but surface has no manifest-backed application"
                    )));
                }
            }
        }
        if let Some(claimed_conv) = claimed_conversation_id
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            match surface.conversation_id.as_deref() {
                Some(own) if own == claimed_conv => {}
                Some(own) => {
                    return Err(DbError::Invalid(format!(
                        "conversation identity mismatch: claimed '{claimed_conv}' but surface belongs to '{own}'"
                    )));
                }
                None => {
                    return Err(DbError::Invalid(format!(
                        "conversation identity mismatch: claimed '{claimed_conv}' but surface has no conversation"
                    )));
                }
            }
        }

        if let Some(app) = authoritative_app.as_deref() {
            if !crate::application_kernel::manifest::application_accepts_mutations(db, app) {
                return Err(DbError::Invalid(format!(
                    "application '{app}' is disabled or suspended and cannot receive mounts"
                )));
            }
        }

        let instance = {
            let trimmed = renderer_instance_id.trim();
            if trimmed.is_empty() {
                format!("rend-{}", Uuid::new_v4())
            } else {
                trimmed.to_string()
            }
        };

        self.generation = self.generation.saturating_add(1);
        let reg = SurfaceMountRegistration {
            surface_id: surface_id.to_string(),
            conversation_id: surface.conversation_id.clone(),
            application_id: authoritative_app,
            window_label: window_label.clone(),
            renderer_instance_id: instance.clone(),
            definition_revision,
            state_revision,
            generation: self.generation,
            ready_at: Some(Instant::now()),
        };
        self.by_surface
            .entry(surface_id.to_string())
            .or_default()
            .insert(instance, reg.clone());
        Ok(reg)
    }

    pub fn unregister(
        &mut self,
        window_label: &str,
        surface_id: &str,
        renderer_instance_id: Option<&str>,
    ) -> bool {
        let window_label = normalize_window_label(window_label);
        let Some(instances) = self.by_surface.get_mut(surface_id) else {
            return false;
        };
        let before = instances.len();
        if let Some(want) = renderer_instance_id
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            if let Some(existing) = instances.get(want) {
                if existing.window_label != window_label {
                    return false;
                }
            }
            instances.remove(want);
        } else {
            instances.retain(|_, reg| reg.window_label != window_label);
        }
        let removed = instances.len() != before;
        if instances.is_empty() {
            self.by_surface.remove(surface_id);
        }
        removed
    }

    /// Drop every mount owned by a window. Does not touch durable application rows.
    pub fn clear_window(&mut self, window_label: &str) {
        let window_label = normalize_window_label(window_label);
        let surfaces: Vec<String> = self.by_surface.keys().cloned().collect();
        for sid in surfaces {
            if let Some(instances) = self.by_surface.get_mut(&sid) {
                instances.retain(|_, reg| reg.window_label != window_label);
                if instances.is_empty() {
                    self.by_surface.remove(&sid);
                }
            }
        }
    }

    pub fn instance_count(&self, surface_id: &str) -> usize {
        self.by_surface
            .get(surface_id)
            .map(|m| m.len())
            .unwrap_or(0)
    }

    /// Fresh mount of `surface_id` on this window, if one exists.
    /// Window label is the Tauri label, never a model-supplied id.
    pub fn fresh_mount(
        &self,
        window_label: &str,
        surface_id: &str,
    ) -> Option<&SurfaceMountRegistration> {
        let label = normalize_window_label(window_label);
        let now = Instant::now();
        self.by_surface
            .get(surface_id)?
            .values()
            .filter(|reg| reg.window_label == label && mount_is_fresh(reg, now, None))
            .max_by_key(|reg| reg.generation)
    }

    pub fn evaluate_renderer_readiness(
        &self,
        surface_id: &str,
        required_window: Option<&str>,
        min_definition_revision: Option<i64>,
    ) -> RendererReadiness {
        let Some(instances) = self.by_surface.get(surface_id) else {
            return RendererReadiness::NotMounted;
        };
        let candidates: Vec<&SurfaceMountRegistration> = if let Some(want) = required_window {
            let label = normalize_window_label(want);
            instances
                .values()
                .filter(|reg| reg.window_label == label)
                .collect()
        } else {
            instances.values().collect()
        };
        if candidates.is_empty() {
            return RendererReadiness::NotMounted;
        }

        let now = Instant::now();
        let mut best: Option<&SurfaceMountRegistration> = None;
        for reg in candidates {
            let Some(ready_at) = reg.ready_at else {
                continue;
            };
            if now.duration_since(ready_at) > MOUNT_STALE_TTL {
                continue;
            }
            if let Some(min_rev) = min_definition_revision {
                if reg.definition_revision < min_rev {
                    continue;
                }
            }
            best = Some(match best {
                Some(cur) if cur.generation >= reg.generation => cur,
                _ => reg,
            });
        }
        match best {
            Some(reg) => RendererReadiness::Ready {
                window_label: reg.window_label.clone(),
                renderer_instance_id: reg.renderer_instance_id.clone(),
                generation: reg.generation,
            },
            None => RendererReadiness::Stale {
                reason: "no fresh mount registration within TTL or revision mismatch".into(),
            },
        }
    }

    pub fn is_mounted(&self, surface_id: &str) -> bool {
        matches!(
            self.evaluate_renderer_readiness(surface_id, None, None),
            RendererReadiness::Ready { .. }
        )
    }

    /// True when a specific renderer instance is freshly mounted for the surface.
    pub fn has_fresh_instance(&self, surface_id: &str, renderer_instance_id: &str) -> bool {
        let Some(instances) = self.by_surface.get(surface_id) else {
            return false;
        };
        let Some(reg) = instances.get(renderer_instance_id) else {
            return false;
        };
        mount_is_fresh(reg, Instant::now(), None)
    }
}

fn normalize_window_label(label: &str) -> String {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        "main".to_string()
    } else {
        trimmed.to_string()
    }
}

fn mount_is_fresh(
    reg: &SurfaceMountRegistration,
    now: Instant,
    min_definition_revision: Option<i64>,
) -> bool {
    let Some(ready_at) = reg.ready_at else {
        return false;
    };
    if now.duration_since(ready_at) > MOUNT_STALE_TTL {
        return false;
    }
    if let Some(min_rev) = min_definition_revision {
        if reg.definition_revision < min_rev {
            return false;
        }
    }
    true
}

/// Durable target readiness (SQLite) is separate from renderer readiness.
/// Renderer defer reasons are prefixed `renderer:` so promotion does not
/// spend the missing-surface retry budget on an unmounted view.
pub fn classify_operation_readiness(
    durable: crate::runtime_v2::patch_scheduler::TargetReadiness,
    renderer: RendererReadiness,
    requires_live_renderer: bool,
    surface_id: &str,
) -> crate::runtime_v2::patch_scheduler::TargetReadiness {
    use crate::runtime_v2::patch_scheduler::TargetReadiness;
    match durable {
        TargetReadiness::Rejected(msg) => TargetReadiness::Rejected(msg),
        TargetReadiness::Deferred {
            target_surface_id,
            reason,
        } => TargetReadiness::Deferred {
            target_surface_id,
            reason,
        },
        TargetReadiness::Ready if !requires_live_renderer => TargetReadiness::Ready,
        TargetReadiness::Ready => match renderer {
            RendererReadiness::Ready { .. } => TargetReadiness::Ready,
            RendererReadiness::NotMounted => TargetReadiness::Deferred {
                target_surface_id: surface_id.to_string(),
                reason: "renderer: renderer not mounted".into(),
            },
            RendererReadiness::Stale { reason } => TargetReadiness::Deferred {
                target_surface_id: surface_id.to_string(),
                reason: format!("renderer: {reason}"),
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{create_conversation, Database, DEFAULT_WORKSPACE_ID};
    use serde_json::json;
    use tempfile::tempdir;

    fn setup_surface(db: &mut Database) -> (String, String) {
        let conv = create_conversation(db, DEFAULT_WORKSPACE_ID, "Mount Conv", None).unwrap();
        let surf = crate::runtime_v2::surfaces::create_inline_surface(
            db,
            &conv.id,
            None,
            None,
            "Mounted",
            &json!({
                "id": "doc-1",
                "name": "Mounted",
                "layout": "stack",
                "components": [{"id": "t", "type": "text", "props": {"text": "hi"}}]
            }),
            &[],
        )
        .unwrap();
        (conv.id, surf.id)
    }

    #[test]
    fn register_rejects_spoofed_conversation() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("m.db")).unwrap();
        let (conv, sid) = setup_surface(&mut db);
        let mut reg = MountRegistry::new();
        let err = reg
            .register(&db, "main", &sid, "rend-1", None, Some("conv-other"), 1, 0)
            .expect_err("spoofed conversation");
        assert!(err.to_string().contains("conversation identity mismatch"));
        let ok = reg
            .register(&db, "main", &sid, "rend-1", None, Some(&conv), 1, 0)
            .unwrap();
        assert_eq!(ok.surface_id, sid);
        assert!(reg.is_mounted(&sid));
    }

    #[test]
    fn register_rejects_spoofed_application_id() {
        use crate::application_kernel::manifest::{upsert_manifest, ApplicationManifest};
        use std::collections::HashMap;

        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("m-app.db")).unwrap();
        let (_conv, sid) = setup_surface(&mut db);

        upsert_manifest(
            &mut db,
            ApplicationManifest {
                schema_version: "1".into(),
                application_id: "app-real".into(),
                instance_id: "inst-real".into(),
                name: "Real".into(),
                description: String::new(),
                version: 1,
                surfaces: vec![],
                routes: vec![],
                data_models: vec![],
                settings: vec![],
                capabilities: vec!["coreside.core".into()],
                permissions: vec![],
                events: vec![],
                tests: vec![],
                search_keywords: vec![],
                tags: vec![],
                agent_description: String::new(),
                project_id: None,
                conversation_id: None,
                organization_id: None,
                ownership: None,
                application_action_access: vec![],
                surface_action_access: HashMap::new(),
                component_action_access: HashMap::new(),
                action_descriptor_hashes: HashMap::new(),
            },
        )
        .unwrap();
        // surfaces.tool_id FK → tools(id); application_id must also exist as a tool row.
        db.conn()
            .execute(
                "INSERT INTO tools (id, workspace_id, name, description, layout, definition_json, current_version, created_at, updated_at)
                 VALUES ('app-real', ?1, 'Real', '', 'stack', '{}', 1, datetime('now'), datetime('now'))",
                [DEFAULT_WORKSPACE_ID],
            )
            .unwrap();
        crate::runtime_v2::surfaces::bind_surface_tool_id(&mut db, &sid, "app-real").unwrap();

        let mut reg = MountRegistry::new();
        let err = reg
            .register(&db, "main", &sid, "rend-1", Some("app-spoofed"), None, 1, 0)
            .expect_err("spoofed application");
        assert!(
            err.to_string().contains("application identity mismatch"),
            "expected app spoof denial, got {err}"
        );
        assert!(!reg.is_mounted(&sid));

        let ok = reg
            .register(&db, "main", &sid, "rend-1", Some("app-real"), None, 1, 0)
            .unwrap();
        assert_eq!(ok.application_id.as_deref(), Some("app-real"));
        assert!(reg.is_mounted(&sid));
    }

    #[test]
    fn wrong_window_is_not_ready_when_required() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("m2.db")).unwrap();
        let (_conv, sid) = setup_surface(&mut db);
        let mut reg = MountRegistry::new();
        reg.register(&db, "main", &sid, "rend-1", None, None, 1, 0)
            .unwrap();
        assert!(matches!(
            reg.evaluate_renderer_readiness(&sid, Some("tool-other"), None),
            RendererReadiness::NotMounted
        ));
        assert!(matches!(
            reg.evaluate_renderer_readiness(&sid, Some("main"), None),
            RendererReadiness::Ready { .. }
        ));
    }

    #[test]
    fn unregister_and_stale_instance_fail_closed() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("m3.db")).unwrap();
        let (_conv, sid) = setup_surface(&mut db);
        let mut reg = MountRegistry::new();
        reg.register(&db, "main", &sid, "rend-1", None, None, 1, 0)
            .unwrap();
        assert!(!reg.unregister("main", &sid, Some("rend-other")));
        assert!(reg.is_mounted(&sid));
        assert!(reg.unregister("main", &sid, Some("rend-1")));
        assert!(!reg.is_mounted(&sid));
    }

    #[test]
    fn durable_ready_without_renderer_when_not_required() {
        use crate::runtime_v2::patch_scheduler::TargetReadiness;
        let out = classify_operation_readiness(
            TargetReadiness::Ready,
            RendererReadiness::NotMounted,
            false,
            "surf",
        );
        assert_eq!(out, TargetReadiness::Ready);
        let deferred = classify_operation_readiness(
            TargetReadiness::Ready,
            RendererReadiness::NotMounted,
            true,
            "surf",
        );
        assert!(matches!(
            deferred,
            TargetReadiness::Deferred { ref reason, .. } if reason.starts_with("renderer:")
        ));
    }

    #[test]
    fn two_renderers_in_one_window_both_stay_mounted() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("m4.db")).unwrap();
        let (_conv, sid) = setup_surface(&mut db);
        let mut reg = MountRegistry::new();
        reg.register(&db, "main", &sid, "inline", None, None, 1, 0)
            .unwrap();
        reg.register(&db, "main", &sid, "canvas", None, None, 1, 0)
            .unwrap();
        assert_eq!(reg.instance_count(&sid), 2);
        assert!(reg.fresh_mount("main", &sid).is_some());
        assert!(reg.unregister("main", &sid, Some("inline")));
        assert_eq!(reg.instance_count(&sid), 1);
        assert!(reg.is_mounted(&sid));
        reg.clear_window("main");
        assert!(!reg.is_mounted(&sid));
        assert!(reg.fresh_mount("main", &sid).is_none());
    }

    #[test]
    fn second_window_mount_does_not_drop_the_first() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("m5.db")).unwrap();
        let (_conv, sid) = setup_surface(&mut db);
        let mut reg = MountRegistry::new();
        reg.register(&db, "main", &sid, "rend-main", None, None, 1, 0)
            .unwrap();
        reg.register(&db, "tool-app", &sid, "rend-tool", None, None, 1, 0)
            .unwrap();
        assert!(reg.fresh_mount("main", &sid).is_some());
        assert!(reg.fresh_mount("tool-app", &sid).is_some());
        reg.clear_window("tool-app");
        assert!(reg.fresh_mount("main", &sid).is_some());
        assert!(reg.fresh_mount("tool-app", &sid).is_none());
    }

    #[test]
    fn stale_instance_cannot_unregister_the_current_one() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("m6.db")).unwrap();
        let (_conv, sid) = setup_surface(&mut db);
        let mut reg = MountRegistry::new();
        reg.register(&db, "main", &sid, "rend-new", None, None, 2, 0)
            .unwrap();
        assert!(!reg.unregister("tool-other", &sid, Some("rend-new")));
        assert!(reg.is_mounted(&sid));
    }

    #[test]
    fn reregister_same_instance_updates_without_dropping_peer() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("m7.db")).unwrap();
        let (_conv, sid) = setup_surface(&mut db);
        let mut reg = MountRegistry::new();
        let first = reg
            .register(&db, "main", &sid, "inline", None, None, 1, 0)
            .unwrap();
        reg.register(&db, "main", &sid, "canvas", None, None, 1, 0)
            .unwrap();
        assert_eq!(reg.instance_count(&sid), 2);

        let updated = reg
            .register(&db, "main", &sid, "inline", None, None, 5, 3)
            .unwrap();
        assert_eq!(updated.renderer_instance_id, "inline");
        assert_eq!(updated.definition_revision, 5);
        assert_eq!(updated.state_revision, 3);
        assert!(
            updated.generation > first.generation,
            "re-register must advance generation for the same instance"
        );
        assert_eq!(
            reg.instance_count(&sid),
            2,
            "re-registering one instance must not drop a peer on the same surface"
        );
        assert!(reg.is_mounted(&sid));
        let ready = reg.evaluate_renderer_readiness(&sid, Some("main"), None);
        match ready {
            RendererReadiness::Ready {
                renderer_instance_id,
                ..
            } => {
                assert!(
                    renderer_instance_id == "inline" || renderer_instance_id == "canvas",
                    "readiness must still resolve to a live instance"
                );
            }
            other => panic!("expected Ready after peer-preserving re-register, got {other:?}"),
        }
        assert!(reg.unregister("main", &sid, Some("canvas")));
        assert_eq!(reg.instance_count(&sid), 1);
        assert!(reg.is_mounted(&sid));
    }
}
