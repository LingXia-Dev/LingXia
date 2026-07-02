//! Process-local URL callback channels for WebView navigation handoff.
//!
//! For flows where the host presents a web surface and waits for a sentinel
//! navigation, such as an authorization page finishing with
//! `lingxia-auth://callback?code=...`:
//!
//! ```no_run
//! # async fn handoff() -> lingxia::Result<()> {
//! let mut callback = lingxia::url_callback::open_channel("lingxia-auth://callback")?;
//! // ... present the web surface, then:
//! let url = callback.recv().await; // "lingxia-auth://callback?code=..."
//! # Ok(())
//! # }
//! ```
//!
//! While the channel is open, any managed WebView navigation matching the
//! callback URL is cancelled instead of loading and delivered to the channel.
//! The sentinel is a plain navigation target string — no AppLink or OS
//! custom-scheme registration is involved. Interception is process-wide, so
//! validate a nonce/state query parameter before trusting the delivered URL.

pub use lingxia_webview::url_callback::UrlCallbackChannel;

/// Open a callback channel for `callback_url`, e.g. `"lingxia-auth://callback"`.
///
/// Matching compares whole URLs with the query, fragment, and any trailing `/`
/// ignored; the scheme and authority compare case-insensitively, the path
/// exactly. When open channels overlap, the most recently opened one matching
/// a URL receives it. Errs when `callback_url` is empty or has no scheme.
pub fn open_channel(callback_url: impl Into<String>) -> crate::Result<UrlCallbackChannel> {
    lingxia_webview::url_callback::open_channel(callback_url)
        .map_err(|err| crate::Error::invalid_request(err.to_string()))
}
