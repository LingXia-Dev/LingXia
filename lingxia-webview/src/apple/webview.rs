use crate::webview::get_webview_delegate;
use crate::{LogLevel, WebViewController, WebViewError};
use block2::{Block, StackBlock};
use dispatch2::DispatchQueue;
use objc2::runtime::{AnyObject, NSObject, ProtocolObject};
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, class, define_class, msg_send, rc::Retained,
};
use objc2_foundation::{
    NSError, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString, NSURL, NSURLRequest,
};
use objc2_web_kit::{
    WKContentRuleList, WKContentRuleListStore, WKNavigation, WKNavigationDelegate,
    WKUserContentController, WKWebViewConfiguration,
};
use std::ptr::NonNull;
use std::sync::Arc;
use tokio::sync::oneshot::Sender;

use crate::webview::WebTag;

const HTTPS_BLOCK_RULE_IDENTIFIER: &str = "LingXiaHTTPSBlocker";
const HTTPS_BLOCK_RULE_JSON: &str = r#"
[
    {
        "trigger": {
            "url-filter": "^https:.*"
        },
        "action": {
            "type": "block"
        }
    }
]
"#;

// Custom Navigation Delegate for handling page lifecycle events
pub struct LingXiaNavigationDelegateIvars {
    webtag: WebTag,
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
        fn did_start_provisional_navigation(
            &self,
            _webview: *mut AnyObject,
            _navigation: &WKNavigation,
        ) {
            let webtag = &self.ivars().webtag;
            let (appid, path) = webtag.extract_parts();

            // Call delegate's on_page_started
            if let Some(delegate) = get_webview_delegate(webtag) {
                delegate.on_page_started();
            }
            log::info!("WebView page started: {} at {}", appid, path);
        }

        #[unsafe(method(webView:didFinishNavigation:))]
        fn did_finish_navigation(&self, _webview: *mut AnyObject, _navigation: &WKNavigation) {
            let webtag = &self.ivars().webtag;
            let (appid, path) = webtag.extract_parts();

            // Call delegate's on_page_finished
            if let Some(delegate) = get_webview_delegate(webtag) {
                delegate.on_page_finished();
            }
            log::info!("WebView page finished: {} at {}", appid, path);
        }

        #[unsafe(method(webView:decidePolicyForNavigationAction:decisionHandler:))]
        fn decide_policy_for_navigation_action(
            &self,
            _webview: *mut AnyObject,
            navigation_action: *mut AnyObject,
            decision_handler: *mut AnyObject,
        ) {
            // Helper function to call decision handler with policy
            let call_decision_handler = |policy: i32| {
                let handler: &Block<dyn Fn(i32)> =
                    unsafe { &*(decision_handler as *const Block<dyn Fn(i32)>) };
                handler.call((policy,));
            };

            // Helper function to allow navigation
            let allow_navigation = || call_decision_handler(1); // WKNavigationActionPolicyAllow = 1
            let cancel_navigation = || call_decision_handler(0); // WKNavigationActionPolicyCancel = 0

            // Extract URL from navigation action
            let url = match self.extract_url_from_navigation_action(navigation_action) {
                Some(url) => url,
                None => {
                    allow_navigation();
                    return;
                }
            };

            // Only intercept HTTPS navigation requests
            if !url.starts_with("https://") {
                allow_navigation();
                return;
            }

            let webtag = &self.ivars().webtag;
            let (_appid, _) = webtag.extract_parts();

            // Build HTTP request for lxapp to check
            let http_request = match http::Request::builder()
                .method("GET")
                .uri(&url)
                .body(Vec::new())
            {
                Ok(req) => req,
                Err(_) => {
                    log::warn!("Failed to build HTTP request for URL: {}", url);
                    allow_navigation();
                    return;
                }
            };

            // Ask delegate if it wants to handle this HTTPS navigation request
            let response = if let Some(delegate) = get_webview_delegate(webtag) {
                delegate.handle_request(http_request)
            } else {
                None
            };
            match response {
                Some(_) => {
                    // log::info!("Miniapp handling HTTPS navigation, canceling: {}", url);
                    cancel_navigation();
                }
                None => {
                    // log::info!("Miniapp allows HTTPS navigation: {}", url);
                    allow_navigation();
                }
            }
        }
    }
);

