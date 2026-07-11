//! WebView controller: UI-thread lifecycle, command dispatch,
//! the message loop, and the `WebViewController` implementation.

use super::*;
use async_trait::async_trait;

pub(crate) const WM_LINGXIA_COMMAND: u32 = WM_APP + 0x154;

pub(crate) const WEBVIEW_SCREENSHOT_TIMEOUT: Duration = Duration::from_secs(4);

pub(crate) enum UiCommand {
    LoadUrl {
        url: String,
        resp: Sender<StdResult<()>>,
    },
    LoadHtml {
        html: String,
        base_url: String,
        history_url: Option<String>,
        resp: Sender<StdResult<()>>,
    },
    ExecJs {
        js: String,
        resp: Sender<StdResult<()>>,
    },
    EvalJs {
        js: String,
        resp: Sender<std::result::Result<serde_json::Value, WebViewScriptError>>,
    },
    PostMessage {
        message: String,
        resp: Sender<StdResult<()>>,
    },
    SetUserAgent {
        ua: String,
        resp: Sender<StdResult<()>>,
    },
    ClearBrowsingData {
        resp: Sender<StdResult<()>>,
    },
    ClearProfileData {
        kind: super::data_store::BrowsingDataKind,
        since_unix_ms: Option<u64>,
        resp: Sender<StdResult<()>>,
    },
    CurrentUrl {
        resp: Sender<StdResult<Option<String>>>,
    },
    Reload {
        resp: Sender<StdResult<()>>,
    },
    GoBack {
        resp: Sender<StdResult<()>>,
    },
    GoForward {
        resp: Sender<StdResult<()>>,
    },
    TakeScreenshot {
        resp: Sender<StdResult<Vec<u8>>>,
    },
    ListCookies {
        resp: Sender<StdResult<Vec<WebViewCookie>>>,
    },
    SetCookie {
        request: WebViewCookieSetRequest,
        resp: Sender<StdResult<()>>,
    },
    DeleteCookie {
        name: String,
        domain: String,
        path: String,
        resp: Sender<StdResult<()>>,
    },
    ClearCookies {
        resp: Sender<StdResult<()>>,
    },
    StartNetworkCapture {
        resp: Sender<StdResult<()>>,
    },
    StopNetworkCapture {
        resp: Sender<StdResult<()>>,
    },
    NetworkEntries {
        resp: Sender<StdResult<NetworkCaptureSnapshot>>,
    },
    ClearNetworkCapture {
        resp: Sender<StdResult<()>>,
    },
    /// Invoke a Chrome DevTools Protocol method (e.g. `Input.dispatchMouseEvent`)
    /// and return its raw JSON result.
    #[cfg_attr(not(feature = "webview-input"), allow(dead_code))]
    CallDevToolsProtocol {
        method: String,
        params: String,
        resp: Sender<StdResult<String>>,
    },
    OpenDevTools {
        resp: Sender<StdResult<()>>,
    },
    /// Position the WebView2 controller within the parent HWND supplied by
    /// the Windows UI layer.
    SetContentBounds {
        bounds: RECT,
        resp: Sender<StdResult<()>>,
    },
    /// Show or hide the WebView2 controller without touching the parent HWND.
    SetContentVisible {
        visible: bool,
        resp: Sender<StdResult<()>>,
    },
    /// Rebind the WebView2 controller to a parent HWND owned by the Windows UI
    /// layer.
    SetParentWindow {
        window: isize,
        resp: Sender<StdResult<()>>,
    },
    NotifyParentPositionChanged {
        resp: Sender<StdResult<()>>,
    },
    Shutdown,
}

pub(crate) struct UiState {
    pub(crate) controller: ICoreWebView2Controller,
    pub(crate) webview: ICoreWebView2,
    pub(crate) hwnd: HWND,
    pub(crate) native_view: WindowsWebViewNativeView,
    pub(crate) webtag_key: String,
    pub(crate) memory_pages: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    pub(crate) transient_user_data_dir: Option<PathBuf>,
    /// Captured network entries (shared with the CDP event handlers).
    pub(crate) network_log: Arc<Mutex<network::NetworkLog>>,
    /// Active CDP `Network` event subscriptions (receiver + token), non-empty
    /// while capture is enabled; used to unsubscribe on stop.
    pub(crate) network_receivers: Vec<(ICoreWebView2DevToolsProtocolEventReceiver, i64)>,
    /// CDP `Runtime`/`Log` subscriptions feeding page console/error/browser
    /// logs to the delegate. Held for the webview's lifetime (capture is always
    /// on); dropping them would stop delivery.
    pub(crate) _console_receivers: Vec<(ICoreWebView2DevToolsProtocolEventReceiver, i64)>,
}

