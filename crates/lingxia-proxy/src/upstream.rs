use crate::error::ProxyError;
use crate::router::{Socks5Credentials, UpstreamConfig};
use fast_socks5::client::{Config as Socks5Config, Socks5Stream};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;

/// Unified I/O handle over either a direct TCP stream or a SOCKS5-tunnelled
/// stream.  Implements `AsyncRead + AsyncWrite + Unpin`.
pub enum UpstreamStream {
    Direct(TcpStream),
    Socks5(Socks5Stream<TcpStream>),
}

impl AsyncRead for UpstreamStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            UpstreamStream::Direct(s) => Pin::new(s).poll_read(cx, buf),
            UpstreamStream::Socks5(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for UpstreamStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match self.get_mut() {
            UpstreamStream::Direct(s) => Pin::new(s).poll_write(cx, buf),
            UpstreamStream::Socks5(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            UpstreamStream::Direct(s) => Pin::new(s).poll_flush(cx),
            UpstreamStream::Socks5(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            UpstreamStream::Direct(s) => Pin::new(s).poll_shutdown(cx),
            UpstreamStream::Socks5(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}

impl Unpin for UpstreamStream {}

/// Open a connection to `target_host:target_port` via `config`.
pub async fn connect_upstream(
    config: &UpstreamConfig,
    target_host: &str,
    target_port: u16,
) -> Result<UpstreamStream, ProxyError> {
    match config {
        UpstreamConfig::Direct => {
            let addr = format!("{target_host}:{target_port}");
            let stream = TcpStream::connect(&addr)
                .await
                .map_err(|e| ProxyError::UpstreamConnect(e.to_string()))?;
            Ok(UpstreamStream::Direct(stream))
        }

        UpstreamConfig::Socks5 {
            host,
            port,
            credentials,
        } => {
            let proxy_addr = format!("{host}:{port}");
            let cfg = Socks5Config::default();

            let stream = match credentials {
                Some(Socks5Credentials { username, password }) => {
                    Socks5Stream::connect_with_password(
                        proxy_addr,
                        target_host.to_owned(),
                        target_port,
                        username.clone(),
                        password.clone(),
                        cfg,
                    )
                    .await?
                }
                None => {
                    Socks5Stream::connect(proxy_addr, target_host.to_owned(), target_port, cfg)
                        .await?
                }
            };

            Ok(UpstreamStream::Socks5(stream))
        }
    }
}