impl LingXiaNavigationDelegate {
    /// Extract URL string from navigation action, returns None if extraction fails
    fn extract_url_from_navigation_action(
        &self,
        navigation_action: *mut AnyObject,
    ) -> Option<String> {
        unsafe {
            let request: *mut AnyObject = msg_send![navigation_action, request];
            if request.is_null() {
                return None;
            }

            let url_obj: *mut AnyObject = msg_send![request, URL];
            if url_obj.is_null() {
                return None;
            }

            let url_string: *mut AnyObject = msg_send![url_obj, absoluteString];
            if url_string.is_null() {
                return None;
            }

            let url_cstring: *const std::ffi::c_char = msg_send![url_string, UTF8String];
            if url_cstring.is_null() {
                return None;
            }

            Some(
                std::ffi::CStr::from_ptr(url_cstring)
                    .to_string_lossy()
                    .to_string(),
            )
        }
    }
    pub fn new(appid: String, path: String, mtm: MainThreadMarker) -> Retained<Self> {
        let webtag = WebTag::new(&appid, &path, None);
        let delegate = mtm
            .alloc::<LingXiaNavigationDelegate>()
            .set_ivars(LingXiaNavigationDelegateIvars { webtag });

        unsafe { msg_send![super(delegate), init] }
    }
}

pub struct WebViewInner {
    webview: *mut AnyObject,
    _navigation_delegate: Retained<LingXiaNavigationDelegate>,
    _message_handler: Retained<LingXiaMessageHandler>,
    _scroll_delegate: std::cell::RefCell<Option<Retained<LingXiaScrollDelegate>>>,
    pub(crate) webtag: WebTag,
}

impl std::fmt::Debug for WebViewInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebViewInner")
            .field("webview", &self.webview)
            .field("webtag", &self.webtag)
            .field("navigation_delegate", &"<LingXiaNavigationDelegate>")
            .field("message_handler", &"<LingXiaMessageHandler>")
            .field("scroll_delegate", &self._scroll_delegate.borrow().is_some())
            .finish()
    }
}

unsafe impl Send for WebViewInner {}
unsafe impl Sync for WebViewInner {}

impl WebViewInner {
    /// Create a new WebView asynchronously with channel sender
    pub(crate) fn create(
        appid: &str,
        path: &str,
        session_id: Option<u64>,
        sender: Sender<Result<Arc<crate::WebView>, WebViewError>>,
    ) {
        let appid_owned = appid.to_string();
        let path_owned = path.to_string();

        // Check if we're already on the main thread
        if let Some(mtm) = MainThreadMarker::new() {
            // Already on main thread, create directly
            Self::create_and_register_sync(&appid_owned, &path_owned, session_id, mtm, sender);
        } else {
            // Not on main thread, dispatch to main thread
            let session_id_copy = session_id;
            DispatchQueue::main().exec_async(move || match MainThreadMarker::new() {
                Some(mtm) => {
                    Self::create_and_register_sync(
                        &appid_owned,
                        &path_owned,
                        session_id_copy,
                        mtm,
                        sender,
                    );
                }
                None => {
                    let error = WebViewError::WebView(
                        "No MainThreadMarker available on main thread".to_string(),
                    );
                    log::error!(
                        "Failed to get MainThreadMarker for {}-{}: {:?}",
                        appid_owned,
                        path_owned,
                        error
                    );
                    let _ = sender.send(Err(error));
                }
            });
        }
    }

    /// Helper method to create and register WebView synchronously on main thread
    fn create_and_register_sync(
        appid: &str,
        path: &str,
        session_id: Option<u64>,
        mtm: MainThreadMarker,
        sender: Sender<Result<Arc<crate::WebView>, WebViewError>>,
    ) {
        let result = Self::create_with_marker(appid, path, session_id, mtm);

        match result {
            Ok(webview_inner) => {
                // Wrap WebViewInner in WebView
                let webview = Arc::new(crate::WebView::new(webview_inner));

                // Register the WebView instance for future lookups
                crate::webview::register_webview(webview.clone());

                let _ = sender.send(Ok(webview));
                log::info!("WebView created successfully for {}-{}", appid, path);
            }
            Err(e) => {
                log::error!("Failed to create WebView for {}-{}: {:?}", appid, path, e);
                let _ = sender.send(Err(e));
            }
        }
    }

