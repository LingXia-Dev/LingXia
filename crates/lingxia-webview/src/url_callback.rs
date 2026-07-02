//! Process-local URL callback channels for WebView navigation handoff.
//!
//! A caller that presents a web surface and waits for a sentinel navigation
//! (e.g. an authorization page finishing with `lingxia-auth://callback?...`)
//! opens a channel for the sentinel URL, presents the surface, and awaits the
//! intercepted URL:
//!
//! ```no_run
//! # async fn handoff() -> Result<(), lingxia_webview::url_callback::InvalidCallbackUrl> {
//! let mut callback = lingxia_webview::url_callback::open_channel("lingxia-auth://callback")?;
//! // ... present the web surface, then:
//! let url = callback.recv().await; // "lingxia-auth://callback?code=..."
//! # Ok(())
//! # }
//! ```
//!
//! While a channel is open, any managed WebView navigation (or new-window
//! request) whose URL matches the channel's callback URL is cancelled instead
//! of loading, and the full URL is delivered to the channel. The sentinel is a
//! plain navigation target string: no AppLink, OS custom-scheme registration,
//! or scheme handler is involved.
//!
//! Interception is process-wide — any web content able to navigate can hit the
//! sentinel — so tie the delivered URL back to the flow that opened the
//! surface (e.g. validate a nonce/state query parameter) before trusting it.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use tokio::sync::mpsc;

/// Error returned by [`open_channel`] for an unusable callback URL.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid callback URL {url:?}: must be an absolute URL with a scheme")]
pub struct InvalidCallbackUrl {
    pub url: String,
}

/// Receives WebView navigations intercepted for its callback URL.
/// Dropping the channel stops the interception.
#[derive(Debug)]
pub struct UrlCallbackChannel {
    id: u64,
    callback_url: String,
    receiver: mpsc::UnboundedReceiver<String>,
}

impl UrlCallbackChannel {
    /// The callback URL this channel matches, in normalized (match-key) form.
    pub fn callback_url(&self) -> &str {
        &self.callback_url
    }

    /// Waits for the next navigation to the callback URL and returns the full
    /// navigated URL, query and fragment included.
    ///
    /// Pends indefinitely until a matching navigation happens — bound the wait
    /// externally (a timeout, or racing dismissal of the presenting surface).
    pub async fn recv(&mut self) -> String {
        self.receiver
            .recv()
            .await
            .expect("registry holds the sender for the channel's lifetime")
    }

    /// Returns an already-intercepted URL without waiting.
    pub fn try_recv(&mut self) -> Option<String> {
        self.receiver.try_recv().ok()
    }
}

impl Drop for UrlCallbackChannel {
    fn drop(&mut self) {
        unregister(self.id);
    }
}

/// Open a callback channel for `callback_url`, e.g. `"lingxia-auth://callback"`.
///
/// Matching compares whole URLs with the query, fragment, and any trailing `/`
/// ignored; the scheme and authority compare case-insensitively, the path
/// exactly. `lingxia-auth://callback?code=x` matches a channel opened for
/// `lingxia-auth://callback`; `lingxia-auth://callback/extra` does not.
///
/// Channels may overlap: the most recently opened channel matching a URL
/// receives it. Errs when `callback_url` is empty or has no scheme.
pub fn open_channel(
    callback_url: impl Into<String>,
) -> Result<UrlCallbackChannel, InvalidCallbackUrl> {
    let raw = callback_url.into();
    let Some(key) = match_key(&raw) else {
        return Err(InvalidCallbackUrl { url: raw });
    };
    let (sender, receiver) = mpsc::unbounded_channel();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let mut entries = registry().lock().unwrap();
    entries.push(Entry {
        id,
        callback_url: key.clone(),
        sender,
    });
    CHANNEL_COUNT.store(entries.len(), Ordering::Release);
    drop(entries);
    Ok(UrlCallbackChannel {
        id,
        callback_url: key,
        receiver,
    })
}

/// Route a navigation URL to the most recently opened matching channel.
/// Returns `true` when a channel took it, in which case the navigation must be
/// cancelled. Managed WebViews dispatch automatically in `handle_navigation`;
/// this is public for platform surfaces backed by a bare native WebView, whose
/// navigation delegates must offer the URL themselves. Called for every
/// navigation, so the no-channel path is one atomic load.
pub fn dispatch(url: &str) -> bool {
    if CHANNEL_COUNT.load(Ordering::Acquire) == 0 {
        return false;
    }
    let Some(key) = match_key(url) else {
        return false;
    };
    let entries = registry().lock().unwrap();
    entries
        .iter()
        .rev()
        .any(|entry| entry.callback_url == key && entry.sender.send(url.trim().to_string()).is_ok())
}

