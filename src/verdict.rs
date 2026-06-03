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