    /// Create WebView with MainThreadMarker (must be called on main thread)
    fn create_with_marker(
        appid: &str,
        path: &str,
        session_id: Option<u64>,
        mtm: MainThreadMarker,
    ) -> Result<Self, WebViewError> {
        unsafe {
            // Create WKWebViewConfiguration
            let config = WKWebViewConfiguration::new(mtm);

            // Use a non-persistent data store to disable DOM Storage (localStorage, etc.)
            let non_persistent_store: *mut AnyObject =
                msg_send![class!(WKWebsiteDataStore), nonPersistentDataStore];
            let _: () = msg_send![&*config, setWebsiteDataStore: non_persistent_store];

            // Access preferences to set security settings
            let prefs: *mut AnyObject = msg_send![&*config, preferences];
            let _: () = msg_send![prefs, setJavaScriptCanOpenWindowsAutomatically: false];

            // Disable local file access for security
            let allow_file_access_key = NSString::from_str("allowFileAccessFromFileURLs");
            let ns_false: *mut AnyObject = msg_send![class!(NSNumber), numberWithBool: false];
            let _: () = msg_send![prefs, setValue:ns_false, forKey:&*allow_file_access_key];

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

            // Register custom scheme handler for lx:// URLs bound to this WebView
            let webtag = WebTag::new(appid, path, session_id);
            if let Some(scheme_handler) =
                super::schemehandler::LingXiaSchemeHandler::new(webtag.clone())
            {
                let lx_scheme = NSString::from_str("lx");
                let _: () = msg_send![&*config, setURLSchemeHandler: &*scheme_handler, forURLScheme: &*lx_scheme];

                log::info!(
                    "Successfully registered scheme `lx` handler for webtag={}",
                    webtag.as_str()
                );
            } else {
                log::error!("CRITICAL: Failed to create scheme `lx` handler!");
                return Err(WebViewError::WebView(
                    "Failed to create scheme `lx` handler".to_string(),
                ));
            }

            // Block all HTTPS subresource loads by default
            Self::enable_https_resource_blocker(&config);

            // Create frame with zero size and hide the webview initially to prevent flicker due to size change on macOS.
            // It will be resized and unhidden by the Swift layout code.
            let frame = NSRect {
                origin: NSPoint { x: 0.0, y: 0.0 },
                size: NSSize {
                    width: 0.0,
                    height: 0.0,
                },
            };

            // Get WKWebView class
            let webview_class = objc2::class!(WKWebView);

            // Allocate WebView
            let webview: *mut AnyObject = msg_send![webview_class, alloc];
            if webview.is_null() {
                log::error!("Failed to allocate WKWebView");
                return Err(WebViewError::WebView(
                    "Failed to allocate WKWebView".to_string(),
                ));
            }

            // Initialize WebView
            let webview: *mut AnyObject =
                msg_send![webview, initWithFrame: frame, configuration: &*config];
            if webview.is_null() {
                log::error!("Failed to initialize WKWebView");
                return Err(WebViewError::WebView(
                    "Failed to initialize WKWebView".to_string(),
                ));
            }

            // Immediately hide the webview. It will be made visible by Swift once it's sized and positioned.
            let _: () = msg_send![webview, setHidden: true];

            // Create navigation delegate
            let navigation_delegate =
                LingXiaNavigationDelegate::new(appid.to_string(), path.to_string(), mtm);

            // Set the navigation delegate on the WebView
            let proto_navigation_delegate: &ProtocolObject<dyn WKNavigationDelegate> =
                ProtocolObject::from_ref(&*navigation_delegate);
            let _: () = msg_send![webview, setNavigationDelegate: Some(proto_navigation_delegate)];

            // Set up message handler for bridge communication
            let message_handler = Self::setup_message_handler(webview, appid, path)?;

            // Create WebViewInner instance with navigation delegate and message handler
            let webview_inner = WebViewInner {
                webview,
                _navigation_delegate: navigation_delegate,
                _message_handler: message_handler,
                _scroll_delegate: std::cell::RefCell::new(None), // Will be set when scroll listener is enabled
                webtag,
            };

            Ok(webview_inner)
        }
    }

