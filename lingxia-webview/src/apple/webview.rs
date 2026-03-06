use crate::webview::get_webview_delegate;
use crate::{LogLevel, WebViewController, WebViewError};
use block2::{Block, StackBlock};
use dispatch2::DispatchQueue;
use objc2::runtime::{AnyObject, NSObject, ProtocolObject};
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, class, define_class, msg_send,
    rc::Retained,
};
use objc2_foundation::{
    NSDate, NSError, NSJSONSerialization, NSJSONWritingOptions, NSObjectProtocol, NSPoint, NSRect,
    NSSize, NSString, NSURL, NSURLRequest,
};
use objc2_web_kit::{
    WKAudiovisualMediaTypes, WKContentRuleList, WKContentRuleListStore, WKNavigation,
    WKNavigationDelegate, WKUIDelegate, WKURLSchemeHandler, WKUserContentController,
    WKWebViewConfiguration, WKWebsiteDataStore,
};
use std::ptr::NonNull;
use std::sync::Arc;
use tokio::sync::oneshot::Sender;

use crate::webview::{EffectiveWebViewCreateOptions, SecurityProfile, WebTag};

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
    intercept_https_navigation: bool,
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

            if !self.ivars().intercept_https_navigation {
                allow_navigation();
                return;
            }

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
    pub fn new(
        appid: String,
        path: String,
        session_id: Option<u64>,
        intercept_https_navigation: bool,
        mtm: MainThreadMarker,
    ) -> Retained<Self> {
        let webtag = WebTag::new(&appid, &path, session_id);
        let delegate =
            mtm.alloc::<LingXiaNavigationDelegate>()
                .set_ivars(LingXiaNavigationDelegateIvars {
                    webtag,
                    intercept_https_navigation,
                });

        unsafe { msg_send![super(delegate), init] }
    }
}

