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

/// Proxy variables in curl's order for the https hosts the CLI talks to. ureq's
/// own lookup reads `ALL_PROXY` first, so a SOCKS `ALL_PROXY` next to a working
/// `HTTPS_PROXY` took the route, and without SOCKS support it went direct.
const PROXY_VARS: &[&str] = &[
    "HTTPS_PROXY",
    "https_proxy",
    "ALL_PROXY",
    "all_proxy",
    "HTTP_PROXY",
    "http_proxy",
];

fn env_proxy() -> Option<ureq::Proxy> {
    proxy_from(|name| std::env::var(name).ok())
}

fn proxy_from(var: impl Fn(&str) -> Option<String>) -> Option<ureq::Proxy> {
    let value = PROXY_VARS
        .iter()
        .filter_map(|name| var(name))
        .map(|value| value.trim().to_string())
        .find(|value| !value.is_empty())?;
    let uri: ureq::http::Uri = if value.contains("://") {
        value.parse().ok()?
    } else {
        format!("http://{value}").parse().ok()?
    };
    let protocol = ureq::ProxyProtocol::try_from(uri.scheme_str()?).ok()?;
    let authority = uri.authority()?;

    let mut proxy = ureq::Proxy::builder(protocol).host(authority.host());
    if let Some(port) = authority.port_u16() {
        proxy = proxy.port(port);
    }
    if let Some((userinfo, _)) = authority.as_str().rsplit_once('@') {
        let (user, password) = userinfo
            .split_once(':')
            .map_or((userinfo, None), |(user, password)| (user, Some(password)));
        proxy = proxy.username(user);
        if let Some(password) = password {
            proxy = proxy.password(password);
        }
    }
    // A proxy built by hand does not read NO_PROXY the way ureq's lookup does.
    let no_proxy = var("NO_PROXY")
        .or_else(|| var("no_proxy"))
        .unwrap_or_default();
    for expr in no_proxy.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        proxy = proxy.no_proxy(expr);
    }
    proxy.build().ok()
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
        .proxy(env_proxy())
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

#[cfg(test)]
mod tests {
    use super::proxy_from;
    use std::collections::HashMap;

    fn proxy(vars: &[(&str, &str)]) -> Option<ureq::Proxy> {
        let vars: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        proxy_from(|name| vars.get(name).cloned())
    }

    #[test]
    fn https_proxy_wins_over_all_proxy() {
        let p = proxy(&[
            ("ALL_PROXY", "socks5://127.0.0.1:1080"),
            ("https_proxy", "http://127.0.0.1:1088"),
        ])
        .unwrap();
        assert_eq!(p.protocol(), ureq::ProxyProtocol::Http);
        assert_eq!((p.host(), p.port()), ("127.0.0.1", 1088));
    }

    #[test]
    fn all_proxy_is_used_when_it_is_the_only_one() {
        let p = proxy(&[("ALL_PROXY", "socks5://127.0.0.1:1080")]).unwrap();
        assert_eq!(p.protocol(), ureq::ProxyProtocol::Socks5);
        assert_eq!(p.port(), 1080);
    }

    #[test]
    fn keeps_credentials_bare_hosts_and_no_proxy() {
        let p = proxy(&[
            ("HTTPS_PROXY", "user:secret@proxy.example:3128"),
            ("NO_PROXY", "localhost, .internal"),
        ])
        .unwrap();
        assert_eq!(p.protocol(), ureq::ProxyProtocol::Http);
        assert_eq!((p.host(), p.port()), ("proxy.example", 3128));
        assert_eq!((p.username(), p.password()), (Some("user"), Some("secret")));
        assert!(p.is_no_proxy(&"http://localhost:8081/x".parse().unwrap()));
        assert!(p.is_no_proxy(&"https://api.internal/x".parse().unwrap()));
        assert!(!p.is_no_proxy(&"https://github.com/".parse().unwrap()));
    }

    #[test]
    fn no_variables_means_no_proxy() {
        assert!(proxy(&[]).is_none());
        assert!(proxy(&[("HTTPS_PROXY", "  ")]).is_none());
    }
}
