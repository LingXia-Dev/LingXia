//! Advanced (bring-your-own-host) mode for the Windows SDK.
//!
//! The default, batteries-included path is [`lingxia_windows_sdk::quick_start`],
//! which opens the SDK's own windows and pumps the message loop for you. This
//! example shows the *other* mode: the host owns its window and message loop and
//! the SDK only supplies the view layer. The host implements
//! [`WindowsHostBackend`] against its own window and registers it with
//! [`set_windows_host_backend`]; LingXia then drives the host's UI through that
//! backend instead of a window the SDK created.
//!
//! Build (the contract surface is Windows-only):
//!
//! ```text
//! cargo build --example advanced_host -p lingxia-windows-sdk \
//!     --no-default-features --features host-api --target x86_64-pc-windows-msvc
//! ```
//!
//! To also boot the lxapp runtime in this mode, enable the `runtime` feature and
//! drive it yourself instead of calling `quick_start`:
//!
//! ```ignore
//! lingxia_windows_contract::set_windows_host_backend(Arc::new(MyHostBackend::new()));
//! let _home_app_id = lingxia_windows_sdk::init_runtime(
//!     lingxia_windows_sdk::WindowsApp::from_env(),
//! )?;
//! // ... then run your own Win32 message loop instead of run_message_loop().
//! ```

#[cfg(windows)]
fn main() {
    advanced::run();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("the advanced_host example only runs on Windows");
}

#[cfg(windows)]
mod advanced {
    use std::sync::Arc;

    use lingxia_webview::{WebTag, WebViewError};
    use lingxia_windows_contract::{
        WindowsContentRect, WindowsHostPanelTab, WindowsHostWindow, WindowsPanelPosition,
        WindowsWebViewContentWindow, WindowsWebViewWindowSnapshot, WindowsHostBackend,
        set_windows_host_backend,
    };

    pub fn run() {
        // Register the host-owned backend *instead of* the SDK default. From
        // here LingXia presents WebViews and panels into the host's own window
        // via these callbacks. (Skipping `install_default_windows_host`.)
        set_windows_host_backend(Arc::new(MyHostBackend));
        boot_and_run();
    }

    // Booting the lxapp runtime needs the `runtime` feature. `init_runtime` is
    // host-agnostic — it opens no window — so the host owns the window and loop.
    #[cfg(feature = "runtime")]
    fn boot_and_run() {
        match lingxia_windows_sdk::init_runtime(lingxia_windows_sdk::WindowsApp::from_env()) {
            Ok(home_app_id) => {
                println!("runtime booted for {home_app_id}; open your own window + pump your own loop");
                // Create your Win32 window for `home_app_id`, then drive messages —
                // e.g. `let _code = lingxia_windows_sdk::run_message_loop();`
            }
            Err(error) => eprintln!("init_runtime failed: {error}"),
        }
    }

    #[cfg(not(feature = "runtime"))]
    fn boot_and_run() {
        println!("registered a custom WindowsHostBackend; enable `runtime` to boot the lxapp runtime");
    }

    /// A skeleton backend. A real host routes each call to its own window —
    /// here every method is a no-op template so the contract surface is visible.
    struct MyHostBackend;

    type R = Result<(), WebViewError>;

    impl WindowsHostBackend for MyHostBackend {
        fn show_webview_as_panel(&self, _webtag: &WebTag, _title: &str, _panel_id: &str) -> R {
            Ok(())
        }
        fn show_webview_as_adaptive_panel(
            &self,
            _webtag: &WebTag,
            _title: &str,
            _panel_id: &str,
            _position: WindowsPanelPosition,
            _preferred_size: Option<i32>,
        ) -> R {
            Ok(())
        }
        fn present_webview_in_active_group(&self, _webtag: &WebTag) -> R {
            Ok(())
        }
        fn present_webview_as_group_main(&self, _webtag: &WebTag, _group_key: String) -> R {
            Ok(())
        }
        fn present_webview_as_overlay(
            &self,
            _webtag: &WebTag,
            _width: f64,
            _height: f64,
            _width_ratio: f64,
            _height_ratio: f64,
            _position: u8,
        ) -> R {
            Ok(())
        }
        fn resize_host_window_content(&self, _webtag: &WebTag, _width: i32, _height: i32) -> R {
            Ok(())
        }
        fn restore_presented_group_main(&self) -> R {
            Ok(())
        }
        fn show_interactive_host_panel(
            &self,
            _panel_id: &str,
            _title: &str,
            _body: &str,
            _position: WindowsPanelPosition,
        ) -> R {
            Ok(())
        }
        fn hide_host_panel(&self, _panel_id: &str) -> R {
            Ok(())
        }
        fn update_host_panel_body(&self, _panel_id: &str, _body: &str) -> R {
            Ok(())
        }
        fn set_host_panel_tabs(&self, _panel_id: &str, _tabs: Vec<WindowsHostPanelTab>) -> bool {
            false
        }
        fn set_host_panel_maximized(&self, _panel_id: &str, _maximized: bool) -> bool {
            false
        }
        fn invalidate_host_panel(&self, _panel_id: &str) -> bool {
            false
        }
        fn is_panel_visible(&self, _panel_id: &str) -> bool {
            false
        }
        fn find_webview_content_window(
            &self,
            _webtag: &WebTag,
        ) -> Option<WindowsWebViewContentWindow> {
            None
        }
        fn webview_window_snapshot(
            &self,
            _webtag: &WebTag,
        ) -> Result<WindowsWebViewWindowSnapshot, WebViewError> {
            Err(WebViewError::WebView("no window in the skeleton backend".to_string()))
        }
        fn show_webview_window(&self, _webtag: &WebTag, _title: &str, _activate: bool) -> R {
            Ok(())
        }
        fn show_webview_window_with_content_size(
            &self,
            _webtag: &WebTag,
            _title: &str,
            _activate: bool,
            _width: Option<i32>,
            _height: Option<i32>,
        ) -> R {
            Ok(())
        }
        fn navigate_webview_window(
            &self,
            _webtag: &WebTag,
            _title: &str,
            _activate: bool,
        ) -> R {
            Ok(())
        }
        fn hide_webview_window(&self, _webtag: &WebTag) -> R {
            Ok(())
        }
        fn request_host_window_layout(&self, _window: WindowsHostWindow) -> bool {
            false
        }
        fn active_content_screen_rect(&self) -> Option<WindowsContentRect> {
            None
        }
        fn post_to_window_thread(
            &self,
            _window: isize,
            _callback: Box<dyn FnOnce() + Send>,
        ) -> bool {
            false
        }
        fn sync_webview_window_layout(&self, _webtag: &WebTag) {}
    }
}