// UI Delegate for browser mode:
// - Handles target="_blank" / window.open() by loading in the same webview
// - Shows native NSAlert for JavaScript alert/confirm/prompt dialogs
pub struct LingXiaUIDelegateIvars {}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "LingXiaUIDelegate"]
    #[thread_kind = MainThreadOnly]
    #[ivars = LingXiaUIDelegateIvars]
    pub struct LingXiaUIDelegate;

    unsafe impl NSObjectProtocol for LingXiaUIDelegate {}

    unsafe impl WKUIDelegate for LingXiaUIDelegate {
        #[unsafe(method(webView:createWebViewWithConfiguration:forNavigationAction:windowFeatures:))]
        fn create_web_view(
            &self,
            webview: *mut AnyObject,
            _configuration: &WKWebViewConfiguration,
            navigation_action: *mut AnyObject,
            _window_features: *mut AnyObject,
        ) -> *mut AnyObject {
            // Load target="_blank" / window.open() URLs in the same webview.
            unsafe {
                let request: *mut AnyObject = msg_send![navigation_action, request];
                if !request.is_null() {
                    let _: () = msg_send![webview, loadRequest: request];
                }
            }
            std::ptr::null_mut()
        }

        // JavaScript alert()
        #[unsafe(method(webView:runJavaScriptAlertPanelWithMessage:initiatedByFrame:completionHandler:))]
        fn run_javascript_alert(
            &self,
            _webview: *mut AnyObject,
            message: &NSString,
            _frame: *mut AnyObject,
            completion_handler: *mut AnyObject,
        ) {
            #[cfg(target_os = "macos")]
            unsafe {
                let alert: *mut AnyObject = msg_send![class!(NSAlert), new];
                let _: () = msg_send![alert, setMessageText: message];
                let ok_title = NSString::from_str("OK");
                let _: () = msg_send![alert, addButtonWithTitle: &*ok_title];
                let _: i64 = msg_send![alert, runModal];
                let _: () = msg_send![alert, release];
            }
            let handler: &Block<dyn Fn()> =
                unsafe { &*(completion_handler as *const Block<dyn Fn()>) };
            handler.call(());
        }

        // JavaScript confirm()
        #[unsafe(method(webView:runJavaScriptConfirmPanelWithMessage:initiatedByFrame:completionHandler:))]
        fn run_javascript_confirm(
            &self,
            _webview: *mut AnyObject,
            message: &NSString,
            _frame: *mut AnyObject,
            completion_handler: *mut AnyObject,
        ) {
            let result;
            #[cfg(target_os = "macos")]
            unsafe {
                let alert: *mut AnyObject = msg_send![class!(NSAlert), new];
                let _: () = msg_send![alert, setMessageText: message];
                let ok_title = NSString::from_str("OK");
                let cancel_title = NSString::from_str("Cancel");
                let _: () = msg_send![alert, addButtonWithTitle: &*ok_title];
                let _: () = msg_send![alert, addButtonWithTitle: &*cancel_title];
                let response: i64 = msg_send![alert, runModal];
                let _: () = msg_send![alert, release];
                // NSAlertFirstButtonReturn = 1000
                result = response == 1000;
            }
            #[cfg(not(target_os = "macos"))]
            {
                // Browser mode is desktop-only in phase-1; fail closed on non-macOS.
                result = false;
            }
            let handler: &Block<dyn Fn(objc2::runtime::Bool)> =
                unsafe { &*(completion_handler as *const Block<dyn Fn(objc2::runtime::Bool)>) };
            handler.call((objc2::runtime::Bool::new(result),));
        }

        // JavaScript prompt()
        #[unsafe(method(webView:runJavaScriptTextInputPanelWithPrompt:defaultText:initiatedByFrame:completionHandler:))]
        fn run_javascript_prompt(
            &self,
            _webview: *mut AnyObject,
            prompt: &NSString,
            default_text: *mut AnyObject,
            _frame: *mut AnyObject,
            completion_handler: *mut AnyObject,
        ) {
            let input_value: *mut AnyObject;
            #[cfg(target_os = "macos")]
            unsafe {
                let alert: *mut AnyObject = msg_send![class!(NSAlert), new];
                let _: () = msg_send![alert, setMessageText: prompt];
                let ok_title = NSString::from_str("OK");
                let cancel_title = NSString::from_str("Cancel");
                let _: () = msg_send![alert, addButtonWithTitle: &*ok_title];
                let _: () = msg_send![alert, addButtonWithTitle: &*cancel_title];

                // Add text input field
                let input: *mut AnyObject = msg_send![class!(NSTextField), alloc];
                let frame = NSRect {
                    origin: NSPoint { x: 0.0, y: 0.0 },
                    size: NSSize {
                        width: 200.0,
                        height: 24.0,
                    },
                };
                let input: *mut AnyObject = msg_send![input, initWithFrame: frame];
                if !default_text.is_null() {
                    let _: () = msg_send![input, setStringValue: default_text];
                }
                let _: () = msg_send![alert, setAccessoryView: input];
                // `input` is +1 from alloc/init; alert retains accessory view.
                let _: () = msg_send![input, release];

                let response: i64 = msg_send![alert, runModal];
                // Read stringValue BEFORE releasing alert, because alert owns
                // the accessory view (input) and release may deallocate it.
                if response == 1000 {
                    input_value = msg_send![input, stringValue];
                } else {
                    input_value = std::ptr::null_mut();
                }
                let _: () = msg_send![alert, release];
            }
            #[cfg(not(target_os = "macos"))]
            {
                // Browser mode is desktop-only in phase-1; fail closed on non-macOS.
                let _ = default_text;
                input_value = std::ptr::null_mut();
            }
            let handler: &Block<dyn Fn(*mut AnyObject)> =
                unsafe { &*(completion_handler as *const Block<dyn Fn(*mut AnyObject)>) };
            handler.call((input_value,));
        }
    }
);

impl LingXiaUIDelegate {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let delegate = mtm
            .alloc::<LingXiaUIDelegate>()
            .set_ivars(LingXiaUIDelegateIvars {});
        unsafe { msg_send![super(delegate), init] }
    }
}

pub struct WebViewInner {
    webview: *mut AnyObject,
    _navigation_delegate: Retained<LingXiaNavigationDelegate>,
    _ui_delegate: Option<Retained<LingXiaUIDelegate>>,
    _message_handler: Retained<LingXiaMessageHandler>,
    pub(crate) webtag: WebTag,
}

