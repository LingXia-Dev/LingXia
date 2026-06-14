//! Windows native-view bridge for WebView2.
//!
//! The Windows UI layer owns HWND creation, presentation, layout, and thread
//! marshalling. `lingxia-webview` only asks that layer for the parent HWND
//! required by WebView2 and then drives the WebView2 controller.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowsWebViewNativeView {
    pub window: isize,
}

pub trait WindowsWebViewNativeViewHost: Send + Sync {
    fn create_webview_parent(&self, webtag: &WebTag) -> StdResult<WindowsWebViewNativeView>;
    fn destroy_webview_parent(&self, webtag_key: &str, view: WindowsWebViewNativeView);

    fn webview_parent_bounds(&self, view: WindowsWebViewNativeView) -> StdResult<RECT> {
        let hwnd = hwnd_from_handle(view.window);
        let mut rect = RECT::default();
        unsafe {
            WindowsAndMessaging::GetClientRect(hwnd, &mut rect)
                .map_err(|err| WebViewError::WebView(format!("GetClientRect failed: {err}")))?;
        }
        Ok(rect)
    }
}

#[derive(Clone)]
pub struct WindowsWebViewHandler {
    webview: Arc<crate::WebView>,
}

impl std::fmt::Debug for WindowsWebViewHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowsWebViewHandler")
            .field("webtag", &self.webview.webtag())
            .finish()
    }
}

impl WindowsWebViewHandler {
    pub fn webtag(&self) -> WebTag {
        self.webview.webtag()
    }

    pub fn native_view(&self) -> WindowsWebViewNativeView {
        WindowsWebViewNativeView {
            window: self.webview.inner.native_view,
        }
    }

    pub fn open_devtools(&self) -> StdResult<()> {
        self.webview.inner.open_devtools()
    }

    pub fn set_content_bounds(
        &self,
        left: i32,
        top: i32,
        width: i32,
        height: i32,
    ) -> StdResult<()> {
        self.webview.inner.set_content_bounds(RECT {
            left,
            top,
            right: left + width.max(0),
            bottom: top + height.max(0),
        })
    }

    pub fn set_content_visible(&self, visible: bool) -> StdResult<()> {
        self.webview.inner.set_content_visible(visible)
    }

    pub fn notify_parent_position_changed(&self) -> StdResult<()> {
        self.webview.inner.notify_parent_position_changed()
    }
}

static WEBVIEW_USER_DATA_DIR: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
static WEBVIEW_DEVTOOLS_ENABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);
static NATIVE_VIEW_HOST: OnceLock<Arc<dyn WindowsWebViewNativeViewHost>> = OnceLock::new();

pub fn set_webview_native_view_host(host: Arc<dyn WindowsWebViewNativeViewHost>) {
    if NATIVE_VIEW_HOST.set(host).is_err() {
        log::warn!("Windows WebView native-view host is already installed; ignoring replacement");
    }
}

pub fn find_webview_handler(webtag: &WebTag) -> Option<WindowsWebViewHandler> {
    find_webview(webtag).map(|webview| WindowsWebViewHandler { webview })
}

pub fn set_webview_user_data_dir(path: impl Into<PathBuf>) {
    let state = WEBVIEW_USER_DATA_DIR.get_or_init(|| Mutex::new(None));
    if let Ok(mut state) = state.lock() {
        *state = Some(path.into());
    }
}

pub(crate) fn configured_webview_user_data_dir() -> Option<PathBuf> {
    WEBVIEW_USER_DATA_DIR
        .get()
        .and_then(|state| state.lock().ok())
        .and_then(|state| state.clone())
}

pub fn set_webview_devtools_enabled(enabled: bool) {
    WEBVIEW_DEVTOOLS_ENABLED.store(enabled, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn webview_devtools_enabled() -> bool {
    WEBVIEW_DEVTOOLS_ENABLED.load(std::sync::atomic::Ordering::Relaxed)
}

pub(crate) fn create_webview_parent(webtag: &WebTag) -> StdResult<WindowsWebViewNativeView> {
    let Some(host) = NATIVE_VIEW_HOST.get() else {
        return Err(WebViewError::WebView(
            "Windows WebView native-view host is not installed".to_string(),
        ));
    };
    host.create_webview_parent(webtag)
}

pub(crate) fn destroy_webview_parent(webtag_key: &str, view: WindowsWebViewNativeView) {
    if let Some(host) = NATIVE_VIEW_HOST.get() {
        host.destroy_webview_parent(webtag_key, view);
    }
}

pub(crate) fn webview_parent_bounds(view: WindowsWebViewNativeView) -> StdResult<RECT> {
    let Some(host) = NATIVE_VIEW_HOST.get() else {
        return Err(WebViewError::WebView(
            "Windows WebView native-view host is not installed".to_string(),
        ));
    };
    host.webview_parent_bounds(view)
}

pub(crate) fn hwnd_from_handle(handle: isize) -> HWND {
    HWND(handle as *mut c_void)
}

pub(crate) fn hwnd_handle(hwnd: HWND) -> isize {
    hwnd.0 as isize
}
