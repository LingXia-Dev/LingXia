// MITM TLS interception.
//
// The CA certificate and key are provided by the caller (subscriber), not
// generated internally.  This separates concerns cleanly:
//
//   - Subscriber generates a CA once (mkcert / openssl), installs it to the
//     system trust store, and hands it to the proxy via set_capture_ca().
//   - Proxy uses the CA key to sign per-domain certificates on the fly.
//   - Proxy presents the original CA DER in the TLS chain so the browser
//     can find the installed root.
//
// Per-connection flow (after CONNECT + 200):
//   tokio::try_join!(
//     TlsAcceptor(forged domain cert) ← client,
//     TlsConnector(trust-all)         → upstream,
//   )
//   → two decrypted async streams → HTTP parser → CapturedSession

use crate::error::ProxyError;
use crate::upstream::UpstreamStream;
use base64::Engine as _;
use rcgen::{CertificateParams, KeyPair};
use rustls::ClientConfig;
use rustls::RootCertStore;
use rustls::ServerConfig;
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer, ServerName};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::{TlsAcceptor, TlsConnector};

/// A CA certificate + private key used by the proxy to forge domain certs.
///
/// Generate once with mkcert or openssl, install to the system trust store,
/// then load here.  See README for step-by-step instructions.
pub struct CaConfig {
    /// Original DER bytes — presented in the TLS chain so the browser can
    /// match it against the installed trusted root.
    pub cert_der: Vec<u8>,
    /// Private key for signing per-domain certificates.
    key: KeyPair,
    /// rcgen Certificate reconstructed from cert_der + key.
    /// Used as the `issuer` argument in `signed_by()`.
    /// Its DER differs from cert_der (fresh serial/dates) but shares the
    /// same SPKI, so AKI in signed domain certs resolves correctly.
    signing_cert: rcgen::Certificate,
}

impl CaConfig {
    /// Load from PEM-encoded certificate and PKCS#8 private key.
    ///
    /// ```ignore
    /// let ca = CaConfig::from_pem(
    ///     &std::fs::read_to_string("rootCA.pem")?,
    ///     &std::fs::read_to_string("rootCA-key.pem")?,
    /// )?;
    /// proxy.set_capture_ca(ca);
    /// ```
    pub fn from_pem(cert_pem: &str, key_pem: &str) -> Result<Self, ProxyError> {
        let cert_der = pem_to_der(cert_pem, "CERTIFICATE")?;
        let key = KeyPair::from_pem(key_pem)
            .map_err(|e| ProxyError::Mitm(format!("CA key load: {e}")))?;

        // Reconstruct signing Certificate from the DER + key.
        // self_signed() yields a fresh cert with the same SPKI; AKI in domain
        // certs is derived from SPKI so the chain resolves against cert_der.
        let params = CertificateParams::from_ca_cert_pem(cert_pem)
            .map_err(|e| ProxyError::Mitm(format!("CA cert parse: {e}")))?;
        let signing_cert = params
            .self_signed(&key)
            .map_err(|e| ProxyError::Mitm(format!("CA signing cert init: {e}")))?;

        Ok(Self {
            cert_der,
            key,
            signing_cert,
        })
    }
}

fn pem_to_der(pem: &str, label: &str) -> Result<Vec<u8>, ProxyError> {
    let header = format!("-----BEGIN {label}-----");
    let footer = format!("-----END {label}-----");
    let b64: String = pem
        .lines()
        .skip_while(|l| !l.trim().starts_with(&header))
        .skip(1)
        .take_while(|l| !l.trim().starts_with(&footer))
        .collect();
    base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|e| ProxyError::Mitm(format!("PEM base64 decode ({label}): {e}")))
}

fn server_config_for(host: &str, ca: &CaConfig) -> Result<Arc<ServerConfig>, ProxyError> {
    let domain_key =
        KeyPair::generate().map_err(|e| ProxyError::Mitm(format!("domain key gen: {e}")))?;

    let params = CertificateParams::new(vec![host.to_string()])
        .map_err(|e| ProxyError::Mitm(format!("domain params: {e}")))?;

    let domain_cert = params
        .signed_by(&domain_key, &ca.signing_cert, &ca.key)
        .map_err(|e| ProxyError::Mitm(format!("domain cert sign: {e}")))?;

    // Chain: domain cert + original CA cert (the one the browser trusts).
    let chain = vec![
        CertificateDer::from(domain_cert.der().to_vec()),
        CertificateDer::from(ca.cert_der.clone()),
    ];
    let key_der = PrivatePkcs8KeyDer::from(domain_key.serialize_der());

    let cfg = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(chain, key_der.into())
        .map_err(|e| ProxyError::Mitm(format!("ServerConfig: {e}")))?;

    Ok(Arc::new(cfg))
}


fn client_config() -> Result<Arc<ClientConfig>, ProxyError> {
    let mut roots = RootCertStore::empty();
    let cert_result = rustls_native_certs::load_native_certs();
    for error in cert_result.errors {
        log::warn!("proxy mitm: failed to load one native root certificate: {error}");
    }
    for cert in cert_result.certs {
        roots
            .add(cert)
            .map_err(|e| ProxyError::Mitm(format!("native root load: {e}")))?;
    }
    if roots.is_empty() {
        return Err(ProxyError::Mitm(
            "no native root certificates available for upstream TLS validation".to_string(),
        ));
    }

    Ok(Arc::new(
        ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth(),
    ))
}

pub type ClientSide = tokio_rustls::server::TlsStream<TcpStream>;
pub type ServerSide = tokio_rustls::client::TlsStream<UpstreamStream>;

/// Perform the MITM TLS handshake on both sides concurrently.
///
/// `client`   — raw TCP stream from the browser (200 already sent).
/// `upstream` — connected upstream stream (Direct or SOCKS5).
/// `ca`       — caller-supplied CA for domain cert signing.
pub async fn intercept(
    client: TcpStream,
    host: &str,
    upstream: UpstreamStream,
    ca: &CaConfig,
) -> Result<(ClientSide, ServerSide), ProxyError> {
    let acceptor = TlsAcceptor::from(server_config_for(host, ca)?);
    let connector = TlsConnector::from(client_config()?);

    let server_name: ServerName<'static> = host
        .to_owned()
        .try_into()
        .map_err(|_| ProxyError::Mitm(format!("invalid server name: {host}")))?;

    let (client_tls, server_tls) = tokio::try_join!(
        acceptor.accept(client),
        connector.connect(server_name, upstream),
    )
    .map_err(|e| ProxyError::Mitm(e.to_string()))?;

    Ok((client_tls, server_tls))
}
