use serde::{Deserialize, Serialize};

/// Consumer desktop resource profiles. Hard ceilings live in the Python sidecar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceProfile {
    Eco,
    Balanced,
    Performance,
}

impl ResourceProfile {
    pub const DEFAULT: Self = Self::Balanced;

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Eco => "eco",
            Self::Balanced => "balanced",
            Self::Performance => "performance",
        }
    }

    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_lowercase().as_str() {
            "eco" => Self::Eco,
            "performance" => Self::Performance,
            _ => Self::Balanced,
        }
    }

    /// Soft guidance for UI — Python enforces real ceilings.
    pub fn concurrency_hint(self) -> u32 {
        match self {
            Self::Eco => 1,
            Self::Balanced => 2,
            Self::Performance => 3,
        }
    }

    pub fn pages_per_request_hint(self) -> u32 {
        match self {
            Self::Eco => 1,
            Self::Balanced => 2,
            Self::Performance => 4,
        }
    }
}

impl Default for ResourceProfile {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl std::fmt::Display for ResourceProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_profiles() {
        assert_eq!(ResourceProfile::parse("eco"), ResourceProfile::Eco);
        assert_eq!(ResourceProfile::parse("PERFORMANCE"), ResourceProfile::Performance);
        assert_eq!(ResourceProfile::parse("nope"), ResourceProfile::Balanced);
    }
}
