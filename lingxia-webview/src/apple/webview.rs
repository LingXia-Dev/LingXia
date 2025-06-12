use dispatch2::DispatchQueue;
use miniapp::{AppUiDelegate, MiniAppError, WebViewController};
use objc2::runtime::{AnyObject, NSObject, ProtocolObject};
use objc2::{define_class, msg_send, rc::Retained, MainThreadMarker, MainThreadOnly, DefinedClass};
use objc2_foundation::{NSObjectProtocol, NSString, NSPoint, NSRect, NSSize, NSURL, NSURLRequest};
use objc2_web_kit::{WKNavigation, WKNavigationDelegate, WKWebViewConfiguration};

// Custom Navigation Delegate for handling page lifecycle events
pub struct LingXiaNavigationDelegateIvars {
    pub appid: String,
    pub path: String,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "LingXiaNavigationDelegate"]
    #[thread_kind = MainThreadOnly]
    #[ivars = LingXiaNavigationDelegateIvars]
    pub struct LingXiaNavigationDelegate;

    unsafe impl NSObjectProtocol for LingXiaNavigationDelegate {}

    unsafe impl WKNavigationDelegate for LingXiaNavigationDelegate {
        #[unsafe(method(webView:didStartProvisionalNavigation:))]
        fn did_start_provisional_navigation(&self, _webview: *mut AnyObject, _navigation: &WKNavigation) {
            let appid = &self.ivars().appid;
            let path = &self.ivars().path;

            // Call miniapp's on_page_started
            let miniapp = miniapp::get(appid.clone());
            miniapp.on_page_started(path.clone());
            log::info!("WebView page started: {} at {}", appid, path);
        }

        #[unsafe(method(webView:didFinishNavigation:))]
        fn did_finish_navigation(&self, _webview: *mut AnyObject, _navigation: &WKNavigation) {
            let appid = &self.ivars().appid;
            let path = &self.ivars().path;

            // Call miniapp's on_page_finished
            let miniapp = miniapp::get(appid.clone());
            miniapp.on_page_finished(path.clone());
            log::info!("WebView page finished: {} at {}", appid, path);
        }
    }
);

impl LingXiaNavigationDelegate {
    pub fn new(appid: String, path: String, mtm: MainThreadMarker) -> Retained<Self> {
        let delegate = mtm
            .alloc::<LingXiaNavigationDelegate>()
            .set_ivars(LingXiaNavigationDelegateIvars { appid, path });

        unsafe { msg_send![super(delegate), init] }
    }
}


pub struct WebViewInner {
    webview: *mut AnyObject,
    // Keep reference to navigation delegate to prevent deallocation (only for main WebView instances)
    _navigation_delegate: Option<Retained<LingXiaNavigationDelegate>>,
}

impl std::fmt::Debug for WebViewInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebViewInner")
            .field("webview", &self.webview)
            .field("has_navigation_delegate", &self._navigation_delegate.is_some())
            .finish()
    }
}

unsafe impl Send for WebViewInner {}
unsafe impl Sync for WebViewInner {}

