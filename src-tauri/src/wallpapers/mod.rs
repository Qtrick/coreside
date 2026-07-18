//! Live wallpaper schemas: validated JSON, local media asset references only.

mod models;
mod registry;
mod validation;

pub use validation::validate_wallpaper_config;
