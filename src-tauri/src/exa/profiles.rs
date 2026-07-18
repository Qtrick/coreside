//! Research spend profiles: Saver / Balanced / Thorough.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SearchProfile {
    #[default]
    Saver,
    Balanced,
    Thorough,
}

impl SearchProfile {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_lowercase().as_str() {
            "balanced" => Self::Balanced,
            "thorough" => Self::Thorough,
            _ => Self::Saver,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Saver => "saver",
            Self::Balanced => "balanced",
            Self::Thorough => "thorough",
        }
    }

    /// Exa Search `type`. Never defaults to deep-reasoning.
    pub fn search_type(self) -> &'static str {
        match self {
            Self::Saver => "fast",
            Self::Balanced => "auto",
            // Thorough may use deep-lite only when budget allows; caller gates deeper modes.
            Self::Thorough => "auto",
        }
    }

    /// Thorough deep mode when policy + budget explicitly allow (not silent default).
    pub fn thorough_deep_type(self) -> Option<&'static str> {
        match self {
            Self::Thorough => Some("deep-lite"),
            _ => None,
        }
    }

    pub fn num_results(self) -> usize {
        match self {
            Self::Saver => 3,
            Self::Balanced => 5,
            Self::Thorough => 8,
        }
    }

    pub fn max_crawl_pages(self) -> usize {
        match self {
            Self::Saver => 2,
            Self::Balanced => 3,
            Self::Thorough => 5,
        }
    }

    pub fn max_refinements(self) -> usize {
        match self {
            Self::Saver => 0,
            Self::Balanced => 1,
            Self::Thorough => 2,
        }
    }

    /// Downgrade automatic search aggressiveness when local budget thresholds are crossed.
    /// Soft: prefer cheaper discovery (Thorough→Balanced); refinements still restricted via
    /// [`Self::max_refinements_for_threshold`]. Critical/Hard force Saver (Hard also blocked by
    /// `assert_can_spend`).
    pub fn effective_for_threshold(self, threshold: crate::exa::BudgetThreshold) -> Self {
        use crate::exa::BudgetThreshold;
        match threshold {
            BudgetThreshold::Ok => self,
            BudgetThreshold::Soft => match self {
                Self::Thorough => Self::Balanced,
                other => other,
            },
            BudgetThreshold::Critical | BudgetThreshold::Hard => Self::Saver,
        }
    }

    /// Soft+ budgets disable automatic refinements (SEARCH_USAGE soft/critical policy).
    pub fn max_refinements_for_threshold(self, threshold: crate::exa::BudgetThreshold) -> usize {
        use crate::exa::BudgetThreshold;
        match threshold {
            BudgetThreshold::Ok => self.max_refinements(),
            BudgetThreshold::Soft | BudgetThreshold::Critical | BudgetThreshold::Hard => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saver_defaults() {
        let p = SearchProfile::Saver;
        assert_eq!(p.search_type(), "fast");
        assert_eq!(p.num_results(), 3);
        assert_eq!(p.max_crawl_pages(), 2);
        assert_eq!(p.max_refinements(), 0);
        assert!(p.thorough_deep_type().is_none());
    }

    #[test]
    fn balanced_and_thorough() {
        assert_eq!(SearchProfile::Balanced.search_type(), "auto");
        assert_eq!(SearchProfile::Balanced.num_results(), 5);
        assert_eq!(SearchProfile::Balanced.max_crawl_pages(), 3);
        assert_eq!(SearchProfile::Thorough.num_results(), 8);
        assert_eq!(SearchProfile::Thorough.max_crawl_pages(), 5);
        assert_eq!(SearchProfile::Thorough.thorough_deep_type(), Some("deep-lite"));
    }

    #[test]
    fn parse_falls_back_to_saver() {
        assert_eq!(SearchProfile::parse(""), SearchProfile::Saver);
        assert_eq!(SearchProfile::parse("SAVER"), SearchProfile::Saver);
        assert_eq!(SearchProfile::parse("balanced"), SearchProfile::Balanced);
    }

    #[test]
    fn soft_steps_down_thorough_not_saver() {
        use crate::exa::BudgetThreshold;
        assert_eq!(
            SearchProfile::Thorough.effective_for_threshold(BudgetThreshold::Soft),
            SearchProfile::Balanced
        );
        assert_eq!(
            SearchProfile::Balanced.effective_for_threshold(BudgetThreshold::Soft),
            SearchProfile::Balanced
        );
        assert_eq!(
            SearchProfile::Balanced.max_refinements_for_threshold(BudgetThreshold::Soft),
            0
        );
        assert_eq!(
            SearchProfile::Balanced.max_refinements_for_threshold(BudgetThreshold::Ok),
            1
        );
    }

    #[test]
    fn critical_and_hard_force_saver() {
        use crate::exa::BudgetThreshold;
        assert_eq!(
            SearchProfile::Thorough.effective_for_threshold(BudgetThreshold::Critical),
            SearchProfile::Saver
        );
        assert_eq!(
            SearchProfile::Balanced.effective_for_threshold(BudgetThreshold::Hard),
            SearchProfile::Saver
        );
        assert_eq!(
            SearchProfile::Balanced.effective_for_threshold(BudgetThreshold::Ok),
            SearchProfile::Balanced
        );
    }
}
