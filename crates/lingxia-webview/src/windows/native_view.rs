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

/// Whether a parent HWND may be destroyed yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParentRetirement {
    Ready,
    /// Foreign controllers are still moving out; ask again shortly.
    Deferred,
}

/// Result of asking a controller to leave a retiring parent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParkOutcome {
    Parked,
    /// The controller was not in that parent, or its UI thread is already gone.
    NotInParent,
    /// Its own parking window is retiring too; it will leave on its own.
    Deferred,
}

pub trait WindowsWebViewNativeViewHost: Send + Sync {
    fn create_webview_parent(&self, webtag: &WebTag) -> StdResult<WindowsWebViewNativeView>;
    fn destroy_webview_parent(&self, webtag_key: &str, view: WindowsWebViewNativeView);

    /// Runs off the owner UI thread while it continues pumping messages.
    /// `Deferred` is polled again; an error means parking itself failed.
    fn prepare_destroy_webview_parent(
        &self,
        _view: WindowsWebViewNativeView,
    ) -> StdResult<ParentRetirement> {
        Ok(ParentRetirement::Ready)
    }

    /// Checked on the controller thread before committing a parent transfer.
    fn can_present_in_parent(&self, _window: isize) -> bool {
        true
    }

    /// Return true only after scheduling a consumed native keyboard shortcut.
    fn handle_accelerator(&self, _webtag: &WebTag, _virtual_key: u32) -> bool {
        false
    }

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

/// Page content a host presents like a WebView without being one (a GPU page). It owns a
/// parent window from [`create_native_content_parent`] and a content window inside it; the
/// host positions, shows and reparents it through [`WindowsWebViewHandler`].
pub trait WindowsNativeContent: Send + Sync {
    fn native_view(&self) -> WindowsWebViewNativeView;
    fn has_parent_window(&self, window: isize) -> bool;
    /// Bounds in the parent's client area, device pixels, and corner radii `[tl, tr, br, bl]`.
    fn set_content_geometry(
        &self,
        left: i32,
        top: i32,
        width: i32,
        height: i32,
        corner_radii: [i32; 4],
    ) -> StdResult<()>;
    fn set_content_visible(&self, visible: bool) -> StdResult<()>;
    fn bring_content_to_front(&self) -> StdResult<()>;
    fn move_focus(&self) -> StdResult<()>;
    fn set_parent_window(&self, window: isize) -> StdResult<()>;
    fn park_from_parent(&self, expected: isize, window: isize) -> StdResult<ParkOutcome>;
    fn notify_parent_position_changed(&self) -> StdResult<()> {
        Ok(())
    }
    /// Physical pixels per CSS pixel for a fit-scaled presentation.
    fn set_rasterization_scale(&self, scale: f64) -> StdResult<()>;
    fn set_preferred_color_scheme(&self, scheme: WindowsPreferredColorScheme);
    fn capture_png(&self) -> StdResult<Vec<u8>>;
    /// The window that draws the content and takes its input, wherever it is parented.
    fn content_window(&self) -> Option<isize> {
        None
    }
    /// The last [`Self::set_rasterization_scale`], while one applies.
    fn content_scale(&self) -> Option<f64> {
        None
    }
    /// The size the content lays out at, in points, when it is not the window's.
    fn viewport(&self) -> Option<[f64; 2]> {
        None
    }
}

#[derive(Clone)]
enum Content {
    WebView(Arc<crate::WebView>),
    Native(WebTag, Arc<dyn WindowsNativeContent>),
}

#[derive(Clone)]
pub struct WindowsWebViewHandler {
    content: Content,
}

impl std::fmt::Debug for WindowsWebViewHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowsWebViewHandler")
            .field("webtag", &self.webtag())
            .field("native", &self.is_native_content())
            .finish()
    }
}

impl WindowsWebViewHandler {
    fn webview(&self) -> Option<&Arc<crate::WebView>> {
        match &self.content {
            Content::WebView(webview) => Some(webview),
            Content::Native(..) => None,
        }
    }

    pub fn webtag(&self) -> WebTag {
        match &self.content {
            Content::WebView(webview) => webview.webtag(),
            Content::Native(webtag, _) => webtag.clone(),
        }
    }