    /// Install a content blocking rule list that blocks all HTTPS subresource loads.
    fn enable_https_resource_blocker(config: &WKWebViewConfiguration) {
        let Some(mtm) = MainThreadMarker::new() else {
            log::warn!("HTTPS resource blocker requires MainThreadMarker");
            return;
        };

        let user_content_controller: *mut WKUserContentController =
            unsafe { msg_send![config, userContentController] };
        let Some(controller_ptr) = NonNull::new(user_content_controller) else {
            log::warn!("Failed to obtain userContentController for HTTPS blocker");
            return;
        };

        let Some(store) = (unsafe { WKContentRuleListStore::defaultStore(mtm) }) else {
            log::warn!("Failed to access WKContentRuleListStore for HTTPS blocker");
            return;
        };

        let controller_ref = unsafe { controller_ptr.as_ref() };
        unsafe {
            controller_ref.removeAllContentRuleLists();
        }

        let identifier = NSString::from_str(HTTPS_BLOCK_RULE_IDENTIFIER);
        let rule_json = NSString::from_str(HTTPS_BLOCK_RULE_JSON);

        let controller_raw = controller_ptr.as_ptr();
        let completion = StackBlock::new(
            move |rule_list_ptr: *mut WKContentRuleList, error_ptr: *mut NSError| {
                if let Some(rule_list) = NonNull::new(rule_list_ptr) {
                    unsafe {
                        let controller = &*controller_raw;
                        controller.addContentRuleList(rule_list.as_ref());
                    }
                    log::info!("HTTPS resource blocking enabled for WebView");
                } else if let Some(error) = NonNull::new(error_ptr) {
                    let description = unsafe {
                        let description_ptr: *mut NSString =
                            msg_send![error.as_ptr(), localizedDescription];
                        description_ptr
                            .as_ref()
                            .map(|ns_string| ns_string.to_string())
                    };

                    if let Some(description) = description {
                        log::error!("Failed to compile HTTPS block rule: {}", description);
                    } else {
                        log::error!("Failed to compile HTTPS block rule with unknown error");
                    }
                } else {
                    log::error!("Failed to compile HTTPS block rule with no error provided");
                }
            },
        )
        .copy();

        unsafe {
            store.compileContentRuleListForIdentifier_encodedContentRuleList_completionHandler(
                Some(&identifier),
                Some(&rule_json),
                Some(&completion),
            );
        }
    }

    /// Set up message handlers for bridge communication (like Swift version)
    fn setup_message_handler(
        webview: *mut AnyObject,
        appid: &str,
        path: &str,
    ) -> Result<Retained<LingXiaMessageHandler>, WebViewError> {
        unsafe {
            // Get the configuration from the WebView
            let config: *mut AnyObject = msg_send![webview, configuration];
            let user_content_controller: *mut AnyObject = msg_send![config, userContentController];

            let console_script = r#"
                (function() {
                    const originalLog = console.log;
                    const originalError = console.error;
                    const originalWarn = console.warn;
                    const originalInfo = console.info;

                    function sendConsoleMessage(level, args) {
                        const message = args.map(arg =>
                            typeof arg === 'object' ? JSON.stringify(arg) : String(arg)
                        ).join(' ');

                        try {
                            // Use dedicated console handler (like Swift version)
                            window.webkit.messageHandlers.LingXiaConsole.postMessage({
                                level: level,
                                message: message
                            });
                        } catch (e) {
                            // Fallback if message handler not ready
                        }
                    }

                    console.log = function(...args) {
                        sendConsoleMessage('log', args);
                        originalLog.apply(console, args);
                    };

                    console.error = function(...args) {
                        sendConsoleMessage('error', args);
                        originalError.apply(console, args);
                    };

                    console.warn = function(...args) {
                        sendConsoleMessage('warn', args);
                        originalWarn.apply(console, args);
                    };

                    console.info = function(...args) {
                        sendConsoleMessage('info', args);
                        originalInfo.apply(console, args);
                    };
                })();
            "#;

            // Inject console interceptor script
            let console_js_string = NSString::from_str(console_script);
            let user_script_class = objc2::class!(WKUserScript);
            let console_user_script: *mut AnyObject = msg_send![user_script_class, alloc];
            let console_user_script: *mut AnyObject = msg_send![console_user_script,
                initWithSource: &*console_js_string,
                injectionTime: 0, // WKUserScriptInjectionTimeAtDocumentStart
                forMainFrameOnly: false];

            let _: () = msg_send![user_content_controller, addUserScript: console_user_script];

            let message_handler = LingXiaMessageHandler::new(
                appid.to_string(),
                path.to_string(),
                MainThreadMarker::new().unwrap(),
            )
            .ok_or_else(|| WebViewError::WebView("Failed to create message handler".to_string()))?;

            // Register message handlers with userContentController (like Swift version)
            let lingxia_name = NSString::from_str("LingXia");
            let console_name = NSString::from_str("LingXiaConsole");
            let _: () = msg_send![user_content_controller, addScriptMessageHandler: &*message_handler, name: &*lingxia_name];
            let _: () = msg_send![user_content_controller, addScriptMessageHandler: &*message_handler, name: &*console_name];

            Ok(message_handler)
        }
    }

    /// Get the raw pointer to the WebView for Swift interop
    pub fn get_swift_webview_ptr(&self) -> usize {
        self.webview as usize
    }

