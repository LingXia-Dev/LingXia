//! Platform-independent parts of the Android Servo backend: document URL
//! stamping and top-level download detection.
use url::Url;

const DOCUMENT_STAMP: &str = "__lxdoc";

/// Servo treats loading the current URL again as a no-op, so every page
/// document gets its own URL. The stamp goes last, keeping the base URL's own
/// encoding, and is never reported upward; see [`unstamped`].
pub(crate) fn stamp_document_url(base_url: &str, stamp: u64) -> Option<String> {
    let mut url = Url::parse(base_url).ok()?;
    let stamp = format!("{DOCUMENT_STAMP}={stamp}");
    let query = match url.query() {
        Some(query) if !query.is_empty() => format!("{query}&{stamp}"),
        _ => stamp,
    };
    url.set_query(Some(&query));
    Some(url.to_string())
}

/// The URL LingXia loaded, without the document stamp.
pub(crate) fn unstamped(url: &str) -> String {
    let Ok(mut parsed) = Url::parse(url) else {
        return url.to_string();
    };
    let Some(query) = parsed.query().map(str::to_owned) else {
        return url.to_string();
    };
    let (rest, last) = query.rsplit_once('&').unwrap_or(("", query.as_str()));
    let is_stamp = last
        .strip_prefix(DOCUMENT_STAMP)
        .and_then(|value| value.strip_prefix('='))
        .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()));
    if !is_stamp {
        return url.to_string();
    }
    parsed.set_query((!rest.is_empty()).then_some(rest));
    parsed.to_string()
}

/// A top-level response Servo cannot present as a document.
pub(crate) fn is_download_response(
    content_disposition: Option<&str>,
    mime_type: Option<&str>,
) -> bool {
    if content_disposition.is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("attachment"))
    }) {
        return true;
    }
    let Some(mime) = mime_type
        .and_then(|mime| mime.split(';').next())
        .map(|mime| mime.trim().to_ascii_lowercase())
        .filter(|mime| !mime.is_empty())
    else {
        return false;
    };
    let renderable = mime.starts_with("text/")
        || mime.starts_with("image/")
        || mime.starts_with("audio/")
        || mime.starts_with("video/")
        || mime.ends_with("+xml")
        || mime.ends_with("+json")
        || matches!(
            mime.as_str(),
            "application/xml" | "application/json" | "multipart/x-mixed-replace"
        );
    !renderable
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamping_round_trips_without_reencoding_the_base_query() {
        let base = "lx://lxapp/demo/pages/home/index.tsx?q=a%20b&x=1#top";
        let stamped = stamp_document_url(base, 7).unwrap();
        assert_eq!(
            stamped,
            "lx://lxapp/demo/pages/home/index.tsx?q=a%20b&x=1&__lxdoc=7#top"
        );
        assert_eq!(unstamped(&stamped), base);
    }

    #[test]
    fn stamping_a_query_less_url_removes_the_query_again() {
        let stamped = stamp_document_url("lingxia://browser/load-error", 3).unwrap();
        assert_eq!(stamped, "lingxia://browser/load-error?__lxdoc=3");
        assert_eq!(unstamped(&stamped), "lingxia://browser/load-error");
    }

    #[test]
    fn unstamping_leaves_page_owned_parameters_alone() {
        for url in [
            "https://example.com/?__lxdoc=abc",
            "https://example.com/?__lxdoc=1&next=2",
            "https://example.com/?my__lxdoc=1",
            "https://example.com/",
        ] {
            assert_eq!(unstamped(url), url);
        }
    }

    #[test]
    fn attachments_and_undisplayable_types_are_downloads() {
        assert!(is_download_response(
            Some("attachment; filename=a.txt"),
            Some("text/plain")
        ));
        assert!(is_download_response(None, Some("application/zip")));
        assert!(is_download_response(
            None,
            Some("application/octet-stream; charset=binary")
        ));
        assert!(!is_download_response(Some("inline"), Some("text/html")));
        assert!(!is_download_response(None, Some("image/svg+xml")));
        assert!(!is_download_response(None, Some("application/json")));
        assert!(!is_download_response(None, None));
    }
}
