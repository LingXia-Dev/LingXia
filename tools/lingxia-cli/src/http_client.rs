use std::sync::OnceLock;
use std::time::Duration;

/// Root certificates shared by every agent below.
///
/// ureq defaults to the bundled Mozilla roots, which reject any chain issued by
/// a root that only lives in the machine's trust store — corporate proxies, TLS
/// inspection appliances, self-managed CAs. Those are exactly the networks where
/// `lingxia upgrade`/`build` still has to reach GitHub, and every other dev tool
/// on the machine (curl, cargo, git) already trusts that root, so load the
/// platform roots and keep the bundled set only as the fallback for hosts that
/// ship none (minimal containers). `rustls-native-certs` also honours
/// `SSL_CERT_FILE`/`SSL_CERT_DIR`, which gives a way in when the root is a file
/// rather than an installed trust anchor.
fn native_root_certs() -> Option<&'static ureq::tls::RootCerts> {
    static ROOTS: OnceLock<Option<ureq::tls::RootCerts>> = OnceLock::new();
    ROOTS
        .get_or_init(|| {
            let loaded = rustls_native_certs::load_native_certs();
            let certs: Vec<ureq::tls::Certificate<'static>> = loaded
                .certs
                .into_iter()
                .map(|c| ureq::tls::Certificate::from_der(c.as_ref()).to_owned())
                .collect();
            (!certs.is_empty()).then(|| ureq::tls::RootCerts::from(certs))
        })
        .as_ref()
}

fn build_agent(timeout: Option<Duration>) -> ureq::Agent {
    let mut tls = ureq::tls::TlsConfig::builder();
    if let Some(roots) = native_root_certs() {
        tls = tls.root_certs(roots.clone());
    }

    ureq::Agent::config_builder()
        .timeout_global(timeout)
        .http_status_as_error(false)
        .tls_config(tls.build())
        .build()
        .new_agent()
}

/// Create a standard ureq agent with LingXia defaults.
pub fn create_agent(timeout_secs: u64) -> ureq::Agent {
    build_agent(Some(Duration::from_secs(timeout_secs)))
}

/// Create a ureq agent that uses native root certificates.
pub fn create_native_roots_agent() -> ureq::Agent {
    build_agent(None)
}

/// Shared native-roots agent for Apple/Harmony API calls.
pub fn shared_native_roots_agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(create_native_roots_agent)
}

pub fn call_with_headers(
    agent: &ureq::Agent,
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
) -> Result<ureq::http::Response<ureq::Body>, ureq::Error> {
    let mut req = match method {
        "GET" => agent.get(url),
        "DELETE" => agent.delete(url),
        _ => panic!("Unsupported method for call_with_headers: {method}"),
    };
    for (name, value) in headers {
        req = req.header(*name, *value);
    }
    req.call()
}

pub fn send_bytes_with_headers(
    agent: &ureq::Agent,
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Result<ureq::http::Response<ureq::Body>, ureq::Error> {
    let mut req = match method {
        "POST" => agent.post(url),
        "PUT" => agent.put(url),
        _ => panic!("Unsupported method for send_bytes_with_headers: {method}"),
    };
    for (name, value) in headers {
        req = req.header(*name, *value);
    }
    req.send(body)
}
