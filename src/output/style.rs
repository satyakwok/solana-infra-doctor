//! Presentation vocabulary and formatting helpers shared by the human
//! renderers. This module is **presentation only** — it never computes a
//! verdict, a score, or any diagnostic result; it just decides how existing
//! results are worded, colored, and laid out for a terminal reader.

use crate::color::Palette;

/// A per-check (or per-category) status word used in human output.
///
/// This is a display vocabulary, distinct from the overall
/// [`Verdict`](crate::verdict::Verdict) (`GOOD`/`WARNING`/`BAD`/`UNKNOWN`):
///
/// - [`Status::Pass`] — the check (or every check in a category) succeeded.
/// - [`Status::Warn`] — only non-critical checks failed; the endpoint is usable
///   but degraded.
/// - [`Status::Fail`] — a critical check failed.
/// - [`Status::Skip`] — the check was not run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The check, or every check in a category, succeeded.
    Pass,
    /// Only non-critical checks failed; usable but degraded.
    Warn,
    /// A critical check failed.
    Fail,
    /// The check was not run.
    Skip,
}

impl Status {
    /// The uppercase status word (`PASS`, `WARN`, `FAIL`, `SKIP`).
    pub fn label(self) -> &'static str {
        match self {
            Status::Pass => "PASS",
            Status::Warn => "WARN",
            Status::Fail => "FAIL",
            Status::Skip => "SKIP",
        }
    }

    /// The status word colored for a terminal: green `PASS`, amber `WARN`, red
    /// `FAIL`, dim `SKIP`. A disabled palette returns the plain word.
    pub fn paint(self, palette: Palette) -> String {
        match self {
            Status::Pass => palette.ok(self.label()),
            Status::Warn => palette.warn(self.label()),
            Status::Fail => palette.fail(self.label()),
            Status::Skip => palette.dim(self.label()),
        }
    }
}

/// Format a millisecond duration with a space between value and unit: `17 ms`.
///
/// # Examples
///
/// ```
/// use solana_infra_doctor::output::style::millis;
/// assert_eq!(millis(17), "17 ms");
/// ```
pub fn millis(value: u128) -> String {
    format!("{value} ms")
}

/// A safe, compact endpoint label for default output: the hostname of an
/// already-redacted URL (so credentials never appear), falling back to the
/// input string if it cannot be parsed.
///
/// # Examples
///
/// ```
/// use solana_infra_doctor::output::style::endpoint_label;
/// assert_eq!(
///     endpoint_label("https://api.mainnet-beta.solana.com/"),
///     "api.mainnet-beta.solana.com"
/// );
/// ```
pub fn endpoint_label(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_string))
        .unwrap_or_else(|| url.to_string())
}
