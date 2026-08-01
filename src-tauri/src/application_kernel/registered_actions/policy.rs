//! Consumer default policy for registered actions.
//!
//! Inputs are already-verified facts (declared access, granted permission,
//! matched grant); this function only decides *how much consent* the call needs.
//! It is pure so the rules can be tested without a database.

use serde::Serialize;

use super::context::{ActionRunContext, Presence};
use super::descriptor::{ActionDescriptor, ActionRisk};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "decision")]
pub enum ActionPolicyDecision {
    /// Run now.
    Allow { source: &'static str },
    /// Ask the user before running.
    RequireApproval { reason: &'static str },
    /// Refuse; asking would not help right now.
    Block { reason: &'static str },
}

#[derive(Debug, Clone, Copy)]
pub struct PolicyFacts {
    pub declared: bool,
    pub permission_granted: bool,
    pub has_grant: bool,
}

pub fn decide(
    ctx: &ActionRunContext,
    descriptor: &ActionDescriptor,
    facts: PolicyFacts,
) -> ActionPolicyDecision {
    if !facts.declared {
        return ActionPolicyDecision::Block {
            reason: "This application did not declare this action.",
        };
    }
    if !facts.permission_granted {
        return ActionPolicyDecision::Block {
            reason: "This application does not have permission for this capability yet.",
        };
    }

    let away = ctx.presence == Presence::Away;

    // Destructive and critical never ride on a standing grant.
    if descriptor.risk == ActionRisk::Destructive {
        return if away {
            ActionPolicyDecision::Block {
                reason: "Deletions are never run while you are away.",
            }
        } else {
            ActionPolicyDecision::RequireApproval {
                reason: "This permanently deletes data, so it always needs your confirmation.",
            }
        };
    }
    if descriptor.critical {
        return if away {
            ActionPolicyDecision::Block {
                reason: "This action always needs you present.",
            }
        } else {
            ActionPolicyDecision::RequireApproval {
                reason: "This action always needs your confirmation.",
            }
        };
    }

    if descriptor.risk == ActionRisk::Read {
        return ActionPolicyDecision::Allow { source: "policy" };
    }

    // Writes.
    if facts.has_grant {
        return ActionPolicyDecision::Allow { source: "grant" };
    }
    if away {
        // Away writes need remembered, application-bound authority, which
        // `grants::match_grant` already refused above.
        return ActionPolicyDecision::RequireApproval {
            reason: "This change was requested while you were away and is waiting for you.",
        };
    }
    ActionPolicyDecision::RequireApproval {
        reason: "This changes your data, so it needs your confirmation.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application_kernel::registered_actions::context::Venue;
    use crate::application_kernel::registered_actions::descriptor::find_action;

    fn ctx(presence: Presence) -> ActionRunContext {
        ActionRunContext {
            actor: "user".into(),
            venue: Venue::Application,
            presence,
            application_id: Some("app-1".into()),
            project_id: None,
            conversation_id: None,
            session_id: "session-test".into(),
            run_id: "run-test".into(),
            trigger: None,
            surface_id: None,
            component_id: None,
            depth: 0,
        }
    }

    fn facts(has_grant: bool) -> PolicyFacts {
        PolicyFacts {
            declared: true,
            permission_granted: true,
            has_grant,
        }
    }

    fn is_allow(d: &ActionPolicyDecision) -> bool {
        matches!(d, ActionPolicyDecision::Allow { .. })
    }
    fn is_ask(d: &ActionPolicyDecision) -> bool {
        matches!(d, ActionPolicyDecision::RequireApproval { .. })
    }
    fn is_block(d: &ActionPolicyDecision) -> bool {
        matches!(d, ActionPolicyDecision::Block { .. })
    }

    #[test]
    fn present_read_runs_automatically() {
        let d = find_action("local_data.query").unwrap();
        assert!(is_allow(&decide(&ctx(Presence::Present), d, facts(false))));
    }

    #[test]
    fn present_write_asks_without_grant_and_runs_with_one() {
        let d = find_action("local_data.write").unwrap();
        assert!(is_ask(&decide(&ctx(Presence::Present), d, facts(false))));
        assert!(is_allow(&decide(&ctx(Presence::Present), d, facts(true))));
    }

    #[test]
    fn present_destructive_always_asks_even_with_a_grant() {
        let d = find_action("local_data.delete").unwrap();
        assert!(is_ask(&decide(&ctx(Presence::Present), d, facts(true))));
    }

    #[test]
    fn critical_always_asks_even_with_a_grant() {
        let d = find_action("export.prepare").unwrap();
        assert!(is_ask(&decide(&ctx(Presence::Present), d, facts(true))));
    }

    #[test]
    fn away_write_needs_remembered_authority_else_parks() {
        let d = find_action("local_data.write").unwrap();
        assert!(is_allow(&decide(&ctx(Presence::Away), d, facts(true))));
        assert!(is_ask(&decide(&ctx(Presence::Away), d, facts(false))));
    }

    #[test]
    fn away_destructive_and_critical_are_blocked() {
        assert!(is_block(&decide(
            &ctx(Presence::Away),
            find_action("local_data.delete").unwrap(),
            facts(true)
        )));
        assert!(is_block(&decide(
            &ctx(Presence::Away),
            find_action("export.prepare").unwrap(),
            facts(true)
        )));
    }

    #[test]
    fn undeclared_or_ungranted_is_blocked() {
        let d = find_action("local_data.query").unwrap();
        assert!(is_block(&decide(
            &ctx(Presence::Present),
            d,
            PolicyFacts {
                declared: false,
                permission_granted: true,
                has_grant: false
            }
        )));
        assert!(is_block(&decide(
            &ctx(Presence::Present),
            d,
            PolicyFacts {
                declared: true,
                permission_granted: false,
                has_grant: false
            }
        )));
    }
}