impl UiState {
    pub(crate) fn notify_parent_position_changed(&self) {
        unsafe {
            let _ = self.controller.NotifyParentWindowPositionChanged();
        }
    }
}

pub struct WebViewInner {
    command_tx: Sender<UiCommand>,
    thread_id: u32,
    join_handle: Mutex<Option<JoinHandle<()>>>,
    pub(crate) webtag: WebTag,
    pub(crate) native_view: isize,
}

impl std::fmt::Debug for WebViewInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebViewInner")
            .field("thread_id", &self.thread_id)
            .field("webtag", &self.webtag)
            .finish()
    }
}

impl WebViewInner {
    pub(crate) fn create(
        appid: &str,
        path: &str,
        session_id: Option<u64>,
        effective_options: EffectiveWebViewCreateOptions,
        sender: WebViewCreateSender,
    ) {
        let webtag = WebTag::new(appid, path, session_id);
        let webtag_for_thread = webtag.clone();
        let effective_options_for_thread = effective_options.clone();
        let (startup_tx, startup_rx) = mpsc::channel();

        let join_handle = thread::Builder::new()
            .name(format!("lingxia-webview-{}", webtag.as_str()))
            .spawn(move || {
                if let Err(err) =
                    run_ui_thread(webtag_for_thread, effective_options_for_thread, startup_tx)
                {
                    log::error!("Windows WebView UI thread failed: {}", err);
                }
            });

        let join_handle = match join_handle {
            Ok(handle) => handle,
            Err(err) => {
                sender.fail(
                    WebViewCreateStage::Requested,
                    WebViewError::WebView(format!(
                        "Failed to spawn Windows WebView thread: {}",
                        err
                    )),
                );
                return;
            }
        };

        match startup_rx.recv() {
            Ok(Ok((command_tx, thread_id, native_view))) => {
                let webview = Arc::new(crate::WebView::new(
                    WebViewInner {
                        command_tx,
                        thread_id,
                        join_handle: Mutex::new(Some(join_handle)),
                        webtag,
                        native_view,
                    },
                    effective_options,
                ));
                register_webview(webview.clone());
                sender.succeed(webview);
            }
            Ok(Err(err)) => {
                sender.fail(WebViewCreateStage::Requested, err);
                let _ = join_handle.join();
            }
            Err(err) => {
                sender.fail(
                    WebViewCreateStage::Requested,
                    WebViewError::WebView(format!(
                        "Windows WebView startup channel failed: {}",
                        err
                    )),
                );
                let _ = join_handle.join();
            }
        }
    }

    /// Send a command to the UI thread and synchronously wait for its reply.
    ///
    /// All synchronous dispatchers are built on this: it guards against
    /// self-deadlock when called from the UI thread itself, wakes the UI
    /// message loop, and waits for the response (optionally with a timeout).
    pub(super) fn dispatch_ui<T>(
        &self,
        make: impl FnOnce(Sender<T>) -> UiCommand,
        timeout: Option<Duration>,
    ) -> std::result::Result<T, UiDispatchError> {
        if unsafe { Threading::GetCurrentThreadId() } == self.thread_id {
            return Err(UiDispatchError::SameThread);
        }

        let (resp_tx, resp_rx) = mpsc::channel();
        self.command_tx
            .send(make(resp_tx))
            .map_err(|_| UiDispatchError::Unavailable)?;

        self.wake_ui_thread();

        recv_reply_pumping(&resp_rx, timeout).map_err(|err| match err {
            mpsc::RecvTimeoutError::Timeout => UiDispatchError::NoReply(Some(err.to_string())),
            mpsc::RecvTimeoutError::Disconnected => UiDispatchError::NoReply(None),
        })
    }

    fn dispatch_command(
        &self,
        command: impl FnOnce(Sender<StdResult<()>>) -> UiCommand,
    ) -> StdResult<()> {
        self.dispatch_ui(command, None)
            .map_err(|err| err.into_webview_error("run synchronous WebView command"))?
    }

