use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProxyError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("SOCKS5 error: {0}")]
    Socks5(#[from] fast_socks5::SocksError),

    #[error("Invalid CONNECT request: {0}")]
    BadRequest(String),

    #[error("Upstream connection failed: {0}")]
    UpstreamConnect(String),

    #[error("Rule-list routing error: {0}")]
    RuleList(String),

    #[error("Proxy already running")]
    AlreadyRunning,

    #[cfg(feature = "capture")]
    #[error("MITM interception failed: {0}")]
    Mitm(String),
}

/// Why a rule list could not be fetched, in the terms a person can act on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FetchFailure {
    /// The SOCKS server could not be reached.
    ProxyUnreachable,
    /// The SOCKS server refused the credentials.
    ProxyRejected,
    /// The list's server could not be reached, through the proxy or directly.
    SourceUnreachable,
    /// The list's server did not answer in time.
    SourceTimedOut,
    /// An answer came, but it is not a rule list.
    NotAList,
}

impl ProxyError {
    /// The kind of a rule-list fetch failure; the error itself stays for logs.
    pub fn fetch_failure(&self) -> FetchFailure {
        use fast_socks5::{ReplyError, SocksError};
        match self {
            ProxyError::Socks5(SocksError::ReplyError(ReplyError::ConnectionTimeout)) => {
                FetchFailure::SourceTimedOut
            }
            // A reply means the proxy is up and could not reach the list for us.
            ProxyError::Socks5(SocksError::ReplyError(_)) => FetchFailure::SourceUnreachable,
            ProxyError::Socks5(
                SocksError::AuthenticationRejected(_) | SocksError::AuthMethodUnacceptable(_),
            ) => FetchFailure::ProxyRejected,
            ProxyError::Socks5(_) => FetchFailure::ProxyUnreachable,
            ProxyError::RuleList(detail) if detail.starts_with("TLS") => {
                FetchFailure::SourceUnreachable
            }
            ProxyError::RuleList(_) => FetchFailure::NotAList,
            _ => FetchFailure::SourceUnreachable,
        }
    }
}

#[cfg(test)]
mod fetch_failure_tests {
    use super::*;
    use fast_socks5::{ReplyError, SocksError};

    #[test]
    fn each_fetch_error_has_the_kind_a_person_can_act_on() {
        let cases = [
            (
                ProxyError::Socks5(SocksError::Io(std::io::ErrorKind::ConnectionRefused.into())),
                FetchFailure::ProxyUnreachable,
            ),
            (
                ProxyError::Socks5(SocksError::AuthenticationRejected("no".into())),
                FetchFailure::ProxyRejected,
            ),
            (
                ProxyError::Socks5(SocksError::ReplyError(ReplyError::HostUnreachable)),
                FetchFailure::SourceUnreachable,
            ),
            (
                ProxyError::Socks5(SocksError::ReplyError(ReplyError::ConnectionTimeout)),
                FetchFailure::SourceTimedOut,
            ),
            (
                ProxyError::UpstreamConnect("refused".into()),
                FetchFailure::SourceUnreachable,
            ),
            (
                ProxyError::RuleList("TLS handshake: bad certificate".into()),
                FetchFailure::SourceUnreachable,
            ),
            (
                ProxyError::RuleList("empty response body".into()),
                FetchFailure::NotAList,
            ),
            (
                ProxyError::Io(std::io::ErrorKind::UnexpectedEof.into()),
                FetchFailure::SourceUnreachable,
            ),
        ];
        for (error, kind) in cases {
            assert_eq!(error.fetch_failure(), kind, "{error}");
        }
    }
}
