//! One-way `window.lingxia.postMessage` mailbox for [`SecurityProfile::Embedded`].
//!
//! Unlike the lxapp bridge, an embedded document never receives routes, storage,
//! or outbound host messages: it gets a single top-level-only `postMessage`
//! entrypoint whose JSON payload size is bounded at 64 KiB and whose `origin`
//! is supplied by the host from platform frame metadata.

use crate::webview::MAX_WEB_MESSAGE_BYTES;

/// Handler name registered in WebKit's `WKUserContentController` and Android's
/// `WebViewCompat.addWebMessageListener`.
#[cfg(any(target_os = "ios", target_os = "macos", test))]
pub(crate) const APPLE_EMBED_HANDLER: &str = "LingXiaEmbed";
#[cfg(test)]
pub(crate) const ANDROID_EMBED_OBJECT: &str = "__LingXiaEmbedBridge";
#[cfg(any(all(target_os = "linux", target_env = "ohos"), test))]
pub(crate) const HARMONY_EMBED_PORT_INIT: &str = "LingXia-embed-port";

/// Maximum wire bytes accepted from the page shim (`'0'|'1'`|'f'|'x' tag + JSON).
pub(crate) const MAX_EMBED_WIRE_BYTES: usize = 1 + MAX_WEB_MESSAGE_BYTES;

/// Validated message from `window.lingxia.postMessage(data, options?)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbedWebMessage {
    /// Validated UTF-8 JSON text (`<= 65536` bytes).
    pub json: String,
    /// `true` when `{ queue: true }` was requested; `false` coalesces per frame.
    pub queued: bool,
    /// Canonical origin attested by the platform (`scheme://host[:port]`).
    pub origin: String,
}

/// Decoded wire frame emitted by the injected embed bootstrap script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EmbedWireFrame {
    Message { json: String, queued: bool },
    Fullscreen(bool),
}

/// Parse and validate a raw wire string from the injected page shim.
pub(crate) fn parse_wire_frame(raw: &str) -> Option<EmbedWireFrame> {
    if raw.is_empty() || raw.len() > MAX_EMBED_WIRE_BYTES {
        return None;
    }
    let (prefix, body) = raw.split_at(1);
    match prefix {
        "0" | "1" => {
            if body.is_empty() || body.len() > MAX_WEB_MESSAGE_BYTES {
                return None;
            }
            serde_json::from_str::<serde::de::IgnoredAny>(body).ok()?;
            Some(EmbedWireFrame::Message {
                json: body.to_string(),
                queued: prefix == "1",
            })
        }
        "f" if body == "1" => Some(EmbedWireFrame::Fullscreen(true)),
        "f" if body == "0" => Some(EmbedWireFrame::Fullscreen(false)),
        _ => None,
    }
}

/// Normalize a platform-reported URL or origin into canonical `scheme://host[:port]`.
///
/// Default ports (`:80` for `http`, `:443` for `https`, and `:0` from WebKit's
/// `WKSecurityOrigin`) are omitted. Opaque or malformed origins return `None`.
pub fn canonical_origin(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("null") {
        return None;
    }
    let without_fragment = trimmed.split('#').next().unwrap_or(trimmed);
    let uri: http::Uri = without_fragment.parse().ok()?;
    let scheme = uri.scheme_str()?.to_ascii_lowercase();
    if !matches!(scheme.as_str(), "https" | "http" | "lx") {
        return None;
    }
    let authority = uri.authority()?;
    let host = authority.host().trim();
    if host.is_empty() {
        return None;
    }
    let host_lower = host.to_ascii_lowercase();
    let formatted_host = if host_lower.contains(':') && !host_lower.starts_with('[') {
        format!("[{host_lower}]")
    } else {
        host_lower
    };
    let default_port = match scheme.as_str() {
        "http" => Some(80),
        "https" => Some(443),
        _ => None,
    };
    match authority.port_u16() {
        Some(0) => Some(format!("{scheme}://{formatted_host}")),
        Some(port) if Some(port) != default_port => {
            Some(format!("{scheme}://{formatted_host}:{port}"))
        }
        _ => Some(format!("{scheme}://{formatted_host}")),
    }
}