    /// Helper method to load URL on main thread
    fn load_url_on_main_thread(&self, url: String) -> Result<(), WebViewError> {
        unsafe {
            let ns_url_string = NSString::from_str(&url);
            if let Some(ns_url) = NSURL::URLWithString(&ns_url_string) {
                let request = NSURLRequest::requestWithURL(&ns_url);
                let _: () = msg_send![self.webview, loadRequest: &*request];
                Ok(())
            } else {
                Err(WebViewError::WebView(format!("Invalid URL: {}", url)))
            }
        }
    }

    /// Helper method to load HTML data on main thread
    fn load_data_on_main_thread(
        &self,
        data: String,
        base_url: String,
        _history_url: Option<String>,
    ) -> Result<(), WebViewError> {
        unsafe {
            let data_nsstring = NSString::from_str(&data);
            let base_url_nsstring = NSString::from_str(&base_url);
            if let Some(base_url_obj) = NSURL::URLWithString(&base_url_nsstring) {
                let _: () = msg_send![self.webview, loadHTMLString: &*data_nsstring, baseURL: &*base_url_obj];
                log::info!("Loaded HTML data into WebView with base URL: {}", base_url);
                Ok(())
            } else {
                Err(WebViewError::WebView(format!(
                    "Invalid base URL: {}",
                    base_url
                )))
            }
        }
    }

    /// Helper method to evaluate JavaScript on main thread
    fn evaluate_javascript_on_main_thread(&self, js: String) -> Result<(), WebViewError> {
        unsafe {
            let js_string = NSString::from_str(&js);
            // Note: evaluateJavaScript is async, but we're treating it as fire-and-forget
            // In a more complete implementation, we might want to handle the completion
            let _: () = msg_send![self.webview, evaluateJavaScript: &*js_string, completionHandler: std::ptr::null::<*const AnyObject>()];
            Ok(())
        }
    }

    /// Helper method to clear browsing data on main thread
    fn clear_browsing_data_on_main_thread(&self) -> Result<(), WebViewError> {
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

    /// Helper method to set user agent on main thread
    fn set_user_agent_on_main_thread(&self, ua: String) -> Result<(), WebViewError> {
        unsafe {
            let ua_string = NSString::from_str(&ua);
            let _: () = msg_send![self.webview, setCustomUserAgent: if ua.is_empty() { std::ptr::null::<NSString>() } else { &*ua_string }];
            Ok(())
        }
    }

    /// Helper method to set scroll listener on main thread using native UIScrollViewDelegate
    fn set_scroll_listener_on_main_thread(
        &self,
        enabled: bool,
        throttle_ms: Option<u64>,
    ) -> Result<(), WebViewError> {
        unsafe {
            // Get the scroll view from the WebView
            let scroll_view: *mut AnyObject = msg_send![self.webview, scrollView];
            if scroll_view.is_null() {
                log::error!("Failed to get scroll view from WebView");
                return Err(WebViewError::WebView(
                    "Failed to get scroll view".to_string(),
                ));
            }

            if enabled {
                // Create scroll delegate if not already created
                if self._scroll_delegate.borrow().is_none() {
                    let throttle = throttle_ms.unwrap_or(100);

                    if let Some(scroll_delegate) = {
                        let (appid, path) = self.webtag.extract_parts();
                        LingXiaScrollDelegate::new(appid, path, throttle)
                    } {
                        // Set the delegate on the scroll view
                        let _: () = msg_send![scroll_view, setDelegate: &*scroll_delegate];
                        *self._scroll_delegate.borrow_mut() = Some(scroll_delegate);
                        log::info!(
                            "Native scroll listener enabled with {}ms throttle",
                            throttle
                        );
                    } else {
                        log::error!("Failed to create scroll delegate");
                        return Err(WebViewError::WebView(
                            "Failed to create scroll delegate".to_string(),
                        ));
                    }
                } else {
                    log::info!("Scroll listener already enabled");
                }
            } else {
                // Disable scroll listener by removing delegate
                if self._scroll_delegate.borrow().is_some() {
                    let _: () =
                        msg_send![scroll_view, setDelegate: std::ptr::null::<*const AnyObject>()];
                    *self._scroll_delegate.borrow_mut() = None;
                    log::info!("Native scroll listener disabled");
                } else {
                    log::info!("Scroll listener already disabled");
                }
            }

            Ok(())
        }
    }
}

impl WebViewController for WebViewInner {
    fn load_url(&self, url: String) -> Result<(), WebViewError> {
        if MainThreadMarker::new().is_some() {
            // Already on main thread, execute directly
            self.load_url_on_main_thread(url)
        } else {
            // Not on main thread, dispatch to main thread using GCD
            let webview_ptr_addr = self.webview as usize;
            let url_clone = url.clone();

            DispatchQueue::main().exec_async(move || unsafe {
                let webview_ptr = webview_ptr_addr as *mut AnyObject;
                let url_nsstring = NSString::from_str(&url_clone);
                let url = NSURL::URLWithString(&url_nsstring);
                if let Some(url) = url {
                    let request = NSURLRequest::requestWithURL(&url);
                    let _: () = msg_send![webview_ptr, loadRequest: &*request];
                }
            });

            Ok(())
        }
    }

