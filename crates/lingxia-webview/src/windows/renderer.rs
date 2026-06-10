//! Window chrome renderer seam.
//!
//! `lingxia-webview` hosts WebView2 content in plain Win32 windows and owns
//! only generic window mechanics. A product layer (e.g. `lingxia-shell`) may
//! register a [`WindowsChromeRenderer`] to take over non-client handling:
//! painting custom chrome (tab bars, sidebars, panels) and mapping points to
//! chrome elements. When no renderer is registered, windows fall back to a
//! standard OS frame (`WS_OVERLAPPEDWINDOW`) with no custom non-client
//! handling and the webview fills the whole client area.

use super::*;

/// Native panel content kind, mirrored from the window-group registry for
/// renderers (which cannot see crate-internal group state).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsNativePanelKind {
    /// Plain title/body text panel.
    Text,
    /// Terminal panel (body is the terminal screen text).
    Terminal,
}

/// Content of a panel that is drawn natively by the chrome renderer
/// (as opposed to panels backed by their own webview window).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsNativePanelContent {
    pub kind: WindowsNativePanelKind,
    pub title: Option<String>,
    pub body: Option<String>,
}

/// One attached panel of a host window, in client coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowsChromePanel {
    /// Stable panel identifier (matches the activator id).
    pub panel_id: String,
    /// Panel rect in host-window client coordinates.
    pub rect: RECT,
    /// `Some` when the panel content is drawn natively by the renderer;
    /// `None` when a separate webview window covers the rect.
    pub native: Option<WindowsNativePanelContent>,
}

/// Attached-group geometry of a host window: the main content card plus the
/// currently attached panels.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowsChromeAttachedState {
    /// Rect of the main webview content card.
    pub main: RECT,
    /// Attached panels in registration order.
    pub panels: Vec<WindowsChromePanel>,
}

/// Everything a chrome renderer needs to paint or hit-test one host window.
#[derive(Debug, Clone)]
pub struct WindowsChromeState {
    /// The host window. Renderers may query window state (e.g. `IsZoomed`).
    pub hwnd: HWND,
    /// Full client rect of the host window.
    pub client: RECT,
    /// The product layout registered via `set_webview_window_layout`.
    pub layout: WindowsWindowLayout,
    /// Group geometry; `Some` only for a group host window with attached
    /// panels.
    pub attached: Option<WindowsChromeAttachedState>,
}

/// Window frame buttons handled by lingxia-webview (minimize/maximize/close
/// are window mechanics; the renderer only decides where they are drawn).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsFrameButton {
    Minimize,
    Maximize,
    Close,
}

/// Result of a chrome hit test, mapped by lingxia-webview onto non-client
/// hit-test codes and chrome events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowsChromeHit {
    /// Draggable title-bar area (`HTCAPTION`).
    Caption,
    /// A window frame button; the click is handled by lingxia-webview.
    FrameButton(WindowsFrameButton),
    /// Navigation-bar back button; dispatched as a chrome event.
    NavigationBack,
    /// Navigation-bar home button; dispatched as a chrome event.
    NavigationHome,
    /// A tab-bar item; dispatched as a chrome event.
    TabBarItem { index: usize },
    /// A panel activator; dispatched as a chrome event.
    PanelActivator { panel_id: String },
    /// A natively drawn panel that accepts keyboard focus (e.g. terminal).
    NativePanel { panel_id: String },
    /// Inert chrome surface: consumes the click without producing an event.
    Chrome,
}

/// Renderer of product window chrome, registered by the shell layer.
///
/// All methods are called on the webview UI thread that owns the window.
pub trait WindowsChromeRenderer: Send + Sync {
    /// Content rect reserved for the webview inside `client` for `layout`
    /// (i.e. the client rect minus all chrome insets).
    fn content_rect(&self, client: RECT, layout: &WindowsWindowLayout) -> RECT;

    /// Gap in pixels inserted between the main content card and attached
    /// panels (also the resize-handle thickness baseline).
    fn panel_gap(&self) -> i32;

    /// Corner radius applied to attached webview surfaces.
    fn panel_corner_radius(&self) -> i32;

    /// Paint the full window chrome into `hdc`.
    fn paint(&self, hdc: HDC, state: &WindowsChromeState);

    /// Map a client-space point to a chrome element, or `None` for plain
    /// client area.
    fn hit_test(&self, state: &WindowsChromeState, point: (i32, i32)) -> Option<WindowsChromeHit>;
}

static WINDOWS_CHROME_RENDERER: OnceLock<Mutex<Option<Arc<dyn WindowsChromeRenderer>>>> =
    OnceLock::new();

/// Registers the process-wide window chrome renderer.
///
/// Must be called before the first window is created; windows created
/// without a registered renderer keep a standard OS frame.
pub fn set_windows_chrome_renderer(renderer: Arc<dyn WindowsChromeRenderer>) {
    let slot = WINDOWS_CHROME_RENDERER.get_or_init(|| Mutex::new(None));
    if let Ok(mut slot) = slot.lock() {
        *slot = Some(renderer);
    }
}

pub(crate) fn windows_chrome_renderer() -> Option<Arc<dyn WindowsChromeRenderer>> {
    WINDOWS_CHROME_RENDERER
        .get()
        .and_then(|slot| slot.lock().ok())
        .and_then(|slot| slot.clone())
}

/// Content rect for the webview; the full client rect when no renderer is
/// registered (plain windows have no chrome insets).
pub(crate) fn renderer_content_rect(client: RECT, layout: &WindowsWindowLayout) -> RECT {
    windows_chrome_renderer()
        .map(|renderer| renderer.content_rect(client, layout))
        .unwrap_or(client)
}

pub(crate) fn renderer_panel_gap() -> i32 {
    windows_chrome_renderer()
        .map(|renderer| renderer.panel_gap())
        .unwrap_or(0)
}

pub(crate) fn renderer_panel_radius() -> i32 {
    windows_chrome_renderer()
        .map(|renderer| renderer.panel_corner_radius())
        .unwrap_or(0)
}

/// WM_PAINT handler used when a renderer is registered: validates the window
/// and delegates the actual drawing to the renderer for chrome-owning windows.
pub(crate) fn paint_window_chrome(hwnd: HWND) {
    let mut paint = PAINTSTRUCT::default();
    unsafe {
        let hdc = BeginPaint(hwnd, &mut paint);
        if !hdc.is_invalid()
            && let Some(renderer) = windows_chrome_renderer()
            && let Some(webtag_key) = window_webtag_key(hwnd)
            && window_draws_shell_chrome(&webtag_key)
        {
            let state = chrome_state_for_window(hwnd, &webtag_key);
            renderer.paint(hdc, &state);
        }
        let _ = EndPaint(hwnd, &paint);
    }
}
