//! Human-facing terminal output helpers.
//!
//! The renderers for each command (`report` for `check`, `compare::render`, and
//! `ws`) build their concise and verbose views on top of these shared helpers:
//!
//! - [`style`] — status vocabulary ([`style::Status`]), unit formatting, and
//!   safe endpoint labels.
//! - [`table`] — a small ANSI-aware column-aligned table.
//!
//! JSON and Markdown output deliberately do **not** use this module: those are
//! machine- and document-facing formats and must never contain terminal styling
//! or human-only layout. Automation should consume `--json`, not the human text.

pub mod style;
pub mod table;
