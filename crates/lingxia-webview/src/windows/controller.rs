//! WebView controller: UI-thread lifecycle, command dispatch,
//! the message loop, and the `WebViewController` implementation.

use super::ui_thread;
use super::*;
use crate::events::normalizer;
use async_trait::async_trait;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

pub(crate) const WEBVIEW_SCREENSHOT_TIMEOUT: Duration = Duration::from_secs(4);
const BROWSER_EMULATION_TIMEOUT: Duration = Duration::from_secs(4);
const EVAL_TIMEOUT: Duration = Duration::from_secs(10);
/// Bounds any other synchronous UI command. A healthy WebView UI thread
/// answers in milliseconds; waiting forever turns one wedged controller into
/// a permanently blocked caller (page teardown holds a transition guard while
/// parking the document, so the whole navigation pipeline stalls behind it).
const UI_COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
const WEBVIEW_UI_RETIRE_TIMEOUT: Duration = Duration::from_secs(2);

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
    LoadTrustedHtml {
        html: String,
        base_url: String,
        history_url: Option<String>,
        intent: crate::TrustedLoadIntent,
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
    PostMessageToDocument {
        expected_generation: crate::DocumentGeneration,
        gate: Arc<dyn crate::DocumentOutboundGate>,
        message: String,
        resp: Sender<StdResult<()>>,
    },
    SetUserAgentOverride {
        user_agent: UserAgentOverride,
        resp: Sender<StdResult<()>>,
    },
    SetPreferredColorScheme {
        scheme: WindowsPreferredColorScheme,
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
    MoveFocus {
        resp: Sender<StdResult<()>>,
    },
    /// Keep an overlapping surface above sibling WebView controllers hosted
    /// by the same parent window.
    BringContentToFront {
        resp: Sender<StdResult<()>>,
    },
    /// Rebind the WebView2 controller to a parent HWND owned by the Windows UI
    /// layer.
    SetParentWindow {
        window: isize,
        resp: Sender<StdResult<()>>,
    },
    /// Move the controller out of `expected` into `window` if it is still
    /// parented there; used while `expected` retires.
    ParkFromParent {
        expected: isize,
        window: isize,
        resp: Sender<StdResult<ParkOutcome>>,
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
    /// Starts retiring the parent HWND; commands keep running until foreign
    /// controllers have left it and the controller is closed.
    Shutdown,
}

#[derive(Default)]
pub(crate) struct PublishedParent {
    current: isize,
    pending: Option<isize>,
}

fn publish_parent_window(parent: &Mutex<PublishedParent>, current: isize) {
    *parent.lock().unwrap_or_else(|err| err.into_inner()) = PublishedParent {
        current,
        pending: None,
    };
}

pub(crate) struct UiState {
    pub(crate) controller: ICoreWebView2Controller,
    pub(crate) webview: ICoreWebView2,
    pub(crate) hosting: HostingMode,
    pub(crate) hwnd: HWND,
    parent_window: Arc<Mutex<PublishedParent>>,
    pub(crate) native_view: WindowsWebViewNativeView,
    pub(crate) native_view_id: NativeWebViewId,
    pub(crate) webtag: WebTag,
    pub(crate) webtag_key: String,
    pub(crate) memory_pages: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    pub(crate) document_authority: Arc<document::WindowsDocumentAuthority>,
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
    /// Whether the host configured coherent UA + Client Hint emulation before
    /// this WebView was created.
    pub(crate) browser_emulation_configured: bool,
}

impl UiState {
    pub(crate) fn notify_parent_position_changed(&self) {
        unsafe {
            let _ = self.controller.NotifyParentWindowPositionChanged();
        }
    }
}

type CommandQueue = Arc<Mutex<VecDeque<UiCommand>>>;

pub struct WebViewInner {
    id: u64,
    commands: CommandQueue,
    pub(crate) webtag: WebTag,
    pub(crate) native_view: isize,
    parent_window: Arc<Mutex<PublishedParent>>,
    pub(crate) composition_hosted: bool,
}

impl std::fmt::Debug for WebViewInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebViewInner")
            .field("id", &self.id)
            .field("webtag", &self.webtag)
            .finish()
    }
}

/// A controller's UI-thread state. `state` is borrowed only while one of its
/// commands runs; a command for it issued further up that stack stays queued
/// until the running one returns.
struct Slot {
    state: RefCell<UiState>,
    commands: CommandQueue,
    retiring: Cell<bool>,
    closed: Cell<bool>,
}

static NEXT_WEBVIEW_ID: AtomicU64 = AtomicU64::new(1);

thread_local! {
    static SLOTS: RefCell<HashMap<u64, Rc<Slot>>> = RefCell::new(HashMap::new());
    /// Webtag keys whose previous controller is still retiring.
    static RETIRING_TAGS: RefCell<HashMap<String, usize>> = RefCell::new(HashMap::new());
}

fn slot(id: u64) -> Option<Rc<Slot>> {
    SLOTS.with(|slots| slots.borrow().get(&id).cloned())
}

fn slot_busy(id: u64) -> bool {
    slot(id).is_some_and(|slot| slot.state.try_borrow_mut().is_err())
}

fn tag_retiring(key: &str) -> bool {
    RETIRING_TAGS.with(|tags| tags.borrow().contains_key(key))
}

/// Runs queued commands for `id` in order. Must be called on the UI thread.
fn drain(id: u64, commands: &CommandQueue) {
    let Some(slot) = slot(id) else {
        // Dropping the commands drops their reply senders: waiters see the
        // controller as gone.
        commands
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clear();
        return;
    };
    loop {
        if slot.closed.get() {
            commands
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .clear();
            return;
        }
        let Ok(mut state) = slot.state.try_borrow_mut() else {
            return;
        };
        let Some(command) = commands
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .pop_front()
        else {
            return;
        };
        match command {
            UiCommand::Shutdown => {
                drop(state);
                begin_retirement(id, &slot);
            }
            command => {
                if let Err(err) = handle_command(&mut state, command) {
                    log::error!("WebView command failed for {}: {err}", state.webtag_key);
                }
            }
        }
    }
}