    /// Native content (a GPU page) rather than a WebView2 controller.
    pub fn is_native_content(&self) -> bool {
        self.webview().is_none()
    }

    /// Native content's own window (not its parent); `None` for a WebView.
    pub fn content_window(&self) -> Option<isize> {
        match &self.content {
            Content::WebView(_) => None,
            Content::Native(_, native) => native.content_window(),
        }
    }

    /// Native content's layout size in points when it is not its window's (a simulated device).
    pub fn content_viewport(&self) -> Option<[f64; 2]> {
        match &self.content {
            Content::WebView(_) => None,
            Content::Native(_, native) => native.viewport(),
        }
    }

    /// Native content's physical pixels per CSS pixel under a fit-scaled presentation.
    pub fn content_scale(&self) -> Option<f64> {
        match &self.content {
            Content::WebView(_) => None,
            Content::Native(_, native) => native.content_scale(),
        }
    }

    pub fn native_view(&self) -> WindowsWebViewNativeView {
        match &self.content {
            Content::WebView(webview) => WindowsWebViewNativeView {
                window: webview.inner.native_view,
            },
            Content::Native(_, native) => native.native_view(),
        }
    }

    /// Whether this is the controller's current parent or pending destination.
    pub fn has_parent_window(&self, window: isize) -> bool {
        match &self.content {
            Content::WebView(webview) => webview.inner.has_parent_window(window),
            Content::Native(_, native) => native.has_parent_window(window),
        }
    }

    pub fn open_devtools(&self) -> StdResult<()> {
        match self.webview() {
            Some(webview) => webview.inner.open_devtools(),
            None => Err(WebViewError::WebView(
                "native page content has no DevTools".to_string(),
            )),
        }
    }

    /// Pins the scheme pages observe via `prefers-color-scheme`.
    pub fn set_preferred_color_scheme(&self, scheme: WindowsPreferredColorScheme) -> StdResult<()> {
        match &self.content {
            Content::WebView(webview) => webview.inner.set_preferred_color_scheme(scheme),
            Content::Native(_, native) => {
                native.set_preferred_color_scheme(scheme);
                Ok(())
            }
        }
    }

    /// Same pin as [`Self::set_preferred_color_scheme`], without waiting for
    /// the WebView UI thread. A caller that already saved the preference uses
    /// this so one stalled controller cannot fail the save.
    pub fn post_preferred_color_scheme(&self, scheme: WindowsPreferredColorScheme) {
        match &self.content {
            Content::WebView(webview) => webview.inner.post_preferred_color_scheme(scheme),
            Content::Native(_, native) => native.set_preferred_color_scheme(scheme),
        }
    }

    /// Applies a coherent UA + User-Agent Client Hints form factor.
    pub fn set_browser_emulation_profile(
        &self,
        profile: WindowsBrowserEmulationProfile,
        reload: bool,
    ) -> StdResult<()> {
        let Some(webview) = self.webview() else {
            return Ok(());
        };
        webview.inner.set_browser_emulation_profile(profile)?;
        if reload {
            webview.reload()?;
        }
        Ok(())
    }

    pub fn set_content_bounds(
        &self,
        left: i32,
        top: i32,
        width: i32,
        height: i32,
    ) -> StdResult<()> {
        let rect = content_rect(left, top, width, height);
        match &self.content {
            Content::WebView(webview) => webview.inner.set_content_bounds(rect),
            Content::Native(_, native) => {
                native.set_content_geometry(left, top, width.max(0), height.max(0), [0; 4])
            }
        }
    }

    /// Bounds plus per-corner rounding applied in one composition commit:
    /// clip radii `[tl, tr, br, bl]` and the `0xAARGB` backdrop the corner
    /// wedges paint outside the arc (alpha 0 = clip only, no wedges).
    /// Windowed hosting applies the bounds and ignores the corner style.
    pub fn set_content_geometry(
        &self,
        left: i32,
        top: i32,
        width: i32,
        height: i32,
        corner_radii: [i32; 4],
        corner_color: u32,
    ) -> StdResult<()> {
        let rect = content_rect(left, top, width, height);
        match &self.content {
            Content::WebView(webview) => {
                webview
                    .inner
                    .set_content_geometry(rect, corner_radii, corner_color)
            }
            Content::Native(_, native) => {
                native.set_content_geometry(left, top, width.max(0), height.max(0), corner_radii)
            }
        }
    }

