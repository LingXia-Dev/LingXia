//! Minimal standalone Win32 surface host for WebView2.
//!
//! This module is compiled when the LingXia host feature is disabled. It is
//! intentionally small: create one HWND per WebView2 controller, keep the
//! controller fitted to the client rect, and expose a handler for UI layers
//! that want to show/hide that surface.

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowsWebViewContentWindow {
    pub window: isize,
    pub content_left: i32,
    pub content_top: i32,
    pub content_width: i32,
    pub content_height: i32,
    pub scale: f64,
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

    pub fn show_window(&self, title: &str) -> StdResult<()> {
        self.show_window_with_activation(title, true)
    }

    pub fn show_window_inactive(&self, title: &str) -> StdResult<()> {
        self.show_window_with_activation(title, false)
    }

    pub fn show_window_with_activation(&self, title: &str, activate: bool) -> StdResult<()> {
        self.webview
            .inner
            .show_window(title.to_string(), activate, WindowsWindowRole::Main)
    }

    pub fn hide(&self) -> StdResult<()> {
        self.webview.inner.hide_window()
    }

    pub fn window_snapshot(&self) -> StdResult<WindowsWebViewWindowSnapshot> {
        self.webview.inner.window_snapshot()
    }

    pub fn open_devtools(&self) -> StdResult<()> {
        self.webview.inner.open_devtools()
    }
}

static WEBVIEW_USER_DATA_DIR: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
static WEBVIEW_DEVTOOLS_ENABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);
static WINDOW_HANDLES: OnceLock<Mutex<HashMap<String, isize>>> = OnceLock::new();

pub fn find_webview_handler(webtag: &WebTag) -> Option<WindowsWebViewHandler> {
    find_webview(webtag).map(|webview| WindowsWebViewHandler { webview })
}

pub fn find_webview_content_window(webtag: &WebTag) -> Option<WindowsWebViewContentWindow> {
    let hwnd = window_handle_for_key(webtag.key())?;
    let mut client = RECT::default();
    unsafe {
        WindowsAndMessaging::GetClientRect(hwnd, &mut client).ok()?;
    }
    let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) };
    Some(WindowsWebViewContentWindow {
        window: hwnd_handle(hwnd),
        content_left: client.left,
        content_top: client.top,
        content_width: (client.right - client.left).max(0),
        content_height: (client.bottom - client.top).max(0),
        scale: if dpi == 0 { 1.0 } else { dpi as f64 / 96.0 },
    })
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

pub(crate) fn create_hidden_window(webtag: &WebTag) -> StdResult<HWND> {
    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WindowsAndMessaging::WM_CLOSE => {
                unsafe {
                    let _ = WindowsAndMessaging::DestroyWindow(hwnd);
                }
                LRESULT(0)
            }
            WindowsAndMessaging::WM_DESTROY => {
                unsafe {
                    WindowsAndMessaging::PostQuitMessage(0);
                }
                LRESULT(0)
            }
            WM_LINGXIA_RUN_CALLBACK => {
                run_posted_window_callback(wparam);
                LRESULT(0)
            }
            _ => unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }

    let class = WNDCLASSW {
        style: WindowsAndMessaging::CS_HREDRAW | WindowsAndMessaging::CS_VREDRAW,
        lpfnWndProc: Some(window_proc),
        lpszClassName: w!("LingXiaStandaloneWebViewHost"),
        ..Default::default()
    };

    unsafe {
        WindowsAndMessaging::RegisterClassW(&class);
        let hwnd = WindowsAndMessaging::CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("LingXiaStandaloneWebViewHost"),
            w!("LingXia WebView"),
            WS_OVERLAPPEDWINDOW,
            WindowsAndMessaging::CW_USEDEFAULT,
            WindowsAndMessaging::CW_USEDEFAULT,
            1024,
            768,
            None,
            None,
            LibraryLoader::GetModuleHandleW(None)
                .ok()
                .map(|module| HINSTANCE(module.0)),
            None,
        )
        .map_err(|err| WebViewError::WebView(format!("CreateWindowExW failed: {err}")))?;
        register_window_handle(webtag.key(), hwnd);
        Ok(hwnd)
    }
}

pub(crate) fn show_native_window(
    state: &mut UiState,
    title: &str,
    activate: bool,
    _role: WindowsWindowRole,
) -> StdResult<()> {
    let title = to_wide(title);
    unsafe {
        let _ = WindowsAndMessaging::SetWindowTextW(state.hwnd, PCWSTR(title.as_ptr()));
        let mut flags = WindowsAndMessaging::SWP_NOMOVE | WindowsAndMessaging::SWP_NOSIZE;
        if !activate {
            flags |= WindowsAndMessaging::SWP_NOACTIVATE;
        }
        WindowsAndMessaging::SetWindowPos(
            state.hwnd,
            None,
            0,
            0,
            0,
            0,
            flags | WindowsAndMessaging::SWP_SHOWWINDOW,
        )
        .map_err(|err| WebViewError::WebView(format!("SetWindowPos failed: {err}")))?;
        if activate {
            let _ = WindowsAndMessaging::BringWindowToTop(state.hwnd);
            let _ = WindowsAndMessaging::SetForegroundWindow(state.hwnd);
        }
    }
    sync_controller_bounds(state)?;
    set_controller_visible(state, true)?;
    state.window_visible = true;
    Ok(())
}