/// Shared JS body that installs `window.lingxia.postMessage(data, options)`
/// given a closure-private `sendWire(str)` function.
#[cfg(any(target_os = "ios", target_os = "macos", windows, test))]
macro_rules! embed_install_body {
    ($send_setup:expr, $fullscreen_polyfill:expr) => {
        concat!(
            "(function(){\n",
            "  if (window !== window.top) return;\n",
            $send_setup,
            "  var p = location.protocol;\n",
            "  if (p !== 'https:' && p !== 'http:' && p !== 'lx:') return;\n",
            "  if (Object.prototype.hasOwnProperty.call(window, 'lingxia')) return;\n",
            "  var enc = typeof TextEncoder === 'function' ? new TextEncoder() : null;\n",
            "  function utf8Len(s) {\n",
            "    if (enc) return enc.encode(s).length;\n",
            "    return unescape(encodeURIComponent(s)).length;\n",
            "  }\n",
            "  var api = Object.freeze({\n",
            "    postMessage: function(data, options) {\n",
            "      if (options !== undefined && options !== null && typeof options !== 'object') {\n",
            "        throw new TypeError('window.lingxia.postMessage: options must be an object');\n",
            "      }\n",
            "      var queue = !!(options && options.queue === true);\n",
            "      var raw = JSON.stringify(data);\n",
            "      if (typeof raw !== 'string') {\n",
            "        throw new TypeError('window.lingxia.postMessage: data must be JSON-serializable');\n",
            "      }\n",
            "      if (utf8Len(raw) > 65536) {\n",
            "        throw new RangeError('window.lingxia.postMessage: payload exceeds 65536 UTF-8 bytes');\n",
            "      }\n",
            "      sendWire((queue ? '1' : '0') + raw);\n",
            "    }\n",
            "  });\n",
            "  try {\n",
            "    Object.defineProperty(window, 'lingxia', { value: api, configurable: false, enumerable: true, writable: false });\n",
            "  } catch (_) {}\n",
            $fullscreen_polyfill,
            "})();\n",
        )
    };
}

/// Apple main-frame bootstrap (`WKUserScript`, `forMainFrameOnly: true`).
/// Also bridges standard Fullscreen API on iPhone `WKWebView` where element
/// fullscreen is not natively exposed on `HTMLElement`.
#[cfg(any(target_os = "ios", target_os = "macos", test))]
pub(crate) const APPLE_BOOTSTRAP_SCRIPT: &str = embed_install_body!(
    concat!(
        "  var h = window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.LingXiaEmbed;\n",
        "  if (!h || typeof h.postMessage !== 'function') return;\n",
        "  var sendWire = h.postMessage.bind(h);\n",
    ),
    concat!(
        "  document.addEventListener('fullscreenchange', function() {\n",
        "    try { sendWire(document.fullscreenElement ? 'f1' : 'f0'); } catch (_) {}\n",
        "  }, true);\n",
        "  document.addEventListener('webkitfullscreenchange', function() {\n",
        "    try { sendWire((document.fullscreenElement || document.webkitFullscreenElement) ? 'f1' : 'f0'); } catch (_) {}\n",
        "  }, true);\n",
        "  if (typeof Element !== 'undefined' && typeof Element.prototype.requestFullscreen !== 'function' && typeof Element.prototype.webkitRequestFullscreen !== 'function') {\n",
        "    var fsEl = null, fsPrev = '';\n",
        "    function exitShim() {\n",
        "      if (!fsEl) return;\n",
        "      var el = fsEl; fsEl = null;\n",
        "      try { el.setAttribute('style', fsPrev); if (!fsPrev) el.removeAttribute('style'); } catch (_) {}\n",
        "      sendWire('f0');\n",
        "      try { document.dispatchEvent(new Event('fullscreenchange', { bubbles: true })); } catch (_) {}\n",
        "    }\n",
        "    try {\n",
        "      Object.defineProperty(Document.prototype, 'fullscreenEnabled', { configurable: true, get: function() { return true; } });\n",
        "      Object.defineProperty(Document.prototype, 'fullscreenElement', { configurable: true, get: function() { return fsEl; } });\n",
        "      Element.prototype.requestFullscreen = function() {\n",
        "        var self = this;\n",
        "        if (fsEl && fsEl !== self) exitShim();\n",
        "        if (fsEl !== self) {\n",
        "          fsPrev = self.getAttribute('style') || '';\n",
        "          fsEl = self;\n",
        "          try { self.style.cssText += ';position:fixed !important;inset:0 !important;width:100vw !important;height:100vh !important;z-index:2147483647 !important;'; } catch (_) {}\n",
        "          sendWire('f1');\n",
        "          try { document.dispatchEvent(new Event('fullscreenchange', { bubbles: true })); } catch (_) {}\n",
        "        }\n",
        "        return Promise.resolve();\n",
        "      };\n",
        "      Document.prototype.exitFullscreen = function() {\n",
        "        exitShim();\n",
        "        return Promise.resolve();\n",
        "      };\n",
        "    } catch (_) {}\n",
        "  }\n",
    )
);

