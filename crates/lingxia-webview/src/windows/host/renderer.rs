//! Window chrome renderer seam.
//!
//! `lingxia-webview` hosts WebView2 content in plain Win32 windows and owns
//! only generic window mechanics. A host layer may register a
//! [`WindowsChromeRenderer`] to take over non-client handling:
//! painting custom chrome (bars, rails, panels) and mapping points to
//! chrome elements. When no renderer is registered, windows fall back to a
//! standard OS frame (`WS_OVERLAPPEDWINDOW`) with no custom non-client
//! handling and the webview fills the whole client area.

use super::*;

/// One generic tab of a host panel's tab strip. Pure layout data: the host
/// integration owns ids, titles, and what activation means; the renderer
/// only draws the strip and maps clicks back to tab ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsHostPanelTab {
    /// Stable tab identifier assigned by the host integration.
    pub id: u64,
    /// Display title (already resolved by the host integration).
    pub title: String,
    /// Whether this tab is the panel's active tab.
    pub active: bool,
}

/// Content of a panel that is drawn by the host chrome renderer
/// (as opposed to panels backed by their own webview window).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsHostPanelContent {
    pub title: Option<String>,
    pub body: Option<String>,
    /// Tab strip drawn in the panel header; empty when the panel has no
    /// tabs (the header then shows `title`). Updated by the host integration
    /// via `set_host_panel_tabs`.
    pub tabs: Vec<WindowsHostPanelTab>,
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
    /// `Some` when the panel content is drawn by the host renderer;
    /// `None` when a separate webview window covers the rect.
    pub host_content: Option<WindowsHostPanelContent>,
    /// Whether the panel lays out flush against the main card (compact
    /// bottom dock: zero gap, only a thin divider strip between the two).
    /// Renderers draw the shared edge with square corners.
    pub docked: bool,
}

/// One panel input passed to the host renderer for attached-surface layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsChromePanelLayoutInput {
    pub panel_id: String,
    pub webtag_key: String,
    pub position: WindowsPanelPosition,
    pub requested_size: Option<i32>,
    pub docked: bool,
    pub maximized: bool,
}

/// One renderer-computed attached panel rect.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowsChromePanelLayout {
    pub panel_id: String,
    pub webtag_key: String,
    pub rect: RECT,
    pub resize_handle: Option<RECT>,
}

/// Renderer-computed attached-surface layout for one host window.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowsChromeAttachedLayout {
    pub main: RECT,
    pub panels: Vec<WindowsChromePanelLayout>,
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
    /// Opaque host-chrome layout registered via `set_webview_window_layout`.
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

/// Generic command emitted by a chrome renderer.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowsChromeCommand {
    pub id: String,
    pub payload: serde_json::Value,
    /// Optional surface id to focus before dispatching the command.
    pub focus: Option<String>,
    /// Command dispatched when the same hit is double-clicked. When absent,
    /// double-click falls back to normal click handling.
    pub double_click: Option<Box<WindowsChromeCommand>>,
    /// Whether right-click dispatch should add `screen_x` / `screen_y` to
    /// an object payload before sending the command.
    pub include_screen_position: bool,
}

impl WindowsChromeCommand {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            payload: serde_json::Value::Null,
            focus: None,
            double_click: None,
            include_screen_position: false,
        }
    }

    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = payload;
        self
    }

    pub fn with_focus(mut self, surface_id: impl Into<String>) -> Self {
        self.focus = Some(surface_id.into());
        self
    }

    pub fn with_double_click(mut self, command: WindowsChromeCommand) -> Self {
        self.double_click = Some(Box::new(command));
        self
    }

    pub fn with_screen_position(mut self) -> Self {
        self.include_screen_position = true;
        self
    }
}

/// Result of a chrome hit test, mapped by lingxia-webview onto non-client
/// hit-test codes and generic chrome commands.
#[derive(Debug, Clone, PartialEq)]
pub enum WindowsChromeHit {
    /// Draggable title-bar area (`HTCAPTION`).
    Caption,
    /// A window frame button; the click is handled by lingxia-webview.
    FrameButton(WindowsFrameButton),
    /// A focusable chrome surface with optional right-click command.
    Focusable {
        id: String,
        context_menu: Option<WindowsChromeCommand>,
    },
    /// A renderer-defined command. The webview host does not interpret the
    /// id or payload.
    Command(WindowsChromeCommand),
    /// Inert chrome surface: consumes the click without producing an event.
    Chrome,
}

/// Renderer of host window chrome.
///
/// All methods are called on the webview UI thread that owns the window.
pub trait WindowsChromeRenderer: Send + Sync {
    /// Content rect reserved for the webview inside `client` for `layout`
    /// (i.e. the client rect minus all chrome insets).
    fn content_rect(&self, client: RECT, layout: &WindowsWindowLayout) -> RECT;

    /// Corner radius applied to attached webview surfaces.
    fn panel_corner_radius(&self) -> i32;

    /// Solid color of the per-pixel-alpha corner caps layered over the
    /// corners of attached card surfaces 閳?the background the rounded cards
    /// visually blend into (normally the window background). The default
    /// `None` disables the caps entirely; attached cards then keep plain
    /// square corners (as in the plain-frame fallback).
    fn card_corner_color(&self) -> Option<COLORREF> {
        None
    }

    /// Compute attached webview/panel surface rects for a host window.
    ///
    /// The WebView layer only applies these rects to child HWNDs; all UI
    /// policy such as side rails, gaps, docking, and resize handles belongs
    /// to the renderer/host layer.
    fn attached_layout(
        &self,
        client: RECT,
        layout: &WindowsWindowLayout,
        panels: &[WindowsChromePanelLayoutInput],
    ) -> Option<WindowsChromeAttachedLayout> {
        let _ = (client, layout, panels);
        None
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

pub(crate) fn renderer_attached_layout(
    client: RECT,
    layout: &WindowsWindowLayout,
    panels: &[WindowsChromePanelLayoutInput],
) -> Option<WindowsChromeAttachedLayout> {
    windows_chrome_renderer()?.attached_layout(client, layout, panels)
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
            && window_draws_host_chrome(&webtag_key)
        {
            let state = chrome_state_for_window(hwnd, &webtag_key);
            // Double-buffer: chrome paints background-then-content, which
            // flickers when drawn straight to the screen (visible on every
            // high-frequency host-panel frame). The buffer is pre-filled from the screen so
            // regions the renderer clips out (e.g. an inline EDIT child)
            // survive the blt unchanged.
            let width = rect_width(&state.client);
            let height = rect_height(&state.client);
            let mem_dc = CreateCompatibleDC(Some(hdc));
            let mem_bitmap = CreateCompatibleBitmap(hdc, width, height);
            if !mem_dc.is_invalid() && !mem_bitmap.is_invalid() {
                let previous = SelectObject(mem_dc, HGDIOBJ(mem_bitmap.0));
                let _ = BitBlt(mem_dc, 0, 0, width, height, Some(hdc), 0, 0, SRCCOPY);
                renderer.paint(mem_dc, &state);
                let _ = BitBlt(hdc, 0, 0, width, height, Some(mem_dc), 0, 0, SRCCOPY);
                SelectObject(mem_dc, previous);
            } else {
                renderer.paint(hdc, &state);
            }
            if !mem_bitmap.is_invalid() {
                let _ = DeleteObject(HGDIOBJ(mem_bitmap.0));
            }
            if !mem_dc.is_invalid() {
                let _ = DeleteDC(mem_dc);
            }
        }
        let _ = EndPaint(hwnd, &paint);
    }
}