/// Upper bound on waiting for foreign controllers to leave a retiring parent.
/// Past it the parent is destroyed anyway: a stuck surface beats a leaked
/// controller that never closes.
const PARENT_RETIREMENT_DEADLINE: Duration = Duration::from_secs(10);
const PARENT_RETIREMENT_POLL: Duration = Duration::from_millis(100);

fn begin_retirement(id: u64, slot: &Slot) {
    if slot.retiring.replace(true) {
        return;
    }
    let key = slot.state.borrow().webtag_key.clone();
    RETIRING_TAGS.with(|tags| *tags.borrow_mut().entry(key).or_default() += 1);
    let deadline = Instant::now() + PARENT_RETIREMENT_DEADLINE;
    ui_thread::post(Box::new(move || retire(id, deadline)));
}

fn retire(id: u64, deadline: Instant) {
    let Some(slot) = slot(id) else {
        return;
    };
    let Ok(view) = slot.state.try_borrow().map(|state| state.native_view) else {
        ui_thread::post_after(PARENT_RETIREMENT_POLL, move || retire(id, deadline));
        return;
    };
    let ready = match prepare_destroy_webview_parent(view) {
        Ok(ParentRetirement::Ready) => true,
        Ok(ParentRetirement::Deferred) => false,
        Err(err) => {
            log::warn!("Windows parent retirement could not park controllers: {err}");
            false
        }
    };
    if !ready {
        if Instant::now() < deadline {
            ui_thread::post_after(PARENT_RETIREMENT_POLL, move || retire(id, deadline));
            return;
        }
        log::warn!("Windows parent retirement timed out; destroying the parent anyway");
    }
    let Ok(mut state) = slot.state.try_borrow_mut() else {
        ui_thread::post_after(PARENT_RETIREMENT_POLL, move || retire(id, deadline));
        return;
    };
    slot.closed.set(true);
    slot.commands
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .clear();
    let key = state.webtag_key.clone();
    SLOTS.with(|slots| slots.borrow_mut().remove(&id));
    struct RetiringTagGuard(String);
    impl Drop for RetiringTagGuard {
        fn drop(&mut self) {
            RETIRING_TAGS.with(|tags| {
                let mut tags = tags.borrow_mut();
                if let Some(count) = tags.get_mut(&self.0) {
                    *count -= 1;
                    if *count == 0 {
                        tags.remove(&self.0);
                    }
                }
            });
        }
    }
    let _retiring_guard = RetiringTagGuard(key);
    cleanup_state(&mut state);
    drop(state);
}