    /// True when this webview renders through the composition-hosted path.
    pub fn is_composition_hosted(&self) -> bool {
        self.webview()
            .is_some_and(|webview| webview.inner.composition_hosted)
    }

    /// `LingXiaWebViewSurface` child HWND when composition-hosted.
    pub fn composition_surface_hwnd(&self) -> Option<isize> {
        self.webview()?;
        super::composition::find_composition_surface_hwnd(self.native_view().window)
    }

    /// `PrintWindow` of the DComp target (including the corner wedges). Not
    /// `CapturePreview`, which sees only WebView2.
    pub fn capture_composition_surface(
        &self,
    ) -> StdResult<super::composition::CompositionSurfacePixels> {
        let hwnd = self.composition_surface_hwnd().ok_or_else(|| {
            WebViewError::WebView("webview is not composition-hosted".to_string())
        })?;
        super::composition::capture_composition_surface_bgra(hwnd)
    }

    /// Rendering scale for a fit-scaled presentation: CSS px = physical px /
    /// `scale`, so a shrunk simulated device keeps its logical viewport.
    pub fn set_rasterization_scale(&self, scale: f64) -> StdResult<()> {
        match &self.content {
            Content::WebView(webview) => webview.inner.set_rasterization_scale(scale),
            Content::Native(_, native) => native.set_rasterization_scale(scale),
        }
    }

    pub fn set_content_visible(&self, visible: bool) -> StdResult<()> {
        match &self.content {
            Content::WebView(webview) => webview.inner.set_content_visible(visible),
            Content::Native(_, native) => native.set_content_visible(visible),
        }
    }

    pub fn bring_content_to_front(&self) -> StdResult<()> {
        match &self.content {
            Content::WebView(webview) => webview.inner.bring_content_to_front(),
            Content::Native(_, native) => native.bring_content_to_front(),
        }
    }

    /// Move keyboard focus into the WebView's content.
    pub fn move_focus(&self) -> StdResult<()> {
        match &self.content {
            Content::WebView(webview) => webview.inner.move_focus(),
            Content::Native(_, native) => native.move_focus(),
        }
    }

    pub fn set_parent_window(&self, window: isize) -> StdResult<()> {
        match &self.content {
            Content::WebView(webview) => webview.inner.set_parent_window(window),
            Content::Native(_, native) => native.set_parent_window(window),
        }
    }

    /// Park only if the controller still belongs to the retiring parent.
    pub fn park_from_parent(&self, expected: isize, window: isize) -> StdResult<ParkOutcome> {
        match &self.content {
            Content::WebView(webview) => webview.inner.park_from_parent(expected, window),
            Content::Native(_, native) => native.park_from_parent(expected, window),
        }
    }

    pub fn notify_parent_position_changed(&self) -> StdResult<()> {
        match &self.content {
            Content::WebView(webview) => webview.inner.notify_parent_position_changed(),
            Content::Native(_, native) => native.notify_parent_position_changed(),
        }
    }

    /// Captures the WebView2 viewport as PNG bytes (`CapturePreview`), blocking
    /// until the capture completes. Unlike a GDI/screen grab this sees only the
    /// webview's own composition — unoccluded, no other windows — which makes it
    /// suitable for transition snapshots.
    pub fn capture_png(&self) -> StdResult<Vec<u8>> {
        match &self.content {
            Content::WebView(webview) => webview.inner.dispatch_screenshot_command(),
            Content::Native(_, native) => native.capture_png(),
        }
    }
}

fn content_rect(left: i32, top: i32, width: i32, height: i32) -> RECT {
    RECT {
        left,
        top,
        right: left + width.max(0),
        bottom: top + height.max(0),
    }
}