    fn load_data(
        &self,
        data: String,
        base_url: String,
        history_url: Option<String>,
    ) -> Result<(), WebViewError> {
        if MainThreadMarker::new().is_some() {
            // Already on main thread, execute directly
            self.load_data_on_main_thread(data, base_url, history_url)
        } else {
            // Not on main thread, dispatch to main thread using GCD
            let webview_ptr_addr = self.webview as usize;
            let data_clone = data.clone();
            let base_url_clone = base_url.clone();

            DispatchQueue::main().exec_async(move || unsafe {
                let webview_ptr = webview_ptr_addr as *mut AnyObject;
                let data_nsstring = NSString::from_str(&data_clone);
                let base_url_nsstring = NSString::from_str(&base_url_clone);
                let base_url = NSURL::URLWithString(&base_url_nsstring);

                if let Some(base_url) = base_url {
                    let _: () = msg_send![webview_ptr, loadHTMLString: &*data_nsstring, baseURL: &*base_url];
                }
            });

            Ok(())
        }
    }

    fn evaluate_javascript(&self, js: String) -> Result<(), WebViewError> {
        if MainThreadMarker::new().is_some() {
            // Already on main thread, execute directly
            self.evaluate_javascript_on_main_thread(js)
        } else {
            // Not on main thread, dispatch to main thread using GCD
            let webview_ptr_addr = self.webview as usize;
            let js_clone = js.clone();

            DispatchQueue::main().exec_async(move || {
                unsafe {
                    let webview_ptr = webview_ptr_addr as *mut AnyObject;
                    let js_nsstring = NSString::from_str(&js_clone);
                    let _: () = msg_send![webview_ptr, evaluateJavaScript: &*js_nsstring, completionHandler: std::ptr::null::<*const AnyObject>()];
                }
            });

            Ok(())
        }
    }

    fn clear_browsing_data(&self) -> Result<(), WebViewError> {
        if MainThreadMarker::new().is_some() {
            // Already on main thread, execute directly
            self.clear_browsing_data_on_main_thread()
        } else {
            // Not on main thread, dispatch to main thread using GCD
            let webview_ptr_addr = self.webview as usize;

            DispatchQueue::main().exec_async(move || {
                unsafe {
                    let webview_ptr = webview_ptr_addr as *mut AnyObject;
                    // Get WKWebsiteDataStore and clear data
                    let configuration: *mut AnyObject = msg_send![webview_ptr, configuration];
                    let data_store: *mut AnyObject = msg_send![configuration, websiteDataStore];
                    let data_types: *mut AnyObject = msg_send![class!(WKWebsiteDataStore), allWebsiteDataTypes];
                    let _: () = msg_send![data_store, removeDataOfTypes: data_types, modifiedSince: std::ptr::null::<*const AnyObject>(), completionHandler: std::ptr::null::<*const AnyObject>()];
                }
            });

            Ok(())
        }
    }

    fn set_user_agent(&self, ua: String) -> Result<(), WebViewError> {
        if MainThreadMarker::new().is_some() {
            // Already on main thread, execute directly
            self.set_user_agent_on_main_thread(ua)
        } else {
            // Not on main thread, dispatch to main thread using GCD
            let webview_ptr_addr = self.webview as usize;
            let ua_clone = ua.clone();

            DispatchQueue::main().exec_async(move || unsafe {
                let webview_ptr = webview_ptr_addr as *mut AnyObject;
                let ua_nsstring = NSString::from_str(&ua_clone);
                let _: () = msg_send![webview_ptr, setCustomUserAgent: &*ua_nsstring];
            });

            Ok(())
        }
    }