    fn dispatch_layout_command(
        &self,
        command: impl FnOnce(Sender<StdResult<()>>) -> UiCommand,
    ) -> StdResult<()> {
        let (resp_tx, resp_rx) = mpsc::channel();
        self.command_tx
            .send(command(resp_tx))
            .map_err(|_| WebViewError::WebView("WebView UI thread is unavailable".to_string()))?;

        self.wake_ui_thread();

        if unsafe { Threading::GetCurrentThreadId() } == self.thread_id {
            return Ok(());
        }

        recv_reply_pumping(&resp_rx, None)
            .map_err(|_| WebViewError::WebView("WebView UI thread did not reply".to_string()))?
            .map_err(|err| WebViewError::WebView(format!("run WebView layout command: {err}")))
    }

    pub(crate) fn set_content_bounds(&self, bounds: RECT) -> StdResult<()> {
        self.dispatch_layout_command(|resp| UiCommand::SetContentBounds { bounds, resp })
    }

    pub(crate) fn set_content_visible(&self, visible: bool) -> StdResult<()> {
        // Layout dispatch: visibility is toggled from host layout passes, which
        // on a desktop (non-framed) page run on this webview's OWN UI thread —
        // the page's host window lives there. The plain synchronous dispatch
        // rejects same-thread calls (`SameThread`), which left the controller
        // permanently hidden after a same-page navigation: the reconcile's
        // show failed, the visibility registry went stale, and every later
        // layout pass failed the same way — a stuck white page.
        self.dispatch_layout_command(|resp| UiCommand::SetContentVisible { visible, resp })
    }

    pub(crate) fn set_parent_window(&self, window: isize) -> StdResult<()> {
        self.dispatch_layout_command(|resp| UiCommand::SetParentWindow { window, resp })
    }

    pub(crate) fn dispatch_screenshot_command(&self) -> StdResult<Vec<u8>> {
        self.dispatch_ui(
            |resp| UiCommand::TakeScreenshot { resp },
            Some(WEBVIEW_SCREENSHOT_TIMEOUT),
        )
        .map_err(|err| match err {
            UiDispatchError::NoReply(Some(detail)) => {
                WebViewError::WebView(format!("WebView screenshot timed out: {detail}"))
            }
            err => err.into_webview_error("capture WebView screenshot"),
        })?
    }

    pub(crate) fn open_devtools(&self) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::OpenDevTools { resp })
    }

    pub(crate) fn notify_parent_position_changed(&self) -> StdResult<()> {
        self.dispatch_layout_command(|resp| UiCommand::NotifyParentPositionChanged { resp })
    }

    fn wake_ui_thread(&self) {
        let posted = unsafe {
            WindowsAndMessaging::PostMessageW(
                Some(hwnd_from_handle(self.native_view)),
                WM_LINGXIA_COMMAND,
                WPARAM::default(),
                LPARAM::default(),
            )
            .is_ok()
        };
        if !posted {
            unsafe {
                let _ = WindowsAndMessaging::PostThreadMessageW(
                    self.thread_id,
                    WM_LINGXIA_COMMAND,
                    WPARAM::default(),
                    LPARAM::default(),
                );
            }
        }
    }

    pub(super) fn dispatch_eval_command(
        &self,
        js: String,
    ) -> std::result::Result<serde_json::Value, WebViewScriptError> {
        self.dispatch_ui(|resp| UiCommand::EvalJs { js, resp }, None)
            .map_err(|err| match err {
                UiDispatchError::SameThread => WebViewScriptError::Platform(
                    "Cannot evaluate JavaScript from WebView UI thread".to_string(),
                ),
                UiDispatchError::Unavailable | UiDispatchError::NoReply(_) => {
                    WebViewScriptError::Destroyed
                }
            })?
    }

    fn dispatch_current_url(&self) -> StdResult<Option<String>> {
        self.dispatch_ui(|resp| UiCommand::CurrentUrl { resp }, None)
            .map_err(|err| err.into_webview_error("read current WebView URL"))?
    }

    fn dispatch_list_cookies(&self) -> StdResult<Vec<WebViewCookie>> {
        self.dispatch_ui(|resp| UiCommand::ListCookies { resp }, None)
            .map_err(|err| err.into_webview_error("list WebView cookies"))?
    }

    fn dispatch_network_entries(&self) -> StdResult<NetworkCaptureSnapshot> {
        self.dispatch_ui(|resp| UiCommand::NetworkEntries { resp }, None)
            .map_err(|err| err.into_webview_error("read network capture"))?
    }

    /// Invoke a Chrome DevTools Protocol method and return its raw JSON
    /// result. Backs the input automation dispatch.
    pub(super) fn dispatch_cdp_command(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> StdResult<String> {
        let (method, params) = (method.to_string(), params.to_string());
        self.dispatch_ui(
            |resp| UiCommand::CallDevToolsProtocol {
                method,
                params,
                resp,
            },
            Some(Duration::from_secs(4)),
        )
        .map_err(|err| err.into_webview_error("run CDP command"))?
    }
}

