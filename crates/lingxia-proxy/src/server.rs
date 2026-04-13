use crate::error::ProxyError;
use crate::router::UpstreamConfig;
use crate::router::{ProxyRouter, RouteDecision};
use crate::upstream::connect_upstream;
use http::Uri;
use log::{debug, warn};
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
#[cfg(feature = "capture")]
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[cfg(feature = "capture")]
use {
    crate::capture::{CapturedSession, handle_http_sessions},
    crate::mitm::CaConfig,
    tokio::sync::broadcast,
};

/// A local HTTP CONNECT proxy.
///
/// ```text
///  WebView  ──CONNECT──▶  LocalProxy  ──(router)──▶  Direct / SOCKS5 upstream
/// ```
///
/// Clone is cheap (Arc-backed).  Pass one clone to `tokio::spawn(proxy.run())`,
/// keep the other for `set_router()` / `set_capture_ca()` calls.
#[derive(Clone)]
pub struct LocalProxy {
    inner: Arc<Inner>,
}

struct Inner {
    router: RwLock<Arc<dyn ProxyRouter>>,
    local_addr: SocketAddr,
    listener: TcpListener,

    #[cfg(feature = "capture")]
    ca: RwLock<Option<Arc<CaConfig>>>,
    #[cfg(feature = "capture")]
    session_tx: broadcast::Sender<Arc<CapturedSession>>,
}