    fn set_scroll_listener_enabled(
        &self,
        enabled: bool,
        throttle_ms: Option<u64>,
    ) -> Result<(), WebViewError> {
        if MainThreadMarker::new().is_some() {
            // Already on main thread, execute directly
            self.set_scroll_listener_on_main_thread(enabled, throttle_ms)
        } else {
            // Cross-thread scroll listener setup is complex and requires webtag
            // For now, return error - this should be called on main thread
            Err(WebViewError::WebView(
                "Scroll listener must be set on main thread".to_string(),
            ))
        }
    }
}

impl Drop for WebViewInner {
    fn drop(&mut self) {
        // WebView cleanup - only cleanup the actual WebView resources
        // Runtime storage is managed separately
        if MainThreadMarker::new().is_some() {
            self.cleanup_webview();
        } else {
            let webview_ptr_addr = self.webview as usize;
            DispatchQueue::main().exec_async(move || {
                unsafe {
                    let webview_ptr = webview_ptr_addr as *mut AnyObject;
                    // Direct cleanup without creating temporary instance
                    let _: () = msg_send![webview_ptr, removeFromSuperview];
                    let _: () = msg_send![webview_ptr, stopLoading];
                }
            });
            log::info!(
                "[WebViewInner] Apple WebViewInner dropped and cleanup requested ({})",
                self.webtag.as_str()
            );
        }
    }
}

impl WebViewInner {
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
            let _: () =
                msg_send![self.webview, setUIDelegate: std::ptr::null::<*const AnyObject>()];

            // Clear scroll view delegate if any
            let scroll_view: *mut AnyObject = msg_send![self.webview, scrollView];
            if !scroll_view.is_null() {
                let _: () =
                    msg_send![scroll_view, setDelegate: std::ptr::null::<*const AnyObject>()];
            }

            // Release the WebView object
            // This is critical for proper memory management
            let _: () = msg_send![self.webview, release];

            log::info!(
                "[WebViewInner] Apple WebView instance completely released: removed from superview, cleared all delegates, and released object ({})",
                self.webtag.as_str()
            );
        }
    }
}

// Message Handler Implementation (like Swift WebViewMessageHandler)
use objc2_web_kit::WKScriptMessageHandler;

#[derive(Debug)]
pub struct LingXiaMessageHandlerIvars {
    appid: String,
    path: String,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "LingXiaMessageHandler"]
    #[thread_kind = MainThreadOnly]
    #[ivars = LingXiaMessageHandlerIvars]
    pub struct LingXiaMessageHandler;

    unsafe impl NSObjectProtocol for LingXiaMessageHandler {}

    unsafe impl WKScriptMessageHandler for LingXiaMessageHandler {
        #[unsafe(method(userContentController:didReceiveScriptMessage:))]
        fn user_content_controller_did_receive_script_message(
            &self,
            _controller: *mut AnyObject,
            message: *mut AnyObject,
        ) {
            unsafe {
                // Get message body
                let body: *mut AnyObject = msg_send![message, body];

                let message_string = if !body.is_null() {
                    // Try to get as string first
                    let is_string: bool = msg_send![body, isKindOfClass: objc2::class!(NSString)];
                    if is_string {
                        let ns_string = &*(body as *const NSString);
                        ns_string.to_string()
                    } else {
                        // Try to convert to JSON if it's a dictionary
                        let is_dict: bool =
                            msg_send![body, isKindOfClass: objc2::class!(NSDictionary)];
                        if is_dict {
                            // Convert NSDictionary to JSON string (like Swift version)
                            let json_data: *mut AnyObject = msg_send![objc2::class!(NSJSONSerialization),
                                dataWithJSONObject: body,
                                options: 0,
                                error: std::ptr::null_mut::<*mut AnyObject>()];

                            if !json_data.is_null() {
                                let json_string: *mut AnyObject =
                                    msg_send![objc2::class!(NSString), alloc];
                                let json_string: *mut AnyObject = msg_send![json_string,
                                    initWithData: json_data,
                                    encoding: 4]; // NSUTF8StringEncoding

                                if !json_string.is_null() {
                                    let ns_string = &*(json_string as *const NSString);
                                    ns_string.to_string()
                                } else {
                                    log::error!("Failed to convert JSON data to string");
                                    return;
                                }
                            } else {
                                log::error!("Failed to serialize dictionary to JSON");
                                return;
                            }
                        } else {
                            log::error!("Unsupported message body type");
                            return;
                        }
                    }
                } else {
                    log::error!("Message body is null");
                    return;
                };

                // Get message name to determine handler type
                let name: *mut AnyObject = msg_send![message, name];
                let handler_name = if !name.is_null() {
                    let ns_string = &*(name as *const NSString);
                    ns_string.to_string()
                } else {
                    "unknown".to_string()
                };

                // Route to appropriate handler based on name (like Swift version)
                match handler_name.as_str() {
                    "LingXia" => {
                        self.handle_bridge_message(message_string);
                    }
                    "LingXiaConsole" => {
                        self.handle_console_message(message_string);
                    }
                    _ => {
                        log::warn!("Unknown message handler: {}", handler_name);
                    }
                }
            }
        }
    }
);