/// Windows document-created bootstrap. WebView2 reports the top-level document's own URL as
/// each message's source, and gives the element fullscreen API natively.
#[cfg(any(windows, test))]
pub(crate) const WINDOWS_BOOTSTRAP_SCRIPT: &str = embed_install_body!(
    concat!(
        "  var w = window.chrome && window.chrome.webview;\n",
        "  if (!w || typeof w.postMessage !== 'function') return;\n",
        "  var sendWire = w.postMessage.bind(w);\n",
    ),
    concat!(
        "  document.addEventListener('fullscreenchange', function() {\n",
        "    try { sendWire(document.fullscreenElement ? 'f1' : 'f0'); } catch (_) {}\n",
        "  }, true);\n",
    )
);

/// Android document-start bootstrap (`WebViewCompat.addDocumentStartJavaScript`),
/// mirrored in `LingXiaWebView.java`.
#[cfg(test)]
pub(crate) const ANDROID_BOOTSTRAP_SCRIPT: &str = concat!(
    "(function(){\n",
    "  var b = window.__LingXiaEmbedBridge;\n",
    "  try { delete window.__LingXiaEmbedBridge; } catch (_) {}\n",
    "  if (window !== window.top || !b || typeof b.postMessage !== 'function') return;\n",
    "  var sendWire = b.postMessage.bind(b);\n",
    "  var p = location.protocol;\n",
    "  if (p !== 'https:' && p !== 'http:' && p !== 'lx:') return;\n",
    "  if (Object.prototype.hasOwnProperty.call(window, 'lingxia')) return;\n",
    "  var enc = typeof TextEncoder === 'function' ? new TextEncoder() : null;\n",
    "  function utf8Len(s) {\n",
    "    if (enc) return enc.encode(s).length;\n",
    "    return unescape(encodeURIComponent(s)).length;\n",
    "  }\n",
    "  var api = Object.freeze({\n",
    "    postMessage: function(data, options) {\n",
    "      if (options !== undefined && options !== null && typeof options !== 'object') {\n",
    "        throw new TypeError('window.lingxia.postMessage: options must be an object');\n",
    "      }\n",
    "      var queue = !!(options && options.queue === true);\n",
    "      var raw = JSON.stringify(data);\n",
    "      if (typeof raw !== 'string') {\n",
    "        throw new TypeError('window.lingxia.postMessage: data must be JSON-serializable');\n",
    "      }\n",
    "      if (utf8Len(raw) > 65536) {\n",
    "        throw new RangeError('window.lingxia.postMessage: payload exceeds 65536 UTF-8 bytes');\n",
    "      }\n",
    "      sendWire((queue ? '1' : '0') + raw);\n",
    "    }\n",
    "  });\n",
    "  try {\n",
    "    Object.defineProperty(window, 'lingxia', { value: api, configurable: false, enumerable: true, writable: false });\n",
    "  } catch (_) {}\n",
    "})();\n",
);

