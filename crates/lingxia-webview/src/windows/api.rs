//! Public Windows API surface: window/panel entry points,
//! handler registries, and the exported layout/event types.

use super::*;

mod registry;

pub(crate) use registry::{
    configured_webview_user_data_dir, invoke_chrome_event_handler, invoke_close_handler,
    remove_chrome_event_handler, remove_close_handler, run_posted_window_callback,
    WM_LINGXIA_RUN_CALLBACK, WINDOW_NATIVE_PANEL_INPUT_HANDLERS,
};
pub use registry::{
    clear_native_panel_input_handler, post_to_window_thread, set_native_panel_input_handler,
    set_webview_chrome_event_handler, set_webview_close_handler, set_webview_user_data_dir,
};

/// Built-in initial outer size of top-level webview host windows, used when
/// no process-wide override was installed via [`set_default_window_size`].
const BUILTIN_DEFAULT_WINDOW_SIZE: (i32, i32) = (1024, 768);

/// Process-wide override of the initial outer window size, set at most once
/// before the first window is created (see [`set_default_window_size`]).
static DEFAULT_WINDOW_SIZE: OnceLock<(i32, i32)> = OnceLock::new();

/// Overrides the initial outer size, in pixels, of top-level webview host
/// windows created after this call — in particular the main window of a
/// host app. Attached surfaces (panels, presented main children) are
/// re-laid out by their group and are unaffected in practice.
///
/// Call once during host bootstrap, before the first webview window is
/// created. The first call wins; later calls and non-positive dimensions
/// are ignored. Without an override windows open at 1024x768.
pub fn set_default_window_size(width: i32, height: i32) {
    if width <= 0 || height <= 0 {
        log::warn!("ignoring non-positive default window size {width}x{height}");
        return;
    }
    if DEFAULT_WINDOW_SIZE.set((width, height)).is_err() {
        log::warn!("default window size already set; ignoring {width}x{height}");
    }
}

/// Initial outer size for newly created top-level webview host windows.
pub(crate) fn default_window_size() -> (i32, i32) {
    DEFAULT_WINDOW_SIZE
        .get()
        .copied()
        .unwrap_or(BUILTIN_DEFAULT_WINDOW_SIZE)
}

/// Handler for structured key input targeted at a native panel.
///
/// Returns `true` when the event was consumed (the window message is then
/// swallowed); `false` lets default window handling proceed.
pub type WindowsPanelInputHandler = Arc<dyn Fn(WindowsPanelKeyEvent) -> bool + Send + Sync>;

