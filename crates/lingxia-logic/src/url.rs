//! Shared http(s) URL helpers for APIs that hand a caller-supplied URL to the
//! host. Every such API gates on the same lxapp domain policy, so they must
//! agree on what counts as an http URL and on which host that URL names.

/// `http://` / `https://`, case-insensitive. Byte-slicing is safe: a
/// multi-byte boundary yields `None` rather than a panic.
pub(crate) fn is_http_url(value: &str) -> bool {
    value
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("http://"))
        || value
            .get(..8)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("https://"))
}

/// Splits `scheme://host[:port][/...]` into a lowercased scheme and its host.
/// Userinfo (`@`) is refused outright — it is the classic way to disguise the
/// real host from a policy check.
pub(crate) fn split_url_scheme_host(url: &str) -> Option<(String, &str)> {
    let (scheme, rest) = url.split_once("://")?;
    let host_port = rest.split(['/', '?', '#']).next()?.trim();
    if host_port.is_empty() || host_port.contains('@') {
        return None;
    }
    let host = if let Some(host) = host_port
        .strip_prefix('[')
        .and_then(|rest| rest.split_once(']').map(|(host, _)| host))
    {
        host
    } else {
        host_port.split(':').next().unwrap_or(host_port)
    };
    if host.is_empty() {
        None
    } else {
        Some((scheme.to_ascii_lowercase(), host.trim_end_matches('.')))
    }
}

#[cfg(test)]
mod tests {
    use super::{is_http_url, split_url_scheme_host};

    #[test]
    fn http_prefixes_are_case_insensitive() {
        assert!(is_http_url("https://cdn.example.com/a.jpg"));
        assert!(is_http_url("HTTP://cdn.example.com/a.jpg"));
        assert!(!is_http_url("lx://usercache/a.png"));
        assert!(!is_http_url("/sandbox/a.png"));
        // A multi-byte boundary must not panic.
        assert!(!is_http_url("héllo"));
    }

    #[test]
    fn host_is_taken_without_port_or_trailing_dot() {
        assert_eq!(
            split_url_scheme_host("https://cdn.example.com:8443/a.jpg?x=1"),
            Some(("https".to_string(), "cdn.example.com"))
        );
        assert_eq!(
            split_url_scheme_host("HTTPS://cdn.example.com./a.jpg"),
            Some(("https".to_string(), "cdn.example.com"))
        );
        assert_eq!(
            split_url_scheme_host("https://[2606:4700::1111]:443/a.jpg"),
            Some(("https".to_string(), "2606:4700::1111"))
        );
    }

    #[test]
    fn userinfo_and_hostless_urls_are_refused() {
        // `trusted.example.com` here is userinfo; the real host is the attacker's.
        assert_eq!(
            split_url_scheme_host("https://trusted.example.com@192.168.1.1/a.jpg"),
            None
        );
        assert_eq!(split_url_scheme_host("https:///a.jpg"), None);
        assert_eq!(split_url_scheme_host("not-a-url"), None);
    }
}