/// HarmonyOS document-start bootstrap (`javaScriptOnDocumentStart` in `LingXiaWebView.ets`).
/// Captures the one-shot `MessagePort` transferred at document commit without exposing it,
/// rejecting any `Window.postMessage` (`ev.source != null`) or guessed token from an `<iframe>`.
#[cfg(test)]
pub(crate) const HARMONY_BOOTSTRAP_SCRIPT: &str = embed_install_body!(
    concat!(
        "  var initToken = \"LingXia-embed-port:<per-view-secret>\";\n",
        "  var port = null, early = [];\n",
        "  window.addEventListener('message', function onInit(ev) {\n",
        "    if (ev && !ev.source && ev.data === initToken && ev.ports && ev.ports.length === 1) {\n",
        "      ev.stopImmediatePropagation();\n",
        "      window.removeEventListener('message', onInit, true);\n",
        "      port = ev.ports[0];\n",
        "      for (var i = 0; i < early.length; i++) {\n",
        "        try { port.postMessage(early[i]); } catch (_) {}\n",
        "      }\n",
        "      early.length = 0;\n",
        "    }\n",
        "  }, true);\n",
        "  function sendWire(w) {\n",
        "    if (port) { port.postMessage(w); }\n",
        "    else if (early.length < 128) { early.push(w); }\n",
        "  }\n",
    ),
    ""
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_coalesced_and_queued_frames_within_cap() {
        assert_eq!(
            parse_wire_frame(r#"0{"lat":31.23,"lng":121.47}"#),
            Some(EmbedWireFrame::Message {
                json: r#"{"lat":31.23,"lng":121.47}"#.to_string(),
                queued: false,
            })
        );
        assert_eq!(
            parse_wire_frame(r#"1"picked""#),
            Some(EmbedWireFrame::Message {
                json: r#""picked""#.to_string(),
                queued: true,
            })
        );
        assert_eq!(
            parse_wire_frame("f1"),
            Some(EmbedWireFrame::Fullscreen(true))
        );
        assert_eq!(
            parse_wire_frame("f0"),
            Some(EmbedWireFrame::Fullscreen(false))
        );

        let exact = format!("0\"{}\"", "a".repeat(MAX_WEB_MESSAGE_BYTES - 2));
        assert_eq!(exact.len(), MAX_EMBED_WIRE_BYTES);
        assert!(parse_wire_frame(&exact).is_some());

        let one_over = format!("0\"{}\"", "a".repeat(MAX_WEB_MESSAGE_BYTES - 1));
        assert!(parse_wire_frame(&one_over).is_none());
        assert!(parse_wire_frame("0not-json").is_none());
        assert!(parse_wire_frame("").is_none());
        assert!(parse_wire_frame("2{}").is_none());
    }

    #[test]
    fn normalizes_origins_and_drops_default_or_zero_ports() {
        assert_eq!(
            canonical_origin("HTTPS://Maps.Example.COM:443/pick?a=1#b"),
            Some("https://maps.example.com".to_string())
        );
        assert_eq!(
            canonical_origin("https://maps.example.com:0"),
            Some("https://maps.example.com".to_string())
        );
        assert_eq!(
            canonical_origin("http://127.0.0.1:18473/page"),
            Some("http://127.0.0.1:18473".to_string())
        );
        assert_eq!(
            canonical_origin("http://127.0.0.1:80/page"),
            Some("http://127.0.0.1".to_string())
        );
        assert_eq!(
            canonical_origin("lx://embed-42/index.html"),
            Some("lx://embed-42".to_string())
        );
        assert_eq!(canonical_origin("null"), None);
        assert_eq!(canonical_origin("about:blank"), None);
        assert_eq!(canonical_origin("data:text/html,hi"), None);
        assert_eq!(canonical_origin(""), None);
    }

    #[test]
    fn bootstrap_scripts_stay_non_empty() {
        assert!(APPLE_BOOTSTRAP_SCRIPT.contains("LingXiaEmbed"));
        assert!(ANDROID_BOOTSTRAP_SCRIPT.contains(ANDROID_EMBED_OBJECT));
        assert!(HARMONY_BOOTSTRAP_SCRIPT.contains(HARMONY_EMBED_PORT_INIT));
        assert!(HARMONY_BOOTSTRAP_SCRIPT.contains("!ev.source"));
        assert!(HARMONY_BOOTSTRAP_SCRIPT.contains("ev.data === initToken"));
    }
}