pub(crate) fn hide_native_window(state: &mut UiState) -> StdResult<()> {
    set_controller_visible(state, false)?;
    unsafe {
        WindowsAndMessaging::SetWindowPos(
            state.hwnd,
            None,
            0,
            0,
            0,
            0,
            WindowsAndMessaging::SWP_NOMOVE
                | WindowsAndMessaging::SWP_NOSIZE
                | WindowsAndMessaging::SWP_NOZORDER
                | WindowsAndMessaging::SWP_NOACTIVATE
                | WindowsAndMessaging::SWP_HIDEWINDOW,
        )
        .map_err(|err| WebViewError::WebView(format!("SetWindowPos failed: {err}")))?;
    }
    state.window_visible = false;
    Ok(())
}

pub(crate) fn window_snapshot(state: &UiState) -> StdResult<WindowsWebViewWindowSnapshot> {
    let mut client = RECT::default();
    unsafe {
        WindowsAndMessaging::GetClientRect(state.hwnd, &mut client)
            .map_err(|err| WebViewError::WebView(format!("GetClientRect failed: {err}")))?;
    }
    Ok(WindowsWebViewWindowSnapshot {
        window_id: hwnd_handle(state.hwnd) as usize,
        webtag_key: state.webtag_key.clone(),
        visible: state.window_visible,
        content_left: client.left,
        content_top: client.top,
        content_width: (client.right - client.left).max(0) as u32,
        content_height: (client.bottom - client.top).max(0) as u32,
    })
}

pub(crate) fn sync_controller_bounds(state: &UiState) -> StdResult<()> {
    let mut rect = RECT::default();
    unsafe {
        WindowsAndMessaging::GetClientRect(state.hwnd, &mut rect)
            .map_err(|err| WebViewError::WebView(format!("GetClientRect failed: {err}")))?;
        state
            .controller
            .SetBounds(rect)
            .map_err(|err| WebViewError::WebView(format!("SetBounds failed: {err}")))?;
    }
    Ok(())
}

pub(crate) fn set_controller_visible(state: &UiState, visible: bool) -> StdResult<()> {
    unsafe {
        state
            .controller
            .SetIsVisible(visible)
            .map_err(|err| WebViewError::WebView(format!("SetIsVisible failed: {err}")))
    }
}

pub(crate) fn register_live_layout_context(_state: &UiState) {}

pub(crate) fn clear_live_layout_context() {}

pub(crate) fn cleanup_standalone_window_state(state: &UiState) {
    remove_window_handle(&state.webtag_key);
}

pub(crate) fn remove_close_handler(_webtag_key: &str) {}

pub(crate) fn remove_chrome_event_handler(_webtag_key: &str) {}

pub(crate) fn remove_window_layout(_webtag_key: &str) {}

const WM_LINGXIA_RUN_CALLBACK: u32 = WM_APP + 0x158;

pub fn post_to_window_thread(window: isize, callback: Box<dyn FnOnce() + Send>) -> bool {
    if !is_window_handle_valid(window) {
        return false;
    }
    let raw = Box::into_raw(Box::new(callback));
    let posted = unsafe {
        WindowsAndMessaging::PostMessageW(
            Some(hwnd_from_handle(window)),
            WM_LINGXIA_RUN_CALLBACK,
            WPARAM(raw as usize),
            LPARAM(0),
        )
        .is_ok()
    };
    if !posted {
        drop(unsafe { Box::from_raw(raw) });
    }
    posted
}

pub(crate) fn run_posted_window_callback(wparam: WPARAM) {
    let raw = wparam.0 as *mut Box<dyn FnOnce() + Send>;
    if raw.is_null() {
        return;
    }
    let callback = unsafe { Box::from_raw(raw) };
    callback();
}

fn register_window_handle(webtag_key: &str, hwnd: HWND) {
    let handles = WINDOW_HANDLES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut handles) = handles.lock() {
        handles.insert(webtag_key.to_string(), hwnd_handle(hwnd));
    }
}

fn remove_window_handle(webtag_key: &str) {
    if let Some(handles) = WINDOW_HANDLES.get()
        && let Ok(mut handles) = handles.lock()
    {
        handles.remove(webtag_key);
    }
}

fn window_handle_for_key(webtag_key: &str) -> Option<HWND> {
    WINDOW_HANDLES
        .get()
        .and_then(|handles| handles.lock().ok())
        .and_then(|handles| handles.get(webtag_key).copied())
        .map(hwnd_from_handle)
}

fn hwnd_handle(hwnd: HWND) -> isize {
    hwnd.0 as isize
}

fn hwnd_from_handle(handle: isize) -> HWND {
    HWND(handle as *mut c_void)
}

fn is_window_handle_valid(handle: isize) -> bool {
    if handle == 0 {
        return false;
    }
    unsafe { WindowsAndMessaging::IsWindow(Some(hwnd_from_handle(handle))).as_bool() }
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