/// `_done` is dropped once creation has finished or failed; callers wait on it.
fn create_on_ui_thread(
    webtag: WebTag,
    effective_options: EffectiveWebViewCreateOptions,
    sender: WebViewCreateSender,
    requested: Instant,
    _done: Sender<()>,
) {
    if sender.is_destroyed() {
        return;
    }
    // A relaunch can reuse a WebTag while the retired controller still closes.
    // Prefer disjoint native lifetimes because overlapping controllers can
    // wedge the replacement, but a stuck retirement must not block every
    // later generation, so the wait is bounded.
    if tag_retiring(webtag.key()) {
        if requested.elapsed() < WEBVIEW_UI_RETIRE_TIMEOUT {
            ui_thread::post_after(Duration::from_millis(20), move || {
                create_on_ui_thread(webtag, effective_options, sender, requested, _done)
            });
            return;
        }
        log::warn!(
            "Windows WebView teardown for {} exceeded {:?}; starting the replacement generation",
            webtag.key(),
            WEBVIEW_UI_RETIRE_TIMEOUT
        );
    }

    let native_view_id = sender.native_view_id();
    let parent_window = Arc::new(Mutex::new(PublishedParent::default()));
    let state = match build_ui_state(
        webtag.clone(),
        effective_options.clone(),
        native_view_id,
        parent_window.clone(),
    ) {
        Ok(state) => state,
        Err(err) => {
            log::error!("Windows WebView creation failed: {err}");
            sender.fail(WebViewCreateStage::Requested, err);
            return;
        }
    };
    let id = NEXT_WEBVIEW_ID.fetch_add(1, Ordering::Relaxed);
    let native_view = state.native_view.window;
    let composition_hosted = matches!(state.hosting, HostingMode::Composition(_));
    let commands = CommandQueue::default();
    SLOTS.with(|slots| {
        slots.borrow_mut().insert(
            id,
            Rc::new(Slot {
                state: RefCell::new(state),
                commands: commands.clone(),
                retiring: Cell::new(false),
                closed: Cell::new(false),
            }),
        )
    });
    let webview = Arc::new(crate::WebView::new(
        WebViewInner {
            id,
            commands,
            webtag,
            native_view,
            parent_window,
            composition_hosted,
        },
        effective_options,
        native_view_id,
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
        crate::webview::destroy_webview_if_matches(&webview.webtag(), &webview);
        return;
    }
    sender.succeed(webview);
}

impl WebViewInner {
    pub(crate) fn has_parent_window(&self, window: isize) -> bool {
        let parent = self
            .parent_window
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        parent.current == window || parent.pending == Some(window)
    }

    pub(crate) fn create(
        appid: &str,
        path: &str,
        session_id: Option<u64>,
        effective_options: EffectiveWebViewCreateOptions,
        sender: WebViewCreateSender,
    ) {
        let webtag = WebTag::new(appid, path, session_id);
        let requested = Instant::now();
        let (done, finished) = mpsc::channel();
        if ui_thread::is_ui_thread() {
            create_on_ui_thread(webtag, effective_options, sender, requested, done);
            // If the previous generation is still retiring, creation deferred
            // onto a timer; pump until it finishes so callers see the WebView
            // registered when `create` returns.
            let _ = ui_thread::pump_until(&finished, None);
            return;
        }
        ui_thread::post(Box::new(move || {
            create_on_ui_thread(webtag, effective_options, sender, requested, done)
        }));
        // Callers rely on the WebView being registered when this returns.
        let _ = recv_reply_pumping(&finished, None);
    }

    /// Queues `command` behind this controller's earlier commands. On the UI
    /// thread it runs before returning unless one of this controller's
    /// commands is already running further up the stack.
    fn send(&self, command: UiCommand) {
        self.commands
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .push_back(command);
        if self.id == 0 {
            return;
        }
        if ui_thread::is_ui_thread() {
            drain(self.id, &self.commands);
        } else {
            let id = self.id;
            let commands = self.commands.clone();
            ui_thread::post(Box::new(move || drain(id, &commands)));
        }
    }

    /// Send a command to the UI thread and wait for its reply.
    ///
    /// On the UI thread the wait runs a nested message loop, so WebView2
    /// completions and other controllers' work keep flowing. Only a command
    /// for a controller whose own command is running further up the stack is
    /// refused: its reply could never arrive.
    pub(super) fn dispatch_ui<T>(
        &self,
        make: impl FnOnce(Sender<T>) -> UiCommand,
        timeout: Option<Duration>,
    ) -> std::result::Result<T, UiDispatchError> {
        let on_ui_thread = ui_thread::is_ui_thread();
        if on_ui_thread && slot_busy(self.id) {
            return Err(UiDispatchError::SameThread);
        }
        let (resp_tx, resp_rx) = mpsc::channel();
        self.send(make(resp_tx));
        let reply = if on_ui_thread {
            ui_thread::pump_until(&resp_rx, timeout)
        } else {
            recv_reply_pumping(&resp_rx, timeout)
        };
        reply.map_err(|err| match err {
            mpsc::RecvTimeoutError::Timeout => UiDispatchError::NoReply(Some(err.to_string())),
            mpsc::RecvTimeoutError::Disconnected => UiDispatchError::NoReply(None),
        })
    }

    fn dispatch_command(
        &self,
        command: impl FnOnce(Sender<StdResult<()>>) -> UiCommand,
    ) -> StdResult<()> {
        self.dispatch_ui(command, Some(UI_COMMAND_TIMEOUT))
            .map_err(|err| err.into_webview_error("run synchronous WebView command"))?
    }

    /// Runs a command whose completion the caller need not await. On the UI
    /// thread it returns the immediate result, or `Ok` when the command was
    /// deferred or completes asynchronously.
    fn dispatch_command_same_thread_safe(
        &self,
        command: impl FnOnce(Sender<StdResult<()>>) -> UiCommand,
    ) -> StdResult<()> {
        self.dispatch_command_same_thread_safe_with_timeout(command, Some(UI_COMMAND_TIMEOUT))
    }

    fn dispatch_command_same_thread_safe_with_timeout(
        &self,
        command: impl FnOnce(Sender<StdResult<()>>) -> UiCommand,
        timeout: Option<Duration>,
    ) -> StdResult<()> {
        let (resp_tx, resp_rx) = mpsc::channel();
        self.send(command(resp_tx));

        if ui_thread::is_ui_thread() {
            return match resp_rx.try_recv() {
                Ok(result) => result
                    .map_err(|err| WebViewError::WebView(format!("run WebView command: {err}"))),
                Err(_) => Ok(()),
            };
        }

        recv_reply_pumping(&resp_rx, timeout)
            .map_err(|err| match err {
                mpsc::RecvTimeoutError::Timeout => {
                    WebViewError::WebView("WebView command timed out".to_string())
                }
                mpsc::RecvTimeoutError::Disconnected => {
                    WebViewError::WebView("WebView UI thread did not reply".to_string())
                }
            })?
            .map_err(|err| WebViewError::WebView(format!("run WebView command: {err}")))
    }

    pub(crate) fn set_content_bounds(&self, bounds: RECT) -> StdResult<()> {
        self.dispatch_command_same_thread_safe(|resp| UiCommand::SetContentBounds { bounds, resp })
    }

    pub(crate) fn set_preferred_color_scheme(
        &self,
        scheme: WindowsPreferredColorScheme,
    ) -> StdResult<()> {
        // Same-thread-safe: the host color-mode handler can run on the window
        // thread after a product appearance change. Blocking `dispatch_ui`
        // would self-deadlock there and freeze the shell.
        self.dispatch_command_same_thread_safe(|resp| UiCommand::SetPreferredColorScheme {
            scheme,
            resp,
        })
    }

    /// Queue a scheme change and return without the UI thread's reply.
    ///
    /// A host appearance change walks every live WebView from the settings
    /// call. Waiting out [`UI_COMMAND_TIMEOUT`] on a hidden controller makes
    /// that call miss the bridge timeout after the preference is already saved.
    pub(crate) fn post_preferred_color_scheme(&self, scheme: WindowsPreferredColorScheme) {
        let (resp_tx, _resp_rx) = mpsc::channel();
        self.send(UiCommand::SetPreferredColorScheme {
            scheme,
            resp: resp_tx,
        });
    }

    pub(crate) fn set_browser_emulation_profile(
        &self,
        profile: WindowsBrowserEmulationProfile,
    ) -> StdResult<()> {
        self.dispatch_ui(
            |resp| UiCommand::SetBrowserEmulationProfile { profile, resp },
            Some(BROWSER_EMULATION_TIMEOUT),
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
        // Host layout passes toggle visibility on the UI thread, sometimes
        // while this controller's own command is running; a refusing dispatch
        // there left the page permanently hidden (a stuck white page).
        self.dispatch_command_same_thread_safe(|resp| UiCommand::SetContentVisible {
            visible,
            resp,
        })
    }

    pub(crate) fn bring_content_to_front(&self) -> StdResult<()> {
        self.dispatch_command_same_thread_safe(|resp| UiCommand::BringContentToFront { resp })
    }

    /// Give the WebView keyboard focus (`MoveFocus`, programmatic), so the
    /// page receives key events without a click into it.
    pub(crate) fn move_focus(&self) -> StdResult<()> {
        self.dispatch_command_same_thread_safe(|resp| UiCommand::MoveFocus { resp })
    }

    pub(crate) fn set_parent_window(&self, window: isize) -> StdResult<()> {
        self.dispatch_command_same_thread_safe(|resp| UiCommand::SetParentWindow { window, resp })
    }

    pub(crate) fn park_from_parent(
        &self,
        expected: isize,
        window: isize,
    ) -> StdResult<ParkOutcome> {
        // A controller already closed has nothing left to protect, and
        // retirement must not wait on it.
        if ui_thread::is_ui_thread() && slot_busy(self.id) {
            return Ok(ParkOutcome::Deferred);
        }
        let (resp, reply) = mpsc::channel();
        self.send(UiCommand::ParkFromParent {
            expected,
            window,
            resp,
        });
        let reply = if ui_thread::is_ui_thread() {
            reply.try_recv().map_err(|err| match err {
                mpsc::TryRecvError::Empty => mpsc::RecvTimeoutError::Timeout,
                mpsc::TryRecvError::Disconnected => mpsc::RecvTimeoutError::Disconnected,
            })
        } else {
            recv_reply_pumping(&reply, Some(UI_COMMAND_TIMEOUT))
        };
        match reply {
            Ok(outcome) => outcome,
            Err(mpsc::RecvTimeoutError::Disconnected) => Ok(ParkOutcome::NotInParent),
            Err(err) => Err(WebViewError::WebView(format!(
                "Park WebView command: {err}"
            ))),
        }
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

    pub(super) fn dispatch_eval_command(
        &self,
        js: String,
    ) -> std::result::Result<serde_json::Value, WebViewScriptError> {
        self.dispatch_ui(|resp| UiCommand::EvalJs { js, resp }, Some(EVAL_TIMEOUT))
            .map_err(map_eval_dispatch_error)?
    }

    fn dispatch_current_url(&self) -> StdResult<Option<String>> {
        self.dispatch_ui(
            |resp| UiCommand::CurrentUrl { resp },
            Some(UI_COMMAND_TIMEOUT),
        )
        .map_err(|err| err.into_webview_error("read current WebView URL"))?
    }

    fn dispatch_list_cookies(&self) -> StdResult<Vec<WebViewCookie>> {
        self.dispatch_ui(
            |resp| UiCommand::ListCookies { resp },
            Some(UI_COMMAND_TIMEOUT),
        )
        .map_err(|err| err.into_webview_error("list WebView cookies"))?
    }

    fn dispatch_network_entries(&self) -> StdResult<NetworkCaptureSnapshot> {
        self.dispatch_ui(
            |resp| UiCommand::NetworkEntries { resp },
            Some(UI_COMMAND_TIMEOUT),
        )
        .map_err(|err| err.into_webview_error("read network capture"))?
    }

    /// Arm the host-issued token on this WebView's STA immediately before the
    /// native navigation call. `NavigationStarting` consumes and attests it.
    pub(crate) fn load_trusted_data(
        &self,
        intent: crate::TrustedLoadIntent,
        request: LoadDataRequest<'_>,
    ) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::LoadTrustedHtml {
            html: request.data.to_string(),
            base_url: request.base_url.to_string(),
            history_url: request.history_url.map(str::to_string),
            intent,
            resp,
        })
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
    /// The UI thread never replied (`Some` carries the timeout error detail).
    NoReply(Option<String>),
}

impl UiDispatchError {
    fn into_webview_error(self, action: &str) -> WebViewError {
        WebViewError::WebView(match self {
            UiDispatchError::SameThread => {
                format!("Cannot {action} from WebView UI thread")
            }
            UiDispatchError::NoReply(detail) => detail
                .map(|detail| format!("WebView UI thread did not reply: {detail}"))
                .unwrap_or_else(|| "WebView UI thread did not reply".to_string()),
        })
    }
}

fn map_eval_dispatch_error(error: UiDispatchError) -> WebViewScriptError {
    match error {
        UiDispatchError::SameThread => WebViewScriptError::Platform(
            "Cannot evaluate JavaScript from WebView UI thread".to_string(),
        ),
        UiDispatchError::NoReply(Some(_)) => WebViewScriptError::Timeout,
        UiDispatchError::NoReply(None) => WebViewScriptError::Destroyed,
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

    fn post_message_to_document(
        &self,
        expected_generation: crate::DocumentGeneration,
        gate: Arc<dyn crate::DocumentOutboundGate>,
        message: &str,
    ) -> StdResult<()> {
        // Ingress holds the document-generation lock while its delegate may
        // emit a response. Queueing is the completion boundary here: the UI
        // thread rechecks both credentials immediately before native delivery.
        let (resp, _ignored) = mpsc::channel();
        self.send(UiCommand::PostMessageToDocument {
            expected_generation,
            gate,
            message: message.to_string(),
            resp,
        });
        Ok(())
    }

    fn exec_js(&self, js: &str) -> StdResult<()> {
        // Fire-and-forget: delegate events call this on the UI thread, which
        // must not wait for the script to complete.
        self.dispatch_command_same_thread_safe_with_timeout(
            |resp| UiCommand::ExecJs {
                js: js.to_string(),
                resp,
            },
            Some(EVAL_TIMEOUT),
        )
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

    async fn take_screenshot_region(&self, region: crate::ScreenshotRegion) -> StdResult<Vec<u8>> {
        use base64::Engine as _;
        // CDP clips are in document coordinates; beyond-viewport capture
        // renders what is scrolled away.
        let (x, y, width, height) = match region {
            crate::ScreenshotRegion::FullPage { width, height } => (0.0, 0.0, width, height),
            crate::ScreenshotRegion::Clip {
                x,
                y,
                width,
                height,
                ..
            } => (x, y, width, height),
        };
        let reply = self.dispatch_cdp_command(
            "Page.captureScreenshot",
            serde_json::json!({
                "format": "png",
                "captureBeyondViewport": true,
                "fromSurface": true,
                "clip": { "x": x, "y": y, "width": width, "height": height, "scale": 1 },
            }),
        )?;
        let value: serde_json::Value = serde_json::from_str(&reply)
            .map_err(|err| WebViewError::WebView(format!("bad screenshot reply: {err}")))?;
        let data = value
            .get("data")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| WebViewError::WebView("screenshot reply has no data".to_string()))?;
        base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|err| WebViewError::WebView(format!("bad screenshot data: {err}")))
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
    pub(crate) fn request_shutdown(&self) {
        self.send(UiCommand::Shutdown);
    }

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
        self.request_shutdown();
    }
}

/// The pieces produced while bringing up a WebView2 controller, returned from
/// the fallible setup closure so a single error path can tear the window down.
type WebViewControllerSetup = (
    ICoreWebView2Controller,
    HostingMode,
    ICoreWebView2,
    Arc<Mutex<HashMap<String, Vec<u8>>>>,
    Arc<document::WindowsDocumentAuthority>,
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

/// Creates the parent window and the WebView2 controller on the UI thread.
fn build_ui_state(
    webtag: WebTag,
    effective_options: EffectiveWebViewCreateOptions,
    native_view_id: NativeWebViewId,
    parent_window: Arc<Mutex<PublishedParent>>,
) -> StdResult<UiState> {
    let native_view = create_webview_parent(&webtag)?;
    let hwnd = hwnd_from_handle(native_view.window);
    publish_parent_window(&parent_window, native_view.window);
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
        let accelerator_tag = webtag.clone();
        unsafe {
            let mut token = 0;
            controller
                .add_AcceleratorKeyPressed(
                    &AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
                        if let Some(args) = args {
                            let mut kind = COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN;
                            args.KeyEventKind(&mut kind)?;
                            if kind == COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN {
                                let mut key = 0;
                                args.VirtualKey(&mut key)?;
                                if handle_host_accelerator(&accelerator_tag, key) {
                                    args.SetHandled(true)?;
                                }
                            }
                        }
                        Ok(())
                    })),
                    &mut token,
                )
                .map_err(|error| WebViewError::WebView(format!("register accelerator: {error}")))?;
        }
        configure_settings(&webview, &effective_options)?;
        configure_context_menu(&webview, &env, &webtag, &effective_options)?;
        // lxapp pages (non-relaxed) get the runtime-owned selection/copy baseline;
        // browser tabs render arbitrary external pages and must not be restyled.
        let inject_platform_baseline = effective_options.profile != SecurityProfile::BrowserRelaxed;
        install_document_scripts(&webview, inject_platform_baseline)?;
        let memory_pages = Arc::new(Mutex::new(HashMap::new()));
        let document_authority = Arc::new(document::WindowsDocumentAuthority::default());
        register_event_handlers(
            &env,
            &webview,
            webtag.clone(),
            native_view_id,
            &effective_options.registered_schemes,
            memory_pages.clone(),
            Arc::clone(&document_authority),
        )?;
        Ok((
            controller,
            hosting,
            webview,
            memory_pages,
            document_authority,
            ephemeral_profile,
        ))
    })();

    let (controller, hosting, webview, memory_pages, document_authority, mut ephemeral_profile) =
        match setup {
            Ok(parts) => parts,
            Err(err) => {
                destroy_webview_parent(webtag.key(), native_view);
                return Err(err);
            }
        };

    // Page-log capture over CDP (console calls, uncaught exceptions,
    // browser-level messages) is wired before the message loop pumps any
    // command, so it is live before the first navigation. Best-effort: a
    // subscribe failure must not fail webview creation.
    let console_receivers = if crate::webview::platform_console_delivery(
        effective_options.profile,
        crate::webview::PlatformConsoleBackend::Windows,
    ) == crate::webview::PlatformConsoleDelivery::DirectDelegate
    {
        match console::subscribe(&webview, &webtag, native_view_id) {
            Ok(receivers) => {
                console::enable(&webview);
                receivers
            }
            Err(err) => {
                log::warn!("console log capture unavailable: {err}");
                Vec::new()
            }
        }
    } else {
        // BrowserRelaxed control documents use the V3-bound main WebMessage
        // channel; CDP has no document-session binding and remains disabled.
        Vec::new()
    };

    let default_user_agent = match user_agent(&webview) {
        Ok(user_agent) => user_agent,
        Err(err) => {
            unsafe {
                let _ = controller.Close();
            }
            if let HostingMode::Composition(surface) = &hosting {
                surface.destroy();
            }
            destroy_webview_parent(webtag.key(), native_view);
            return Err(err);
        }
    };
    let configured_profile = browser_emulation::configured_profile();
    let mut state = UiState {
        controller,
        webview,
        hosting,
        hwnd,
        parent_window,
        native_view,
        native_view_id,
        webtag: webtag.clone(),
        webtag_key,
        memory_pages,
        document_authority,
        ephemeral_user_data_dir: ephemeral_profile.take(),
        network_log: Arc::new(Mutex::new(network::NetworkLog::default())),
        network_receivers: Vec::new(),
        _console_receivers: console_receivers,
        default_user_agent,
        browser_emulation_configured: configured_profile.is_some(),
    };

    let bootstrap = (|| -> StdResult<()> {
        if let Some(profile) = configured_profile
            && profile != WindowsBrowserEmulationProfile::Desktop
        {
            let (profile_tx, profile_rx) = mpsc::channel();
            browser_emulation::apply_profile(
                &state.webview,
                &state.default_user_agent,
                profile,
                profile_tx,
            );
            ui_thread::pump_until(&profile_rx, Some(BROWSER_EMULATION_TIMEOUT)).map_err(
                |err| match err {
                    mpsc::RecvTimeoutError::Timeout => {
                        WebViewError::WebView("WebView2 browser emulation timed out".to_string())
                    }
                    mpsc::RecvTimeoutError::Disconnected => {
                        WebViewError::WebView("WebView2 callback channel disconnected".to_string())
                    }
                },
            )??;
        }
        // Appearance is a dev nicety, not a correctness precondition like UA
        // emulation: skip the neutral value and never let a failure (e.g. a
        // WebView2 runtime without ICoreWebView2_13) abort webview creation.
        if let Some(scheme) =
            color_scheme::lxapp_color_scheme(&webtag).or_else(color_scheme::configured_color_scheme)
            && scheme != WindowsPreferredColorScheme::Auto
            && let Err(err) =
                color_scheme::apply_color_scheme(&state.webview, &state.controller, scheme)
        {
            log::warn!("Failed to apply preferred color scheme at startup: {err}");
        }
        Ok(())
    })();
    if let Err(err) = bootstrap {
        cleanup_state(&mut state);
        return Err(err);
    }

    Ok(state)
}