#[derive(Debug)]
struct Entry {
    id: u64,
    callback_url: String,
    sender: mpsc::UnboundedSender<String>,
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static CHANNEL_COUNT: AtomicUsize = AtomicUsize::new(0);
static REGISTRY: OnceLock<Mutex<Vec<Entry>>> = OnceLock::new();

fn registry() -> &'static Mutex<Vec<Entry>> {
    REGISTRY.get_or_init(|| Mutex::new(Vec::new()))
}

fn unregister(id: u64) {
    if let Some(registry) = REGISTRY.get() {
        let mut entries = registry.lock().unwrap();
        entries.retain(|entry| entry.id != id);
        CHANNEL_COUNT.store(entries.len(), Ordering::Release);
    }
}

/// Normalize a URL to its match key: query/fragment stripped, trailing `/`
/// trimmed, scheme and authority lowercased (both are case-insensitive per
/// RFC 3986), path left exact. `None` when the URL has no usable scheme.
fn match_key(url: &str) -> Option<String> {
    let base = strip_query_fragment(url.trim()).trim_end_matches('/');
    let scheme_end = base.find(':')?;
    let (scheme, rest) = base.split_at(scheme_end);
    if scheme.is_empty()
        || !scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    {
        return None;
    }
    let mut key = scheme.to_ascii_lowercase();
    if let Some(authority_and_path) = rest.strip_prefix("://") {
        let (authority, path) = match authority_and_path.find('/') {
            Some(idx) => authority_and_path.split_at(idx),
            None => (authority_and_path, ""),
        };
        key.push_str("://");
        key.push_str(&authority.to_ascii_lowercase());
        key.push_str(path);
    } else {
        key.push_str(rest);
    }
    Some(key)
}

fn strip_query_fragment(value: &str) -> &str {
    let end = value.find(['?', '#']).unwrap_or(value.len());
    &value[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_receives_matching_custom_scheme_url() {
        let mut channel = open_channel("lingxia-test-basic://callback").expect("open channel");

        assert!(dispatch("lingxia-test-basic://callback?code=abc&next=%2F"));
        assert_eq!(
            channel.try_recv().as_deref(),
            Some("lingxia-test-basic://callback?code=abc&next=%2F")
        );
    }

    #[test]
    fn https_callback_matches_exact_path_only() {
        let mut channel =
            open_channel("https://auth.test-exact.example/callback").expect("open channel");

        assert!(!dispatch("https://auth.test-exact.example/other?code=abc"));
        assert!(!dispatch("https://auth.test-exact.example/callback/extra"));
        assert!(channel.try_recv().is_none());

        assert!(dispatch(
            "https://auth.test-exact.example/callback/?code=abc#frag"
        ));
        assert_eq!(
            channel.try_recv().as_deref(),
            Some("https://auth.test-exact.example/callback/?code=abc#frag")
        );
    }

    #[test]
    fn scheme_and_authority_match_case_insensitively() {
        let mut channel = open_channel("Lingxia-Test-Case://CallBack/Path").expect("open channel");

        assert!(dispatch("lingxia-test-case://callback/Path?x=1"));
        assert_eq!(
            channel.try_recv().as_deref(),
            Some("lingxia-test-case://callback/Path?x=1")
        );
        // The path is case-sensitive.
        assert!(!dispatch("lingxia-test-case://callback/path"));
    }

    #[test]
    fn newest_matching_channel_wins_until_dropped() {
        let mut first = open_channel("lingxia-test-lifo://callback").expect("open first");
        let mut second = open_channel("lingxia-test-lifo://callback").expect("open second");

        assert!(dispatch("lingxia-test-lifo://callback?to=second"));
        assert!(first.try_recv().is_none());
        assert_eq!(
            second.try_recv().as_deref(),
            Some("lingxia-test-lifo://callback?to=second")
        );

        drop(second);
        assert!(dispatch("lingxia-test-lifo://callback?to=first"));
        assert_eq!(
            first.try_recv().as_deref(),
            Some("lingxia-test-lifo://callback?to=first")
        );
    }

    #[test]
    fn dropped_channel_stops_interception() {
        let channel = open_channel("lingxia-test-drop://callback").expect("open channel");
        drop(channel);

        assert!(!dispatch("lingxia-test-drop://callback?code=abc"));
    }

    #[test]
    fn rejects_callback_url_without_scheme() {
        assert!(open_channel("").is_err());
        assert!(open_channel("   ").is_err());
        assert!(open_channel("callback").is_err());
        assert!(open_channel("/path/only").is_err());
        assert!(open_channel(":no-scheme").is_err());
    }

    #[tokio::test]
    async fn recv_returns_dispatched_url() {
        let mut channel = open_channel("lingxia-test-recv://callback").expect("open channel");
        assert!(dispatch("lingxia-test-recv://callback?ok=1"));
        assert_eq!(channel.recv().await, "lingxia-test-recv://callback?ok=1");
    }
}