/// Failure modes of [`WebViewInner::dispatch_ui`].
#[derive(Debug)]
pub(crate) enum UiDispatchError {
    /// Called from the UI thread itself; blocking would self-deadlock.
    SameThread,
    /// The UI thread command channel is closed.
    Unavailable,
    /// The UI thread never replied (`Some` carries the timeout error detail).
    NoReply(Option<String>),
}

impl UiDispatchError {
    fn into_webview_error(self, action: &str) -> WebViewError {
        WebViewError::WebView(match self {
            UiDispatchError::SameThread => {
                format!("Cannot {action} from WebView UI thread")
            }
            UiDispatchError::Unavailable => "WebView UI thread is unavailable".to_string(),
            UiDispatchError::NoReply(detail) => detail
                .map(|detail| format!("WebView UI thread did not reply: {detail}"))
                .unwrap_or_else(|| "WebView UI thread did not reply".to_string()),
        })
    }
}

#[async_trait]
impl WebViewController for WebViewInner {
    fn load_url(&self, url: &str) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::LoadUrl {
            url: url.to_string(),
            resp,
        })
    }

    fn load_data(&self, request: LoadDataRequest<'_>) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::LoadHtml {
            html: request.data.to_string(),
            base_url: request.base_url.to_string(),
            history_url: request.history_url.map(str::to_string),
            resp,
        })
    }

    fn exec_js(&self, js: &str) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::ExecJs {
            js: js.to_string(),
            resp,
        })
    }

    async fn eval_js(
        &self,
        js: &str,
    ) -> std::result::Result<serde_json::Value, WebViewScriptError> {
        self.dispatch_eval_command(js.to_string())
    }

    fn post_message(&self, message: &str) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::PostMessage {
            message: message.to_string(),
            resp,
        })
    }

    fn clear_browsing_data(&self) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::ClearBrowsingData { resp })
    }

    fn set_user_agent(&self, ua: &str) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::SetUserAgent {
            ua: ua.to_string(),
            resp,
        })
    }

    async fn current_url(&self) -> StdResult<Option<String>> {
        self.dispatch_current_url()
    }

    fn reload(&self) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::Reload { resp })
    }

    fn go_back(&self) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::GoBack { resp })
    }

    fn go_forward(&self) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::GoForward { resp })
    }

    async fn take_screenshot(&self) -> StdResult<Vec<u8>> {
        self.dispatch_screenshot_command()
    }

    async fn list_cookies(&self) -> StdResult<Vec<WebViewCookie>> {
        self.dispatch_list_cookies()
    }

    async fn set_cookie(&self, request: WebViewCookieSetRequest) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::SetCookie { request, resp })
    }

    async fn delete_cookie(&self, name: &str, domain: &str, path: &str) -> StdResult<()> {
        let (name, domain, path) = (name.to_string(), domain.to_string(), path.to_string());
        self.dispatch_command(|resp| UiCommand::DeleteCookie {
            name,
            domain,
            path,
            resp,
        })
    }

    async fn clear_cookies(&self) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::ClearCookies { resp })
    }

    async fn clear_site_data(
        &self,
        url: &str,
        options: ClearSiteDataOptions,
    ) -> StdResult<ClearSiteDataResult> {
        let uri = url
            .parse::<http::Uri>()
            .map_err(|_| WebViewError::WebView("site data URL is invalid".to_string()))?;
        let host = uri
            .host()
            .filter(|host| !host.is_empty())
            .ok_or_else(|| WebViewError::WebView("site data URL has no host".to_string()))?
            .trim_start_matches('.')
            .to_ascii_lowercase();
        if !matches!(uri.scheme_str(), Some("http" | "https")) {
            return Err(WebViewError::WebView(
                "site data URL must use HTTP or HTTPS".to_string(),
            ));
        }
        let current = self.current_url().await?.ok_or_else(|| {
            WebViewError::WebView("current WebView has no website URL".to_string())
        })?;
        let current_host = current
            .parse::<http::Uri>()
            .ok()
            .and_then(|uri| uri.host().map(str::to_ascii_lowercase));
        if current_host.as_deref() != Some(host.as_str()) {
            return Err(WebViewError::WebView(
                "current website changed before its data could be cleared".to_string(),
            ));
        }

        if options.site_data {
            for cookie in self.list_cookies().await? {
                let domain = cookie.domain.trim_start_matches('.').to_ascii_lowercase();
                let matches = if cookie.host_only {
                    domain == host
                } else {
                    host == domain
                        || host
                            .strip_suffix(&domain)
                            .is_some_and(|prefix| prefix.ends_with('.'))
                };
                if matches {
                    self.delete_cookie(&cookie.name, &cookie.domain, &cookie.path)
                        .await?;
                }
            }
        }

        let mut storage_types = Vec::new();
        if options.site_data {
            storage_types.extend([
                "file_systems",
                "indexeddb",
                "local_storage",
                "websql",
                "service_workers",
            ]);
        }
        if options.cache {
            storage_types.extend(["appcache", "cache_storage"]);
        }
        if !storage_types.is_empty() {
            let scheme = uri.scheme_str().unwrap_or("https");
            let authority = uri.authority().ok_or_else(|| {
                WebViewError::WebView("site data URL has no authority".to_string())
            })?;
            self.dispatch_cdp_command(
                "Storage.clearDataForOrigin",
                serde_json::json!({
                    "origin": format!("{scheme}://{authority}"),
                    "storageTypes": storage_types.join(","),
                }),
            )?;
        }

        Ok(ClearSiteDataResult {
            // WebView2 only exposes HTTP-cache clearing at profile scope. Cache
            // Storage is cleared above, but shared network cache is preserved.
            cache_cleared: false,
            site_data_cleared: options.site_data,
        })
    }

    async fn start_network_capture(&self) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::StartNetworkCapture { resp })
    }

    async fn stop_network_capture(&self) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::StopNetworkCapture { resp })
    }

    async fn network_entries(&self) -> StdResult<NetworkCaptureSnapshot> {
        self.dispatch_network_entries()
    }

    async fn clear_network_capture(&self) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::ClearNetworkCapture { resp })
    }
}

