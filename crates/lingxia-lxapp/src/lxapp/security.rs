use crate::error::LxAppError;
use std::collections::HashSet;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub struct NetworkSecurity {
    /// Normalized domains that are trusted for network requests.
    ///
    /// Empty means deny all. Use `"*"` to explicitly allow all domains.
    trusted_domains: HashSet<String>,
}

impl NetworkSecurity {
    /// Creates a new empty NetworkSecurity configuration
    pub fn new() -> Self {
        Self {
            trusted_domains: HashSet::new(),
        }
    }

    /// Checks if a domain is allowed for network access.
    ///
    /// Empty means deny all. Use `"*"` to explicitly allow all domains.
    ///
    /// `dev_session` relaxes exactly one rule: a non-public address the lxapp
    /// trusts becomes reachable, so a suite can run against a fixture server
    /// on loopback instead of against the public internet.
    ///
    /// This grants no new authority. A dev session already carries an
    /// automation channel that evaluates arbitrary code in the Logic runtime,
    /// so anything this permits was reachable already; what it buys is a
    /// deterministic fixture. The gate is the dev session, not the spelling of
    /// the entry — `"*"` and a named host both work, and a release build has
    /// no dev session at all.
    pub fn is_domain_allowed_in(&self, domain: &str, dev_session: bool) -> bool {
        let runtime_host = domain.trim().trim_end_matches('.');
        let ip_host = runtime_host
            .strip_prefix('[')
            .and_then(|host| host.strip_suffix(']'))
            .unwrap_or(runtime_host);
        let parsed_address = ip_host.parse::<IpAddr>().ok();
        if parsed_address.is_some_and(|address| !is_public_network_address(address)) {
            if !dev_session {
                return false;
            }
            return self.trusted_domains.contains("*")
                || normalize_trusted_domain(domain)
                    .is_some_and(|host| self.trusted_domains.contains(&host));
        }
        if parsed_address.is_none() && !runtime_host.contains('.') {
            return false;
        }
        let Some(domain) = normalize_trusted_domain(domain) else {
            return self.trusted_domains.contains("*") && ip_host.parse::<Ipv6Addr>().is_ok();
        };
        if self.trusted_domains.contains("*") {
            return true;
        };
        self.trusted_domains.contains(&domain)
            || self.trusted_domains.iter().any(|trusted| {
                trusted
                    .strip_prefix("*.")
                    .is_some_and(|suffix| domain.ends_with(&format!(".{suffix}")))
            })
    }

    /// Set trusted domains from a list, replacing the current policy.
    pub(crate) fn set_domains(&mut self, domains: &[String]) {
        self.trusted_domains.clear();
        for domain in domains
            .iter()
            .filter_map(|domain| normalize_trusted_domain(domain))
        {
            self.trusted_domains.insert(domain);
        }
    }
}

/// Returns whether an address is suitable for untrusted lxapp network access.
/// Local, private, link-local, documentation, benchmark, multicast, and other
/// special-use ranges are intentionally excluded.
pub fn is_public_network_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_public_ipv4(address),
        IpAddr::V6(address) => is_public_ipv6(address),
    }
}

fn is_public_ipv4(address: Ipv4Addr) -> bool {
    let value = u32::from(address);
    ![
        (0x0000_0000, 8),
        (0x0a00_0000, 8),
        (0x6440_0000, 10),
        (0x7f00_0000, 8),
        (0xa9fe_0000, 16),
        (0xac10_0000, 12),
        (0xc000_0000, 24),
        (0xc000_0200, 24),
        (0xc058_6300, 24),
        (0xc0a8_0000, 16),
        (0xc612_0000, 15),
        (0xc633_6400, 24),
        (0xcb00_7100, 24),
        (0xe000_0000, 4),
        (0xf000_0000, 4),
    ]
    .into_iter()
    .any(|(network, prefix)| value >> (32 - prefix) == network >> (32 - prefix))
}

fn is_public_ipv6(address: Ipv6Addr) -> bool {
    if let Some(ipv4) = address.to_ipv4() {
        return is_public_ipv4(ipv4);
    }
    let segments = address.segments();
    !(address.is_unspecified()
        || address.is_loopback()
        || segments[0] & 0xfe00 == 0xfc00
        || segments[0] & 0xffc0 == 0xfe80
        || segments[0] & 0xffc0 == 0xfec0
        || segments[0] & 0xff00 == 0xff00
        || (segments[0] == 0x2001 && segments[1] == 0x0db8))
}

