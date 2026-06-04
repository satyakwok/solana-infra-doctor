//! gRPC-specific error classification. These are distinct from the HTTP
//! JSON-RPC `ErrorKind` taxonomy so a gRPC failure is described in gRPC terms
//! (transport vs. status code vs. stream behavior) rather than being forced into
//! an HTTP shape.

use serde::Serialize;
use tonic::Code;

/// A classified gRPC failure. Serializes as a stable `snake_case` string so the
/// JSON shape is machine-friendly and forward-compatible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GrpcErrorKind {
    /// The supplied gRPC URL could not be parsed or used an unsupported scheme.
    InvalidGrpcUrl,
    /// The host could not be resolved.
    DnsError,
    /// A transport-level connection failure (refused, reset, unreachable).
    ConnectError,
    /// A TLS handshake or certificate failure.
    TlsError,
    /// A local timeout (connection or request) elapsed before a response.
    Timeout,
    /// The server reported `UNAUTHENTICATED` (missing/invalid token).
    Unauthenticated,
    /// The server reported `PERMISSION_DENIED` (token lacks access).
    PermissionDenied,
    /// The server reported `UNAVAILABLE` (overloaded, draining, or down).
    Unavailable,
    /// The server reported `RESOURCE_EXHAUSTED` (rate/quota limit).
    ResourceExhausted,
    /// The method is not implemented by this endpoint.
    Unimplemented,
    /// The server reported `DEADLINE_EXCEEDED`.
    DeadlineExceeded,
    /// The server reported `INVALID_ARGUMENT`.
    InvalidArgument,
    /// The server reported `INTERNAL`.
    Internal,
    /// A response or stream update was malformed or unexpected.
    MalformedResponse,
    /// The stream closed before the diagnostic completed.
    StreamClosed,
    /// The stream produced a first update but then went quiet.
    StreamStalled,
    /// The stream produced no update before the deadline.
    NoFirstEvent,
    /// An unclassified error.
    UnknownError,
}

impl GrpcErrorKind {
    /// Map a tonic status [`Code`] to a gRPC error kind.
    pub fn from_code(code: Code) -> Self {
        match code {
            Code::Unauthenticated => Self::Unauthenticated,
            Code::PermissionDenied => Self::PermissionDenied,
            Code::Unavailable => Self::Unavailable,
            Code::ResourceExhausted => Self::ResourceExhausted,
            Code::Unimplemented => Self::Unimplemented,
            Code::DeadlineExceeded => Self::DeadlineExceeded,
            Code::InvalidArgument => Self::InvalidArgument,
            Code::Internal => Self::Internal,
            _ => Self::UnknownError,
        }
    }

    /// Whether this kind represents an authentication/authorization failure.
    pub fn is_auth_failure(self) -> bool {
        matches!(self, Self::Unauthenticated | Self::PermissionDenied)
    }

    /// The stable lowercase string used in JSON and reports.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidGrpcUrl => "invalid_grpc_url",
            Self::DnsError => "dns_error",
            Self::ConnectError => "connect_error",
            Self::TlsError => "tls_error",
            Self::Timeout => "timeout",
            Self::Unauthenticated => "unauthenticated",
            Self::PermissionDenied => "permission_denied",
            Self::Unavailable => "unavailable",
            Self::ResourceExhausted => "resource_exhausted",
            Self::Unimplemented => "unimplemented",
            Self::DeadlineExceeded => "deadline_exceeded",
            Self::InvalidArgument => "invalid_argument",
            Self::Internal => "internal",
            Self::MalformedResponse => "malformed_response",
            Self::StreamClosed => "stream_closed",
            Self::StreamStalled => "stream_stalled",
            Self::NoFirstEvent => "no_first_event",
            Self::UnknownError => "unknown_error",
        }
    }
}

impl std::fmt::Display for GrpcErrorKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(all(test, not(coverage)))]
mod tests {
    use super::*;

    #[test]
    fn maps_status_codes_to_kinds() {
        assert_eq!(
            GrpcErrorKind::from_code(Code::Unauthenticated),
            GrpcErrorKind::Unauthenticated
        );
        assert_eq!(
            GrpcErrorKind::from_code(Code::PermissionDenied),
            GrpcErrorKind::PermissionDenied
        );
        assert_eq!(
            GrpcErrorKind::from_code(Code::Unavailable),
            GrpcErrorKind::Unavailable
        );
        assert_eq!(
            GrpcErrorKind::from_code(Code::ResourceExhausted),
            GrpcErrorKind::ResourceExhausted
        );
        assert_eq!(
            GrpcErrorKind::from_code(Code::Unimplemented),
            GrpcErrorKind::Unimplemented
        );
        assert_eq!(
            GrpcErrorKind::from_code(Code::DeadlineExceeded),
            GrpcErrorKind::DeadlineExceeded
        );
        assert_eq!(
            GrpcErrorKind::from_code(Code::InvalidArgument),
            GrpcErrorKind::InvalidArgument
        );
        assert_eq!(
            GrpcErrorKind::from_code(Code::Internal),
            GrpcErrorKind::Internal
        );
        // Anything unmapped collapses to unknown.
        assert_eq!(
            GrpcErrorKind::from_code(Code::DataLoss),
            GrpcErrorKind::UnknownError
        );
    }

    #[test]
    fn auth_failures_are_flagged() {
        assert!(GrpcErrorKind::Unauthenticated.is_auth_failure());
        assert!(GrpcErrorKind::PermissionDenied.is_auth_failure());
        assert!(!GrpcErrorKind::Unavailable.is_auth_failure());
        assert!(!GrpcErrorKind::Timeout.is_auth_failure());
    }

    #[test]
    fn serializes_snake_case() {
        let json = serde_json::to_string(&GrpcErrorKind::NoFirstEvent).unwrap();
        assert_eq!(json, "\"no_first_event\"");
        assert_eq!(GrpcErrorKind::StreamClosed.as_str(), "stream_closed");
    }

    #[test]
    fn every_kind_has_a_stable_string_matching_serde() {
        use GrpcErrorKind::*;
        let all = [
            InvalidGrpcUrl,
            DnsError,
            ConnectError,
            TlsError,
            Timeout,
            Unauthenticated,
            PermissionDenied,
            Unavailable,
            ResourceExhausted,
            Unimplemented,
            DeadlineExceeded,
            InvalidArgument,
            Internal,
            MalformedResponse,
            StreamClosed,
            StreamStalled,
            NoFirstEvent,
            UnknownError,
        ];
        for kind in all {
            let as_str = kind.as_str();
            assert!(!as_str.is_empty());
            // Display and as_str agree.
            assert_eq!(kind.to_string(), as_str);
            // serde serialization matches as_str (quoted).
            let json = serde_json::to_string(&kind).unwrap();
            assert_eq!(json, format!("\"{as_str}\""));
        }
    }
}