impl LocalProxy {
    /// Bind on `addr` (use `"127.0.0.1:0"` to let the OS pick a free port).
    pub async fn bind(
        addr: &str,
        initial_router: Arc<dyn ProxyRouter>,
    ) -> Result<Self, ProxyError> {
        let listener = TcpListener::bind(addr).await?;
        let local_addr = listener.local_addr()?;

        #[cfg(feature = "capture")]
        let (session_tx, _) = broadcast::channel(512);

        Ok(Self {
            inner: Arc::new(Inner {
                router: RwLock::new(initial_router),
                local_addr,
                listener,
                #[cfg(feature = "capture")]
                ca: RwLock::new(None),
                #[cfg(feature = "capture")]
                session_tx,
            }),
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.inner.local_addr
    }

    /// Swap the routing policy.  Takes effect for all new connections.
    pub fn set_router(&self, router: Arc<dyn ProxyRouter>) {
        *self.inner.router.write().unwrap() = router;
    }

    /// Install the CA certificate and key used for HTTPS MITM.
    ///
    /// Call this before subscribing with `session_receiver()`.
    /// The CA cert must already be installed in the system/browser trust store
    /// by the caller — the proxy does not install it automatically.
    ///
    /// See README for how to generate and install the CA with mkcert or openssl.
    #[cfg(feature = "capture")]
    pub fn set_capture_ca(&self, ca: CaConfig) {
        *self.inner.ca.write().unwrap() = Some(Arc::new(ca));
    }

    /// Subscribe to structured HTTP sessions from all captured tunnels.
    ///
    /// HTTPS tunnels are MITM-intercepted only when a CA has been set via
    /// `set_capture_ca()`.  Without a CA, HTTPS traffic is forwarded opaquely.
    ///
    /// Filter in the consumer — it's one line, no proxy-side overhead:
    /// ```ignore
    /// while let Ok(s) = rx.recv().await {
    ///     if !s.host.ends_with("openai.com") { continue; }
    ///     agent.process(serde_json::to_value(&*s)?).await;
    /// }
    /// ```
    #[cfg(feature = "capture")]
    pub fn session_receiver(&self) -> broadcast::Receiver<Arc<CapturedSession>> {
        self.inner.session_tx.subscribe()
    }

    /// Drive the accept loop.  Blocks until the listener errors.
    pub async fn run(&self) {
        loop {
            match self.inner.listener.accept().await {
                Ok((stream, peer)) => {
                    debug!("proxy: accepted {peer}");
                    let proxy = self.clone();
                    tokio::spawn(async move {
                        if let Err(e) = proxy.handle_connection(stream).await {
                            debug!("proxy: {peer} — {e}");
                        }
                    });
                }
                Err(e) => {
                    warn!("proxy: accept error: {e}");
                    break;
                }
            }
        }
    }

    // ── per-connection ────────────────────────────────────────────────────

    async fn handle_connection(&self, mut stream: TcpStream) -> Result<(), ProxyError> {
        match read_proxy_request(&mut stream).await? {
            ProxyRequest::Connect { host, port } => {
                let upstream_cfg = self.route_upstream(&host, port)?;

                #[cfg(feature = "capture")]
                {
                    return self
                        .handle_connect_with_capture(stream, host, port, upstream_cfg)
                        .await;
                }

                #[cfg(not(feature = "capture"))]
                {
                    let mut up = connect_upstream(&upstream_cfg, &host, port).await?;
                    stream
                        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                        .await?;
                    tokio::io::copy_bidirectional(&mut stream, &mut up).await?;
                    Ok(())
                }
            }
            ProxyRequest::ForwardHttp {
                host,
                port,
                initial_bytes,
            } => {
                let upstream_cfg = self.route_upstream(&host, port)?;

                #[cfg(feature = "capture")]
                {
                    return self
                        .handle_forward_http_with_capture(
                            stream,
                            initial_bytes,
                            host,
                            port,
                            upstream_cfg,
                        )
                        .await;
                }

                #[cfg(not(feature = "capture"))]
                {
                    let mut up = connect_upstream(&upstream_cfg, &host, port).await?;
                    up.write_all(&initial_bytes).await?;
                    tokio::io::copy_bidirectional(&mut stream, &mut up).await?;
                    Ok(())
                }
            }
        }
    }

    fn route_upstream(&self, host: &str, port: u16) -> Result<UpstreamConfig, ProxyError> {
        let router = self.inner.router.read().unwrap();
        match router.route(host, port)? {
            RouteDecision::Upstream(cfg) => Ok(cfg),
            RouteDecision::Block => Err(ProxyError::UpstreamConnect(format!(
                "{host}:{port} blocked by policy"
            ))),
        }
    }

    #[cfg(feature = "capture")]
    async fn handle_connect_with_capture(
        &self,
        mut stream: TcpStream,
        host: String,
        port: u16,
        upstream_cfg: UpstreamConfig,
    ) -> Result<(), ProxyError> {
        let tx = self.inner.session_tx.clone();
        let has_consumer = tx.receiver_count() > 0;
        let ca = self.inner.ca.read().unwrap().clone(); // Option<Arc<CaConfig>>

        // HTTPS with active subscribers AND a CA loaded → MITM.
        if port == 443 && has_consumer && ca.is_some() {
            let up_stream = connect_upstream(&upstream_cfg, &host, port).await?;
            stream
                .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                .await?;

            let (client_tls, server_tls) =
                crate::mitm::intercept(stream, &host, up_stream, ca.as_deref().unwrap()).await?;

            handle_http_sessions(host, port, true, client_tls, server_tls, tx)
                .await
                .map_err(ProxyError::Io)?;

            return Ok(());
        }

        // Plain HTTP with active subscribers → parse directly (no TLS needed).
        if port != 443 && has_consumer {
            let up_stream = connect_upstream(&upstream_cfg, &host, port).await?;
            stream
                .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                .await?;

            handle_http_sessions(host, port, false, stream, up_stream, tx)
                .await
                .map_err(ProxyError::Io)?;

            return Ok(());
        }

        // No subscribers, or HTTPS without CA → plain tunnel, zero overhead.
        let mut up = connect_upstream(&upstream_cfg, &host, port).await?;
        stream
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await?;
        tokio::io::copy_bidirectional(&mut stream, &mut up).await?;
        Ok(())
    }

    #[cfg(feature = "capture")]
    async fn handle_forward_http_with_capture(
        &self,
        stream: TcpStream,
        initial_bytes: Vec<u8>,
        host: String,
        port: u16,
        upstream_cfg: UpstreamConfig,
    ) -> Result<(), ProxyError> {
        let tx = self.inner.session_tx.clone();
        let has_consumer = tx.receiver_count() > 0;

        let up_stream = connect_upstream(&upstream_cfg, &host, port).await?;
        if has_consumer {
            let client = PrefixedIo::new(stream, initial_bytes);
            handle_http_sessions(host, port, false, client, up_stream, tx)
                .await
                .map_err(ProxyError::Io)?;
            return Ok(());
        }

        let mut stream = stream;
        let mut up = up_stream;
        up.write_all(&initial_bytes).await?;
        tokio::io::copy_bidirectional(&mut stream, &mut up).await?;
        Ok(())
    }
}

enum ProxyRequest {
    Connect {
        host: String,
        port: u16,
    },
    ForwardHttp {
        host: String,
        port: u16,
        initial_bytes: Vec<u8>,
    },
}

// ── HTTP proxy request parser ──────────────────────────────────────────────

async fn read_proxy_request(stream: &mut TcpStream) -> Result<ProxyRequest, ProxyError> {
    let mut buf = Vec::with_capacity(512);
    let mut tmp = [0u8; 1];

    loop {
        stream.read_exact(&mut tmp).await?;
        buf.push(tmp[0]);
        if buf.ends_with(b"\r\n\r\n") {
            break;
        }
        if buf.len() > 8192 {
            return Err(ProxyError::BadRequest("CONNECT headers too large".into()));
        }
    }

    let text =
        std::str::from_utf8(&buf).map_err(|_| ProxyError::BadRequest("Non-UTF8 CONNECT".into()))?;

    let first_line = text
        .lines()
        .next()
        .ok_or_else(|| ProxyError::BadRequest("Empty request".into()))?;

    let mut parts = first_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| ProxyError::BadRequest("No method".into()))?;
    let target = parts
        .next()
        .ok_or_else(|| ProxyError::BadRequest("No request target".into()))?;
    let version = parts
        .next()
        .ok_or_else(|| ProxyError::BadRequest("No HTTP version".into()))?;

    if method.eq_ignore_ascii_case("CONNECT") {
        let (host, port) = parse_authority_host_port(target, None)?;
        return Ok(ProxyRequest::Connect { host, port });
    }

    let (host, port, upstream_target) = parse_forward_target(target, text)?;
    let first_line_end = buf
        .windows(2)
        .position(|window| window == b"\r\n")
        .ok_or_else(|| ProxyError::BadRequest("Malformed request line".into()))?;
    let mut initial_bytes = format!("{method} {upstream_target} {version}\r\n").into_bytes();
    initial_bytes.extend_from_slice(&buf[first_line_end + 2..]);
    Ok(ProxyRequest::ForwardHttp {
        host,
        port,
        initial_bytes,
    })
}

fn parse_forward_target(
    target: &str,
    request_text: &str,
) -> Result<(String, u16, String), ProxyError> {
    if target.starts_with("http://") || target.starts_with("https://") {
        let uri: Uri = target
            .parse()
            .map_err(|e| ProxyError::BadRequest(format!("Bad absolute-form URI: {e}")))?;
        let scheme = uri
            .scheme_str()
            .ok_or_else(|| ProxyError::BadRequest("Absolute-form URI missing scheme".into()))?;
        if scheme.eq_ignore_ascii_case("https") {
            return Err(ProxyError::BadRequest(
                "HTTPS absolute-form requests must use CONNECT".into(),
            ));
        }
        let host = uri
            .host()
            .ok_or_else(|| ProxyError::BadRequest("Absolute-form URI missing host".into()))?
            .to_string();
        let port = uri.port_u16().unwrap_or(80);
        let path = uri
            .path_and_query()
            .map(|pq| pq.as_str().to_string())
            .unwrap_or_else(|| "/".to_string());
        return Ok((host, port, path));
    }

    let authority = request_text
        .lines()
        .skip(1)
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.trim()
                .eq_ignore_ascii_case("host")
                .then(|| value.trim().to_string())
        })
        .ok_or_else(|| ProxyError::BadRequest("HTTP proxy request missing Host header".into()))?;
    let (host, port) = parse_authority_host_port(&authority, Some(80))?;
    Ok((host, port, target.to_string()))
}

