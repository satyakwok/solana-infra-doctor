//! gRPC endpoint parsing, validation, and redaction. Like [`RpcEndpoint`], a
//! gRPC URL is treated as secret-bearing (it may embed a token in userinfo, the
//! query string, or a path segment) and is redacted before any output.
//!
//! [`RpcEndpoint`]: crate::rpc::RpcEndpoint

use super::error_kind::GrpcErrorKind;
use url::Url;

/// A validated `http`/`https` Yellowstone gRPC endpoint. `https` selects TLS
/// (h2 over TLS); `http` is plaintext h2 (used for local/test endpoints).
///
/// Its `Debug` is redacted so credentials never leak through logging or panics.
#[derive(Clone)]
pub struct GrpcEndpoint {
    url: Url,
}

impl std::fmt::Debug for GrpcEndpoint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GrpcEndpoint")
            .field("url", &self.redacted())
            .finish()
    }
}

impl GrpcEndpoint {
    /// Parse and validate a gRPC URL, rejecting non-`http(s)` schemes and
    /// hostless URLs. The error carries a redaction-safe [`GrpcErrorKind`].
    pub fn parse(input: &str) -> Result<Self, (GrpcErrorKind, String)> {
        let url = Url::parse(input)
            .map_err(|error| (GrpcErrorKind::InvalidGrpcUrl, error.to_string()))?;

        match url.scheme() {
            "http" | "https" => {}
            scheme => {
                return Err((
                    GrpcErrorKind::InvalidGrpcUrl,
                    format!("unsupported scheme '{scheme}', expected http or https"),
                ));
            }
        }

        if url.host_str().is_none() {
            return Err((GrpcErrorKind::InvalidGrpcUrl, "missing host".to_string()));
        }

        Ok(Self { url })
    }

    /// Whether the endpoint uses TLS (`https`).
    pub fn is_tls(&self) -> bool {
        self.url.scheme() == "https"
    }

    /// The host portion (used for the TLS SNI / domain name).
    pub fn domain(&self) -> &str {
        self.url.host_str().unwrap_or_default()
    }

    /// The endpoint as a connection target string for the transport layer.
    pub fn connect_target(&self) -> String {
        self.url.as_str().to_string()
    }

    /// The URL with any credentials or likely token redacted, safe to display.
    pub fn redacted(&self) -> String {
        crate::redact::redact_url(&self.url)
    }
}

#[cfg(all(test, not(coverage)))]
mod tests {
    use super::*;

    #[test]
    fn accepts_https_and_http() {
        assert!(
            GrpcEndpoint::parse("https://grpc.example.com:443")
                .unwrap()
                .is_tls()
        );
        assert!(
            !GrpcEndpoint::parse("http://127.0.0.1:10000")
                .unwrap()
                .is_tls()
        );
    }

    #[test]
    fn rejects_non_http_scheme() {
        let (kind, _) = GrpcEndpoint::parse("grpc://example.com").unwrap_err();
        assert_eq!(kind, GrpcErrorKind::InvalidGrpcUrl);
    }

    #[test]
    fn rejects_unparseable() {
        let (kind, _) = GrpcEndpoint::parse("not a url").unwrap_err();
        assert_eq!(kind, GrpcErrorKind::InvalidGrpcUrl);
    }

    #[test]
    fn redacts_token_in_url() {
        let endpoint =
            GrpcEndpoint::parse("https://user:secret@grpc.example.com/v2/SUPERSECRETTOKENVALUE01")
                .unwrap();
        let redacted = endpoint.redacted();
        assert!(!redacted.contains("secret"));
        assert!(!redacted.contains("SUPERSECRETTOKENVALUE01"));
        assert!(redacted.contains("***"));
    }

    #[test]
    fn exposes_domain_and_target() {
        let endpoint = GrpcEndpoint::parse("https://grpc.example.com:443").unwrap();
        assert_eq!(endpoint.domain(), "grpc.example.com");
        assert!(
            endpoint
                .connect_target()
                .starts_with("https://grpc.example.com")
        );
    }

    #[test]
    fn debug_is_redacted() {
        let endpoint = GrpcEndpoint::parse("https://user:topsecret@grpc.example.com").unwrap();
        let debug = format!("{endpoint:?}");
        assert!(!debug.contains("topsecret"));
        assert!(debug.contains("***"));
    }
}
