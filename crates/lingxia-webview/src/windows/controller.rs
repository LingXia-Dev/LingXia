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
    SetUserAgentOverride {
        user_agent: UserAgentOverride,
        resp: Sender<StdResult<()>>,
    },
    SetBrowserEmulationProfile {
        profile: WindowsBrowserEmulationProfile,
        resp: Sender<StdResult<String>>,
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
    /// Position the controller and set the per-corner rounding (radii +
    /// wedge backdrop color) in one composition commit, so bounds and
    /// corners never present out of sync. Windowed hosting applies the
    /// bounds and ignores the corner style.
    SetContentGeometry {
        bounds: RECT,
        radii: [i32; 4],
        corner_color: u32,
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
    /// Rendering scale for a fit-scaled presentation (the device-frame
    /// runner shrunk to the work area): CSS px = physical px / scale, so the
    /// page keeps the simulated device's logical viewport.
    SetRasterizationScale {
        scale: f64,
        resp: Sender<StdResult<()>>,
    },
    Shutdown,
}

pub(crate) struct UiState {
    pub(crate) controller: ICoreWebView2Controller,
    pub(crate) webview: ICoreWebView2,
    pub(crate) hosting: HostingMode,
    pub(crate) hwnd: HWND,
    pub(crate) native_view: WindowsWebViewNativeView,
    pub(crate) webtag_key: String,
    pub(crate) memory_pages: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    pub(crate) ephemeral_user_data_dir: Option<PathBuf>,
    /// Captured network entries (shared with the CDP event handlers).
    pub(crate) network_log: Arc<Mutex<network::NetworkLog>>,
    /// Active CDP `Network` event subscriptions (receiver + token), non-empty
    /// while capture is enabled; used to unsubscribe on stop.
    pub(crate) network_receivers: Vec<(ICoreWebView2DevToolsProtocolEventReceiver, i64)>,
    /// CDP `Runtime`/`Log` subscriptions feeding page console/error/browser
    /// logs to the delegate. Held for the webview's lifetime (capture is always
    /// on); dropping them would stop delivery.
    pub(crate) _console_receivers: Vec<(ICoreWebView2DevToolsProtocolEventReceiver, i64)>,
    /// Engine-supplied UA captured before any host override.
    pub(crate) default_user_agent: String,
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
    pub(crate) composition_hosted: bool,
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
            Ok(Ok((command_tx, thread_id, native_view, composition_hosted))) => {
                let webview = Arc::new(crate::WebView::new(
                    WebViewInner {
                        command_tx,
                        thread_id,
                        join_handle: Mutex::new(Some(join_handle)),
                        webtag,
                        native_view,
                        composition_hosted,
                    },
                    effective_options,
                ));
                if sender.is_destroyed() {
                    log::info!(
                        "Windows WebView for {} was destroyed during creation; discarding",
                        webview.webtag().key()
                    );
                    return;
                }
                register_webview(webview.clone());
                // Destruction can race the registry insertion. Re-check after
                // registration so the destroyed generation never remains as a
                // zombie that blocks a later same-tag reactivation.
                if sender.is_destroyed() {
                    log::info!(
                        "Windows WebView for {} was destroyed during registration; discarding",
                        webview.webtag().key()
                    );
                    crate::webview::destroy_webview(&webview.webtag());
                    return;
                }
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

    /// Queue a fire-and-forget command and wake the UI thread; when already
    /// on that thread, skip the reply wait (waiting would deadlock — the
    /// thread cannot pump the queue while blocked). Errors on the same-thread
    /// path surface only in the UI thread's own logging.
    fn dispatch_command_same_thread_safe(
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
            .map_err(|err| WebViewError::WebView(format!("run WebView command: {err}")))
    }

    pub(crate) fn set_content_bounds(&self, bounds: RECT) -> StdResult<()> {
        self.dispatch_command_same_thread_safe(|resp| UiCommand::SetContentBounds { bounds, resp })
    }

    pub(crate) fn set_browser_emulation_profile(
        &self,
        profile: WindowsBrowserEmulationProfile,
    ) -> StdResult<()> {
        self.dispatch_ui(
            |resp| UiCommand::SetBrowserEmulationProfile { profile, resp },
            None,
        )
        .map_err(|err| err.into_webview_error("set browser emulation profile"))??;
        Ok(())
    }

    pub(crate) fn set_content_geometry(
        &self,
        bounds: RECT,
        radii: [i32; 4],
        corner_color: u32,
    ) -> StdResult<()> {
        self.dispatch_command_same_thread_safe(|resp| UiCommand::SetContentGeometry {
            bounds,
            radii,
            corner_color,
            resp,
        })
    }

    pub(crate) fn set_content_visible(&self, visible: bool) -> StdResult<()> {
        // Layout dispatch: visibility is toggled from host layout passes, which
        // on a desktop (non-framed) page run on this webview's OWN UI thread —
        // the page's host window lives there. The plain synchronous dispatch
        // rejects same-thread calls (`SameThread`), which left the controller
        // permanently hidden after a same-page navigation: the reconcile's
        // show failed, the visibility registry went stale, and every later
        // layout pass failed the same way — a stuck white page.
        self.dispatch_command_same_thread_safe(|resp| UiCommand::SetContentVisible {
            visible,
            resp,
        })
    }

    pub(crate) fn set_parent_window(&self, window: isize) -> StdResult<()> {
        self.dispatch_command_same_thread_safe(|resp| UiCommand::SetParentWindow { window, resp })
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
        self.dispatch_command_same_thread_safe(|resp| UiCommand::NotifyParentPositionChanged {
            resp,
        })
    }

    pub(crate) fn set_rasterization_scale(&self, scale: f64) -> StdResult<()> {
        self.dispatch_command_same_thread_safe(|resp| UiCommand::SetRasterizationScale {
            scale,
            resp,
        })
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
        self.dispatch_command_same_thread_safe(|resp| UiCommand::LoadHtml {
            html: request.data.to_string(),
            base_url: request.base_url.to_string(),
            history_url: request.history_url.map(str::to_string),
            resp,
        })
    }

    fn exec_js(&self, js: &str) -> StdResult<()> {
        // Fire-and-forget script injection is called from delegate events,
        // which are delivered on this webview's own UI thread — queue without
        // waiting there so callers stay platform-innocent.
        self.dispatch_command_same_thread_safe(|resp| UiCommand::ExecJs {
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

    fn set_user_agent_override(&self, user_agent: UserAgentOverride) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::SetUserAgentOverride { user_agent, resp })
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

/// Published once the UI thread is up: the command channel, the thread id,
/// the native-view HWND handle, and whether composition hosting engaged.
pub(crate) type WebViewStartup = (Sender<UiCommand>, u32, isize, bool);

pub(crate) fn run_ui_thread(
    webtag: WebTag,
    effective_options: EffectiveWebViewCreateOptions,
    startup_tx: Sender<StdResult<WebViewStartup>>,
) -> StdResult<()> {
    unsafe {
        // OleInitialize = CoInitializeEx(STA) + OLE (clipboard/drag-drop);
        // composition hosting registers the surface window as a drop target.
        windows::Win32::System::Ole::OleInitialize(None)
            .map_err(|err| WebViewError::WebView(format!("OleInitialize failed: {err}")))?;
    }

    let result = run_ui_thread_inner(webtag, effective_options, startup_tx);

    unsafe {
        windows::Win32::System::Ole::OleUninitialize();
    }

    result
}

/// The pieces produced while bringing up a WebView2 controller, returned from
/// the fallible setup closure so a single error path can tear the window down.
type WebViewControllerSetup = (
    ICoreWebView2Controller,
    HostingMode,
    ICoreWebView2,
    Arc<Mutex<HashMap<String, Vec<u8>>>>,
    EphemeralProfileGuard,
);

struct EphemeralProfileGuard(Option<PathBuf>);

impl EphemeralProfileGuard {
    fn take(&mut self) -> Option<PathBuf> {
        self.0.take()
    }
}

impl Drop for EphemeralProfileGuard {
    fn drop(&mut self) {
        if let Some(dir) = self.0.take() {
            schedule_ephemeral_profile_cleanup(dir);
        }
    }
}

pub(crate) fn run_ui_thread_inner(
    webtag: WebTag,
    effective_options: EffectiveWebViewCreateOptions,
    startup_tx: Sender<StdResult<WebViewStartup>>,
) -> StdResult<()> {
    ensure_message_queue();

    let native_view = create_webview_parent(&webtag)?;
    let hwnd = hwnd_from_handle(native_view.window);
    let webtag_key = webtag.key().to_string();

    // After the window exists, every failure must report the real error to the
    // creator and destroy the window (which also frees the WindowUserData box).
    let setup = (|| -> StdResult<WebViewControllerSetup> {
        let (env, ephemeral_user_data_dir) = create_environment(&webtag, &effective_options)?;
        let ephemeral_profile = EphemeralProfileGuard(ephemeral_user_data_dir);
        let (controller, hosting) = create_hosting_controller(&env, hwnd)?;
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
        Ok((
            controller,
            hosting,
            webview,
            memory_pages,
            ephemeral_profile,
        ))
    })();

    let (controller, hosting, webview, memory_pages, mut ephemeral_profile) = match setup {
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
            matches!(hosting, HostingMode::Composition(_)),
        )))
        .is_err()
    {
        unsafe {
            let _ = controller.Close();
        }
        if let HostingMode::Composition(surface) = &hosting {
            surface.destroy();
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

    let default_user_agent = user_agent(&webview)?;
    let mut state = UiState {
        controller,
        webview,
        hosting,
        hwnd,
        native_view,
        webtag_key,
        memory_pages,
        ephemeral_user_data_dir: ephemeral_profile.take(),
        network_log: Arc::new(Mutex::new(network::NetworkLog::default())),
        network_receivers: Vec::new(),
        _console_receivers: console_receivers,
        default_user_agent,
    };

    if let Some(profile) = browser_emulation::configured_profile() {
        let (profile_tx, _profile_rx) = mpsc::channel();
        browser_emulation::apply_profile(
            &state.webview,
            &state.default_user_agent,
            profile,
            profile_tx,
        );
    }

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
        UiCommand::SetUserAgentOverride { user_agent, resp } => {
            let user_agent = match user_agent {
                UserAgentOverride::Default => state.default_user_agent.as_str(),
                UserAgentOverride::Custom(ref user_agent) => user_agent.as_str(),
            };
            let result = set_user_agent_override(&state.webview, user_agent);
            let _ = resp.send(result);
        }
        UiCommand::SetBrowserEmulationProfile { profile, resp } => {
            browser_emulation::apply_profile(
                &state.webview,
                &state.default_user_agent,
                profile,
                resp,
            );
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
            let result = set_content_geometry(state, bounds, None);
            let _ = resp.send(result);
        }
        UiCommand::SetContentGeometry {
            bounds,
            radii,
            corner_color,
            resp,
        } => {
            let result = set_content_geometry(state, bounds, Some((radii, corner_color)));
            let _ = resp.send(result);
        }
        UiCommand::SetContentVisible { visible, resp } => {
            let result = set_controller_visible(state, visible);
            let _ = resp.send(result);
        }
        UiCommand::SetParentWindow { window, resp } => {
            let hwnd = hwnd_from_handle(window);
            // A windowed controller can skip a same-parent request. A
            // composition surface cannot: destroying its former host also
            // destroys the child surface, and Windows may reuse that host's
            // HWND for the replacement. `set_parent` detects and rebuilds
            // that dead surface even when the numeric parent is unchanged.
            let composition_hosted = matches!(&state.hosting, HostingMode::Composition(_));
            if !should_update_parent(composition_hosted, state.hwnd == hwnd) {
                let _ = resp.send(Ok(()));
                return Ok(false);
            }
            let result = match &mut state.hosting {
                HostingMode::Windowed => unsafe {
                    state.controller.SetParentWindow(hwnd).map_err(|err| {
                        WebViewError::WebView(format!("SetParentWindow failed: {err}"))
                    })
                },
                // The surface window moves hosts; WebView2's own parent stays
                // the surface window, so its composition target survives.
                HostingMode::Composition(surface) => surface.set_parent(&state.controller, hwnd),
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
        UiCommand::SetRasterizationScale { scale, resp } => {
            let result = state
                .controller
                .cast::<ICoreWebView2Controller3>()
                .map_err(|err| WebViewError::WebView(format!("Controller3 cast failed: {err}")))
                .and_then(|controller3| unsafe {
                    controller3
                        .SetRasterizationScale(scale.max(0.1))
                        .map_err(|err| {
                            WebViewError::WebView(format!("SetRasterizationScale failed: {err}"))
                        })
                });
            let _ = resp.send(result);
        }
        UiCommand::Shutdown => return Ok(true),
    }

    Ok(false)
}

fn should_update_parent(composition_hosted: bool, same_parent: bool) -> bool {
    composition_hosted || !same_parent
}

pub(crate) fn cleanup_state(state: &mut UiState) {
    unsafe {
        let _ = state.controller.Close();
    }
    if let HostingMode::Composition(surface) = &state.hosting {
        surface.destroy();
    }
    destroy_webview_parent(&state.webtag_key, state.native_view);
    if let Some(dir) = state.ephemeral_user_data_dir.take() {
        schedule_ephemeral_profile_cleanup(dir);
    }
}

fn schedule_ephemeral_profile_cleanup(dir: PathBuf) {
    let _ = std::thread::Builder::new()
        .name("lingxia-webview-profile-cleanup".to_string())
        .spawn(move || {
            // WebView2 releases files asynchronously after Controller.Close.
            // Retry off the UI thread so an ephemeral profile is not left on
            // disk merely because the first removal raced COM teardown.
            for attempt in 0..20 {
                if !dir.exists() || std::fs::remove_dir_all(&dir).is_ok() {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(50 + attempt * 25));
            }
            log::warn!("failed to remove ephemeral WebView2 profile {dir:?}");
        });
}

pub(crate) fn set_controller_visible(state: &mut UiState, visible: bool) -> StdResult<()> {
    match &mut state.hosting {
        HostingMode::Windowed => unsafe {
            state
                .controller
                .SetIsVisible(visible)
                .map_err(|err| WebViewError::WebView(format!("SetIsVisible failed: {err}")))
        },
        HostingMode::Composition(surface) => surface.set_visible(&state.controller, visible),
    }
}

/// Applies content bounds (and, on the composition path, the per-corner
/// rounding; `None` keeps the last applied style). Windowed hosting positions
/// the controller directly and has no corners to update.
fn set_content_geometry(
    state: &mut UiState,
    bounds: RECT,
    corners: Option<([i32; 4], u32)>,
) -> StdResult<()> {
    match &mut state.hosting {
        HostingMode::Windowed => unsafe {
            state
                .controller
                .SetBounds(bounds)
                .map_err(|err| WebViewError::WebView(format!("SetBounds failed: {err}")))
        },
        HostingMode::Composition(surface) => {
            surface.set_geometry(&state.controller, bounds, corners)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::should_update_parent;

    #[test]
    fn composition_rechecks_a_reused_parent_handle() {
        assert!(should_update_parent(true, true));
        assert!(!should_update_parent(false, true));
        assert!(should_update_parent(false, false));
    }
}