/// Security privilege handle.
///
/// Producers of privileged APIs create a typed handle for their privilege id
/// and pass it to [`crate::LxApp::has_security_privilege`]. Core runtime does
/// not define built-in privilege names.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LxAppSecurityPrivilege {
    id: Arc<str>,
}

impl LxAppSecurityPrivilege {
    /// Create a typed handle for a producer-defined security privilege id.
    ///
    /// This only normalizes and validates the id. It does not grant any
    /// capability; each privileged API must still call
    /// [`crate::LxApp::has_security_privilege`] before doing sensitive work.
    pub fn new(privilege: impl AsRef<str>) -> Result<Self, LxAppError> {
        let normalized = normalize_security_privilege_id(privilege.as_ref()).ok_or_else(|| {
            LxAppError::InvalidParameter(format!(
                "security privilege id must be a lowercase identifier: {:?}",
                privilege.as_ref()
            ))
        })?;

        Ok(Self::registered(normalized))
    }

    pub(crate) fn registered(id: String) -> Self {
        Self {
            id: Arc::from(id.into_boxed_str()),
        }
    }

    pub fn as_str(&self) -> &str {
        self.id.as_ref()
    }
}

impl AsRef<str> for LxAppSecurityPrivilege {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for LxAppSecurityPrivilege {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

pub(crate) fn normalize_trusted_domain(domain: &str) -> Option<String> {
    let trimmed = domain.trim().trim_end_matches('.');
    if trimmed == "*" {
        return Some("*".to_string());
    }
    if trimmed.is_empty()
        || trimmed.contains("://")
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains(':')
        || trimmed.chars().any(char::is_whitespace)
    {
        return None;
    }

    if let Some(suffix) = trimmed.strip_prefix("*.") {
        if suffix.contains('*') || !suffix.contains('.') {
            return None;
        }
        return is_valid_trusted_host(suffix).then(|| format!("*.{}", suffix.to_ascii_lowercase()));
    }

    if trimmed.contains('*') {
        return None;
    }

    if is_valid_trusted_host(trimmed) {
        Some(trimmed.to_ascii_lowercase())
    } else {
        None
    }
}

pub(crate) fn is_valid_trusted_host(host: &str) -> bool {
    if host.is_empty() || host.len() > 253 {
        return false;
    }
    if host.parse::<std::net::Ipv4Addr>().is_ok() {
        return true;
    }

    host.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    })
}

