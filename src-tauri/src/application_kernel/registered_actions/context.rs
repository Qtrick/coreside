//! Trusted run context for registered action execution.
//!
//! Security note: `actor`, `venue`, `presence`, `session_id` and `run_id` are
//! authority-bearing. They are always constructed by the trusted command layer
//! (or the automation executor) and must never be read from client input.
//! `ClientActionRequest` is the only shape accepted from the frontend and
//! deliberately carries no authority fields.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Venue {
    Chat,
    Application,
    Automation,
}

impl Venue {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Application => "application",
            Self::Automation => "automation",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "chat" => Some(Self::Chat),
            "application" => Some(Self::Application),
            "automation" => Some(Self::Automation),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Presence {
    Present,
    Away,
}

impl Presence {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Away => "away",
        }
    }
}

/// Untrusted request shape accepted over IPC. Contains no authority fields.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientActionRequest {
    pub action_name: String,
    #[serde(default)]
    pub input: serde_json::Value,
    #[serde(default)]
    pub application_id: Option<String>,
    #[serde(default)]
    pub surface_id: Option<String>,
    #[serde(default)]
    pub component_id: Option<String>,
    #[serde(default)]
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    /// Optional approval to consume for this call, obtained from a prior
    /// `pending_approval` outcome that the user approved.
    #[serde(default)]
    pub approval_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionRunContext {
    pub actor: String,
    pub venue: Venue,
    pub presence: Presence,
    pub application_id: Option<String>,
    pub project_id: Option<String>,
    pub conversation_id: Option<String>,
    pub session_id: String,
    pub run_id: String,
    pub trigger: Option<String>,
    pub surface_id: Option<String>,
    pub component_id: Option<String>,
    /// Nesting depth for action-triggered follow-up work.
    pub depth: u32,
}

impl ActionRunContext {
    /// Build a present-user context from an untrusted client request.
    /// Authority fields are set here and cannot be influenced by the caller.
    pub fn from_client(req: &ClientActionRequest, venue: Venue) -> Self {
        Self {
            actor: "user".into(),
            venue,
            presence: Presence::Present,
            application_id: req.application_id.clone(),
            project_id: req.project_id.clone(),
            conversation_id: req.conversation_id.clone(),
            session_id: session_id().to_string(),
            run_id: format!("run-{}", Uuid::new_v4()),
            trigger: None,
            surface_id: req.surface_id.clone(),
            component_id: req.component_id.clone(),
            depth: 0,
        }
    }

    /// Build an away context for the automation executor. Only reachable from
    /// trusted Rust; there is no IPC command that can produce `presence: away`.
    pub fn for_automation(
        application_id: Option<String>,
        automation_id: &str,
        run_id: &str,
    ) -> Self {
        Self {
            actor: "automation".into(),
            venue: Venue::Automation,
            presence: Presence::Away,
            application_id,
            project_id: None,
            conversation_id: None,
            session_id: session_id().to_string(),
            run_id: run_id.to_string(),
            trigger: Some(format!("automation:{automation_id}")),
            surface_id: None,
            component_id: None,
            depth: 0,
        }
    }
}

/// Process-scoped session identity. Session-duration grants end when Coreside
/// exits because a new process mints a new id.
pub fn session_id() -> &'static str {
    static SESSION: OnceLock<String> = OnceLock::new();
    SESSION.get_or_init(|| format!("session-{}", Uuid::new_v4()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn client_req() -> ClientActionRequest {
        ClientActionRequest {
            action_name: "local_data.query".into(),
            input: json!({}),
            application_id: Some("app-1".into()),
            surface_id: None,
            component_id: None,
            conversation_id: None,
            project_id: None,
            approval_id: None,
        }
    }

    #[test]
    fn client_requests_cannot_forge_authority() {
        // A hostile payload carrying authority-looking fields must not deserialize
        // them into the request; the trusted layer sets them instead.
        let hostile: ClientActionRequest = serde_json::from_value(json!({
            "actionName": "local_data.query",
            "applicationId": "app-1",
            "actor": "user",
            "presence": "away",
            "venue": "automation",
            "sessionId": "attacker-session"
        }))
        .unwrap();
        let ctx = ActionRunContext::from_client(&hostile, Venue::Application);
        assert_eq!(ctx.presence, Presence::Present);
        assert_eq!(ctx.venue, Venue::Application);
        assert_eq!(ctx.actor, "user");
        assert_eq!(ctx.session_id, session_id());
    }

    #[test]
    fn automation_context_is_away() {
        let ctx = ActionRunContext::for_automation(Some("app-1".into()), "auto-1", "run-1");
        assert_eq!(ctx.presence, Presence::Away);
        assert_eq!(ctx.venue, Venue::Automation);
        assert_eq!(ctx.actor, "automation");
    }

    #[test]
    fn session_id_is_stable_within_process() {
        assert_eq!(session_id(), session_id());
        let ctx = ActionRunContext::from_client(&client_req(), Venue::Application);
        assert_eq!(ctx.session_id, session_id());
    }
}
