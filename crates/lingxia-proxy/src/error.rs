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
