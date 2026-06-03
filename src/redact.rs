//! Centralized redaction for RPC URLs and any text that may embed them.
//!
//! Private RPC endpoints frequently carry credentials in basic-auth userinfo,
//! query parameters (`?api-key=...`), or path tokens (`/v2/<key>`). This module
//! is the single place that strips those before a URL is shown in terminal,
//! JSON, Markdown, or error output.

use url::Url;

/// Path segments that mark the *next* segment as a provider token.
const TOKEN_PATH_MARKERS: [&str; 4] = ["v1", "v2", "api", "project"];

/// Minimum length for a path segment to be treated as a likely opaque token.
const TOKEN_MIN_LEN: usize = 20;

const SCHEMES: [&str; 4] = ["https://", "http://", "wss://", "ws://"];

/// Redact a parsed URL: mask basic-auth userinfo, drop the entire query string
/// and fragment, and mask likely provider tokens in the path. Dropping the whole
/// query is intentional and conservative — it covers every secret-bearing
/// parameter (api-key, token, access_token, ...) without enumerating them.
pub fn redact_url(url: &Url) -> String {
    let mut redacted = url.clone();

    if redacted.password().is_some() {
        let _ = redacted.set_password(Some("***"));
    }
    if !redacted.username().is_empty() {
        let _ = redacted.set_username("***");
    }

    if let Some(segments) = url.path_segments() {
        let masked = redact_path_segments(segments);
        redacted.set_path(&format!("/{}", masked.join("/")));
    }

    redacted.set_query(None);
    redacted.set_fragment(None);
    redacted.to_string()
}

/// Redact every URL-looking substring inside arbitrary text (for example the
/// `Display` output of a `reqwest::Error`, which embeds the request URL).
pub fn redact_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(offset) = next_scheme(rest) {
        result.push_str(&rest[..offset]);
        let tail = &rest[offset..];
        let end = tail
            .find(|c: char| {
                c.is_whitespace()
                    || matches!(
                        c,
                        '"' | '\'' | '(' | ')' | '[' | ']' | '<' | '>' | '|' | '\\'
                    )
            })
            .unwrap_or(tail.len());
        let candidate = &tail[..end];
        let replacement = Url::parse(candidate)
            .map_or_else(|_| candidate.to_string(), |parsed| redact_url(&parsed));
        result.push_str(&replacement);
        rest = &tail[end..];
    }

    result.push_str(rest);
    result
}

fn redact_path_segments<'a>(segments: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut masked = Vec::new();
    let mut previous_is_marker = false;

    for segment in segments {
        let redact = !segment.is_empty() && (previous_is_marker || looks_like_token(segment));
        masked.push(if redact {
            "***".to_string()
        } else {
            segment.to_string()
        });
        previous_is_marker = TOKEN_PATH_MARKERS.contains(&segment.to_ascii_lowercase().as_str());
    }

    masked
}

fn looks_like_token(segment: &str) -> bool {
    segment.len() >= TOKEN_MIN_LEN
        && segment
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn next_scheme(text: &str) -> Option<usize> {
    SCHEMES.iter().filter_map(|scheme| text.find(scheme)).min()
}

#[cfg(all(test, not(coverage)))]
mod tests {
    use super::*;

    #[test]
    fn redacts_query_userinfo_and_path_tokens() {
        let url =
            Url::parse("https://user:pass@rpc.example.com/v2/SECRETKEY?api-key=FAKE").unwrap();
        let redacted = redact_url(&url);
        assert!(!redacted.contains("FAKE"));
        assert!(!redacted.contains("SECRETKEY"));
        assert!(!redacted.contains("pass"));
        assert!(redacted.contains("***"));
    }

    #[test]
    fn keeps_public_url_readable() {
        let url = Url::parse("https://api.mainnet-beta.solana.com").unwrap();
        assert_eq!(redact_url(&url), "https://api.mainnet-beta.solana.com/");
    }

    #[test]
    fn sanitizes_url_inside_text() {
        let text =
            "error sending request for url (https://rpc.helius.xyz/?api-key=FAKE_SECRET_123)";
        let redacted = redact_text(text);
        assert!(!redacted.contains("FAKE_SECRET_123"));
        assert!(redacted.contains("https://rpc.helius.xyz/"));
    }
}