impl WebViewInner {
    pub(crate) fn clear_profile_data(
        &self,
        kind: super::data_store::BrowsingDataKind,
        since_unix_ms: Option<u64>,
    ) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::ClearProfileData {
            kind,
            since_unix_ms,
            resp,
        })
    }
}

impl Drop for WebViewInner {
    fn drop(&mut self) {
        let _ = self.command_tx.send(UiCommand::Shutdown);
        self.wake_ui_thread();
        // Never block on the UI thread's exit: the dropping thread may itself
        // own windows (another webview's UI thread running a layout callback),
        // and the dying thread's WebView2 teardown can message those windows
        // synchronously — a join would deadlock against that send. The thread
        // exits on its own once it processes Shutdown.
        if let Ok(mut guard) = self.join_handle.lock() {
            drop(guard.take());
        }
    }
}

pub(crate) fn run_ui_thread(
    webtag: WebTag,
    effective_options: EffectiveWebViewCreateOptions,
    startup_tx: Sender<StdResult<(Sender<UiCommand>, u32, isize)>>,
) -> StdResult<()> {
    unsafe {
        windows::Win32::System::Com::CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|err| WebViewError::WebView(format!("CoInitializeEx failed: {err}")))?;
    }

    let result = run_ui_thread_inner(webtag, effective_options, startup_tx);

    unsafe {
        windows::Win32::System::Com::CoUninitialize();
    }

    result
}

/// The pieces produced while bringing up a WebView2 controller, returned from
/// the fallible setup closure so a single error path can tear the window down.
type WebViewControllerSetup = (
    ICoreWebView2Controller,
    ICoreWebView2,
    Arc<Mutex<HashMap<String, Vec<u8>>>>,
    Option<PathBuf>,
);

