//! WebView controller: UI-thread lifecycle, command dispatch,
//! the message loop, and the `WebViewController` implementation.

use super::*;
use async_trait::async_trait;

pub(crate) const WM_LINGXIA_COMMAND: u32 = WM_APP + 0x154;

pub(crate) const WM_LINGXIA_LAYOUT: u32 = WM_APP + 0x155;

pub(crate) const WEBVIEW_SCREENSHOT_TIMEOUT: Duration = Duration::from_secs(4);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WindowsWindowRole {
    Main,
    Panel { panel_id: String },
}

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
    WindowSnapshot {
        resp: Sender<StdResult<WindowsWebViewWindowSnapshot>>,
    },
    ShowWindow {
        title: String,
        activate: bool,
        role: WindowsWindowRole,
        resp: Sender<StdResult<()>>,
    },
    PresentAsGroupMain {
        group_key: String,
        resp: Sender<StdResult<()>>,
    },
    HideWindow {
        resp: Sender<StdResult<()>>,
    },
    SetWindowLayout {
        layout: WindowsWindowLayout,
        resp: Sender<StdResult<()>>,
    },
    Shutdown,
}

pub(crate) struct UiState {
    pub(crate) controller: ICoreWebView2Controller,
    pub(crate) webview: ICoreWebView2,
    pub(crate) hwnd: HWND,
    pub(crate) webtag_key: String,
    pub(crate) window_visible: bool,
    pub(crate) memory_pages: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

pub struct WebViewInner {
    command_tx: Sender<UiCommand>,
    thread_id: u32,
    join_handle: Mutex<Option<JoinHandle<()>>>,
    pub(crate) webtag: WebTag,
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
            Ok(Ok((command_tx, thread_id))) => {
                let webview = Arc::new(crate::WebView::new(
                    WebViewInner {
                        command_tx,
                        thread_id,
                        join_handle: Mutex::new(Some(join_handle)),
                        webtag,
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
    fn dispatch_ui<T>(
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

        unsafe {
            let _ = WindowsAndMessaging::PostThreadMessageW(
                self.thread_id,
                WM_LINGXIA_COMMAND,
                WPARAM::default(),
                LPARAM::default(),
            );
        }

        match timeout {
            Some(timeout) => resp_rx
                .recv_timeout(timeout)
                .map_err(|err| UiDispatchError::NoReply(Some(err.to_string()))),
            None => resp_rx.recv().map_err(|_| UiDispatchError::NoReply(None)),
        }
    }

    fn dispatch_command(
        &self,
        command: impl FnOnce(Sender<StdResult<()>>) -> UiCommand,
    ) -> StdResult<()> {
        self.dispatch_ui(command, None)
            .map_err(|err| err.into_webview_error("run synchronous WebView command"))?
    }

    pub(crate) fn show_window(
        &self,
        title: String,
        activate: bool,
        role: WindowsWindowRole,
    ) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::ShowWindow {
            title,
            activate,
            role,
            resp,
        })
    }

    pub(crate) fn hide_window(&self) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::HideWindow { resp })
    }

    pub(crate) fn present_as_group_main(&self, group_key: String) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::PresentAsGroupMain { group_key, resp })
    }

    pub(crate) fn set_window_layout(&self, layout: WindowsWindowLayout) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::SetWindowLayout { layout, resp })
    }

    fn dispatch_screenshot_command(&self) -> StdResult<Vec<u8>> {
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

    pub(crate) fn window_snapshot(&self) -> StdResult<WindowsWebViewWindowSnapshot> {
        self.dispatch_ui(|resp| UiCommand::WindowSnapshot { resp }, None)
            .map_err(|err| err.into_webview_error("inspect WebView window"))?
    }

    fn dispatch_eval_command(
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
}

/// Failure modes of [`WebViewInner::dispatch_ui`].
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
}

impl Drop for WebViewInner {
    fn drop(&mut self) {
        remove_close_handler(self.webtag.key());
        remove_chrome_event_handler(self.webtag.key());
        remove_window_layout(self.webtag.key());

        let _ = self.command_tx.send(UiCommand::Shutdown);
        unsafe {
            let _ = WindowsAndMessaging::PostThreadMessageW(
                self.thread_id,
                WM_LINGXIA_COMMAND,
                WPARAM::default(),
                LPARAM::default(),
            );
        }

        if unsafe { Threading::GetCurrentThreadId() } == self.thread_id {
            // The last Arc<WebView> was dropped by a callback running on the
            // UI thread itself; joining here would self-deadlock, so detach.
            log::debug!(
                "Dropping WebViewInner for {} on its own UI thread; detaching instead of joining",
                self.webtag.key()
            );
            return;
        }

        if let Ok(mut guard) = self.join_handle.lock()
            && let Some(handle) = guard.take()
        {
            let _ = handle.join();
        }
    }
}