/// Waits off the UI thread for a dispatched command's reply while delivering
/// incoming cross-thread sent messages (via `PeekMessageW(PM_NOREMOVE)`). A
/// caller that owns windows would otherwise deadlock: the UI thread serving
/// the command can itself block in a synchronous send to one of them.
pub(crate) fn recv_reply_pumping<T>(
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
                dispatch_com_calls();
            },
            reply => return reply,
        }
    }
}

thread_local! {
    static COM_WINDOW: std::cell::Cell<Option<isize>> = const { std::cell::Cell::new(None) };
}

/// Run COM calls marshalled into this thread's STA. COM posts them to the
/// thread's hidden `OleMainThreadWndClass` window, so the sent-message pump
/// above never sees them: a WebView2 callback into a waiting host thread then
/// stalls both threads until the command times out. Only that window's
/// messages are dispatched; other posted UI work stays queued.
fn dispatch_com_calls() {
    let window = COM_WINDOW.with(|cached| {
        // Not cached while absent: COM may initialize this thread later.
        cached.get().or_else(|| {
            let found = com_window_for_current_thread();
            if found.is_some() {
                cached.set(found);
            }
            found
        })
    });
    let Some(window) = window else {
        return;
    };
    let window = HWND(window as *mut _);
    let mut msg = MSG::default();
    unsafe {
        while WindowsAndMessaging::PeekMessageW(
            &mut msg,
            Some(window),
            0,
            0,
            WindowsAndMessaging::PM_REMOVE,
        )
        .as_bool()
        {
            WindowsAndMessaging::DispatchMessageW(&msg);
        }
    }
}

