//! Existing WebView surface handler APIs for Windows host layers.

use super::*;

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

/// Handle to an already-created Windows WebView surface.
///
/// UI layers find this handle and decide how/when to present the surface.
/// The handle only dispatches generic WebView/Win32 surface commands; shell
/// policy, product layout, app menus, icons, and device chrome live outside
/// `lingxia-webview`.
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

    pub fn show_window(&self, title: &str) -> StdResult<()> {
        self.show_window_with_activation(title, true)
    }

    pub fn show_window_inactive(&self, title: &str) -> StdResult<()> {
        self.show_window_with_activation(title, false)
    }

    pub fn show_panel(&self, title: &str, panel_id: &str) -> StdResult<()> {
        self.webview.inner.show_window(
            title.to_string(),
            true,
            WindowsWindowRole::Panel {
                panel_id: panel_id.to_string(),
            },
        )
    }

    pub fn show_window_with_activation(&self, title: &str, activate: bool) -> StdResult<()> {
        self.webview
            .inner
            .show_window(title.to_string(), activate, WindowsWindowRole::Main)
    }

    pub fn hide(&self) -> StdResult<()> {
        self.webview.inner.hide_window()
    }

    pub fn set_layout(&self, layout: WindowsWindowLayout) -> StdResult<()> {
        self.webview.inner.set_window_layout(layout)
    }

    pub fn present_as_group_main(&self, group_key: String) -> StdResult<()> {
        self.webview.inner.present_as_group_main(group_key)
    }

    pub fn present_in_active_group(&self) -> StdResult<()> {
        let group_key = active_group_key()
            .ok_or_else(|| WebViewError::WebView("no active Windows host group".to_string()))?;
        self.present_as_group_main(group_key)
    }

    /// Presents this webview as a floating card layered over the active group's
    /// main content (an `overlay`-kind surface): a second layer within the
    /// content area that does not displace the main and follows the host window.
    /// `width`/`height` are logical pixels (0 = derive from the ratio or a
    /// default); `width_ratio`/`height_ratio` are fractions of the content area;
    /// `position` mirrors the `SurfacePosition` discriminants (0 center, 1
    /// bottom, 2 left, 3 right, 4 top).
    pub fn present_as_overlay(
        &self,
        width: f64,
        height: f64,
        width_ratio: f64,
        height_ratio: f64,
        position: u8,
    ) -> StdResult<()> {
        let group_key = active_group_key()
            .ok_or_else(|| WebViewError::WebView("no active Windows host group".to_string()))?;
        self.webview.inner.present_as_overlay(
            group_key,
            OverlayCardPlacement {
                width,
                height,
                width_ratio,
                height_ratio,
                anchor: OverlayAnchor::from_position(position),
            },
        )
    }

    pub fn window_snapshot(&self) -> StdResult<WindowsWebViewWindowSnapshot> {
        self.webview.inner.window_snapshot()
    }

    pub fn open_devtools(&self) -> StdResult<()> {
        self.webview.inner.open_devtools()
    }

    /// Positions the WebView2 content at `(left, top)` with size `width` x
    /// `height`, in physical pixels relative to the window the controller is
    /// parented to. The host UI layer owns layout and tells the surface where
    /// to render; the webview only applies the rect.
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

    /// Shows or hides the WebView2 content without affecting the host window.
    pub fn set_content_visible(&self, visible: bool) -> StdResult<()> {
        self.webview.inner.set_content_visible(visible)
    }

    pub fn resize_host_content(&self, width: i32, height: i32) -> StdResult<()> {
        resize_webview_host_content(&self.webtag(), width, height)
    }
}

pub fn find_webview_handler(webtag: &WebTag) -> Option<WindowsWebViewHandler> {
    find_webview(webtag).map(|webview| WindowsWebViewHandler { webview })
}

/// Overrides the host group a webview belongs to. By default every page of an
/// app/session shares one host window (`appid#session`); a webview given a
/// unique override group becomes its own group's host — a standalone
/// top-level window. Used to present a `window`-kind surface as a separate
/// window. Call before showing the webview; clear it when the surface closes.
pub fn set_webview_group_override(webtag: &WebTag, group_key: &str) {
    set_group_override(webtag.key(), group_key);
}

/// Clears a group override registered via [`set_webview_group_override`].
pub fn clear_webview_group_override(webtag: &WebTag) {
    clear_group_override(webtag.key());
}

/// Makes a webview's host window keep the standard OS frame (native title bar +
/// minimize / maximize / close) instead of custom chrome. Used for a
/// standalone `window`-kind surface so its own top-level window has real
/// window controls. Call before showing; clear when the surface closes.
pub fn set_webview_os_frame(webtag: &WebTag) {
    set_os_frame(webtag.key());
    // If the window already exists, force a frame recalc so the native frame
    // appears without waiting for the next size change.
    if let Some(hwnd) = window_handle_for_key(webtag.key()) {
        unsafe {
            let _ = WindowsAndMessaging::SetWindowPos(
                hwnd,
                None,
                0,
                0,
                0,
                0,
                WindowsAndMessaging::SWP_NOMOVE
                    | WindowsAndMessaging::SWP_NOSIZE
                    | WindowsAndMessaging::SWP_NOZORDER
                    | WindowsAndMessaging::SWP_NOACTIVATE
                    | WindowsAndMessaging::SWP_FRAMECHANGED,
            );
        }
    }
}

/// Clears an OS-frame mark registered via [`set_webview_os_frame`].
pub fn clear_webview_os_frame(webtag: &WebTag) {
    clear_os_frame(webtag.key());
}

pub fn set_webview_window_layout(webtag: &WebTag, layout: WindowsWindowLayout) -> StdResult<()> {
    let Some(handler) = find_webview_handler(webtag) else {
        // The webview may still be creating (e.g. the first switch to a
        // tab page syncs its layout before the page webview exists). The
        // layout registries and the group host don't need it: record the
        // layout and repaint the host so chrome updates immediately.
        set_window_layout_for_key(&webtag.key(), layout);
        let group_key = layout_group_key_for_webtag(&webtag.key());
        request_group_chrome_refresh(&group_key);
        return Ok(());
    };
    handler.set_layout(layout)
}

/// Hides the currently presented group-main surface and restores the prior
/// main webview. No-op when nothing is presented.
pub fn restore_presented_group_main() -> StdResult<()> {
    let Some(group_key) = active_group_key() else {
        return Ok(());
    };
    let Some(presented) = take_presented_main(&group_key) else {
        return Ok(());
    };
    match presented.previous_main_key {
        Some(previous) => set_group_active_main(&group_key, &previous),
        None => remove_group_active_main(&group_key, &presented.presented_key),
    }
    layout_group_windows(&group_key);
    request_group_chrome_refresh(&group_key);
    Ok(())
}

/// Whether newly created webviews allow the WebView2 DevTools.
static WEBVIEW_DEVTOOLS_ENABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);

pub fn set_webview_devtools_enabled(enabled: bool) {
    WEBVIEW_DEVTOOLS_ENABLED.store(enabled, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn webview_devtools_enabled() -> bool {
    WEBVIEW_DEVTOOLS_ENABLED.load(std::sync::atomic::Ordering::Relaxed)
}

/// Resizes the top-level window presenting `webtag` so its client area is
/// exactly `width` x `height` physical pixels.
fn resize_webview_host_content(webtag: &WebTag, width: i32, height: i32) -> StdResult<()> {
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
