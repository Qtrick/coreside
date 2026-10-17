//! Central theme-preset capacity policy.
//!
//! Only server-sourced plan IDs raise the Free limit. Catalog stubs and
//! client-claimed Pro status are never authoritative.

/// Free plan maximum saved theme presets.
pub const FREE_THEME_PRESET_LIMIT: u32 = 10;
/// Provisional Personal plan limit (central constant — do not scatter).
pub const PERSONAL_THEME_PRESET_LIMIT: u32 = 25;
/// Provisional Pro plan limit (central constant — do not scatter).
pub const PRO_THEME_PRESET_LIMIT: u32 = 50;

/// Resolve the max preset count for a plan presentation.
///
/// `source` must be `"server"` for anything above Free. Offline / contract
/// defaults always resolve to Free capacity.
pub fn max_presets_for_plan(plan_id: &str, source: &str) -> u32 {
    if source != "server" {
        return FREE_THEME_PRESET_LIMIT;
    }
    match plan_id.trim().to_ascii_lowercase().as_str() {
        "pro" => PRO_THEME_PRESET_LIMIT,
        "personal" => PERSONAL_THEME_PRESET_LIMIT,
        _ => FREE_THEME_PRESET_LIMIT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_server_source_is_always_free() {
        assert_eq!(max_presets_for_plan("pro", "contract_catalog"), 10);
        assert_eq!(max_presets_for_plan("pro", "contract_default"), 10);
        assert_eq!(max_presets_for_plan("pro", ""), 10);
    }

    #[test]
    fn server_pro_and_personal_raise_limit() {
        assert_eq!(max_presets_for_plan("pro", "server"), 50);
        assert_eq!(max_presets_for_plan("personal", "server"), 25);
        assert_eq!(max_presets_for_plan("free", "server"), 10);
        assert_eq!(max_presets_for_plan("unknown", "server"), 10);
    }
}