impl LingXiaMessageHandler {
    /// Create a new message handler
    pub fn new(appid: String, path: String, mtm: MainThreadMarker) -> Option<Retained<Self>> {
        unsafe {
            let instance = Self::alloc(mtm);
            let instance = instance.set_ivars(LingXiaMessageHandlerIvars { appid, path });
            let instance: Retained<Self> = msg_send![super(instance), init];
            Some(instance)
        }
    }

    /// Handle bridge messages
    fn handle_bridge_message(&self, message: String) {
        let ivars = self.ivars();

        let webtag = WebTag::new(&ivars.appid, &ivars.path, None);
        if let Some(delegate) = get_webview_delegate(&webtag) {
            delegate.handle_post_message(message);
        }
    }

    /// Handle console messages
    fn handle_console_message(&self, message: String) {
        let ivars = self.ivars();

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&message) {
            if let (Some(level), Some(console_message)) = (
                json.get("level").and_then(|v| v.as_str()),
                json.get("message").and_then(|v| v.as_str()),
            ) {
                let log_level = match level {
                    "error" => LogLevel::Error,
                    "warn" => LogLevel::Warn,
                    "info" => LogLevel::Info,
                    "debug" => LogLevel::Debug,
                    _ => LogLevel::Info,
                };

                let webtag = WebTag::new(&ivars.appid, &ivars.path, None);
                if let Some(delegate) = get_webview_delegate(&webtag) {
                    delegate.log(log_level, console_message);
                }
            } else {
                log::error!("Failed to parse console message fields: {}", message);
            }
        } else {
            log::error!("Failed to parse console message JSON: {}", message);
        }
    }
}

// Scroll Delegate Implementation for native scroll monitoring
#[derive(Debug)]
pub struct LingXiaScrollDelegateIvars {
    appid: String,
    path: String,
    throttle_ms: u64,
    last_scroll_time: std::cell::Cell<u64>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "LingXiaScrollDelegate"]
    #[thread_kind = MainThreadOnly]
    #[ivars = LingXiaScrollDelegateIvars]
    pub struct LingXiaScrollDelegate;

    unsafe impl NSObjectProtocol for LingXiaScrollDelegate {}

    // Implement UIScrollViewDelegate methods
    impl LingXiaScrollDelegate {
        #[unsafe(method(scrollViewDidScroll:))]
        fn scroll_view_did_scroll(&self, scroll_view: *mut AnyObject) {
            unsafe {
                // Get current time in milliseconds
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;

                // Check throttling
                let last_time = self.ivars().last_scroll_time.get();
                if now - last_time < self.ivars().throttle_ms {
                    return; // Skip this scroll event due to throttling
                }
                self.ivars().last_scroll_time.set(now);

                // Get scroll position
                let content_offset: NSPoint = msg_send![scroll_view, contentOffset];
                let content_size: NSSize = msg_send![scroll_view, contentSize];
                let frame_size: NSSize = msg_send![scroll_view, frame];

                let scroll_x = content_offset.x as i32;
                let scroll_y = content_offset.y as i32;
                let _max_scroll_x = (content_size.width - frame_size.width).max(0.0) as i32;
                let _max_scroll_y = (content_size.height - frame_size.height).max(0.0) as i32;

                // Call delegate's on_page_scroll_changed
                let webtag = WebTag::new(&self.ivars().appid, &self.ivars().path, None);
                if let Some(delegate) = get_webview_delegate(&webtag) {
                    delegate.on_page_scroll_changed(
                        scroll_x,
                        scroll_y,
                        _max_scroll_x,
                        _max_scroll_y,
                    );
                }
            }
        }
    }
);

impl LingXiaScrollDelegate {
    /// Create a new LingXiaScrollDelegate
    pub fn new(appid: String, path: String, throttle_ms: u64) -> Option<Retained<Self>> {
        let mtm = match MainThreadMarker::new() {
            Some(marker) => marker,
            None => {
                log::error!("Not on main thread when creating scroll delegate");
                return None;
            }
        };

        unsafe {
            let instance = Self::alloc(mtm);
            let instance = instance.set_ivars(LingXiaScrollDelegateIvars {
                appid,
                path,
                throttle_ms,
                last_scroll_time: std::cell::Cell::new(0),
            });
            let instance: Retained<Self> = msg_send![super(instance), init];
            Some(instance)
        }
    }
}