impl std::fmt::Debug for WebViewInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebViewInner")
            .field("webview", &self.webview)
            .field("webtag", &self.webtag)
            .field("navigation_delegate", &"<LingXiaNavigationDelegate>")
            .field("message_handler", &"<LingXiaMessageHandler>")
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
        effective_options: EffectiveWebViewCreateOptions,
        sender: Sender<Result<Arc<crate::WebView>, WebViewError>>,
    ) {
        let appid_owned = appid.to_string();
        let path_owned = path.to_string();

        // Check if we're already on the main thread
        if let Some(mtm) = MainThreadMarker::new() {
            // Already on main thread, create directly
            Self::create_and_register_sync(
                &appid_owned,
                &path_owned,
                session_id,
                effective_options,
                mtm,
                sender,
            );
        } else {
            // Not on main thread, dispatch to main thread
            let session_id_copy = session_id;
            let options_copy = effective_options.clone();
            DispatchQueue::main().exec_async(move || match MainThreadMarker::new() {
                Some(mtm) => {
                    Self::create_and_register_sync(
                        &appid_owned,
                        &path_owned,
                        session_id_copy,
                        options_copy,
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
        effective_options: EffectiveWebViewCreateOptions,
        mtm: MainThreadMarker,
        sender: Sender<Result<Arc<crate::WebView>, WebViewError>>,
    ) {
        let result = Self::create_with_marker(appid, path, session_id, &effective_options, mtm);

        match result {
            Ok(webview_inner) => {
                // Wrap WebViewInner in WebView
                let webview = Arc::new(crate::WebView::new(webview_inner, effective_options));

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
        effective_options: &EffectiveWebViewCreateOptions,
        mtm: MainThreadMarker,
    ) -> Result<Self, WebViewError> {
        unsafe {
            // Create WKWebViewConfiguration
            let config = WKWebViewConfiguration::new(mtm);

            // Access preferences to set security settings
            let prefs = config.preferences();
            let is_browser_profile = effective_options.profile == SecurityProfile::BrowserRelaxed;
            let is_browser = is_browser_profile && cfg!(target_os = "macos");
            let is_strict = effective_options.profile == SecurityProfile::StrictDefault;
            if is_browser_profile && !is_browser {
                log::warn!(
                    "BrowserRelaxed profile requested on non-macOS Apple target; browser-only features are disabled"
                );
            }
            prefs.setJavaScriptCanOpenWindowsAutomatically(is_browser);

            // Disable local file access for security
            let allow_file_access_key = NSString::from_str("allowFileAccessFromFileURLs");
            let ns_false: *mut AnyObject =
                msg_send![class!(NSNumber), numberWithBool: objc2::runtime::Bool::NO];
            let prefs_obj: *mut AnyObject = Retained::as_ptr(&prefs).cast_mut().cast();
            let _: () = msg_send![prefs_obj, setValue:ns_false, forKey:&*allow_file_access_key];

            // Configure media playback
            config.setMediaTypesRequiringUserActionForPlayback(WKAudiovisualMediaTypes::None);

            // Disable navigation restrictions for better compatibility
            config.setLimitsNavigationsToAppBoundDomains(false);

            // Disable HTTPS upgrade for local development
            config.setUpgradeKnownHostsToHTTPS(false);

            // Profile policy on Apple data store:
            // - StrictDefault: non-persistent (ephemeral DOM storage, cleared on destroy)
            // - BrowserRelaxed: default (persistent localStorage / database)
            if is_strict {
                let non_persistent_store = WKWebsiteDataStore::nonPersistentDataStore(mtm);
                config.setWebsiteDataStore(&non_persistent_store);
            }

            // Register custom scheme handler for lx:// URLs bound to this WebView
            let webtag = WebTag::new(appid, path, session_id);
            if let Some(scheme_handler) =
                super::schemehandler::LingXiaSchemeHandler::new(webtag.clone())
            {
                let lx_scheme = NSString::from_str("lx");
                let proto_scheme_handler: &ProtocolObject<dyn WKURLSchemeHandler> =
                    ProtocolObject::from_ref(&*scheme_handler);
                config.setURLSchemeHandler_forURLScheme(Some(proto_scheme_handler), &lx_scheme);

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

            // Profile policy on Apple:
            // - StrictDefault: block HTTPS subresources
            // - BrowserRelaxed: allow HTTPS subresources (no blocker on fresh config)
            if is_strict {
                Self::enable_https_resource_blocker(&config);
            }

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
            let navigation_delegate = LingXiaNavigationDelegate::new(
                appid.to_string(),
                path.to_string(),
                session_id,
                // Profile policy on Apple:
                // - StrictDefault: intercept https:// top-level navigation
                // - BrowserRelaxed: do not intercept
                is_strict,
                mtm,
            );

            // Set the navigation delegate on the WebView
            let proto_navigation_delegate: &ProtocolObject<dyn WKNavigationDelegate> =
                ProtocolObject::from_ref(&*navigation_delegate);
            let _: () = msg_send![webview, setNavigationDelegate: Some(proto_navigation_delegate)];

            // Set UI delegate for browser mode (handles target="_blank" and window.open)
            let ui_delegate = if is_browser {
                let delegate = LingXiaUIDelegate::new(mtm);
                let proto_ui_delegate: &ProtocolObject<dyn WKUIDelegate> =
                    ProtocolObject::from_ref(&*delegate);
                let _: () = msg_send![webview, setUIDelegate: Some(proto_ui_delegate)];
                Some(delegate)
            } else {
                None
            };

            // Set up message handler for bridge communication
            let message_handler = Self::setup_message_handler(webview, appid, path, session_id)?;

            // Create WebViewInner instance with navigation delegate and message handler
            let webview_inner = WebViewInner {
                webview,
                _navigation_delegate: navigation_delegate,
                _ui_delegate: ui_delegate,
                _message_handler: message_handler,
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
                    log::info!("HTTPS subresource blocking is ON for WebView");
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
        session_id: Option<u64>,
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
            let injection_time: objc2::ffi::NSInteger = 0; // WKUserScriptInjectionTimeAtDocumentStart
            let console_user_script: *mut AnyObject = msg_send![console_user_script,
                initWithSource: &*console_js_string,
                injectionTime: injection_time,
                forMainFrameOnly: false];

            let _: () = msg_send![user_content_controller, addUserScript: console_user_script];

            let message_handler = LingXiaMessageHandler::new(
                appid.to_string(),
                path.to_string(),
                session_id,
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
                let _: *mut AnyObject = msg_send![self.webview, loadRequest: &*request];
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
                let _: *mut AnyObject = msg_send![self.webview, loadHTMLString: &*data_nsstring, baseURL: &*base_url_obj];
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
            let completion =
                StackBlock::new(|_result: *mut AnyObject, _error: *mut NSError| {}).copy();
            // Note: evaluateJavaScript is async, but we're treating it as fire-and-forget
            // In a more complete implementation, we might want to handle the completion
            let _: () = msg_send![
                self.webview,
                evaluateJavaScript: &*js_string,
                completionHandler: Some(&*completion)
            ];
            Ok(())
        }
    }

    /// Helper method to clear browsing data on main thread
    fn clear_browsing_data_on_main_thread(&self) -> Result<(), WebViewError> {
        unsafe {
            let Some(mtm) = MainThreadMarker::new() else {
                return Err(WebViewError::WebView("Not on main thread".to_string()));
            };

            // Get the website data store from the configuration
            let config: *mut AnyObject = msg_send![self.webview, configuration];
            let config_ref = &*(config as *const WKWebViewConfiguration);
            let data_store = config_ref.websiteDataStore();

            let all_types = WKWebsiteDataStore::allWebsiteDataTypes(mtm);
            let distant_past = NSDate::distantPast();
            let completion = StackBlock::new(|| {}).copy();
            data_store.removeDataOfTypes_modifiedSince_completionHandler(
                &all_types,
                &distant_past,
                &completion,
            );

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
                    let _: *mut AnyObject = msg_send![webview_ptr, loadRequest: &*request];
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
                    let _: *mut AnyObject =
                        msg_send![webview_ptr, loadHTMLString: &*data_nsstring, baseURL: &*base_url];
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

            DispatchQueue::main().exec_async(move || unsafe {
                let webview_ptr = webview_ptr_addr as *mut AnyObject;
                let js_nsstring = NSString::from_str(&js_clone);
                let completion =
                    StackBlock::new(|_result: *mut AnyObject, _error: *mut NSError| {}).copy();
                let _: () = msg_send![
                    webview_ptr,
                    evaluateJavaScript: &*js_nsstring,
                    completionHandler: Some(&*completion)
                ];
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

            DispatchQueue::main().exec_async(move || unsafe {
                let webview_ptr = webview_ptr_addr as *mut AnyObject;
                let Some(mtm) = MainThreadMarker::new() else {
                    return;
                };

                let configuration: *mut AnyObject = msg_send![webview_ptr, configuration];
                let config_ref = &*(configuration as *const WKWebViewConfiguration);
                let data_store = config_ref.websiteDataStore();

                let data_types = WKWebsiteDataStore::allWebsiteDataTypes(mtm);
                let distant_past = NSDate::distantPast();
                let completion = StackBlock::new(|| {}).copy();
                data_store.removeDataOfTypes_modifiedSince_completionHandler(
                    &data_types,
                    &distant_past,
                    &completion,
                );
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
}

impl Drop for WebViewInner {
    fn drop(&mut self) {
        // WebView cleanup - only cleanup the actual WebView resources
        // Runtime storage is managed separately
        if MainThreadMarker::new().is_some() {
            // Wrap in ObjC exception handler — WKWebView dealloc can throw
            // ObjC exceptions that Rust's catch_unwind cannot handle.
            let result =
                objc2::exception::catch(std::panic::AssertUnwindSafe(|| self.cleanup_webview()));
            if let Err(exception) = result {
                log::error!(
                    "[WebViewInner] ObjC exception during WebView cleanup ({}): {:?}",
                    self.webtag.as_str(),
                    exception
                );
            }
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

            // Clear scroll view delegate if any (iOS only — macOS WKWebView has no scrollView property)
            #[cfg(target_os = "ios")]
            {
                let scroll_view: *mut AnyObject = msg_send![self.webview, scrollView];
                if !scroll_view.is_null() {
                    let _: () =
                        msg_send![scroll_view, setDelegate: std::ptr::null::<*const AnyObject>()];
                }
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
    session_id: Option<u64>,
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
                    let is_string: objc2::runtime::Bool =
                        msg_send![body, isKindOfClass: objc2::class!(NSString)];
                    if is_string.as_bool() {
                        let ns_string = &*(body as *const NSString);
                        ns_string.to_string()
                    } else {
                        // Try to convert to JSON if it's a dictionary
                        let is_dict: objc2::runtime::Bool =
                            msg_send![body, isKindOfClass: objc2::class!(NSDictionary)];
                        if is_dict.as_bool() {
                            // Convert NSDictionary to JSON string (like Swift version)
                            let body_obj: &AnyObject = &*(body as *const AnyObject);
                            let json_data =
                                match NSJSONSerialization::dataWithJSONObject_options_error(
                                    body_obj,
                                    NSJSONWritingOptions(0),
                                ) {
                                    Ok(data) => data,
                                    Err(err) => {
                                        log::error!(
                                            "Failed to serialize dictionary to JSON: {}",
                                            err.localizedDescription().to_string()
                                        );
                                        return;
                                    }
                                };

                            let json_string = match NSString::initWithData_encoding(
                                NSString::alloc(),
                                &json_data,
                                4, // NSUTF8StringEncoding
                            ) {
                                Some(s) => s,
                                None => {
                                    log::error!("Failed to convert JSON data to string");
                                    return;
                                }
                            };
                            json_string.to_string()
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
    pub fn new(
        appid: String,
        path: String,
        session_id: Option<u64>,
        mtm: MainThreadMarker,
    ) -> Option<Retained<Self>> {
        unsafe {
            let instance = Self::alloc(mtm);
            let instance = instance.set_ivars(LingXiaMessageHandlerIvars {
                appid,
                path,
                session_id,
            });
            let instance: Retained<Self> = msg_send![super(instance), init];
            Some(instance)
        }
    }

    /// Handle bridge messages
    fn handle_bridge_message(&self, message: String) {
        let ivars = self.ivars();

        let webtag = WebTag::new(&ivars.appid, &ivars.path, ivars.session_id);
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

                let webtag = WebTag::new(&ivars.appid, &ivars.path, ivars.session_id);
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