fn parse_authority_host_port(
    authority: &str,
    default_port: Option<u16>,
) -> Result<(String, u16), ProxyError> {
    let authority = authority.trim();
    if authority.is_empty() {
        return Err(ProxyError::BadRequest("Empty authority".into()));
    }
    if let Some(host) = authority.strip_prefix('[')
        && let Some((host, rest)) = host.split_once(']')
    {
        let port = if let Some(rest) = rest.strip_prefix(':') {
            rest.parse()
                .map_err(|_| ProxyError::BadRequest(format!("Bad port in '{authority}'")))?
        } else {
            default_port
                .ok_or_else(|| ProxyError::BadRequest(format!("No port in '{authority}'")))?
        };
        return Ok((host.to_string(), port));
    }

    if let Some((host, port)) = authority.rsplit_once(':')
        && !host.is_empty()
        && let Ok(port) = port.parse()
    {
        return Ok((host.to_string(), port));
    }

    Ok((
        authority.to_string(),
        default_port.ok_or_else(|| ProxyError::BadRequest(format!("No port in '{authority}'")))?,
    ))
}

#[cfg(feature = "capture")]
struct PrefixedIo<T> {
    prefix: Vec<u8>,
    prefix_pos: usize,
    inner: T,
}

#[cfg(feature = "capture")]
impl<T> PrefixedIo<T> {
    fn new(inner: T, prefix: Vec<u8>) -> Self {
        Self {
            prefix,
            prefix_pos: 0,
            inner,
        }
    }
}

#[cfg(feature = "capture")]
impl<T> AsyncRead for PrefixedIo<T>
where
    T: AsyncRead + Unpin,
{
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        if self.prefix_pos < self.prefix.len() {
            let remaining = &self.prefix[self.prefix_pos..];
            let to_copy = remaining.len().min(buf.remaining());
            buf.put_slice(&remaining[..to_copy]);
            self.prefix_pos += to_copy;
            return std::task::Poll::Ready(Ok(()));
        }
        std::pin::Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

#[cfg(feature = "capture")]
impl<T> AsyncWrite for PrefixedIo<T>
where
    T: AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}