impl WebViewInner {
    /// Create a new WebView using objc2 directly
    pub(crate) fn create(appid: &str, path: &str) -> Result<Self, MiniAppError> {
        // Ensure we're on the main thread for WebView creation
        let mtm = MainThreadMarker::new().ok_or_else(|| {
            MiniAppError::WebView("WebView creation must be on main thread".to_string())
        })?;

        log::info!(
            "Starting WebView creation for appid={}, path={}",
            appid,
            path
        );

        unsafe {
            // Create WKWebViewConfiguration
            let config = WKWebViewConfiguration::new(mtm);
            log::debug!("WKWebViewConfiguration created");

            // Set up comprehensive WebView configuration
            let _: () = msg_send![&*config, setAllowsInlineMediaPlayback: true];

            // Configure media playback
            let media_types: i32 = 0; // WKAudiovisualMediaTypeNone
            let _: () =
                msg_send![&*config, setMediaTypesRequiringUserActionForPlayback: media_types];

            // Disable navigation restrictions for better compatibility
            if objc2::msg_send![&*config, respondsToSelector: objc2::sel!(setLimitsNavigationsToAppBoundDomains:)]
            {
                let _: () = msg_send![&*config, setLimitsNavigationsToAppBoundDomains: false];
            }

            // Disable HTTPS upgrade for local development
            if objc2::msg_send![&*config, respondsToSelector: objc2::sel!(setUpgradeKnownHostsToHTTPS:)]
            {
                let _: () = msg_send![&*config, setUpgradeKnownHostsToHTTPS: false];
            }

            // Create frame (will be set properly when attached to view)
            let frame = NSRect {
                origin: NSPoint { x: 0.0, y: 0.0 },
                size: NSSize {
                    width: 320.0,
                    height: 568.0,
                }, // Default iPhone size
            };

            // Get WKWebView class
            let webview_class = objc2::class!(WKWebView);

            // Allocate WebView
            let webview: *mut AnyObject = msg_send![webview_class, alloc];
            if webview.is_null() {
                log::error!("Failed to allocate WKWebView");
                return Err(MiniAppError::WebView(
                    "Failed to allocate WKWebView".to_string(),
                ));
            }

            // Initialize WebView
            let webview: *mut AnyObject =
                msg_send![webview, initWithFrame: frame, configuration: &*config];
            if webview.is_null() {
                log::error!("Failed to initialize WKWebView");
                return Err(MiniAppError::WebView(
                    "Failed to initialize WKWebView".to_string(),
                ));
            }

            // Create navigation delegate
            let navigation_delegate = LingXiaNavigationDelegate::new(
                appid.to_string(),
                path.to_string(),
                MainThreadMarker::new().unwrap()
            );

            // Set the navigation delegate on the WebView
            let proto_navigation_delegate: &ProtocolObject<dyn WKNavigationDelegate> = ProtocolObject::from_ref(&*navigation_delegate);
            let _: () = msg_send![webview, setNavigationDelegate: Some(proto_navigation_delegate)];

            // Create WebViewInner instance with navigation delegate
            let webview_inner = WebViewInner {
                webview,
                _navigation_delegate: Some(navigation_delegate),
            };

            log::info!(
                "WebView created successfully for appid={}, path={}",
                appid,
                path
            );

            Ok(webview_inner)
        }
    }

    /// Get the raw pointer to the WebView for Swift interop
    pub(crate) fn get_swift_webview_ptr(&self) -> usize {
        self.webview as usize
    }

    /// Helper method to load URL on main thread
    fn load_url_on_main_thread(&self, url: String) -> Result<(), MiniAppError> {
        unsafe {
            let ns_url_string = NSString::from_str(&url);
            if let Some(ns_url) = NSURL::URLWithString(&ns_url_string) {
                let request = NSURLRequest::requestWithURL(&ns_url);
                let _: () = msg_send![self.webview, loadRequest: &*request];
                Ok(())
            } else {
                Err(MiniAppError::WebView(format!("Invalid URL: {}", url)))
            }
        }
    }

    /// Helper method to evaluate JavaScript on main thread
    fn evaluate_javascript_on_main_thread(&self, js: String) -> Result<(), MiniAppError> {
        unsafe {
            let js_string = NSString::from_str(&js);
            // Note: evaluateJavaScript is async, but we're treating it as fire-and-forget
            // In a more complete implementation, we might want to handle the completion
            let _: () = msg_send![self.webview, evaluateJavaScript: &*js_string, completionHandler: std::ptr::null::<*const AnyObject>()];
            Ok(())
        }
    }

    /// Helper method to clear browsing data on main thread
    fn clear_browsing_data_on_main_thread(&self) -> Result<(), MiniAppError> {
        unsafe {
            // Get the website data store from the configuration
            let config: *mut AnyObject = msg_send![self.webview, configuration];
            let data_store: *mut AnyObject = msg_send![config, websiteDataStore];

            // Get all website data types
            let webstore_class = objc2::class!(WKWebsiteDataStore);
            let all_types: *mut AnyObject = msg_send![webstore_class, allWebsiteDataTypes];

            // Remove all data (this is async but we're treating it as fire-and-forget)
            let date_class = objc2::class!(NSDate);
            let distant_past: *mut AnyObject = msg_send![date_class, distantPast];
            let _: () = msg_send![data_store, removeDataOfTypes: all_types, modifiedSince: distant_past, completionHandler: std::ptr::null::<*const AnyObject>()];

            log::info!("Clearing browsing data");
            Ok(())
        }
    }

    /// Helper method to set devtools on main thread
    fn set_devtools_on_main_thread(&self, enabled: bool) -> Result<(), MiniAppError> {
        unsafe {
            // Check if isInspectable is available (iOS 16.4+)
            let responds: bool =
                msg_send![self.webview, respondsToSelector: objc2::sel!(setInspectable:)];
            if responds {
                let _: () = msg_send![self.webview, setInspectable: enabled];
                log::info!("Devtools {}", if enabled { "enabled" } else { "disabled" });
            }
            Ok(())
        }
    }

