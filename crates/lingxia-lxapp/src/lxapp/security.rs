use crate::error::LxAppError;
use std::collections::HashSet;
use std::fmt;
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
    pub fn is_domain_allowed(&self, domain: &str) -> bool {
        self.trusted_domains.contains("*")
            || normalize_trusted_domain(domain)
                .is_some_and(|domain| self.trusted_domains.contains(&domain))
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
        LxAppSecurityPrivilege, NetworkSecurity, is_valid_trusted_host,
        normalize_security_privilege_id, normalize_trusted_domain,
    };

    #[test]
    fn creates_producer_defined_security_privilege() {
        let privilege = LxAppSecurityPrivilege::new("agent.automation").unwrap();
        assert_eq!(privilege.as_str(), "agent.automation");
        assert_eq!(privilege.to_string(), "agent.automation");
        assert_eq!(privilege.as_ref(), "agent.automation");
    }

    #[test]
    fn rejects_invalid_security_privilege_id() {
        assert!(normalize_security_privilege_id("Agent Automation").is_none());
        assert!(LxAppSecurityPrivilege::new("Agent Automation").is_err());
    }

    #[test]
    fn empty_trusted_domains_denies_all() {
        let security = NetworkSecurity::new();
        assert!(!security.is_domain_allowed("example.com"));
    }

    #[test]
    fn wildcard_trusted_domain_allows_all() {
        let mut security = NetworkSecurity::new();
        security.set_domains(&["*".to_string()]);
        assert!(security.is_domain_allowed("example.com"));
        assert!(security.is_domain_allowed("api.lingxia.app"));
    }

    #[test]
    fn trusted_domain_matching_normalizes_runtime_host() {
        let mut security = NetworkSecurity::new();
        security.set_domains(&[" API.Example.COM. ".to_string()]);

        assert!(security.is_domain_allowed("api.example.com"));
        assert!(security.is_domain_allowed("API.EXAMPLE.COM."));
        assert!(!security.is_domain_allowed("cdn.example.com"));
    }

    #[test]
    fn rejects_invalid_trusted_domain_shape() {
        assert!(normalize_trusted_domain("https://api.example.com").is_none());
        assert!(normalize_trusted_domain("api.example.com/path").is_none());
        assert!(normalize_trusted_domain("api.example.com:443").is_none());
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
}