pub(crate) fn run_ui_thread_inner(
    webtag: WebTag,
    effective_options: EffectiveWebViewCreateOptions,
    startup_tx: Sender<StdResult<(Sender<UiCommand>, u32, isize)>>,
) -> StdResult<()> {
    ensure_message_queue();

    let native_view = create_webview_parent(&webtag)?;
    let hwnd = hwnd_from_handle(native_view.window);
    let webtag_key = webtag.key().to_string();

    // After the window exists, every failure must report the real error to the
    // creator and destroy the window (which also frees the WindowUserData box).
    let setup = (|| -> StdResult<WebViewControllerSetup> {
        let (env, transient_user_data_dir) = create_environment(&webtag, &effective_options)?;
        let controller = create_controller(&env, hwnd)?;
        let webview = unsafe {
            controller
                .CoreWebView2()
                .map_err(|err| WebViewError::WebView(format!("CoreWebView2 failed: {err}")))?
        };

        let bounds = webview_parent_bounds(native_view)?;
        unsafe {
            controller
                .SetBounds(bounds)
                .map_err(|err| WebViewError::WebView(format!("SetBounds failed: {err}")))?;
        }
        configure_controller(&controller)?;
        configure_settings(&webview, &effective_options)?;
        let menu_appid = webtag.extract_appid();
        let menu_path = webtag.extract_parts().1;
        configure_context_menu(&webview, &env, &menu_appid, &menu_path, &effective_options)?;
        // lxapp pages (non-relaxed) get the runtime-owned selection/copy baseline;
        // browser tabs render arbitrary external pages and must not be restyled.
        let inject_platform_baseline = effective_options.profile != SecurityProfile::BrowserRelaxed;
        install_document_scripts(&webview, inject_platform_baseline)?;
        let memory_pages = Arc::new(Mutex::new(HashMap::new()));
        register_event_handlers(
            &env,
            &webview,
            webtag.clone(),
            &effective_options.registered_schemes,
            memory_pages.clone(),
        )?;
        Ok((controller, webview, memory_pages, transient_user_data_dir))
    })();

    let (controller, webview, memory_pages, transient_user_data_dir) = match setup {
        Ok(parts) => parts,
        Err(err) => {
            let _ = startup_tx.send(Err(err.clone()));
            destroy_webview_parent(webtag.key(), native_view);
            return Err(err);
        }
    };

    let (command_tx, command_rx) = mpsc::channel();
    if startup_tx
        .send(Ok((
            command_tx,
            unsafe { Threading::GetCurrentThreadId() },
            native_view.window,
        )))
        .is_err()
    {
        unsafe {
            let _ = controller.Close();
        }
        destroy_webview_parent(webtag.key(), native_view);
        return Err(WebViewError::WebView(
            "Failed to publish WebView startup".to_string(),
        ));
    }

    // Page-log capture over CDP (console calls, uncaught exceptions,
    // browser-level messages) is wired before the message loop pumps any
    // command, so it is live before the first navigation. Best-effort: a
    // subscribe failure must not fail webview creation.
    let console_receivers = match console::subscribe(&webview, &webtag) {
        Ok(receivers) => {
            console::enable(&webview);
            receivers
        }
        Err(err) => {
            log::warn!("console log capture unavailable: {err}");
            Vec::new()
        }
    };

    let mut state = UiState {
        controller,
        webview,
        hwnd,
        native_view,
        webtag_key,
        memory_pages,
        transient_user_data_dir,
        network_log: Arc::new(Mutex::new(network::NetworkLog::default())),
        network_receivers: Vec::new(),
        _console_receivers: console_receivers,
    };

    message_loop(&mut state, command_rx)
}

/// Waits for a dispatched command's reply while delivering incoming
/// cross-thread sent messages (via `PeekMessageW(PM_NOREMOVE)`). A caller on
/// a window-owning thread (e.g. the host window's UI thread) would otherwise
/// deadlock: the WebView UI thread serving the command can itself block in a
/// synchronous send back to one of the caller's windows.
fn recv_reply_pumping<T>(
    resp_rx: &Receiver<T>,
    timeout: Option<Duration>,
) -> std::result::Result<T, mpsc::RecvTimeoutError> {
    const PUMP_SLICE: Duration = Duration::from_millis(10);
    let deadline = timeout.map(|timeout| std::time::Instant::now() + timeout);
    loop {
        let slice = match deadline {
            Some(deadline) => {
                let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) else {
                    return Err(mpsc::RecvTimeoutError::Timeout);
                };
                PUMP_SLICE.min(left)
            }
            None => PUMP_SLICE,
        };
        match resp_rx.recv_timeout(slice) {
            Err(mpsc::RecvTimeoutError::Timeout) => unsafe {
                let mut msg = MSG::default();
                let _ = WindowsAndMessaging::PeekMessageW(
                    &mut msg,
                    None,
                    0,
                    0,
                    WindowsAndMessaging::PM_NOREMOVE,
                );
            },
            reply => return reply,
        }
    }
}

pub(crate) fn ensure_message_queue() {
    let mut msg = MSG::default();
    unsafe {
        let _ = WindowsAndMessaging::PeekMessageW(
            &mut msg,
            None,
            0,
            0,
            WindowsAndMessaging::PM_NOREMOVE,
        );
    }
}