    /// Helper method to set user agent on main thread
    fn set_user_agent_on_main_thread(&self, ua: String) -> Result<(), MiniAppError> {
        unsafe {
            let ua_string = NSString::from_str(&ua);
            let _: () = msg_send![self.webview, setCustomUserAgent: if ua.is_empty() { std::ptr::null::<NSString>() } else { &*ua_string }];
            Ok(())
        }
    }
}

impl WebViewController for WebViewInner {
    fn load_url(&self, url: String) -> Result<(), MiniAppError> {
        if MainThreadMarker::new().is_some() {
            // Already on main thread, execute directly
            self.load_url_on_main_thread(url)
        } else {
            // Not on main thread, dispatch to main thread using GCD
            let webview_ptr_addr = self.webview as usize;
            let url_clone = url.clone();

            DispatchQueue::main().exec_async(move || {
                // Reconstruct the WebViewInner from the pointer address
                let webview_ptr = webview_ptr_addr as *mut AnyObject;
                let temp_webview = WebViewInner::create_temporary(webview_ptr);
                if let Err(e) = temp_webview.load_url_on_main_thread(url_clone) {
                    log::error!("Failed to load URL on main thread: {}", e);
                }
            });

            Ok(())
        }
    }

    fn evaluate_javascript(&self, js: String) -> Result<(), MiniAppError> {
        if MainThreadMarker::new().is_some() {
            // Already on main thread, execute directly
            self.evaluate_javascript_on_main_thread(js)
        } else {
            // Not on main thread, dispatch to main thread using GCD
            let webview_ptr_addr = self.webview as usize;
            let js_clone = js.clone();

            DispatchQueue::main().exec_async(move || {
                // Reconstruct the WebViewInner from the pointer address
                let webview_ptr = webview_ptr_addr as *mut AnyObject;
                let temp_webview = WebViewInner::create_temporary(webview_ptr);
                if let Err(e) = temp_webview.evaluate_javascript_on_main_thread(js_clone) {
                    log::error!("Failed to evaluate JavaScript on main thread: {}", e);
                }
            });

            Ok(())
        }
    }

    fn clear_browsing_data(&self) -> Result<(), MiniAppError> {
        if MainThreadMarker::new().is_some() {
            // Already on main thread, execute directly
            self.clear_browsing_data_on_main_thread()
        } else {
            // Not on main thread, dispatch to main thread using GCD
            let webview_ptr_addr = self.webview as usize;

            DispatchQueue::main().exec_async(move || {
                // Reconstruct the WebViewInner from the pointer address
                let webview_ptr = webview_ptr_addr as *mut AnyObject;
                let temp_webview = WebViewInner::create_temporary(webview_ptr);
                if let Err(e) = temp_webview.clear_browsing_data_on_main_thread() {
                    log::error!("Failed to clear browsing data on main thread: {}", e);
                }
            });

            Ok(())
        }
    }

    fn set_devtools(&self, enabled: bool) -> Result<(), MiniAppError> {
        if MainThreadMarker::new().is_some() {
            // Already on main thread, execute directly
            self.set_devtools_on_main_thread(enabled)
        } else {
            // Not on main thread, dispatch to main thread using GCD
            let webview_ptr_addr = self.webview as usize;

            DispatchQueue::main().exec_async(move || {
                // Reconstruct the WebViewInner from the pointer address
                let webview_ptr = webview_ptr_addr as *mut AnyObject;
                let temp_webview = WebViewInner::create_temporary(webview_ptr);
                if let Err(e) = temp_webview.set_devtools_on_main_thread(enabled) {
                    log::error!("Failed to set devtools on main thread: {}", e);
                }
            });

            Ok(())
        }
    }

    fn post_message(&self, message: String) -> Result<(), MiniAppError> {
        // Escape the JSON message for safe JavaScript injection
        // Since message is already JSON, we need to escape it properly for JS string literal
        let escaped_message = message
            .replace('\\', "\\\\") // Escape backslashes first
            .replace('"', "\\\"") // Escape double quotes
            .replace('\n', "\\n") // Escape newlines
            .replace('\r', "\\r") // Escape carriage returns
            .replace('\t', "\\t"); // Escape tabs

        // Call the global receiver function defined in webview-bridge.js
        let js_code = format!(
            "if (typeof window.__LingXiaRecvMessage === 'function') {{ \
                window.__LingXiaRecvMessage(\"{}\"); \
            }} else {{ \
                console.warn('[LingXia] __LingXiaRecvMessage not available'); \
            }}",
            escaped_message
        );

        // Use evaluateJavaScript to send the message to the WebView
        self.evaluate_javascript(js_code)
    }