static WEBVIEW_USER_DATA_DIR: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
static WEBVIEW_DEVTOOLS_ENABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);
static NATIVE_VIEW_HOST: OnceLock<Arc<dyn WindowsWebViewNativeViewHost>> = OnceLock::new();
static NATIVE_CONTENT: OnceLock<Mutex<HashMap<String, Arc<dyn WindowsNativeContent>>>> =
    OnceLock::new();

fn native_content() -> std::sync::MutexGuard<'static, HashMap<String, Arc<dyn WindowsNativeContent>>>
{
    NATIVE_CONTENT
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

pub fn set_webview_native_view_host(host: Arc<dyn WindowsWebViewNativeViewHost>) {
    if NATIVE_VIEW_HOST.set(host).is_err() {
        log::warn!("Windows WebView native-view host is already installed; ignoring replacement");
    }
}

/// A WebView's handler, else the native content registered under `webtag`.
pub fn find_webview_handler(webtag: &WebTag) -> Option<WindowsWebViewHandler> {
    if let Some(webview) = find_webview(webtag) {
        return Some(WindowsWebViewHandler {
            content: Content::WebView(webview),
        });
    }
    let native = native_content().get(webtag.key()).cloned()?;
    Some(WindowsWebViewHandler {
        content: Content::Native(webtag.clone(), native),
    })
}

/// Present `content` under `webtag`: hosts reach it through [`find_webview_handler`].
pub fn register_native_content(webtag: &WebTag, content: Arc<dyn WindowsNativeContent>) {
    native_content().insert(webtag.key().to_string(), content);
}

/// Every webtag [`find_webview_handler`] resolves: WebViews, then native content.
pub fn list_content_webtags() -> Vec<WebTag> {
    let mut webtags = crate::runtime::list_webviews();
    webtags.extend(
        native_content()
            .keys()
            .map(|key| WebTag::from(key.as_str())),
    );
    webtags
}

/// Remove `webtag`'s native content while it is still `content`.
pub fn unregister_native_content(webtag: &WebTag, content: &Arc<dyn WindowsNativeContent>) {
    let mut registry = native_content();
    if registry
        .get(webtag.key())
        .is_some_and(|current| Arc::ptr_eq(current, content))
    {
        registry.remove(webtag.key());
    }
}

/// A parent window for native content, made by the installed host as a WebView's is. Call on
/// the thread that will own the content.
pub fn create_native_content_parent(webtag: &WebTag) -> StdResult<WindowsWebViewNativeView> {
    create_webview_parent(webtag)
}

pub fn destroy_native_content_parent(webtag_key: &str, view: WindowsWebViewNativeView) {
    destroy_webview_parent(webtag_key, view);
}

/// Move other pages out of `view` before it is destroyed. Call off the parent's thread, which
/// keeps pumping; `Deferred` is asked again.
pub fn retire_native_content_parent(view: WindowsWebViewNativeView) -> StdResult<ParentRetirement> {
    prepare_destroy_webview_parent(view)
}

/// The scheme pages of `webtag`'s lxapp follow: the lxapp's own, else new WebViews'.
pub fn preferred_color_scheme_for(webtag: &WebTag) -> WindowsPreferredColorScheme {
    super::color_scheme::lxapp_color_scheme(webtag)
        .or_else(super::color_scheme::configured_color_scheme)
        .unwrap_or(WindowsPreferredColorScheme::Auto)
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

pub(crate) fn prepare_destroy_webview_parent(
    view: WindowsWebViewNativeView,
) -> StdResult<ParentRetirement> {
    NATIVE_VIEW_HOST
        .get()
        .map_or(Ok(ParentRetirement::Ready), |host| {
            host.prepare_destroy_webview_parent(view)
        })
}

pub(crate) fn can_present_in_parent(window: isize) -> bool {
    NATIVE_VIEW_HOST
        .get()
        .is_none_or(|host| host.can_present_in_parent(window))
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

pub(crate) fn handle_host_accelerator(webtag: &WebTag, virtual_key: u32) -> bool {
    NATIVE_VIEW_HOST
        .get()
        .is_some_and(|host| host.handle_accelerator(webtag, virtual_key))
}
