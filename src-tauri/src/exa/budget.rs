//! Local monthly Exa spend budget (independent of Exa account balance).

use serde::{Deserialize, Serialize};

use super::errors::ExaError;
use super::usage::{month_key_now, sum_month_actual_cost};
use crate::db::{get_setting, set_setting, Database, DbResult};

pub const SETTING_MONTHLY_BUDGET: &str = "exaMonthlyBudgetUsd";
pub const SETTING_SOFT_PERCENT: &str = "exaBudgetSoftPercent";
pub const SETTING_CRITICAL_PERCENT: &str = "exaBudgetCriticalPercent";
pub const SETTING_HARD_PERCENT: &str = "exaBudgetHardPercent";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BudgetThreshold {
    Ok,
    Soft,
    Critical,
    Hard,
}

impl BudgetThreshold {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Soft => "soft",
            Self::Critical => "critical",
            Self::Hard => "hard",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetConfig {
    /// Optional local monthly cap in USD. Empty / None = unlimited.
    pub monthly_budget_usd: Option<f64>,
    pub soft_percent: f64,
    pub critical_percent: f64,
    pub hard_percent: f64,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            monthly_budget_usd: None,
            soft_percent: 75.0,
            critical_percent: 90.0,
            hard_percent: 100.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetStatus {
    pub month_key: String,
    pub spent_usd: f64,
    pub budget_usd: Option<f64>,
    pub remaining_usd: Option<f64>,
    pub percent_used: Option<f64>,
    pub threshold: BudgetThreshold,
    /// Distinct from Exa account credits / HTTP 402.
    pub note: String,
}

pub fn load_budget_config(db: &Database) -> BudgetConfig {
    let mut cfg = BudgetConfig::default();
    if let Ok(Some(raw)) = get_setting(db, SETTING_MONTHLY_BUDGET) {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            if let Ok(v) = trimmed.parse::<f64>() {
                if v > 0.0 {
                    cfg.monthly_budget_usd = Some(v);
                }
            }
        }
    }
    if let Ok(Some(raw)) = get_setting(db, SETTING_SOFT_PERCENT) {
        if let Ok(v) = raw.trim().parse::<f64>() {
            cfg.soft_percent = v;
        }
    }
    if let Ok(Some(raw)) = get_setting(db, SETTING_CRITICAL_PERCENT) {
        if let Ok(v) = raw.trim().parse::<f64>() {
            cfg.critical_percent = v;
        }
    }
    if let Ok(Some(raw)) = get_setting(db, SETTING_HARD_PERCENT) {
        if let Ok(v) = raw.trim().parse::<f64>() {
            cfg.hard_percent = v;
        }
    }
    cfg
}

pub fn save_budget_config(db: &mut Database, cfg: &BudgetConfig) -> DbResult<()> {
    let budget = cfg
        .monthly_budget_usd
        .map(|v| format!("{v}"))
        .unwrap_or_default();
    set_setting(db, SETTING_MONTHLY_BUDGET, &budget)?;
    set_setting(db, SETTING_SOFT_PERCENT, &format!("{}", cfg.soft_percent))?;
    set_setting(
        db,
        SETTING_CRITICAL_PERCENT,
        &format!("{}", cfg.critical_percent),
    )?;
    set_setting(db, SETTING_HARD_PERCENT, &format!("{}", cfg.hard_percent))?;
    Ok(())
}

pub fn classify_threshold(spent: f64, cfg: &BudgetConfig) -> BudgetThreshold {
    let Some(budget) = cfg.monthly_budget_usd.filter(|b| *b > 0.0) else {
        return BudgetThreshold::Ok;
    };
    let pct = (spent / budget) * 100.0;
    if pct >= cfg.hard_percent {
        BudgetThreshold::Hard
    } else if pct >= cfg.critical_percent {
        BudgetThreshold::Critical
    } else if pct >= cfg.soft_percent {
        BudgetThreshold::Soft
    } else {
        BudgetThreshold::Ok
    }
}

pub fn budget_status(db: &Database) -> BudgetStatus {
    let cfg = load_budget_config(db);
    let month_key = month_key_now();
    let spent = sum_month_actual_cost(db, &month_key).unwrap_or(0.0);
    let threshold = classify_threshold(spent, &cfg);
    let percent_used = cfg
        .monthly_budget_usd
        .map(|b| if b <= 0.0 { 0.0 } else { (spent / b) * 100.0 });
    let remaining = cfg.monthly_budget_usd.map(|b| (b - spent).max(0.0));
    BudgetStatus {
        month_key,
        spent_usd: spent,
        budget_usd: cfg.monthly_budget_usd,
        remaining_usd: remaining,
        percent_used,
        threshold,
        note: "Local Coreside monthly cap — not Exa account balance".into(),
    }
}

/// Hard-stop before calling Exa when local budget is exhausted.
pub fn assert_can_spend(db: &Database, estimated_additional: f64) -> Result<(), ExaError> {
    let cfg = load_budget_config(db);
    let Some(budget) = cfg.monthly_budget_usd.filter(|b| *b > 0.0) else {
        return Ok(());
    };
    let month_key = month_key_now();
    let spent = sum_month_actual_cost(db, &month_key).unwrap_or(0.0);
    let projected = spent + estimated_additional.max(0.0);
    let pct = (projected / budget) * 100.0;
    if pct >= cfg.hard_percent {
        return Err(ExaError::LocalBudgetReached);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_classification() {
        let cfg = BudgetConfig {
            monthly_budget_usd: Some(10.0),
            soft_percent: 75.0,
            critical_percent: 90.0,
            hard_percent: 100.0,
        };
        assert_eq!(classify_threshold(0.0, &cfg), BudgetThreshold::Ok);
        assert_eq!(classify_threshold(7.5, &cfg), BudgetThreshold::Soft);
        assert_eq!(classify_threshold(9.0, &cfg), BudgetThreshold::Critical);
        assert_eq!(classify_threshold(10.0, &cfg), BudgetThreshold::Hard);
        assert_eq!(classify_threshold(12.0, &cfg), BudgetThreshold::Hard);
    }

    #[test]
    fn unlimited_when_no_budget() {
        let cfg = BudgetConfig::default();
        assert_eq!(classify_threshold(1000.0, &cfg), BudgetThreshold::Ok);
    }
}
