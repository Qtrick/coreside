//! Reserved core resource IDs that AI and user tools must never mutate.

/// All protected resource identifiers.
pub const PROTECTED_IDS: &[&str] = &[
    "core.branding",
    "core.branding.in_app_logo",
    "core.branding.dock_icon",
    "core.settings",
    "core.settings.appearance",
    "core.settings.ai",
    "core.settings.ai.providers",
    "core.settings.ai.credentials",
    "core.settings.agent.action_log",
    "core.settings.data",
    "core.settings.accessibility",
    "core.settings.about",
    "core.navigation",
    "core.security",
    "core.database",
    "core.versioning",
    "core.credentials",
    "core.automations.scheduler",
    "core.exports",
    "core.provider_registry",
    "core.projects",
    "core.projects.index",
    "core.projects.summaries",
    "core.projects.retrieval",
    "core.search",
    "core.search.sessions",
    "core.search.providers",
    "core.search.exa",
    "core.search.exa.credentials",
    "core.search.exa.client",
    "core.search.exa.usage",
    "core.search.exa.budget",
    "core.search.exa.mode_policy",
    "core.search.exa.result_limits",
    "core.search.exa.query_cache",
    "core.search.exa.cost_accounting",
    "core.research.orchestrator",
    "core.research.crawler_handoff",
    "core.settings.search_profile",
    "core.readability.engine",
    "core.readability.semantic_tokens",
    "core.readability.minimum_contrast",
    "core.wallpapers.renderer",
    "core.wallpapers.security",
    "core.wallpapers.motion_limits",
    "core.crawler.runtime",
    "core.crawler.sidecar",
    "core.crawler.protocol",
    "core.crawler.resource_limits",
    "core.crawler.robots",
    "core.crawler.compliance",
    "core.crawler.cache_policy",
    "core.crawler.cleanup",
    "core.crawler.domain_limits",
    "core.crawler.browser_config",
    "core.crawler.installation",
    "core.research.capability_registry",
    "core.media",
    "core.media.assets",
    "core.media.validation",
    "core.wallpaper",
    "core.wallpaper.renderer",
    "core.agent.tool_loop",
    "core.navigation.chat_router",
    "core.application_kernel",
    "core.application_manifest.schema",
    "core.change_compiler",
    "core.change_impact",
    "core.permission_engine",
    "core.policy_engine",
    "core.generated_data.compiler",
    "core.generated_data.migrations",
    "core.application_testing",
    "core.visual_verification",
    "core.recovery_mode",
    "core.last_known_good",
    "core.multiwindow_sync",
    "core.conflict_resolution",
    "core.performance_limits",
    "core.package_validator",
    "core.package_trust",
    "core.enterprise_policy_hook",
    "core.lifecycle_manager",
    "core.garbage_collection",
    "core.preservation.engine",
    "core.patch.scheduler",
    "core.drafts.engine",
    "core.app_routes.engine",
    "core.context.ledger",
    "core.provider.conformance",
    "core.continuity.engine",
    "core.manual_edit.provenance",
    "core.window_orchestrator",
    "core.window_bounds_policy",
    "core.adaptive_window_sizing",
    "core.window_animation",
    "core.monitor_bounds",
    "core.viewport_contract",
    "core.layout_modes",
];

/// Returns `true` if `id` is an exact protected resource or starts with `core.`.
pub fn is_protected(id: &str) -> bool {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.starts_with("core.") {
        return true;
    }
    PROTECTED_IDS.contains(&trimmed)
}

/// Err when `id` is protected; Ok otherwise.
pub fn assert_not_protected(id: &str) -> Result<(), String> {
    if is_protected(id) {
        Err(format!("Protected core resource cannot be modified: {id}"))
    } else {
        Ok(())
    }
}

/// All protected IDs as an owned list (for IPC / diagnostics).
pub fn list_protected_ids() -> Vec<&'static str> {
    PROTECTED_IDS.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_exact_protected_ids() {
        assert!(is_protected("core.branding"));
        assert!(is_protected("core.branding.in_app_logo"));
        assert!(is_protected("core.branding.dock_icon"));
        assert!(is_protected("core.settings.appearance"));
        assert!(is_protected("core.settings.ai"));
        assert!(is_protected("core.navigation"));
        assert!(is_protected("core.security"));
        assert!(assert_not_protected("core.settings").is_err());
    }

    #[test]
    fn detects_core_prefix() {
        assert!(is_protected("core.anything.custom"));
        assert!(is_protected("core."));
    }

    #[test]
    fn allows_user_tool_ids() {
        assert!(!is_protected("water-tracker"));
        assert!(!is_protected("tool.quiz"));
        assert!(!is_protected("my.settings.theme"));
        assert!(!is_protected("added.setting.foo"));
        assert!(assert_not_protected("water-tracker").is_ok());
    }

    #[test]
    fn list_contains_branding_and_settings() {
        let ids = list_protected_ids();
        assert!(ids.contains(&"core.branding"));
        assert!(ids.contains(&"core.settings.about"));
        assert!(ids.contains(&"core.versioning"));
        assert!(ids.contains(&"core.settings.ai.providers"));
        assert!(ids.contains(&"core.settings.ai.credentials"));
        assert!(ids.contains(&"core.settings.agent.action_log"));
        assert!(ids.contains(&"core.credentials"));
        assert!(ids.contains(&"core.provider_registry"));
        assert!(ids.contains(&"core.projects"));
        assert!(ids.contains(&"core.search"));
        assert!(ids.contains(&"core.search.exa"));
        assert!(ids.contains(&"core.search.exa.credentials"));
        assert!(ids.contains(&"core.search.exa.budget"));
        assert!(ids.contains(&"core.search.exa.query_cache"));
        assert!(ids.contains(&"core.research.orchestrator"));
        assert!(ids.contains(&"core.research.crawler_handoff"));
        assert!(ids.contains(&"core.readability.engine"));
        assert!(ids.contains(&"core.wallpapers.renderer"));
        assert!(ids.contains(&"core.settings.search_profile"));
        assert!(ids.contains(&"core.crawler.runtime"));
        assert!(ids.contains(&"core.crawler.installation"));
        assert!(ids.contains(&"core.research.capability_registry"));
        assert!(ids.contains(&"core.media"));
        assert!(ids.contains(&"core.wallpaper"));
        assert!(ids.contains(&"core.agent.tool_loop"));
        assert!(ids.contains(&"core.navigation.chat_router"));
        assert!(ids.contains(&"core.application_kernel"));
        assert!(ids.contains(&"core.recovery_mode"));
        assert!(ids.contains(&"core.package_validator"));
        assert!(ids.contains(&"core.enterprise_policy_hook"));
        assert!(ids.contains(&"core.preservation.engine"));
        assert!(ids.contains(&"core.patch.scheduler"));
        assert!(ids.contains(&"core.continuity.engine"));
        assert!(ids.contains(&"core.window_orchestrator"));
        assert!(ids.contains(&"core.adaptive_window_sizing"));
    }
}
