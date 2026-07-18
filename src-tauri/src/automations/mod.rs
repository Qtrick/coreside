//! Local periodic automations (run only while Coreside is open).

mod executor;
mod models;
mod scheduler;
mod validation;

#[allow(unused_imports)] // Public automation API surface.
pub use executor::execute_automation;
pub use models::{Automation, AutomationAction, AutomationRun, AutomationTrigger, MissedRunPolicy};
pub use scheduler::{compute_next_run, run_now, spawn_scheduler, SchedulerHandle};
#[allow(unused_imports)] // Re-exported for callers / docs.
pub use validation::{validate_automation, MIN_AI_INTERVAL_MINUTES, MIN_INTERVAL_MINUTES};
