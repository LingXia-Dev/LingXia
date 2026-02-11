use std::sync::OnceLock;
use std::time::Duration;

/// Create a standard ureq agent with LingXia defaults.
pub fn create_agent(timeout_secs: u64) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(timeout_secs)))
        .http_status_as_error(false)
        .build()
        .into()
}

/// Create a ureq agent that uses native root certificates.
pub fn create_native_roots_agent() -> ureq::Agent {
    use ureq::tls::{RootCerts, TlsConfig};

    let native_certs = rustls_native_certs::load_native_certs();
    let certs: Vec<ureq::tls::Certificate<'static>> = native_certs
        .certs
        .into_iter()
        .map(|c| ureq::tls::Certificate::from_der(c.as_ref()).to_owned())
        .collect();

    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .tls_config(
            TlsConfig::builder()
                .root_certs(RootCerts::from(certs))
                .build(),
        )
        .build()
        .new_agent()
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