/// Structured key event forwarded to a native panel input handler.
///
/// `lingxia-webview` does not interpret keys (e.g. terminal escape
/// sequences); it only reports what the window received. `character` is set
/// for translated character input (`WM_CHAR`); for raw key-down input
/// (`WM_KEYDOWN`) it is `None` and `vk` carries the virtual-key code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowsPanelKeyEvent {
    /// Virtual-key code for key-down events; `0` for character events.
    pub vk: u32,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    /// Translated character for character events.
    pub character: Option<char>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowsChromeEvent {
    TabBarClick { index: usize },
    PanelActivatorClick { panel_id: String },
    NavigationBack,
    NavigationHome,
    /// Click on the sidebar "New Tab" browser row.
    BrowserNewTabClick,
    /// Click on a sidebar browser-tab row.
    BrowserTabClick { tab_id: String },
    /// Click on the close glyph of a sidebar browser-tab row.
    BrowserTabCloseClick { tab_id: String },
    /// Click on a tab in a native panel's header tab strip.
    NativePanelTabClick { panel_id: String, tab_id: u64 },
    /// Click on the close glyph of a native panel tab.
    NativePanelTabCloseClick { panel_id: String, tab_id: u64 },
    /// Click on the new-tab button of a native panel header.
    NativePanelNewTabClick { panel_id: String },
    /// Click on the maximize/restore toggle of a native panel header.
    NativePanelMaximizeClick { panel_id: String },
    /// Double-click on the active tab title of a native panel header; the
    /// product layer starts an inline rename in response.
    NativePanelTabRenameRequest { panel_id: String, tab_id: u64 },
    /// Right-click on a native panel's content area, with the click point
    /// in screen coordinates (products typically show a context menu there).
    NativePanelRightClick {
        panel_id: String,
        screen_x: i32,
        screen_y: i32,
    },
    /// Click on the top-bar address-bar back button.
    BrowserNavBackClick,
    /// Click on the top-bar address-bar forward button.
    BrowserNavForwardClick,
    /// Click on the top-bar address-bar reload button.
    BrowserNavReloadClick,
    /// Click on the top-bar URL capsule (the product layer starts an
    /// inline address edit in response).
    BrowserAddressBarClick,
    /// Click on the top-bar sidebar collapse/expand toggle.
    SidebarToggleClick,
    /// Click on the sidebar group header chevron (collapses/expands the
    /// group's items). `group` is the generic id supplied through
    /// [`WindowsTabBarLayout::group_id`].
    SidebarGroupToggleClick { group: String },
    /// Click on a sidebar header action button. `action_id` is the generic
    /// id supplied through [`WindowsSidebarActionLayout`].
    SidebarActionClick { action_id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowsTabBarPosition {
    #[default]
    Bottom,
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsNavigationBarLayout {
    pub visible: bool,
    pub title: String,
    pub background_color: u32,
    pub text_color: u32,
    pub show_back_button: bool,
    pub show_home_button: bool,
    pub height: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsTabBarItemLayout {
    pub page_path: String,
    pub text: String,
    pub icon_path: String,
    pub selected_icon_path: String,
    pub badge: Option<String>,
    pub has_red_dot: bool,
}

/// One browser-tab row of the sidebar browser section. Pure layout data:
/// the product layer owns titles (including any URL fallback) and the
/// meaning of `active`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsBrowserTabItemLayout {
    pub tab_id: String,
    pub title: String,
    pub active: bool,
    /// PNG-encoded favicon of the tab's current page, when known. `Arc`'d so
    /// per-sync layout clones share the bytes; the renderer draws it left of
    /// the title (text-only row when `None`).
    pub favicon_png: Option<Arc<Vec<u8>>>,
}

/// One sidebar header action button. Pure layout data: the product layer
/// owns the action id and the glyph (an icon-font codepoint string); the
/// renderer only draws the button and maps clicks back to the id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsSidebarActionLayout {
    pub id: String,
    /// Icon-font glyph drawn on the button (e.g. a Segoe Fluent Icons
    /// codepoint), supplied by the product layer.
    pub glyph: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsTabBarLayout {
    pub visible: bool,
    pub position: WindowsTabBarPosition,
    pub dimension: i32,
    pub app_name: String,
    /// Generic group identifier echoed back in
    /// [`WindowsChromeEvent::SidebarGroupToggleClick`].
    pub group_id: String,
    pub color: u32,
    pub selected_color: u32,
    pub background_color: u32,
    pub border_color: u32,
    pub selected_index: i32,
    pub items: Vec<WindowsTabBarItemLayout>,
    /// Whether the whole sidebar is collapsed (width 0, content expands);
    /// the top-bar sidebar toggle stays visible so it can be re-expanded.
    pub collapsed: bool,
    /// Whether the lxapp items group is collapsed (items hidden, the
    /// browser section moves up under the group header).
    pub items_collapsed: bool,
    /// Browser-tab rows drawn under the regular items (sidebar positions
    /// only). Empty when the product has no browser tabs to show.
    pub browser_tabs: Vec<WindowsBrowserTabItemLayout>,
    /// Whether a "New Tab" row is drawn under the browser-tab rows.
    pub show_browser_new_tab: bool,
    /// Header action buttons drawn in the caption strip right of the
    /// sidebar toggle; hidden while the sidebar is collapsed.
    pub header_actions: Vec<WindowsSidebarActionLayout>,
}

/// Top-bar address-bar section: a centered URL capsule with back/forward/
/// reload buttons. Pure layout data; navigation policy (what the buttons
/// do, how input resolves) lives entirely in the product layer.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WindowsAddressBarLayout {
    pub visible: bool,
    /// Text shown inside the URL capsule (current URL or page title,
    /// already resolved by the product layer).
    pub url_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowsPanelPosition {
    Left,
    #[default]
    Right,
    Bottom,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsPanelActivatorLayout {
    pub id: String,
    pub label: String,
    pub icon_path: String,
    pub position: WindowsPanelPosition,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WindowsWindowLayout {
    pub navigation_bar: Option<WindowsNavigationBarLayout>,
    /// Browser address-bar section of the top bar; `None` when no browser
    /// surface is presented.
    pub address_bar: Option<WindowsAddressBarLayout>,
    pub tab_bar: Option<WindowsTabBarLayout>,
    pub panel_activators: Vec<WindowsPanelActivatorLayout>,
}

/// Geometry of a webview's own content window, for product layers that
/// place native child controls (embedded components) over the rendered
/// page. Unlike [`WindowsWebViewWindowSnapshot`], `window` is always the
/// webview's own window — the correct parent for overlay children — never
/// the group host.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowsWebViewContentWindow {
    /// Raw handle of the webview's own window (parent for overlay children).
    pub window: isize,
    /// WebView2 content origin within the window's client area, physical px.
    pub content_left: i32,
    pub content_top: i32,
    /// WebView2 content size, physical px.
    pub content_width: i32,
    pub content_height: i32,
    /// Physical pixels per CSS pixel (window DPI / 96).
    pub scale: f64,
}

/// Resolves the content-window geometry for `webtag`, or `None` while the
/// webview has no registered window yet (not shown/attached). Pure registry
/// and Win32 reads; safe to call from any thread.
pub fn webview_content_window(webtag: &WebTag) -> Option<WindowsWebViewContentWindow> {
    let hwnd = window_handle_for_key(webtag.key())?;
    let mut client = RECT::default();
    unsafe {
        WindowsAndMessaging::GetClientRect(hwnd, &mut client).ok()?;
    }
    let content = controller_bounds_for_window(hwnd, webtag.key(), client);
    let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) };
    let scale = if dpi == 0 { 1.0 } else { dpi as f64 / 96.0 };
    Some(WindowsWebViewContentWindow {
        window: hwnd_handle(hwnd),
        content_left: content.left,
        content_top: content.top,
        content_width: (content.right - content.left).max(0),
        content_height: (content.bottom - content.top).max(0),
        scale,
    })
}

/// Top-level host window currently presenting a webview surface.
///
/// For a standalone webview this is its own window. For attached main
/// surfaces and panels, this resolves to the shell group host that actually
/// owns window chrome, menus, sizing, and host-level presentation effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowsWebViewHostWindow {
    /// Raw host `HWND` as an integer handle.
    pub window: isize,
}

pub(crate) fn webview_host_hwnd(webtag: &WebTag) -> StdResult<HWND> {
    let hwnd = window_handle_for_key(webtag.key()).ok_or_else(|| {
        WebViewError::WebView(format!("no window registered for {}", webtag.key()))
    })?;
    match window_attachment(webtag.key()) {
        Some(WindowAttachment {
            group_key,
            kind: WindowAttachmentKind::MainChild | WindowAttachmentKind::Panel { .. },
        }) => host_handle_for_group(&group_key).ok_or_else(|| {
            WebViewError::WebView(format!(
                "no host window for Windows shell group {group_key}"
            ))
        }),
        _ => Ok(hwnd),
    }
}

/// Resolves the host window currently presenting `webtag`.
///
/// This is generic webview hosting state; product layers can use the handle
/// with [`post_to_window_thread`] for host-window UI work without knowing
/// how LingXia shell groups attach child webviews internally.
pub fn webview_host_window(webtag: &WebTag) -> StdResult<WindowsWebViewHostWindow> {
    webview_host_hwnd(webtag).map(|window| WindowsWebViewHostWindow {
        window: hwnd_handle(window),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsWebViewWindowSnapshot {
    pub window_id: usize,
    pub webtag_key: String,
    pub visible: bool,
    pub content_left: i32,
    pub content_top: i32,
    pub content_width: u32,
    pub content_height: u32,
}

pub fn show_webview_window(webtag: &WebTag, title: &str) -> StdResult<()> {
    show_webview_window_with_activation(webtag, title, true)
}

pub fn show_webview_window_inactive(webtag: &WebTag, title: &str) -> StdResult<()> {
    show_webview_window_with_activation(webtag, title, false)
}

pub fn show_webview_panel(webtag: &WebTag, title: &str, panel_id: &str) -> StdResult<()> {
    let webview = find_webview(webtag)
        .ok_or_else(|| WebViewError::WebView(format!("WebView not found for {}", webtag.key())))?;
    webview.inner.show_window(
        title.to_string(),
        true,
        WindowsWindowRole::Panel {
            panel_id: panel_id.to_string(),
        },
    )
}

pub(crate) fn show_webview_window_with_activation(
    webtag: &WebTag,
    title: &str,
    activate: bool,
) -> StdResult<()> {
    let webview = find_webview(webtag)
        .ok_or_else(|| WebViewError::WebView(format!("WebView not found for {}", webtag.key())))?;
    webview
        .inner
        .show_window(title.to_string(), activate, WindowsWindowRole::Main)
}

pub fn hide_webview_window(webtag: &WebTag) -> StdResult<()> {
    let webview = find_webview(webtag)
        .ok_or_else(|| WebViewError::WebView(format!("WebView not found for {}", webtag.key())))?;
    webview.inner.hide_window()
}

pub fn set_webview_window_layout(webtag: &WebTag, layout: WindowsWindowLayout) -> StdResult<()> {
    let Some(webview) = find_webview(webtag) else {
        // The webview may still be creating (e.g. the first switch to a
        // tab page syncs its layout before the page webview exists). The
        // layout registries and the group host don't need it: record the
        // layout and repaint the host so chrome (tab highlight, navbar)
        // updates immediately; controller bounds sync once the webview's
        // own layout path runs after creation.
        set_window_layout_for_key(&webtag.key(), layout);
        let group_key = layout_group_key_for_webtag(&webtag.key());
        request_group_shell_refresh(&group_key);
        return Ok(());
    };
    webview.inner.set_window_layout(layout)
}

/// Presents `webtag`'s window as the main-content surface of the active
/// shell group: the window is reparented into the group host (SetParent /
/// child-style machinery, same as attached main children) and shown over
/// the group's main content card. The previously active main webview is
/// hidden and remembered so [`restore_presented_group_main`] can bring it
/// back. Pure window mechanics — callers own all policy (which webview to
/// present and when).
pub fn present_webview_as_group_main(webtag: &WebTag) -> StdResult<()> {
    let group_key = active_group_key()
        .ok_or_else(|| WebViewError::WebView("no active Windows shell group".to_string()))?;
    let webview = find_webview(webtag)
        .ok_or_else(|| WebViewError::WebView(format!("WebView not found for {}", webtag.key())))?;
    webview.inner.present_as_group_main(group_key)
}

/// Hides the webview presented via [`present_webview_as_group_main`] (if
/// any) in the active group and restores the previously active main
/// webview. No-op when nothing is presented.
pub fn restore_presented_group_main() -> StdResult<()> {
    let Some(group_key) = active_group_key() else {
        return Ok(());
    };
    let Some(presented) = take_presented_main(&group_key) else {
        return Ok(());
    };
    match presented.previous_main_key {
        Some(previous) => set_group_active_main(&group_key, &previous),
        // No known previous main: drop the entry so the host surface shows.
        None => remove_group_active_main(&group_key, &presented.presented_key),
    }
    layout_group_windows(&group_key);
    request_group_shell_refresh(&group_key);
    Ok(())
}

pub fn webview_window_snapshot(webtag: &WebTag) -> StdResult<WindowsWebViewWindowSnapshot> {
    let webview = find_webview(webtag)
        .ok_or_else(|| WebViewError::WebView(format!("WebView not found for {}", webtag.key())))?;
    webview.inner.window_snapshot()
}

/// Whether newly created webviews allow the WebView2 DevTools (F12 inside
/// the page, "Inspect" in the context menu, [`open_webview_devtools`]).
/// Defaults to enabled, matching WebView2 itself.
static WEBVIEW_DEVTOOLS_ENABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);

/// Enables or disables the WebView2 DevTools on webviews created after this
/// call (existing webviews are unaffected). DevTools are enabled by default;
/// products that must lock their pages down can turn them off during host
/// bootstrap, before the first webview is created.
pub fn set_webview_devtools_enabled(enabled: bool) {
    WEBVIEW_DEVTOOLS_ENABLED.store(enabled, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn webview_devtools_enabled() -> bool {
    WEBVIEW_DEVTOOLS_ENABLED.load(std::sync::atomic::Ordering::Relaxed)
}

/// Opens the WebView2 DevTools window for `webtag`'s webview. The call is
/// dispatched as a synchronous command on the webview's UI thread (like
/// eval/screenshot) and maps to `ICoreWebView2::OpenDevToolsWindow`.
/// Requires DevTools to be enabled (see [`set_webview_devtools_enabled`]).
pub fn open_webview_devtools(webtag: &WebTag) -> StdResult<()> {
    let webview = find_webview(webtag)
        .ok_or_else(|| WebViewError::WebView(format!("WebView not found for {}", webtag.key())))?;
    webview.inner.open_devtools()
}

/// Resizes the top-level window presenting `webtag` so its client (content)
/// area is exactly `width` x `height` physical pixels. Attached surfaces
/// (presented main children, panels) resolve to their group host window.
/// The non-client frame — caption, borders, and an attached menu bar (see
/// [`set_windows_app_menu`](super::set_windows_app_menu)) — is accounted
/// for via `AdjustWindowRectExForDpi` at the window's current DPI; with a
/// registered chrome renderer the client area covers the whole window, so
/// this sizes the outer window directly. Pure geometry mechanics: which
/// size to apply and when is caller policy. Safe to call from any thread.
pub fn resize_webview_window_content(webtag: &WebTag, width: i32, height: i32) -> StdResult<()> {
    if width <= 0 || height <= 0 {
        return Err(WebViewError::WebView(format!(
            "invalid window content size {width}x{height}"
        )));
    }
    let hwnd = webview_host_hwnd(webtag)?;

    let mut rect = RECT {
        left: 0,
        top: 0,
        right: width,
        bottom: height,
    };
    unsafe {
        let style = WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWL_STYLE);
        let ex_style =
            WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWL_EXSTYLE);
        let has_menu = !WindowsAndMessaging::GetMenu(hwnd).is_invalid();
        let dpi = windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd);
        // With a chrome renderer WM_NCCALCSIZE makes the client area cover
        // the window, so the frame delta computed here must stay zero.
        if windows_chrome_renderer().is_none() {
            windows::Win32::UI::HiDpi::AdjustWindowRectExForDpi(
                &mut rect,
                WindowsAndMessaging::WINDOW_STYLE(style as u32),
                has_menu,
                WindowsAndMessaging::WINDOW_EX_STYLE(ex_style as u32),
                if dpi == 0 { 96 } else { dpi },
            )
            .map_err(|err| {
                WebViewError::WebView(format!("AdjustWindowRectExForDpi failed: {err}"))
            })?;
        }
        WindowsAndMessaging::SetWindowPos(
            hwnd,
            None,
            0,
            0,
            rect.right - rect.left,
            rect.bottom - rect.top,
            WindowsAndMessaging::SWP_NOMOVE
                | WindowsAndMessaging::SWP_NOZORDER
                | WindowsAndMessaging::SWP_NOACTIVATE,
        )
        .map_err(|err| WebViewError::WebView(format!("SetWindowPos failed: {err}")))?;
    }
    Ok(())
}

pub fn is_panel_visible(panel_id: &str) -> bool {
    active_group_key()
        .map(|group_key| {
            group_panels(&group_key)
                .into_iter()
                .any(|panel| panel.panel_id == panel_id)
        })
        .unwrap_or(false)
}

pub fn show_native_panel(
    panel_id: &str,
    title: &str,
    body: &str,
    position: WindowsPanelPosition,
) -> StdResult<()> {
    show_native_group_panel(panel_id, title, body, position, NativePanelKind::Text)
}

pub fn show_native_terminal_panel(
    panel_id: &str,
    title: &str,
    body: &str,
    position: WindowsPanelPosition,
) -> StdResult<()> {
    show_native_group_panel(panel_id, title, body, position, NativePanelKind::Terminal)
}

pub(crate) fn show_native_group_panel(
    panel_id: &str,
    title: &str,
    body: &str,
    position: WindowsPanelPosition,
    native_kind: NativePanelKind,
) -> StdResult<()> {
    let group_key = active_group_key()
        .ok_or_else(|| WebViewError::WebView("no active Windows shell group".to_string()))?;
    let Some(_host) = host_handle_for_group(&group_key) else {
        return Err(WebViewError::WebView(format!(
            "active Windows shell group has no host: {group_key}"
        )));
    };

    register_group_panel(
        &group_key,
        GroupPanel {
            webtag_key: native_panel_key(panel_id),
            panel_id: panel_id.to_string(),
            position,
            native_kind,
            native_title: Some(title.to_string()),
            native_body: Some(body.to_string()),
            native_tabs: Vec::new(),
            maximized: false,
        },
    );
    if native_kind == NativePanelKind::Terminal {
        set_active_native_panel(Some(panel_id.to_string()));
    }
    layout_group_windows(&group_key);
    request_group_shell_refresh(&group_key);
    Ok(())
}

pub fn update_native_panel_body(panel_id: &str, body: &str) -> StdResult<()> {
    let Some(group_key) = update_group_panel_body(panel_id, body.to_string()) else {
        return Ok(());
    };
    request_group_shell_refresh(&group_key);
    Ok(())
}

/// Replaces the header tab strip of a native panel and repaints the host
/// chrome. The strip is generic data: ids, titles, and the active flag are
/// owned by the product layer. Returns `false` when no group currently
/// hosts the panel.
pub fn set_native_panel_tabs(panel_id: &str, tabs: Vec<WindowsNativePanelTab>) -> bool {
    let Some(group_key) = update_group_panel_tabs(panel_id, tabs) else {
        return false;
    };
    request_group_shell_refresh(&group_key);
    true
}

/// Sets whether a native panel is maximized over the whole content area
/// (the main card collapses while maximized) and re-runs the group layout.
/// Pure rect mechanics; the toggle policy lives in the product layer.
/// Returns `false` when no group currently hosts the panel.
pub fn set_native_panel_maximized(panel_id: &str, maximized: bool) -> bool {
    let Some(group_key) = update_group_panel_maximized(panel_id, maximized) else {
        return false;
    };
    layout_group_windows(&group_key);
    request_group_shell_refresh(&group_key);
    true
}

/// Repaints the host-window region covering `panel_id` without changing the
/// panel's registered body text (for panels whose content is drawn by the
/// chrome renderer from external state). Returns `false` when no group
/// currently hosts the panel.
pub fn invalidate_native_panel(panel_id: &str) -> bool {
    let Some(group_key) = group_key_for_panel(panel_id) else {
        return false;
    };
    // Content-only updates (terminal output) repaint just the panel rect;
    // a full-window invalidation here would repaint the sidebar on every
    // terminal frame, which reads as chrome flicker.
    request_group_panel_repaint(&group_key, panel_id);
    true
}

#[inline]
pub fn hide_native_panel(panel_id: &str) -> StdResult<()> {
    hide_panel(panel_id)
}

pub fn hide_panel(panel_id: &str) -> StdResult<()> {
    let group_key = active_group_key()
        .ok_or_else(|| WebViewError::WebView("no active Windows shell group".to_string()))?;
    remove_group_panel_by_panel_id(&group_key, panel_id);
    layout_group_windows(&group_key);
    request_group_shell_refresh(&group_key);
    Ok(())
}