pub(crate) fn message_loop(state: &mut UiState, command_rx: Receiver<UiCommand>) -> StdResult<()> {
    let mut msg = MSG::default();

    loop {
        while let Ok(command) = command_rx.try_recv() {
            if handle_command(state, command)? {
                cleanup_state(state);
                return Ok(());
            }
        }

        let status = unsafe { WindowsAndMessaging::GetMessageW(&mut msg, None, 0, 0).0 };
        match status {
            -1 => {
                cleanup_state(state);
                return Err(WebViewError::WebView(
                    "GetMessageW failed in WebView loop".to_string(),
                ));
            }
            0 => {
                cleanup_state(state);
                return Ok(());
            }
            _ => {
                // Window messages still need normal dispatch; only the
                // command wake is consumed by this loop.
                if msg.message != WM_LINGXIA_COMMAND {
                    unsafe {
                        let _ = WindowsAndMessaging::TranslateMessage(&msg);
                        WindowsAndMessaging::DispatchMessageW(&msg);
                    }
                }
            }
        }
    }
}

pub(crate) fn handle_command(state: &mut UiState, command: UiCommand) -> StdResult<bool> {
    match command {
        UiCommand::LoadUrl { url, resp } => {
            clear_memory_pages(&state.memory_pages);
            let result = unsafe {
                let url = CoTaskMemPWSTR::from(url.as_str());
                state
                    .webview
                    .Navigate(*url.as_ref().as_pcwstr())
                    .map_err(|err| WebViewError::WebView(format!("Navigate failed: {err}")))
            };
            let _ = resp.send(result);
        }
        UiCommand::LoadHtml {
            html,
            base_url,
            history_url,
            resp,
        } => {
            let navigation_url = history_url.unwrap_or_else(|| base_url.clone());
            clear_memory_pages(&state.memory_pages);
            store_memory_page(
                &state.memory_pages,
                &navigation_url,
                prepare_navigation_html(&html, &base_url, &navigation_url),
            );
            if navigation_url != base_url {
                store_memory_page(&state.memory_pages, &base_url, html.into_bytes());
            }
            let result = unsafe {
                let url = CoTaskMemPWSTR::from(navigation_url.as_str());
                state
                    .webview
                    .Navigate(*url.as_ref().as_pcwstr())
                    .map_err(|err| WebViewError::WebView(format!("Navigate failed: {err}")))
            };
            let _ = resp.send(result);
        }
        UiCommand::ExecJs { js, resp } => {
            start_execute_script(&state.webview, &js, resp, |result| {
                result
                    .map(|_| ())
                    .map_err(|err| WebViewError::WebView(format!("ExecuteScript failed: {err}")))
            });
        }
        UiCommand::EvalJs { js, resp } => {
            start_execute_script(&state.webview, &js, resp, |result| {
                result.and_then(|json| decode_script_result(&json))
            });
        }
        UiCommand::PostMessage { message, resp } => {
            let result = unsafe {
                let message = CoTaskMemPWSTR::from(message.as_str());
                state
                    .webview
                    .PostWebMessageAsString(*message.as_ref().as_pcwstr())
                    .map_err(|err| {
                        WebViewError::WebView(format!("PostWebMessageAsString failed: {err}"))
                    })
            };
            let _ = resp.send(result);
        }
        UiCommand::SetUserAgent { ua, resp } => {
            let result = set_user_agent(&state.webview, &ua);
            let _ = resp.send(result);
        }
        UiCommand::ClearBrowsingData { resp } => {
            if let Err(err) = begin_clear_browsing_data(&state.webview, resp.clone()) {
                let _ = resp.send(Err(err));
            }
        }
        UiCommand::ClearProfileData {
            kind,
            since_unix_ms,
            resp,
        } => {
            if let Err(err) =
                begin_clear_profile_data(&state.webview, kind, since_unix_ms, resp.clone())
            {
                let _ = resp.send(Err(err));
            }
        }
        UiCommand::CurrentUrl { resp } => {
            let result = current_url(&state.webview);
            let _ = resp.send(result);
        }
        UiCommand::Reload { resp } => {
            let result = unsafe {
                state
                    .webview
                    .Reload()
                    .map_err(|err| WebViewError::WebView(format!("Reload failed: {err}")))
            };
            let _ = resp.send(result);
        }
        UiCommand::GoBack { resp } => {
            let result = go_history(&state.webview, HistoryDirection::Back);
            let _ = resp.send(result);
        }
        UiCommand::GoForward { resp } => {
            let result = go_history(&state.webview, HistoryDirection::Forward);
            let _ = resp.send(result);
        }
        UiCommand::TakeScreenshot { resp } => {
            start_capture_preview_png(&state.webview, resp);
        }
        UiCommand::ListCookies { resp } => {
            start_list_cookies(&state.webview, resp);
        }
        UiCommand::SetCookie { request, resp } => {
            let result = set_cookie(&state.webview, &request);
            let _ = resp.send(result);
        }
        UiCommand::DeleteCookie {
            name,
            domain,
            path,
            resp,
        } => {
            let result = delete_cookie(&state.webview, &name, &domain, &path);
            let _ = resp.send(result);
        }
        UiCommand::ClearCookies { resp } => {
            let result = clear_cookies(&state.webview);
            let _ = resp.send(result);
        }
        UiCommand::StartNetworkCapture { resp } => {
            if !state.network_receivers.is_empty() {
                let _ = resp.send(Ok(())); // already capturing
            } else {
                match network::subscribe(&state.webview, &state.network_log) {
                    Ok(receivers) => {
                        state.network_receivers = receivers;
                        // Reply only once Network.enable has taken effect, so
                        // the caller can navigate immediately without missing
                        // the first requests.
                        network::enable_domain(&state.webview, resp);
                    }
                    Err(err) => {
                        let _ = resp.send(Err(err));
                    }
                }
            }
        }
        UiCommand::StopNetworkCapture { resp } => {
            network::stop_capture(&state.webview, &mut state.network_receivers);
            let _ = resp.send(Ok(()));
        }
        UiCommand::NetworkEntries { resp } => {
            let snapshot = state
                .network_log
                .lock()
                .map(|log| log.snapshot())
                .unwrap_or_default();
            let _ = resp.send(Ok(snapshot));
        }
        UiCommand::ClearNetworkCapture { resp } => {
            if let Ok(mut log) = state.network_log.lock() {
                log.clear();
            }
            let _ = resp.send(Ok(()));
        }
        UiCommand::CallDevToolsProtocol {
            method,
            params,
            resp,
        } => {
            start_call_devtools_protocol(&state.webview, &method, &params, resp);
        }
        UiCommand::OpenDevTools { resp } => {
            let result = unsafe {
                state.webview.OpenDevToolsWindow().map_err(|err| {
                    WebViewError::WebView(format!("OpenDevToolsWindow failed: {err}"))
                })
            };
            let _ = resp.send(result);
        }
        UiCommand::SetContentBounds { bounds, resp } => {
            let result = unsafe {
                state
                    .controller
                    .SetBounds(bounds)
                    .map_err(|err| WebViewError::WebView(format!("SetBounds failed: {err}")))
            };
            let _ = resp.send(result);
        }
        UiCommand::SetContentVisible { visible, resp } => {
            let result = set_controller_visible(state, visible);
            let _ = resp.send(result);
        }
        UiCommand::SetParentWindow { window, resp } => {
            let hwnd = hwnd_from_handle(window);
            // Re-parenting to the current parent still tears down and
            // re-attaches the composition target, blanking the content for a
            // frame - layout passes re-assert the parent on every sync, so
            // short-circuit the no-op.
            if state.hwnd == hwnd {
                let _ = resp.send(Ok(()));
                return Ok(false);
            }
            let result = unsafe {
                state
                    .controller
                    .SetParentWindow(hwnd)
                    .map_err(|err| WebViewError::WebView(format!("SetParentWindow failed: {err}")))
            };
            if result.is_ok() {
                state.hwnd = hwnd;
            }
            let _ = resp.send(result);
        }
        UiCommand::NotifyParentPositionChanged { resp } => {
            state.notify_parent_position_changed();
            let _ = resp.send(Ok(()));
        }
        UiCommand::Shutdown => return Ok(true),
    }

    Ok(false)
}

pub(crate) fn cleanup_state(state: &mut UiState) {
    unsafe {
        let _ = state.controller.Close();
    }
    destroy_webview_parent(&state.webtag_key, state.native_view);
    if let Some(dir) = state.transient_user_data_dir.take()
        && let Err(err) = std::fs::remove_dir_all(&dir)
    {
        log::debug!("failed to remove strict WebView2 profile {dir:?}: {err}");
    }
}

pub(crate) fn set_controller_visible(state: &UiState, visible: bool) -> StdResult<()> {
    unsafe {
        state
            .controller
            .SetIsVisible(visible)
            .map_err(|err| WebViewError::WebView(format!("SetIsVisible failed: {err}")))
    }
}