    fn set_user_agent(&self, ua: String) -> Result<(), MiniAppError> {
        if MainThreadMarker::new().is_some() {
            // Already on main thread, execute directly
            self.set_user_agent_on_main_thread(ua)
        } else {
            // Not on main thread, dispatch to main thread using GCD
            let webview_ptr_addr = self.webview as usize;
            let ua_clone = ua.clone();

            DispatchQueue::main().exec_async(move || {
                // Reconstruct the WebViewInner from the pointer address
                let webview_ptr = webview_ptr_addr as *mut AnyObject;
                let temp_webview = WebViewInner::create_temporary(webview_ptr);
                if let Err(e) = temp_webview.set_user_agent_on_main_thread(ua_clone) {
                    log::error!("Failed to set user agent on main thread: {}", e);
                }
            });

            Ok(())
        }
    }

    fn set_scroll_listener_enabled(
        &self,
        enabled: bool,
        throttle_ms: Option<u64>,
    ) -> Result<(), MiniAppError> {
        if MainThreadMarker::new().is_some() {
            Ok(())
        } else {
            // Not on main thread, dispatch to main thread using GCD
            let _webview_ptr_addr = self.webview as usize;
            let throttle = throttle_ms.unwrap_or(100);

            DispatchQueue::main().exec_async(move || {
                // For now, just log since this is a no-op
                log::info!(
                    "Scroll listener {} (throttle: {}ms) - dispatched",
                    if enabled { "enabled" } else { "disabled" },
                    throttle
                );
            });

            Ok(())
        }
    }
}

impl Drop for WebViewInner {
    fn drop(&mut self) {
        // Check if we're on main thread - if not, dispatch cleanup to main thread
        if MainThreadMarker::new().is_some() {
            // Already on main thread, perform cleanup directly
            self.cleanup_webview();
        } else {
            // Not on main thread, dispatch cleanup to main thread using GCD
            let webview_ptr_addr = self.webview as usize;

            DispatchQueue::main().exec_async(move || {
                // Reconstruct the WebViewInner from the pointer address for cleanup
                let webview_ptr = webview_ptr_addr as *mut AnyObject;
                let temp_webview = WebViewInner::create_temporary(webview_ptr);
                temp_webview.cleanup_webview();
            });
        }
    }
}

impl WebViewInner {
    /// Create a temporary WebViewInner for operations that don't need navigation delegate
    /// This is used when we need to call methods on a WebView from a different thread
    fn create_temporary(webview_ptr: *mut AnyObject) -> Self {
        WebViewInner {
            webview: webview_ptr,
            _navigation_delegate: None, // No delegate needed for temporary operations
        }
    }

    /// Cleanup WebView resources on main thread and properly release the WebView
    fn cleanup_webview(&self) {
        unsafe {
            // Remove from superview if attached to prevent memory leaks
            let _: () = msg_send![self.webview, removeFromSuperview];

            // Stop loading any ongoing requests
            let _: () = msg_send![self.webview, stopLoading];

            // Clear navigation delegate to prevent callbacks after deallocation
            let _: () = msg_send![self.webview, setNavigationDelegate: std::ptr::null::<*const AnyObject>()];

            // Clear UI delegate to prevent callbacks after deallocation
            let _: () = msg_send![self.webview, setUIDelegate: std::ptr::null::<*const AnyObject>()];

            // Clear configuration delegate to prevent callbacks after deallocation
            let _: () = msg_send![self.webview, setConfiguration: std::ptr::null::<*const AnyObject>()];

            // Clear scroll view delegate if any
            let scroll_view: *mut AnyObject = msg_send![self.webview, scrollView];
            if !scroll_view.is_null() {
                let _: () =
                    msg_send![scroll_view, setDelegate: std::ptr::null::<*const AnyObject>()];
            }

            // Explicitly release the WebView object
            // This is critical for proper memory management
            let _: () = msg_send![self.webview, release];

            log::info!(
                "WebView instance completely released: removed from superview, cleared all delegates, and released object"
            );
        }
    }
}
