//! Cohesive, dependency-free ANSI styling for human terminal output.
//!
//! The palette is **semantic, not decorative**: color encodes meaning (status,
//! hierarchy) and is used with restraint. Secondary labels are muted, headings
//! are bold, the product title carries a calm accent, and verdict / OK / FAIL
//! markers use a small professional truecolor set (GitHub-dark inspired).
//!
//! The CLI decides whether color is enabled (TTY detection, `--color`, and
//! `NO_COLOR`). When a [`Palette`] is disabled every helper returns the plain
//! text unchanged, so non-TTY / piped / `--json` output stays byte-identical to
//! the uncolored output.

use crate::verdict::Verdict;

// 24-bit truecolor SGR foreground fragments. A small, deliberately restrained
// palette: one accent, one muted, and three status hues. Tuned for legibility
// on dark terminals while staying readable on light ones.
const ACCENT: &str = "38;2;88;166;255"; // calm azure — product identity, not neon
const MUTED: &str = "38;2;139;148;158"; // gray — secondary labels, dividers, detail
const GOOD: &str = "38;2;63;185;80"; // green — healthy / OK
const WARNING: &str = "38;2;210;153;34"; // amber — degraded / caution
const BAD: &str = "38;2;248;81;73"; // red — failing / FAIL

/// A styling palette. When `enabled` is false, all helpers are no-ops and return
/// the input text unchanged.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    enabled: bool,
}

impl Palette {
    /// Construct a palette that is on (`true`) or a no-op (`false`).
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    /// Resolve whether color should be applied.
    ///
    /// In `auto` mode color is enabled only on a real terminal that has not
    /// opted out: `NO_COLOR` must be unset, `TERM` must not be `dumb`, and the
    /// output must not be machine-readable JSON. `always` forces color for any
    /// non-JSON output (useful for capturing screenshots), and `never` disables
    /// it entirely. JSON is never colored regardless of choice.
    ///
    /// # Examples
    ///
    /// ```
    /// use solana_infra_doctor::color::{ColorChoice, Palette};
    ///
    /// // auto: a real terminal with no opt-out gets color
    /// assert!(Palette::resolve(ColorChoice::Auto, true, false, false, false).enabled());
    /// // auto: piped output (not a terminal) stays plain
    /// assert!(!Palette::resolve(ColorChoice::Auto, false, false, false, false).enabled());
    /// // JSON is never colored
    /// assert!(!Palette::resolve(ColorChoice::Always, true, false, false, true).enabled());
    /// ```
    pub fn resolve(
        choice: ColorChoice,
        stdout_is_terminal: bool,
        no_color: bool,
        term_dumb: bool,
        json: bool,
    ) -> Self {
        let enabled = match choice {
            ColorChoice::Always => !json,
            ColorChoice::Never => false,
            ColorChoice::Auto => stdout_is_terminal && !no_color && !term_dumb && !json,
        };
        Self { enabled }
    }

    /// Whether this palette emits ANSI codes.
    pub fn enabled(self) -> bool {
        self.enabled
    }

