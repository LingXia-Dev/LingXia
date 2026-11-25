//! How transfers the browser makes on a page's behalf reach the network.

/// The browser's network decision, shared by everything it fetches for its pages.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum BrowserNetwork {
    /// The process-wide transport.
    #[default]
    Runtime,
    /// A required proxy is disconnected, so nothing may go out.
    Blocked,
    /// Only through this proxy URL; `None` is explicitly direct.
    Explicit(Option<String>),
}

/// A client that goes only through `proxy`, or direct for `None`, never through the process-wide
/// HTTP override: a global override could route the proxy engine's own traffic into itself.
#[cfg(feature = "http-proxy")]
pub fn explicit_proxy_client(
    proxy: Option<&str>,
    connect_timeout: Option<std::time::Duration>,
    read_timeout: std::time::Duration,
) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder()
        .no_proxy()
        .read_timeout(read_timeout);
    if let Some(timeout) = connect_timeout {
        builder = builder.connect_timeout(timeout);
    }
    if let Some(url) = proxy {
        builder = builder.proxy(reqwest::Proxy::all(url).map_err(|error| error.to_string())?);
    }
    builder.build().map_err(|error| error.to_string())
}
