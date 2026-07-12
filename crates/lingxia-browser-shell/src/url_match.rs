//! Shared URL comparison key for bookmarks and history.

/// Dedup key: trimmed, fragment stripped, trailing `/` stripped, and the
/// scheme+host lowered (paths stay case-sensitive).
pub(crate) fn normalize_url(raw: &str) -> String {
    let mut s = raw.trim();
    if let Some(hash) = s.find('#') {
        s = &s[..hash];
    }
    let s = s.strip_suffix('/').unwrap_or(s);
    match s.split_once("://") {
        Some((scheme, rest)) => {
            let (host, path) = match rest.find(['/', '?']) {
                Some(i) => (&rest[..i], &rest[i..]),
                None => (rest, ""),
            };
            format!(
                "{}://{}{}",
                scheme.to_ascii_lowercase(),
                host.to_ascii_lowercase(),
                path
            )
        }
        None => s.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_fragment_slash_and_lowers_host() {
        assert_eq!(
            normalize_url("HTTPS://Example.COM/Path/#frag"),
            "https://example.com/Path"
        );
        assert_eq!(normalize_url("https://example.com/"), "https://example.com");
        assert_eq!(
            normalize_url("https://example.com/A?q=B"),
            "https://example.com/A?q=B"
        );
        assert_eq!(
            normalize_url("https://EXAMPLE.com?q=CaseSensitive"),
            "https://example.com?q=CaseSensitive"
        );
        assert_eq!(
            normalize_url("HTTPS://Example.COM/Path?q=Case#fragment"),
            "https://example.com/Path?q=Case"
        );
    }
}