    /// Wrap `text` in the given SGR parameter string (e.g. `"1"`, `"38;2;…"`),
    /// resetting afterwards. A no-op when the palette is disabled.
    fn sgr(self, params: &str, text: &str) -> String {
        if self.enabled {
            format!("\x1b[{params}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    /// Product title / brand moment: bold accent.
    pub fn title(self, text: &str) -> String {
        self.sgr(&format!("1;{ACCENT}"), text)
    }

    /// Section heading: bold, no color.
    pub fn heading(self, text: &str) -> String {
        self.sgr("1", text)
    }

    /// Secondary label / divider / supporting detail: muted gray.
    pub fn label(self, text: &str) -> String {
        self.sgr(MUTED, text)
    }

    /// Bold text.
    pub fn bold(self, text: &str) -> String {
        self.sgr("1", text)
    }

    /// Dimmed (muted gray) text.
    pub fn dim(self, text: &str) -> String {
        self.sgr(MUTED, text)
    }

    /// Color a verdict (bold): GOOD green, WARNING amber, BAD red, UNKNOWN muted.
    pub fn verdict(self, verdict: Verdict) -> String {
        let color = match verdict {
            Verdict::Good => GOOD,
            Verdict::Warning => WARNING,
            Verdict::Bad => BAD,
            Verdict::Unknown => MUTED,
        };
        self.sgr(&format!("1;{color}"), &verdict.to_string())
    }

    /// Success marker (e.g. `PASS`): bold green.
    pub fn ok(self, text: &str) -> String {
        self.sgr(&format!("1;{GOOD}"), text)
    }

    /// Caution marker (e.g. `WARN`): bold amber.
    pub fn warn(self, text: &str) -> String {
        self.sgr(&format!("1;{WARNING}"), text)
    }

    /// Failure marker (e.g. `FAIL`): bold red.
    pub fn fail(self, text: &str) -> String {
        self.sgr(&format!("1;{BAD}"), text)
    }

    /// Affirmative inline value (e.g. `yes`): green.
    pub fn good(self, text: &str) -> String {
        self.sgr(GOOD, text)
    }

    /// Negative inline value (e.g. `no`): red.
    pub fn bad(self, text: &str) -> String {
        self.sgr(BAD, text)
    }
}

/// When to apply color, mirroring the common `--color` convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ColorChoice {
    /// Color only on a terminal that has not opted out (the default).
    Auto,
    /// Always color non-JSON output.
    Always,
    /// Never emit color.
    Never,
}

#[cfg(all(test, not(coverage)))]
mod tests {
    use super::*;

    #[test]
    fn disabled_palette_is_a_no_op() {
        let p = Palette::new(false);
        assert_eq!(p.verdict(Verdict::Good), "GOOD");
        assert_eq!(p.title("x"), "x");
        assert_eq!(p.heading("x"), "x");
        assert_eq!(p.label("x"), "x");
        assert_eq!(p.bold("x"), "x");
        assert_eq!(p.dim("x"), "x");
        assert_eq!(p.ok("OK"), "OK");
        assert_eq!(p.fail("FAIL"), "FAIL");
        assert_eq!(p.good("yes"), "yes");
        assert_eq!(p.bad("no"), "no");
        assert!(!p.enabled());
    }

    #[test]
    fn enabled_palette_wraps_ansi() {
        let p = Palette::new(true);
        assert_eq!(p.verdict(Verdict::Bad), "\x1b[1;38;2;248;81;73mBAD\x1b[0m");
        assert_eq!(
            p.verdict(Verdict::Good),
            "\x1b[1;38;2;63;185;80mGOOD\x1b[0m"
        );
        assert_eq!(
            p.verdict(Verdict::Warning),
            "\x1b[1;38;2;210;153;34mWARNING\x1b[0m"
        );
        assert_eq!(
            p.verdict(Verdict::Unknown),
            "\x1b[1;38;2;139;148;158mUNKNOWN\x1b[0m"
        );
        assert_eq!(p.title("T"), "\x1b[1;38;2;88;166;255mT\x1b[0m");
        assert_eq!(p.heading("H"), "\x1b[1mH\x1b[0m");
        assert_eq!(p.label("L"), "\x1b[38;2;139;148;158mL\x1b[0m");
        assert_eq!(p.ok("OK"), "\x1b[1;38;2;63;185;80mOK\x1b[0m");
        assert_eq!(p.fail("FAIL"), "\x1b[1;38;2;248;81;73mFAIL\x1b[0m");
        assert_eq!(p.good("yes"), "\x1b[38;2;63;185;80myes\x1b[0m");
        assert_eq!(p.bad("no"), "\x1b[38;2;248;81;73mno\x1b[0m");
        assert!(p.enabled());
    }

    #[test]
    fn resolution_rules() {
        // (choice, is_terminal, no_color, term_dumb, json)
        assert!(Palette::resolve(ColorChoice::Always, false, false, false, false).enabled());
        assert!(!Palette::resolve(ColorChoice::Always, true, false, false, true).enabled()); // json
        assert!(!Palette::resolve(ColorChoice::Never, true, false, false, false).enabled());
        assert!(Palette::resolve(ColorChoice::Auto, true, false, false, false).enabled());
        assert!(!Palette::resolve(ColorChoice::Auto, false, false, false, false).enabled()); // not tty
        assert!(!Palette::resolve(ColorChoice::Auto, true, true, false, false).enabled()); // NO_COLOR
        assert!(!Palette::resolve(ColorChoice::Auto, true, false, true, false).enabled());
        // TERM=dumb
    }
}
