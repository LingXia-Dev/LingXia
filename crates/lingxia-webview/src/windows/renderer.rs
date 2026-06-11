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

/// One generic tab of a native panel's tab strip. Pure layout data: the
/// product layer owns ids, titles, and what activation means; the renderer
/// only draws the strip and maps clicks back to tab ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsNativePanelTab {
    /// Stable tab identifier assigned by the product layer.
    pub id: u64,
    /// Display title (already resolved by the product layer).
    pub title: String,
    /// Whether this tab is the panel's active tab.
    pub active: bool,
}

/// Content of a panel that is drawn natively by the chrome renderer
/// (as opposed to panels backed by their own webview window).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsNativePanelContent {
    pub kind: WindowsNativePanelKind,
    pub title: Option<String>,
    pub body: Option<String>,
    /// Tab strip drawn in the panel header; empty when the panel has no
    /// tabs (the header then shows `title`). Updated by the product layer
    /// via `set_native_panel_tabs`.
    pub tabs: Vec<WindowsNativePanelTab>,
    /// Whether the panel is currently maximized over the whole content
    /// area (the renderer draws the restore glyph instead of maximize).
    pub maximized: bool,
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
    /// Whether the panel lays out flush against the main card (compact
    /// bottom dock: zero gap, only a thin divider strip between the two).
    /// Renderers draw the shared edge with square corners.
    pub docked: bool,
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
    /// Frame button currently under the cursor, for hover painting.
    /// Tracked by lingxia-webview (client + non-client mouse messages);
    /// `None` when no frame button is hovered.
    pub frame_button_hover: Option<WindowsFrameButton>,
    /// Frame button with an in-progress left click (mouse down, not yet
    /// released), for pressed painting. `None` when no click is in flight.
    pub frame_button_pressed: Option<WindowsFrameButton>,
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
    /// The sidebar "New Tab" browser row; dispatched as a chrome event.
    BrowserNewTab,
    /// A sidebar browser-tab row; dispatched as a chrome event.
    BrowserTab { tab_id: String },
    /// The close glyph of a sidebar browser-tab row; dispatched as a
    /// chrome event.
    BrowserTabClose { tab_id: String },
    /// A panel activator; dispatched as a chrome event.
    PanelActivator { panel_id: String },
    /// A natively drawn panel that accepts keyboard focus (e.g. terminal).
    NativePanel { panel_id: String },
    /// A tab in a native panel's header tab strip; dispatched as a chrome
    /// event (double-clicking the active tab dispatches a rename request).
    NativePanelTab { panel_id: String, tab_id: u64 },
    /// The close glyph of a native panel tab; dispatched as a chrome event.
    NativePanelTabClose { panel_id: String, tab_id: u64 },
    /// The new-tab button of a native panel header; dispatched as a chrome
    /// event.
    NativePanelNewTab { panel_id: String },
    /// The maximize/restore toggle of a native panel header; dispatched as
    /// a chrome event.
    NativePanelMaximize { panel_id: String },
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

    /// Solid color of the per-pixel-alpha corner caps layered over the
    /// corners of attached card surfaces — the background the rounded cards
    /// visually blend into (normally the window background). The default
    /// `None` disables the caps entirely; attached cards then keep plain
    /// square corners (as in the plain-frame fallback).
    fn card_corner_color(&self) -> Option<COLORREF> {
        None
    }

    /// Rect a maximized native panel expands to. The default is the content
    /// rect (panel covers the webview area); product renderers typically
    /// return the whole client area below the caption strip so a maximized
    /// panel covers the sidebar as well.
    fn maximized_panel_rect(&self, client: RECT, layout: &WindowsWindowLayout) -> RECT {
        self.content_rect(client, layout)
    }

    /// Paint the full window chrome into `hdc`.
    fn paint(&self, hdc: HDC, state: &WindowsChromeState);

    /// Map a client-space point to a chrome element, or `None` for plain
    /// client area.
    fn hit_test(&self, state: &WindowsChromeState, point: (i32, i32)) -> Option<WindowsChromeHit>;

    /// Rect of `button` in client coordinates, used by lingxia-webview to
    /// invalidate only the affected button on hover/pressed changes. The
    /// default `None` means the renderer draws no frame buttons (or does not
    /// expose their rects); hover changes then fall back to a full-window
    /// invalidation.
    fn frame_button_rect(
        &self,
        state: &WindowsChromeState,
        button: WindowsFrameButton,
    ) -> Option<RECT> {
        let _ = (state, button);
        None
    }
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

/// Corner-cap color for attached cards; `None` (no renderer, or a renderer
/// that opts out) disables the corner-cap overlays.
pub(crate) fn renderer_card_corner_color() -> Option<COLORREF> {
    windows_chrome_renderer().and_then(|renderer| renderer.card_corner_color())
}

/// Rect a maximized native panel expands to; the content rect when no
/// renderer is registered.
pub(crate) fn renderer_maximized_panel_rect(client: RECT, layout: &WindowsWindowLayout) -> RECT {
    windows_chrome_renderer()
        .map(|renderer| renderer.maximized_panel_rect(client, layout))
        .unwrap_or_else(|| renderer_content_rect(client, layout))
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