pub(crate) fn normalize_security_privilege_id(privilege: &str) -> Option<String> {
    let trimmed = privilege.trim();
    if trimmed.is_empty()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains(':')
        || trimmed.chars().any(char::is_whitespace)
    {
        return None;
    }

    if trimmed
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'_'))
    {
        Some(trimmed.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{
        LxAppSecurityPrivilege, NetworkSecurity, is_public_network_address, is_valid_trusted_host,
        normalize_security_privilege_id, normalize_trusted_domain,
    };
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    #[test]
    fn creates_producer_defined_security_privilege() {
        let privilege = LxAppSecurityPrivilege::new("downloads").unwrap();
        assert_eq!(privilege.as_str(), "downloads");
        assert_eq!(privilege.to_string(), "downloads");
        assert_eq!(privilege.as_ref(), "downloads");
    }

    #[test]
    fn rejects_invalid_security_privilege_id() {
        assert!(normalize_security_privilege_id("Agent Automation").is_none());
        assert!(LxAppSecurityPrivilege::new("Agent Automation").is_err());
    }

    #[test]
    fn empty_trusted_domains_denies_all() {
        let security = NetworkSecurity::new();
        assert!(!security.is_domain_allowed_in("example.com", false));
    }

    #[test]
    fn wildcard_trusted_domain_allows_all() {
        let mut security = NetworkSecurity::new();
        security.set_domains(&["*".to_string()]);
        assert!(security.is_domain_allowed_in("example.com", false));
        assert!(security.is_domain_allowed_in("api.lingxia.app", false));
        assert!(!security.is_domain_allowed_in("localhost", false));
        assert!(!security.is_domain_allowed_in("127.0.0.1", false));
    }

    #[test]
    fn a_dev_session_reaches_only_a_loopback_host_the_lxapp_named() {
        let mut security = NetworkSecurity::default();
        security.set_domains(&["127.0.0.1".to_string(), "example.com".to_string()]);

        // A release build never reaches the host's own network.
        assert!(!security.is_domain_allowed_in("127.0.0.1", false));
        // A dev session does, but only because the lxapp asked for it.
        assert!(security.is_domain_allowed_in("127.0.0.1", true));
        assert!(!security.is_domain_allowed_in("192.168.1.10", true));
        assert!(!security.is_domain_allowed_in("10.0.0.1", true));
        // Public hosts are unaffected either way.
        assert!(security.is_domain_allowed_in("example.com", false));
        assert!(!security.is_domain_allowed_in("other.example", true));
    }

    #[test]
    fn a_wildcard_opens_a_private_address_only_inside_a_dev_session() {
        let mut security = NetworkSecurity::default();
        security.set_domains(&["*".to_string()]);

        assert!(security.is_domain_allowed_in("example.com", false));
        // A shipped app with `*` still never reaches the user's own network.
        assert!(!security.is_domain_allowed_in("127.0.0.1", false));
        assert!(!security.is_domain_allowed_in("192.168.1.10", false));
        assert!(!security.is_domain_allowed_in("[::1]", false));
        // Under a dev session `*` covers a fixture server like anything else.
        assert!(security.is_domain_allowed_in("127.0.0.1", true));
        assert!(security.is_domain_allowed_in("192.168.1.10", true));
    }

    #[test]
    fn an_empty_policy_denies_loopback_even_in_a_dev_session() {
        let security = NetworkSecurity::default();
        assert!(!security.is_domain_allowed_in("127.0.0.1", true));
    }

    #[test]
    fn trusted_domain_matching_normalizes_runtime_host() {
        let mut security = NetworkSecurity::new();
        security.set_domains(&[" API.Example.COM. ".to_string()]);

        assert!(security.is_domain_allowed_in("api.example.com", false));
        assert!(security.is_domain_allowed_in("API.EXAMPLE.COM.", false));
        assert!(!security.is_domain_allowed_in("cdn.example.com", false));
    }

    #[test]
    fn trusted_domain_matching_supports_subdomain_wildcard() {
        let mut security = NetworkSecurity::new();
        security.set_domains(&["*.example.com".to_string()]);

        assert!(security.is_domain_allowed_in("cdn.example.com", false));
        assert!(security.is_domain_allowed_in("img.cdn.example.com", false));
        assert!(!security.is_domain_allowed_in("example.com", false));
        assert_eq!(
            normalize_trusted_domain("*.Example.COM."),
            Some("*.example.com".to_string())
        );
    }

    #[test]
    fn rejects_invalid_trusted_domain_shape() {
        assert!(normalize_trusted_domain("https://api.example.com").is_none());
        assert!(normalize_trusted_domain("api.example.com/path").is_none());
        assert!(normalize_trusted_domain("api.example.com:443").is_none());
        assert!(normalize_trusted_domain("*example.com").is_none());
        assert!(normalize_trusted_domain("api.*.example.com").is_none());
        assert!(normalize_trusted_domain("*.com").is_none());
        assert!(normalize_trusted_domain("api_internal.example.com").is_none());
        assert!(normalize_trusted_domain("-api.example.com").is_none());
        assert!(normalize_trusted_domain("api-.example.com").is_none());
        assert!(normalize_trusted_domain("api..example.com").is_none());
        assert!(normalize_trusted_domain(".").is_none());
    }

    #[test]
    fn accepts_localhost_and_ipv4_hosts() {
        assert!(is_valid_trusted_host("localhost"));
        assert!(is_valid_trusted_host("127.0.0.1"));
        assert_eq!(
            normalize_trusted_domain("LOCALHOST"),
            Some("localhost".to_string())
        );
    }

    #[test]
    fn identifies_only_public_network_addresses() {
        for address in [
            IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254)),
            IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(198, 18, 0, 1)),
            IpAddr::V6(Ipv6Addr::LOCALHOST),
            "fc00::1".parse().unwrap(),
            "fe80::1".parse().unwrap(),
            "::ffff:127.0.0.1".parse().unwrap(),
        ] {
            assert!(!is_public_network_address(address), "{address}");
        }
        assert!(is_public_network_address("1.1.1.1".parse().unwrap()));
        assert!(is_public_network_address(
            "2606:4700:4700::1111".parse().unwrap()
        ));
    }
}