pub(crate) fn run_ui_thread(
    webtag: WebTag,
    effective_options: EffectiveWebViewCreateOptions,
    startup_tx: Sender<StdResult<(Sender<UiCommand>, u32)>>,
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

pub(crate) fn run_ui_thread_inner(
    webtag: WebTag,
    effective_options: EffectiveWebViewCreateOptions,
    startup_tx: Sender<StdResult<(Sender<UiCommand>, u32)>>,
) -> StdResult<()> {
    ensure_message_queue();

    let hwnd = create_hidden_window(&webtag)?;
    let webtag_key = webtag.key().to_string();

    // After the window exists, every failure must report the real error to the
    // creator and destroy the window (which also frees the WindowUserData box).
    let setup = (|| -> StdResult<(ICoreWebView2Controller, ICoreWebView2, Arc<Mutex<HashMap<String, Vec<u8>>>>)> {
        let env = create_environment(&effective_options)?;
        let controller = create_controller(&env, hwnd)?;
        let webview = unsafe {
            controller
                .CoreWebView2()
                .map_err(|err| WebViewError::WebView(format!("CoreWebView2 failed: {err}")))?
        };

        configure_controller(&controller)?;
        configure_settings(&webview)?;
        install_document_scripts(&webview)?;
        let memory_pages = Arc::new(Mutex::new(HashMap::new()));
        register_event_handlers(
            &env,
            &webview,
            webtag.clone(),
            &effective_options.registered_schemes,
            memory_pages.clone(),
        )?;
        Ok((controller, webview, memory_pages))
    })();

    let (controller, webview, memory_pages) = match setup {
        Ok(parts) => parts,
        Err(err) => {
            let _ = startup_tx.send(Err(err.clone()));
            unsafe {
                let _ = WindowsAndMessaging::DestroyWindow(hwnd);
            }
            return Err(err);
        }
    };

    let (command_tx, command_rx) = mpsc::channel();
    if startup_tx
        .send(Ok((command_tx, unsafe { Threading::GetCurrentThreadId() })))
        .is_err()
    {
        unsafe {
            let _ = controller.Close();
            let _ = WindowsAndMessaging::DestroyWindow(hwnd);
        }
        return Err(WebViewError::WebView(
            "Failed to publish WebView startup".to_string(),
        ));
    }

    let mut state = UiState {
        controller,
        webview,
        hwnd,
        webtag_key,
        window_visible: false,
        memory_pages,
    };

    // Let the window procedure drive layout directly (required during the
    // modal move/size loop, where this command loop is starved).
    register_live_layout_context(&state);

    message_loop(&mut state, command_rx)
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
                // WM_LINGXIA_LAYOUT is handled by the window procedure (so
                // it also works inside modal move/size loops); dispatch it
                // like any other window message.
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
            let result = clear_browsing_data(&state.webview);
            let _ = resp.send(result);
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
        UiCommand::WindowSnapshot { resp } => {
            let result = window_snapshot(state);
            let _ = resp.send(result);
        }
        UiCommand::ShowWindow {
            title,
            activate,
            role,
            resp,
        } => {
            let result = show_native_window(state, &title, activate, role);
            let _ = resp.send(result);
        }
        UiCommand::PresentAsGroupMain { group_key, resp } => {
            let result = present_native_window_as_group_main(state, &group_key);
            let _ = resp.send(result);
        }
        UiCommand::HideWindow { resp } => {
            let result = hide_native_window(state);
            let _ = resp.send(result);
        }
        UiCommand::SetWindowLayout { layout, resp } => {
            let result = set_native_window_layout(state, layout);
            let _ = resp.send(result);
        }
        UiCommand::Shutdown => return Ok(true),
    }

    Ok(false)
}

pub(crate) fn cleanup_state(state: &mut UiState) {
    clear_live_layout_context();
    cleanup_window_state(state);
    unsafe {
        let _ = state.controller.Close();
        let _ = WindowsAndMessaging::DestroyWindow(state.hwnd);
    }
}

pub(crate) fn cleanup_window_state(state: &UiState) {
    let attachment = remove_window_attachment(&state.webtag_key);
    remove_window_handle(&state.webtag_key);
    remove_window_layout(&state.webtag_key);
    remove_close_handler(&state.webtag_key);
    remove_chrome_event_handler(&state.webtag_key);

    if let Some(attachment) = attachment {
        match attachment.kind {
            WindowAttachmentKind::MainHost => {
                if let Some(hosts) = WINDOW_GROUP_HOSTS.get()
                    && let Ok(mut hosts) = hosts.lock()
                    && hosts.get(&attachment.group_key).copied() == Some(hwnd_handle(state.hwnd))
                {
                    hosts.remove(&attachment.group_key);
                }
                if let Some(active) = WINDOW_GROUP_ACTIVE_MAIN.get()
                    && let Ok(mut active) = active.lock()
                {
                    active.remove(&attachment.group_key);
                }
                if let Some(active_group) = WINDOW_ACTIVE_GROUP.get()
                    && let Ok(mut active_group) = active_group.lock()
                    && active_group.as_deref() == Some(attachment.group_key.as_str())
                {
                    *active_group = None;
                }
                remove_group_layout(&attachment.group_key);
            }
            WindowAttachmentKind::MainChild => {
                // A presented main surface that goes away restores the main
                // webview it displaced.
                if let Some(presented) =
                    take_presented_main_if(&attachment.group_key, &state.webtag_key)
                {
                    if let Some(previous) = presented.previous_main_key {
                        set_group_active_main(&attachment.group_key, &previous);
                    }
                    layout_group_windows(&attachment.group_key);
                    request_group_shell_refresh(&attachment.group_key);
                }
                if let Some(active) = WINDOW_GROUP_ACTIVE_MAIN.get()
                    && let Ok(mut active) = active.lock()
                    && active
                        .get(&attachment.group_key)
                        .is_some_and(|key| key == &state.webtag_key)
                {
                    active.remove(&attachment.group_key);
                }
            }
            WindowAttachmentKind::Panel { .. } => {
                remove_group_panel(&attachment.group_key, &state.webtag_key);
                layout_group_windows(&attachment.group_key);
                request_group_shell_refresh(&attachment.group_key);
            }
        }
    }
}
