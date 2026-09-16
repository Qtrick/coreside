//! Registered action runtime — bounded, auditable execution for generated
//! applications.
//!
//! Terminology: a `RegisteredAction` is an executable capability described by an
//! `ActionDescriptor`. It is deliberately not called a "Tool", because in
//! Coreside a Tool is a user-created personal application.
//!
//! Runtime action risk (`read` / `write` / `destructive`) is a separate axis
//! from Application Kernel operation risk (`automatic` / `lightweight` /
//! `strong`). Kernel risk classifies durable transactions; action risk
//! classifies runtime calls.
//!
//! Attribution: the consent model implemented here — descriptor hashing,
//! remembered grants with scope and duration, single-use approvals, and
//! presence-aware policy — is a conceptual reimplementation inspired by Vendo
//! (Apache License 2.0, <https://github.com/vendo-ai/vendo>). No Vendo code was
//! copied; the storage model, policy defaults, and enforcement boundaries are
//! Coreside's own and live in trusted Rust.

pub mod approvals;
pub mod audit;
pub mod breakers;
pub mod canonical;
pub mod context;
pub mod descriptor;
pub mod gateway;
pub mod grants;
pub mod handlers;
pub mod policy;
pub mod schema;

#[cfg(test)]
pub mod testing;

// Public surface of the runtime; some entries are used only by tests or by
// callers added in later phases.
#[allow(unused_imports)]
pub use context::{ActionRunContext, ClientActionRequest, Presence, Venue};
#[allow(unused_imports)]
pub use descriptor::{
    action_names, catalog_json, find_action, registered_actions_catalog_markdown,
    ActionDescriptor, ActionRisk, BUNDLED_ACTIONS,
};
#[allow(unused_imports)]
pub use gateway::{
    execute_registered_action, execute_registered_action_trusted, generated_execution_allowed,
    ActionOutcome,
};
#[allow(unused_imports)]
pub use grants::{GrantDuration, GrantScope, RuntimeGrant};