fn com_window_for_current_thread() -> Option<isize> {
    let thread = unsafe { Threading::GetCurrentThreadId() };
    let mut after = None;
    loop {
        let window = unsafe {
            WindowsAndMessaging::FindWindowExW(
                Some(WindowsAndMessaging::HWND_MESSAGE),
                after,
                windows::core::w!("OleMainThreadWndClass"),
                None,
            )
        }
        .ok()?;
        if unsafe { WindowsAndMessaging::GetWindowThreadProcessId(window, None) } == thread {
            return Some(window.0 as isize);
        }
        after = Some(window);
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
        UiCommand::LoadTrustedHtml {
            html,
            base_url,
            history_url,
            intent,
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
            if let Some(replaced) = state.document_authority.arm(intent, navigation_url.clone()) {
                normalizer::revoke_trusted_load(&state.webtag, state.native_view_id, replaced);
            }
            let result = unsafe {
                let url = CoTaskMemPWSTR::from(navigation_url.as_str());
                state
                    .webview
                    .Navigate(*url.as_ref().as_pcwstr())
                    .map_err(|err| WebViewError::WebView(format!("Navigate failed: {err}")))
            };
            if result.is_err() {
                state.document_authority.revoke_if_matches(intent);
            }
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
            start_awaiting_eval(&state.webview, &js, resp);
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
        UiCommand::PostMessageToDocument {
            expected_generation,
            gate,
            message,
            resp,
        } => {
            let mut result = Err(WebViewError::WebView(
                "document-bound message target is no longer current".to_string(),
            ));
            let mut with_document = || {
                let mut with_session = || unsafe {
                    let message = CoTaskMemPWSTR::from(message.as_str());
                    result = state
                        .webview
                        .PostWebMessageAsString(*message.as_ref().as_pcwstr())
                        .map_err(|err| {
                            WebViewError::WebView(format!("PostWebMessageAsString failed: {err}"))
                        });
                };
                let _ = gate.with_active(&mut with_session);
            };
            // Match ingress lock order (document generation, then session)
            // so an outbound post cannot deadlock a concurrent callback.
            let _ = normalizer::with_current_document_binding(
                state.native_view_id,
                expected_generation,
                &mut with_document,
            );
            let _ = resp.send(result);
        }
        UiCommand::SetUserAgentOverride { user_agent, resp } => {
            if state.browser_emulation_configured {
                let _ = resp.send(Err(WebViewError::WebView(
                    "per-WebView user-agent overrides conflict with host browser emulation"
                        .to_string(),
                )));
                return Ok(false);
            }
            let user_agent = match user_agent {
                UserAgentOverride::Default => state.default_user_agent.as_str(),
                UserAgentOverride::Custom(ref user_agent) => user_agent.as_str(),
            };
            let result = set_user_agent_override(&state.webview, user_agent);
            let _ = resp.send(result);
        }
        UiCommand::SetPreferredColorScheme { scheme, resp } => {
            let result =
                color_scheme::apply_color_scheme(&state.webview, &state.controller, scheme);
            if let Err(error) = &result {
                log::warn!("failed to apply preferred color scheme: {error}");
            }
            let _ = resp.send(result);
        }
        UiCommand::SetBrowserEmulationProfile { profile, resp } => {
            if state.browser_emulation_configured {
                browser_emulation::apply_profile(
                    &state.webview,
                    &state.default_user_agent,
                    profile,
                    resp,
                );
            } else {
                let _ = resp.send(Err(WebViewError::WebView(
                    "browser emulation was not configured before WebView creation".to_string(),
                )));
            }
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
        UiCommand::BringContentToFront { resp } => {
            let result = bring_controller_content_to_front(state);
            let _ = resp.send(result);
        }
        UiCommand::MoveFocus { resp } => {
            // A hidden controller refuses MoveFocus (E_INVALIDARG). Focus
            // follows a layout the controller may not have reached yet; a
            // controller that is not shown simply keeps focus where it is.
            let result = unsafe {
                let mut visible = windows::core::BOOL::default();
                match state.controller.IsVisible(&mut visible) {
                    Ok(()) if !visible.as_bool() => Ok(()),
                    Ok(()) => state
                        .controller
                        .MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC)
                        .map_err(|err| WebViewError::WebView(format!("MoveFocus failed: {err}"))),
                    Err(err) => Err(WebViewError::WebView(format!("IsVisible failed: {err}"))),
                }
            };
            let _ = resp.send(result);
        }
        UiCommand::SetParentWindow { window, resp } => {
            // Publish intent before admission. Retirement either sees this
            // transfer and queues a later park, or admission rejects its host.
            state
                .parent_window
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .pending = Some(window);
            if !can_present_in_parent(window) {
                publish_parent_window(&state.parent_window, state.hwnd.0 as isize);
                let _ = resp.send(Err(WebViewError::WebView(
                    "Windows parent is retiring".to_string(),
                )));
                return Ok(false);
            }
            let hwnd = hwnd_from_handle(window);
            // A windowed controller can skip a same-parent request. A
            // composition surface cannot: destroying its former host also
            // destroys the child surface, and Windows may reuse that host's
            // HWND for the replacement. `set_parent` detects and rebuilds
            // that dead surface even when the numeric parent is unchanged.
            let composition_hosted = matches!(&state.hosting, HostingMode::Composition(_));
            if !should_update_parent(composition_hosted, state.hwnd == hwnd) {
                publish_parent_window(&state.parent_window, state.hwnd.0 as isize);
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
            publish_parent_window(&state.parent_window, state.hwnd.0 as isize);
            let _ = resp.send(result);
        }
        UiCommand::ParkFromParent {
            expected,
            window,
            resp,
        } => {
            let result = if state.hwnd != hwnd_from_handle(expected) {
                Ok(ParkOutcome::NotInParent)
            } else if !can_present_in_parent(window) {
                Ok(ParkOutcome::Deferred)
            } else {
                state
                    .parent_window
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .pending = Some(window);
                let parent = hwnd_from_handle(window);
                let transfer =
                    set_controller_visible(state, false).and_then(|()| match &mut state.hosting {
                        HostingMode::Windowed => unsafe {
                            state.controller.SetParentWindow(parent).map_err(|err| {
                                WebViewError::WebView(format!("SetParentWindow failed: {err}"))
                            })
                        },
                        HostingMode::Composition(surface) => {
                            surface.set_parent(&state.controller, parent)
                        }
                    });
                let result = transfer.map(|()| {
                    state.hwnd = parent;
                    ParkOutcome::Parked
                });
                publish_parent_window(&state.parent_window, state.hwnd.0 as isize);
                result
            };
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
        // Intercepted by `drain`; listed only for exhaustiveness.
        UiCommand::Shutdown => {}
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

fn bring_controller_content_to_front(state: &mut UiState) -> StdResult<()> {
    match &mut state.hosting {
        // Windowed WebView2 does not expose its child HWND. Reassert
        // visibility after overlay layout so it remains the latest presented
        // sibling controller.
        HostingMode::Windowed => unsafe {
            state
                .controller
                .SetIsVisible(true)
                .map_err(|err| WebViewError::WebView(format!("SetIsVisible failed: {err}")))
        },
        HostingMode::Composition(surface) => surface.bring_to_front(&state.controller),
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
    use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};
    use windows::core::{Interface, Ref, implement};

    #[implement(IClassFactory, Agile = false)]
    struct ApartmentProbe {
        owner: u32,
    }

    impl IClassFactory_Impl for ApartmentProbe_Impl {
        fn CreateInstance(
            &self,
            _: Ref<windows::core::IUnknown>,
            _: *const windows::core::GUID,
            _: *mut *mut std::ffi::c_void,
        ) -> windows::core::Result<()> {
            assert_eq!(
                unsafe { super::Threading::GetCurrentThreadId() },
                self.owner
            );
            Err(windows::Win32::Foundation::E_NOTIMPL.into())
        }

        fn LockServer(&self, _: windows::core::BOOL) -> windows::core::Result<()> {
            assert_eq!(
                unsafe { super::Threading::GetCurrentThreadId() },
                self.owner
            );
            Ok(())
        }
    }

    #[test]
    fn reply_wait_services_com_calls_back_into_the_waiting_apartment() {
        use windows::Win32::System::Com::{
            COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize, IStream,
            Marshal::CoMarshalInterThreadInterfaceInStream,
            StructuredStorage::CoGetInterfaceAndReleaseStream,
        };

        std::thread::spawn(|| unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().unwrap();
            let probe: IClassFactory = ApartmentProbe {
                owner: super::Threading::GetCurrentThreadId(),
            }
            .into();
            let stream = CoMarshalInterThreadInterfaceInStream(&IClassFactory::IID, &probe)
                .unwrap()
                .into_raw() as usize;
            let (sender, receiver) = std::sync::mpsc::channel();
            let worker = std::thread::spawn(move || {
                CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().unwrap();
                let stream = std::mem::ManuallyDrop::new(IStream::from_raw(stream as *mut _));
                let proxy: IClassFactory = CoGetInterfaceAndReleaseStream(&*stream).unwrap();
                let result = proxy
                    .CreateInstance::<_, windows::core::IUnknown>(None)
                    .map(|_| ())
                    .map_err(|err| err.code());
                drop(proxy);
                CoUninitialize();
                sender.send(result).unwrap();
            });
            let reply =
                super::recv_reply_pumping(&receiver, Some(std::time::Duration::from_secs(2)));
            // Keep the apartment available for cleanup even if the regression
            // returns a timeout; joining the marshaling worker must not hang.
            if reply.is_err() {
                let mut msg = super::MSG::default();
                while super::WindowsAndMessaging::PeekMessageW(
                    &mut msg,
                    None,
                    0,
                    0,
                    super::WindowsAndMessaging::PM_REMOVE,
                )
                .as_bool()
                {
                    super::WindowsAndMessaging::DispatchMessageW(&msg);
                }
            }
            assert_eq!(reply.unwrap(), Err(windows::Win32::Foundation::E_NOTIMPL));
            worker.join().unwrap();
            drop(probe);
            CoUninitialize();
        })
        .join()
        .unwrap();
    }

    #[test]
    fn retirement_tracks_both_parents_until_transfer_finishes() {
        let inner = super::WebViewInner {
            id: 0,
            commands: Default::default(),
            webtag: super::WebTag::new("parent-transfer-test", "home", None),
            native_view: 10,
            composition_hosted: true,
            parent_window: Default::default(),
        };
        super::publish_parent_window(&inner.parent_window, 10);
        inner.parent_window.lock().unwrap().pending = Some(20);
        assert!(
            inner.has_parent_window(10),
            "failed transfer must keep its source protected"
        );
        assert!(
            inner.has_parent_window(20),
            "retirement must see an admitted incoming transfer"
        );
        super::publish_parent_window(&inner.parent_window, 10);
        assert!(inner.has_parent_window(10));
        assert!(!inner.has_parent_window(20));
        inner.parent_window.lock().unwrap().pending = Some(20);
        super::publish_parent_window(&inner.parent_window, 20);
        assert!(!inner.has_parent_window(10));
        assert!(inner.has_parent_window(20));
    }

    use super::{
        UiCommand, UiDispatchError, WebViewInner, map_eval_dispatch_error, should_update_parent,
        ui_thread,
    };
    use crate::{
        DocumentGeneration, DocumentOutboundGate, WebTag, WebViewController, WebViewScriptError,
    };
    use std::sync::{Arc, mpsc};

    struct AlwaysActive;

    impl DocumentOutboundGate for AlwaysActive {
        fn with_active(&self, action: &mut dyn FnMut()) -> bool {
            action();
            true
        }
    }

    #[test]
    fn retired_webview_requests_shutdown_before_final_drop() {
        let inner = WebViewInner {
            id: 0,
            commands: Default::default(),
            webtag: WebTag::new("lifecycle-test", "home", Some(1)),
            native_view: 0,
            parent_window: Default::default(),
            composition_hosted: false,
        };

        inner.request_shutdown();

        let command = inner.commands.lock().unwrap().pop_front().unwrap();
        assert!(matches!(command, UiCommand::Shutdown));
    }

    #[test]
    fn document_messages_enqueue_without_reply_wait_and_preserve_fifo() {
        let inner = WebViewInner {
            id: 0,
            commands: Default::default(),
            webtag: WebTag::new("document-queue-test", "home", Some(1)),
            native_view: 0,
            parent_window: Default::default(),
            composition_hosted: false,
        };

        for (generation, message) in [(7, "helloAck"), (7, "ready")] {
            inner
                .post_message_to_document(
                    DocumentGeneration::new(generation),
                    Arc::new(AlwaysActive),
                    message,
                )
                .unwrap();
        }

        let mut queue = inner.commands.lock().unwrap();
        for expected in ["helloAck", "ready"] {
            let UiCommand::PostMessageToDocument {
                expected_generation,
                message,
                ..
            } = queue.pop_front().unwrap()
            else {
                panic!("expected a document-bound message command");
            };
            assert_eq!(expected_generation, DocumentGeneration::new(7));
            assert_eq!(message, expected);
        }
    }

    #[test]
    fn ui_thread_wait_runs_work_posted_behind_it() {
        // A wait on the UI thread must run work queued after the waiting task;
        // otherwise any reply produced by that work could never arrive.
        let outcome = ui_thread::run_on_webview_ui_thread(|| {
            let (tx, rx) = mpsc::channel();
            ui_thread::post(Box::new(move || {
                let _ = tx.send(ui_thread::is_ui_thread());
            }));
            ui_thread::pump_until(&rx, Some(std::time::Duration::from_secs(5)))
        });
        assert_eq!(outcome.map(|reply| reply.ok()), Some(Some(true)));
    }

    #[test]
    fn ui_thread_wait_runs_work_already_queued_behind_it() {
        // Two tasks queued under a single wake must not deadlock when the first
        // enters `pump_until` waiting for the second.
        let outcome = ui_thread::run_on_webview_ui_thread(|| {
            let (tx, rx) = mpsc::channel();
            let (done_tx, done_rx) = mpsc::channel();
            ui_thread::post(Box::new(move || {
                let result = ui_thread::pump_until(&rx, Some(std::time::Duration::from_secs(5)));
                let _ = done_tx.send(result.ok());
            }));
            ui_thread::post(Box::new(move || {
                let _ = tx.send(ui_thread::is_ui_thread());
            }));
            ui_thread::pump_until(&done_rx, Some(std::time::Duration::from_secs(5)))
        });
        assert_eq!(outcome.and_then(|reply| reply.ok()), Some(Some(true)));
    }

    #[test]
    fn composition_rechecks_a_reused_parent_handle() {
        assert!(should_update_parent(true, true));
        assert!(!should_update_parent(false, true));
        assert!(should_update_parent(false, false));
    }

    #[test]
    fn eval_dispatch_timeout_is_reported_as_script_timeout() {
        assert_eq!(
            map_eval_dispatch_error(UiDispatchError::NoReply(Some("timed out".to_string()))),
            WebViewScriptError::Timeout
        );
    }

    #[test]
    fn eval_dispatch_disconnect_is_reported_as_destroyed() {
        assert_eq!(
            map_eval_dispatch_error(UiDispatchError::NoReply(None)),
            WebViewScriptError::Destroyed
        );
    }
}
