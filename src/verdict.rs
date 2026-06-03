use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Verdict {
    Good,
    Warning,
    Bad,
    Unknown,
}

impl Verdict {
    pub fn exit_code(self) -> i32 {
        match self {
            Self::Good => 0,
            Self::Warning => 1,
            Self::Bad => 2,
            Self::Unknown => 3,
        }
    }
}

impl std::fmt::Display for Verdict {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Good => "GOOD",
            Self::Warning => "WARNING",
            Self::Bad => "BAD",
            Self::Unknown => "UNKNOWN",
        };
        formatter.write_str(value)
    }
}

#[cfg(all(test, not(coverage)))]
mod tests {
    use super::*;

    #[test]
    fn maps_verdicts_to_exit_codes() {
        assert_eq!(Verdict::Good.exit_code(), 0);
        assert_eq!(Verdict::Warning.exit_code(), 1);
        assert_eq!(Verdict::Bad.exit_code(), 2);
        assert_eq!(Verdict::Unknown.exit_code(), 3);
    }

    #[test]
    fn displays_uppercase_verdicts() {
        assert_eq!(Verdict::Good.to_string(), "GOOD");
        assert_eq!(Verdict::Warning.to_string(), "WARNING");
        assert_eq!(Verdict::Bad.to_string(), "BAD");
        assert_eq!(Verdict::Unknown.to_string(), "UNKNOWN");
    }
}
