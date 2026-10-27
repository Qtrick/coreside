//! Local periodic automations (run only while Coreside is open).

mod ai_exec;
mod executor;
mod models;
mod outcome;
mod scheduler;
mod validation;

#[allow(unused_imports)] // Public automation API surface.
pub use executor::execute_automation;
pub use models::{
    parse_trigger_timezone, Automation, AutomationAction, AutomationRun, AutomationTrigger,
    MissedRunPolicy,
};
pub use outcome::AutomationRunOutcome;
pub use scheduler::{
    claim_run, compute_next_run, run_now, spawn_scheduler, RunOrigin, SchedulerHandle,
};
#[allow(unused_imports)] // Re-exported for callers / docs.
pub use validation::{validate_automation, MIN_AI_INTERVAL_MINUTES, MIN_INTERVAL_MINUTES};
