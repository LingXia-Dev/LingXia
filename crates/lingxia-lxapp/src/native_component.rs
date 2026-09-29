use std::sync::{Arc, OnceLock};

/// In-process receiver of embedded native-component messages.
///
/// On iOS/Android/HarmonyOS the page's component messages
/// (`component.mount`, `component.update`, `component.unmount`, ...) reach
/// the platform component manager through platform channels
/// (WKScriptMessageHandler, JavascriptInterface, embed proxy) without
/// passing through this crate. Platforms whose component manager is plain
/// in-process Rust (currently Windows) register a host here instead; the
/// page delegate then forwards every component message it receives from
/// the view, keeping this crate platform-agnostic.
pub trait NativeComponentHost: Send + Sync {
    /// Handles one raw component message (JSON text) sent by `page`'s view.
    fn on_component_message(&self, page: &crate::PageInstance, message_json: &str);

    /// Tears down every component mounted by the page identified by
    /// `page_key` (the page's webview-tag key); called when the page
    /// instance is destroyed.
    fn on_page_destroyed(&self, page_key: &str);

    /// Notifies that the page identified by `page_key` left or re-entered
    /// the foreground (page navigation): hidden pages must hide their
    /// component overlays and pause playback, mirroring the
    /// inactive/active handling of the platform component managers.
    /// Default: no-op (platform channels deliver their own lifecycle).
    fn on_page_visibility(&self, _page_key: &str, _visible: bool) {}
}

static NATIVE_COMPONENT_HOST: OnceLock<Arc<dyn NativeComponentHost>> = OnceLock::new();

/// Registers the process-wide native-component host. Returns `false` when a
/// host was already registered (the first registration wins).
pub fn register_native_component_host(host: Arc<dyn NativeComponentHost>) -> bool {
    NATIVE_COMPONENT_HOST.set(host).is_ok()
}

/// Routes a component message from a page view to the registered host;
/// pages on platforms without an in-process host drop the message (their
/// component traffic never reaches this path).
pub(crate) fn dispatch_component_message(page: &crate::PageInstance, message_json: &str) {
    match NATIVE_COMPONENT_HOST.get() {
        Some(host) => host.on_component_message(page, message_json),
        None => {
            crate::debug!("no native-component host registered; dropping component message");
        }
    }
}

/// Notifies the registered host (if any) that a page instance is gone.
pub(crate) fn notify_page_destroyed(page_key: &str) {
    if let Some(host) = NATIVE_COMPONENT_HOST.get() {
        host.on_page_destroyed(page_key);
    }
}

/// Notifies the registered host (if any) that a page left or re-entered
/// the foreground.
pub(crate) fn notify_page_visibility(page_key: &str, visible: bool) {
    if let Some(host) = NATIVE_COMPONENT_HOST.get() {
        host.on_page_visibility(page_key, visible);
    }
}
