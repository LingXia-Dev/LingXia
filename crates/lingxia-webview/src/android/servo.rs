use crate::events::normalizer::{self, NativeKey, NativeNavigationResult, NativeSignal};
use crate::webview::{
    EffectiveWebViewCreateOptions, ProxyActivation, ProxyApplyReport, ProxyConfig, SecurityProfile,
    WebTag, WebViewDataMode, find_webview_by_native_view_id,
};
use crate::{
    ClearSiteDataOptions, ClearSiteDataResult, ContextualSchemeRequest, DownloadRequest,
    FileChooserRequest, FileChooserResponse, LoadError, LoadErrorKind, LogLevel, NativeWebViewId,
    NavigationPolicy, NavigationRequest, NetworkBody, NetworkCaptureSnapshot, NetworkEntry,
    NewWindowPolicy, SchemeRequestFrame, UserAgentOverride, WebMessageFrame, WebMessageSource,
    WebMessageTransport, WebResourceBody, WebResourceResponse, WebViewCookie,
    WebViewCookieSameSite, WebViewCookieSetRequest, WebViewError,
};
use base64::Engine as _;
use cookie::{Cookie, SameSite};
use dpi::PhysicalSize;
use euclid::Scale;
use jni::objects::{JObject, JString};
use jni::sys::{jboolean, jfloat, jint, jlong};
use jni::{EnvUnowned, errors::ThrowRuntimeExAndDefault, jni_sig, jni_str};
use raw_window_handle::{
    AndroidDisplayHandle, AndroidNdkWindowHandle, DisplayHandle, RawDisplayHandle, RawWindowHandle,
    WindowHandle,
};
use servo::protocol_handler::{
    DoneChannel, FetchContext, HttpStatus, NetworkError, ProtocolHandler, ProtocolRegistry,
    Request, ResourceFetchTiming, Response, ResponseBody,
};
use servo::{
    Code, CompositionEvent, CompositionState, ConsoleLogLevel, CookieSource,
    CreateNewWebViewRequest, EmbedderControl, EmbedderControlId, EventLoopWaker, ImeEvent,
    InputEvent, InputMethodControl, InputMethodType, Key, KeyState, KeyboardEvent, LoadStatus,
    Location, Modifiers, NamedKey, PixelFormat, PrefValue, Preferences, RenderingContext, RgbColor,
    SelectElementOptionOrOptgroup, Servo, ServoBuilder, SimpleDialog, StorageType, TouchEvent,
    TouchEventType, TouchId, TouchPointerType, UserContentManager, UserScript, WebResourceLoad,
    WebView, WebViewBuilder, WebViewDelegate, WebViewId, WheelDelta, WheelEvent, WheelMode,
    WindowRenderingContext,
};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::future::{self, Future};
use std::io::Read;
use std::os::fd::FromRawFd;
use std::path::PathBuf;
use std::pin::Pin;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::oneshot;
use url::Url;

use super::webview::complete_pending_eval_request;
use crate::servo_document::{is_download_response, stamp_document_url, unstamped};

/// Servo's protocol registry is fixed when the engine starts, so these are the
/// only builder schemes it can route to LingXia handlers.
const ROUTED_SCHEMES: [&str; 2] = ["lx", "lingxia"];
/// Same outer bound as every other platform's native message ingress.
const MAX_WEB_MESSAGE_BYTES: usize = 64 * 1024;
/// A `window.open()` probe that never navigates is discarded after this delay.
const NEW_WINDOW_PROBE_TIMEOUT: Duration = Duration::from_secs(5);
/// A navigation Servo never resolves stops holding later loads after this.
const NAVIGATION_SETTLE_TIMEOUT: Duration = Duration::from_secs(5);
const CAPTURE_LIMIT: usize = 1_000;
const CAPTURE_BODY_LIMIT: usize = 8 * 1024 * 1024;

/// Security-profile behavior which Servo cannot express as per-view settings.
#[derive(Clone, Copy, Debug)]
pub(super) struct ServoPolicy {
    strict_profile: bool,
    ephemeral: bool,
    new_windows: bool,
}

impl ServoPolicy {
    pub(super) fn from_options(options: &EffectiveWebViewCreateOptions) -> Self {
        let strict_profile = options.profile == SecurityProfile::StrictDefault;
        Self {
            strict_profile,
            ephemeral: options.data_mode == WebViewDataMode::Ephemeral,
            new_windows: !strict_profile || options.has_new_window_handler,
        }
    }
}

/// Refuse a builder scheme Servo could never deliver, instead of creating a
/// WebView whose handler silently never runs.
pub(super) fn validate_create_options(
    options: &EffectiveWebViewCreateOptions,
) -> Result<(), WebViewError> {
    match options
        .registered_schemes
        .iter()
        .find(|scheme| !ROUTED_SCHEMES.contains(&scheme.as_str()))
    {
        Some(scheme) => Err(WebViewError::InvalidCreateOptions(format!(
            "the Servo backend cannot route custom scheme '{scheme}'"
        ))),
        None => Ok(()),
    }
}

fn js_value_to_json(value: servo::JSValue) -> serde_json::Value {
    match value {
        servo::JSValue::Undefined | servo::JSValue::Null => serde_json::Value::Null,
        servo::JSValue::Boolean(value) => value.into(),
        servo::JSValue::Number(value) => serde_json::Number::from_f64(value)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        servo::JSValue::String(value)
        | servo::JSValue::Element(value)
        | servo::JSValue::ShadowRoot(value)
        | servo::JSValue::Frame(value)
        | servo::JSValue::Window(value) => value.into(),
        servo::JSValue::Array(values) => values
            .into_iter()
            .map(js_value_to_json)
            .collect::<Vec<_>>()
            .into(),
        servo::JSValue::Object(values) => values
            .into_iter()
            .map(|(key, value)| (key, js_value_to_json(value)))
            .collect::<serde_json::Map<_, _>>()
            .into(),
    }
}

fn complete_java_evaluation(
    request_id: u64,
    result: Result<servo::JSValue, servo::JavaScriptEvaluationError>,
) {
    if request_id == 0 {
        return;
    }
    let value = match result {
        Ok(value) => {
            serde_json::to_string(&js_value_to_json(value)).unwrap_or_else(|_| "null".to_string())
        }
        Err(error) => {
            log::warn!("Servo JavaScript evaluation failed: {error:?}");
            "null".to_string()
        }
    };
    if let Err(error) = super::jni_env::with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
        let class =
            super::jni_env::get_lingxia_webview_class().ok_or("LingXiaWebView class not cached")?;
        let value = env.new_string(value)?;
        env.call_static_method(
            class,
            jni_str!("completeServoEvaluation"),
            jni_sig!("(JLjava/lang/String;)V"),
            &[(request_id as jlong).into(), (&value).into()],
        )?;
        Ok(())
    }) {
        log::warn!("Failed to return Servo JavaScript result to Java: {error}");
    }
}

fn input_method_type_id(input_type: InputMethodType) -> jint {
    match input_type {
        InputMethodType::Color => 0,
        InputMethodType::Date => 1,
        InputMethodType::DatetimeLocal => 2,
        InputMethodType::Email => 3,
        InputMethodType::Month => 4,
        InputMethodType::Number => 5,
        InputMethodType::Password => 6,
        InputMethodType::Search => 7,
        InputMethodType::Tel => 8,
        InputMethodType::Text => 9,
        InputMethodType::Time => 10,
        InputMethodType::Url => 11,
        InputMethodType::Week => 12,
    }
}

/// The concrete LingXia WebView a Servo view belongs to. Java callbacks and
/// Servo delegate callbacks carry both halves, so a replaced view's late
/// callback can never reach its successor under the same tag.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ViewKey {
    webtag: WebTag,
    native_view_id: NativeWebViewId,
}

impl ViewKey {
    fn new(webtag: &WebTag, native_view_id: NativeWebViewId) -> Self {
        Self {
            webtag: webtag.clone(),
            native_view_id,
        }
    }

    fn java_id(&self) -> jlong {
        self.native_view_id.raw() as jlong
    }

    fn submit(&self, signal: NativeSignal) {
        normalizer::submit(&self.webtag, self.native_view_id, signal);
    }

    fn webview(&self) -> Option<Arc<crate::WebView>> {
        find_webview_by_native_view_id(&self.webtag, self.native_view_id)
    }
}

fn show_java_input_method(view: &ViewKey, control: &InputMethodControl) {
    if let Err(error) = super::jni_env::with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
        let class =
            super::jni_env::get_lingxia_webview_class().ok_or("LingXiaWebView class not cached")?;
        let webtag = env.new_string(view.webtag.as_str())?;
        let text = env.new_string(control.text())?;
        let insertion_point = control
            .insertion_point()
            .and_then(|point| jint::try_from(point).ok())
            .unwrap_or(-1);
        let multiline: jboolean = control.multiline();
        let allow_virtual_keyboard: jboolean = control.allow_virtual_keyboard();
        env.call_static_method(
            class,
            jni_str!("showServoInputMethod"),
            jni_sig!("(Ljava/lang/String;JILjava/lang/String;IZZ)V"),
            &[
                (&webtag).into(),
                view.java_id().into(),
                input_method_type_id(control.input_method_type()).into(),
                (&text).into(),
                insertion_point.into(),
                multiline.into(),
                allow_virtual_keyboard.into(),
            ],
        )?;
        Ok(())
    }) {
        log::warn!(
            "Failed to show Android input method for {}: {error}",
            view.webtag
        );
    }
}

fn hide_java_input_method(view: &ViewKey) {
    if let Err(error) = super::jni_env::with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
        let class =
            super::jni_env::get_lingxia_webview_class().ok_or("LingXiaWebView class not cached")?;
        let webtag = env.new_string(view.webtag.as_str())?;
        env.call_static_method(
            class,
            jni_str!("hideServoInputMethod"),
            jni_sig!("(Ljava/lang/String;J)V"),
            &[(&webtag).into(), view.java_id().into()],
        )?;
        Ok(())
    }) {
        log::warn!(
            "Failed to hide Android input method for {}: {error}",
            view.webtag
        );
    }
}

fn show_java_embedder_control(view: &ViewKey, token: u64, kind: &str, payload: &str) {
    if let Err(error) = super::jni_env::with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
        let class =
            super::jni_env::get_lingxia_webview_class().ok_or("LingXiaWebView class not cached")?;
        let webtag = env.new_string(view.webtag.as_str())?;
        let kind = env.new_string(kind)?;
        let payload = env.new_string(payload)?;
        env.call_static_method(
            class,
            jni_str!("showServoEmbedderControl"),
            jni_sig!("(Ljava/lang/String;JJLjava/lang/String;Ljava/lang/String;)V"),
            &[
                (&webtag).into(),
                view.java_id().into(),
                (token as jlong).into(),
                (&kind).into(),
                (&payload).into(),
            ],
        )?;
        Ok(())
    }) {
        log::warn!(
            "Failed to show Android embedder control for {}: {error}",
            view.webtag
        );
    }
}

fn hide_java_embedder_control(view: &ViewKey, token: u64) {
    if let Err(error) = super::jni_env::with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
        let class =
            super::jni_env::get_lingxia_webview_class().ok_or("LingXiaWebView class not cached")?;
        let webtag = env.new_string(view.webtag.as_str())?;
        env.call_static_method(
            class,
            jni_str!("hideServoEmbedderControl"),
            jni_sig!("(Ljava/lang/String;JJ)V"),
            &[
                (&webtag).into(),
                view.java_id().into(),
                (token as jlong).into(),
            ],
        )?;
        Ok(())
    }) {
        log::warn!(
            "Failed to hide Android embedder control for {}: {error}",
            view.webtag
        );
    }
}

#[derive(Clone, Copy, Debug)]
enum ViewMessage {
    NativeComponent,
    Scroll,
}

fn confirm_window_released(release_token: u64) {
    if let Err(error) = super::jni_env::with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
        let class =
            super::jni_env::get_lingxia_webview_class().ok_or("LingXiaWebView class not cached")?;
        env.call_static_method(
            class,
            jni_str!("servoWindowReleased"),
            jni_sig!("(J)V"),
            &[(release_token as jlong).into()],
        )?;
        Ok(())
    }) {
        log::warn!("Failed to return a released Servo window to Java: {error}");
    }
}

fn dispatch_java_view_message(view: &ViewKey, kind: ViewMessage, message: &str) {
    if let Err(error) = super::jni_env::with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
        let class =
            super::jni_env::get_lingxia_webview_class().ok_or("LingXiaWebView class not cached")?;
        let webtag = env.new_string(view.webtag.as_str())?;
        let message = env.new_string(message)?;
        let args = [(&webtag).into(), view.java_id().into(), (&message).into()];
        let method = match kind {
            ViewMessage::NativeComponent => jni_str!("dispatchServoNativeComponentMessage"),
            ViewMessage::Scroll => jni_str!("dispatchServoScroll"),
        };
        env.call_static_method(
            class,
            method,
            jni_sig!("(Ljava/lang/String;JLjava/lang/String;)V"),
            &args,
        )?;
        Ok(())
    }) {
        log::warn!(
            "Failed to dispatch Servo {kind:?} message for {}: {error}",
            view.webtag
        );
    }
}

unsafe extern "C" {
    fn mallopt(param: libc::c_int, value: libc::c_int) -> libc::c_int;
}

/// Bionic's `M_BIONIC_SET_HEAP_TAGGING_LEVEL` / `M_HEAP_TAGGING_LEVEL_NONE`.
const M_BIONIC_SET_HEAP_TAGGING_LEVEL: libc::c_int = -204;
const M_HEAP_TAGGING_LEVEL_NONE: libc::c_int = 0;

/// SpiderMonkey NaN-boxes pointers into 47 bits, but Android 11+ tags the top
/// byte of every heap pointer on arm64. Stop tagging before Servo allocates
/// anything a JS value can hold; bionic still untags earlier pointers on free.
fn disable_heap_pointer_tagging() {
    if unsafe { mallopt(M_BIONIC_SET_HEAP_TAGGING_LEVEL, M_HEAP_TAGGING_LEVEL_NONE) } == 0 {
        log::debug!("Heap pointer tagging was not active");
    }
}

#[link(name = "android")]
unsafe extern "C" {
    fn ANativeWindow_fromSurface(
        env: *mut jni::sys::JNIEnv,
        surface: jni::sys::jobject,
    ) -> *mut libc::c_void;
    fn ANativeWindow_release(window: *mut libc::c_void);
}

struct NativeWindow(NonNull<libc::c_void>);

impl Drop for NativeWindow {
    fn drop(&mut self) {
        unsafe { ANativeWindow_release(self.0.as_ptr()) };
    }
}

struct RuntimeHandle {
    native_view_id: NativeWebViewId,
    capture: Arc<Mutex<CaptureState>>,
}

#[derive(Default)]
struct CaptureState {
    enabled: bool,
    entries: VecDeque<NetworkEntry>,
    dropped: u64,
}

static RUNTIMES: OnceLock<Mutex<HashMap<String, RuntimeHandle>>> = OnceLock::new();
static EMBEDDER_CONTROLS: OnceLock<Mutex<HashMap<String, HashMap<u64, EmbedderControl>>>> =
    OnceLock::new();
static RUNTIME_SENDER: OnceLock<mpsc::Sender<RuntimeCommand>> = OnceLock::new();
static SERVO_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();
static WEBVIEW_TAGS: OnceLock<Mutex<HashMap<WebViewId, ViewKey>>> = OnceLock::new();
static DOCUMENTS: OnceLock<Mutex<HashMap<String, PageDocuments>>> = OnceLock::new();
static NAVIGATION_FAILURES: OnceLock<Mutex<HashMap<WebViewId, LoadError>>> = OnceLock::new();
static BROWSER_STATES: OnceLock<Mutex<HashMap<String, (NativeWebViewId, BrowserState)>>> =
    OnceLock::new();
static NEXT_LOAD_KEY: AtomicU64 = AtomicU64::new(1);
static BROWSER_CONTROL_DEGRADED: AtomicU64 = AtomicU64::new(0);
static NEXT_DOCUMENT_STAMP: AtomicU64 = AtomicU64::new(1);

/// Pages delivered through `load_data`, by stamped URL. A few recent ones stay
/// servable: a park immediately followed by a re-entry issues two loads, and
/// the superseded navigation may still be fetching its document.
struct PageDocuments {
    native_view_id: NativeWebViewId,
    recent: VecDeque<(String, Vec<u8>)>,
}

const RETAINED_PAGE_DOCUMENTS: usize = 4;

fn runtimes() -> &'static Mutex<HashMap<String, RuntimeHandle>> {
    RUNTIMES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn embedder_controls() -> &'static Mutex<HashMap<String, HashMap<u64, EmbedderControl>>> {
    EMBEDDER_CONTROLS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn webview_tags() -> &'static Mutex<HashMap<WebViewId, ViewKey>> {
    WEBVIEW_TAGS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn documents() -> &'static Mutex<HashMap<String, PageDocuments>> {
    DOCUMENTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn navigation_failures() -> &'static Mutex<HashMap<WebViewId, LoadError>> {
    NAVIGATION_FAILURES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn take_navigation_failure(webview_id: WebViewId) -> Option<LoadError> {
    navigation_failures()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&webview_id)
}

fn view_for_servo_webview(webview_id: WebViewId) -> Option<ViewKey> {
    webview_tags()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&webview_id)
        .cloned()
}

fn is_registered(view: &ViewKey) -> bool {
    runtimes()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(view.webtag.as_str())
        .is_some_and(|runtime| runtime.native_view_id == view.native_view_id)
}

/// A trusted browser document loaded without a provable transport.
pub(super) fn report_browser_control_degraded() {
    let count = BROWSER_CONTROL_DEGRADED.fetch_add(1, Ordering::Relaxed) + 1;
    log::warn!(
        "metric=browser_control_bridge_degraded reason=servo_unproven_transport count={count}"
    );
}

pub fn set_data_dir(path: PathBuf) {
    if let Err(path) = SERVO_DATA_DIR.set(path) {
        log::debug!(
            "Servo data directory was already configured: {}",
            path.display()
        );
    }
}

enum RuntimeCommand {
    Register {
        view: ViewKey,
        policy: ServoPolicy,
    },
    Unregister(ViewKey),
    Dispatch {
        view: ViewKey,
        command: Command,
    },
    Proxy(Option<ProxyConfig>),
    Download {
        view: ViewKey,
        request: DownloadRequest,
    },
    Wake,
}

enum Command {
    SurfaceCreated {
        native_window: usize,
        width: u32,
        height: u32,
        density: f32,
    },
    /// Java frees the window's buffers only after this unbinds it; the token
    /// names that pending release.
    SurfaceDestroyed(u64),
    Resize(u32, u32),
    Paint,
    SetThrottled(bool),
    /// The window's texture left or rejoined the screen; nothing consumes
    /// frames while it is away.
    SetSurfaceShown(bool),
    Touch(TouchEventType, i32, f32, f32),
    Wheel(f64, f64),
    Input(InputEvent),
    Load(String),
    LoadData {
        data: String,
        base_url: String,
        trusted: Option<crate::TrustedLoadIntent>,
    },
    ApplyPendingLoad,
    Exec(String),
    Evaluate {
        script: String,
        request_id: u64,
    },
    /// `eval_js`: a parse guard, then the wrapped script. The result comes back
    /// through `LingXiaProxy.resolveEval`; only a failure to run is reported here.
    EvaluateEnvelope {
        scripts: [String; 2],
        request_id: u64,
        token: String,
    },
    CurrentUrl(oneshot::Sender<Option<String>>),
    PostMessage(String),
    ClearBrowsingData,
    SetUserAgent(UserAgentOverride),
    Reload,
    Back,
    Forward,
    ListCookies(oneshot::Sender<Result<Vec<WebViewCookie>, String>>),
    SetCookie(WebViewCookieSetRequest, oneshot::Sender<Result<(), String>>),
    DeleteCookie {
        name: String,
        domain: String,
        path: String,
        reply: oneshot::Sender<Result<(), String>>,
    },
    ClearCookies(oneshot::Sender<()>),
    ClearSiteData {
        url: String,
        options: ClearSiteDataOptions,
        reply: oneshot::Sender<Result<ClearSiteDataResult, String>>,
    },
    NewWindow {
        probe: WebViewId,
        url: Option<String>,
    },
}

/// Last URL/title/history state Servo reported, so Java getters on the UI
/// thread never wait on the engine thread.
#[derive(Clone, Default)]
struct BrowserState {
    url: String,
    title: String,
    can_go_back: bool,
    can_go_forward: bool,
}

#[derive(Clone)]
struct SenderWaker(mpsc::Sender<RuntimeCommand>);

impl EventLoopWaker for SenderWaker {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(self.clone())
    }

    fn wake(&self) {
        let _ = self.0.send(RuntimeCommand::Wake);
    }
}

fn runtime_sender() -> &'static mpsc::Sender<RuntimeCommand> {
    RUNTIME_SENDER.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        let thread_tx = tx.clone();
        std::thread::Builder::new()
            .name("lx-servo".into())
            .spawn(move || run(thread_tx, rx))
            .expect("failed to start Servo event thread");
        tx
    })
}

pub(super) fn register(webtag: &WebTag, native_view_id: NativeWebViewId, policy: ServoPolicy) {
    let view = ViewKey::new(webtag, native_view_id);
    let mut runtimes = runtimes().lock().unwrap_or_else(|e| e.into_inner());
    if runtimes
        .get(webtag.as_str())
        .is_some_and(|runtime| runtime.native_view_id == native_view_id)
    {
        return;
    }
    // A same-tag successor replaces the old view's runtime; the old view's
    // later teardown then matches nothing.
    runtimes.insert(
        webtag.to_string(),
        RuntimeHandle {
            native_view_id,
            capture: Arc::new(Mutex::new(CaptureState::default())),
        },
    );
    let _ = runtime_sender().send(RuntimeCommand::Register { view, policy });
}

pub(super) fn unregister(webtag: &WebTag, native_view_id: NativeWebViewId) {
    let removed = {
        let mut runtimes = runtimes().lock().unwrap_or_else(|e| e.into_inner());
        let current = runtimes
            .get(webtag.as_str())
            .is_some_and(|runtime| runtime.native_view_id == native_view_id);
        current && runtimes.remove(webtag.as_str()).is_some()
    };
    if !removed {
        return;
    }
    embedder_controls()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .remove(webtag.as_str());
    browser_states()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .remove(webtag.as_str());
    let _ = runtime_sender().send(RuntimeCommand::Unregister(ViewKey::new(
        webtag,
        native_view_id,
    )));
    disable_network_observer_if_idle();
}

fn send(view: &ViewKey, command: Command) -> Result<(), WebViewError> {
    if !is_registered(view) {
        return Err(WebViewError::WebView(format!(
            "Servo backend is not ready for {}",
            view.webtag
        )));
    }
    runtime_sender()
        .send(RuntimeCommand::Dispatch {
            view: view.clone(),
            command,
        })
        .map_err(|_| WebViewError::WebView(format!("Servo backend stopped for {}", view.webtag)))
}

fn run(tx: mpsc::Sender<RuntimeCommand>, rx: mpsc::Receiver<RuntimeCommand>) {
    disable_heap_pointer_tagging();
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let Some(data_dir) = SERVO_DATA_DIR.get().cloned() else {
        log::error!("Servo data directory must be configured before creating a WebView");
        return;
    };
    if let Err(error) = std::fs::create_dir_all(&data_dir) {
        log::error!(
            "Failed to create Servo data directory {}: {error}",
            data_dir.display()
        );
        return;
    }
    let mut protocols = ProtocolRegistry::default();
    for scheme in ROUTED_SCHEMES {
        protocols
            .register(scheme, SchemeProtocolHandler { scheme })
            .expect("LingXia protocols should only be registered once");
    }
    servo_net::set_navigation_observer(Some(Arc::new(ServoNavigationObserver)));

    let opts = servo::Opts {
        config_dir: Some(data_dir),
        ..Default::default()
    };
    let servo = ServoBuilder::default()
        .opts(opts)
        .preferences(Preferences::default())
        .protocol_registry(protocols)
        .event_loop_waker(Box::new(SenderWaker(tx)))
        .build();
    let mut states = HashMap::<String, EngineState>::new();

    while let Ok(runtime_command) = rx.recv() {
        match runtime_command {
            RuntimeCommand::Register { view, policy } => {
                log::info!("Registering Servo WebView state for {}", view.webtag);
                if policy.ephemeral {
                    // Servo has one site-data store per process. Mirror the
                    // Android single-profile fallback: nothing persistent may
                    // be visible to an ephemeral view, nor survive it.
                    clear_all_site_data(&servo);
                }
                if let Some(mut replaced) = states.insert(
                    view.webtag.to_string(),
                    EngineState::new(view.clone(), policy),
                ) {
                    replaced.destroy_surface();
                }
            }
            RuntimeCommand::Unregister(view) => {
                let current = states
                    .get(view.webtag.as_str())
                    .is_some_and(|state| state.view_key == view);
                if current && let Some(mut state) = states.remove(view.webtag.as_str()) {
                    state.destroy_surface();
                    if state.policy.ephemeral {
                        clear_all_site_data(&servo);
                    }
                }
                let mut documents = documents().lock().unwrap_or_else(|e| e.into_inner());
                if documents
                    .get(view.webtag.as_str())
                    .is_some_and(|document| document.native_view_id == view.native_view_id)
                {
                    documents.remove(view.webtag.as_str());
                }
            }
            RuntimeCommand::Dispatch { view, command } => {
                match states.get_mut(view.webtag.as_str()) {
                    Some(state) if state.view_key == view => state.handle(&servo, command),
                    // No state renders for this view any more.
                    _ => match command {
                        Command::SurfaceCreated { native_window, .. } => {
                            if let Some(native_window) =
                                NonNull::new(native_window as *mut libc::c_void)
                            {
                                unsafe { ANativeWindow_release(native_window.as_ptr()) };
                            }
                        }
                        Command::SurfaceDestroyed(release_token) => {
                            confirm_window_released(release_token)
                        }
                        _ => {}
                    },
                }
            }
            RuntimeCommand::Proxy(config) => {
                let (http, https, bypass) = config
                    .map(|config| {
                        let proxy = format!("http://{}:{}", config.host, config.port);
                        (proxy.clone(), proxy, config.bypass.join(","))
                    })
                    .unwrap_or_default();
                servo.set_preference("network_http_proxy_uri", PrefValue::Str(http));
                servo.set_preference("network_https_proxy_uri", PrefValue::Str(https));
                servo.set_preference("network_http_no_proxy", PrefValue::Str(bypass));
            }
            RuntimeCommand::Download { view, mut request } => {
                // The claimed navigation was aborted and will produce no document.
                if let Some(state) = states.get(view.webtag.as_str())
                    && state.view_key == view
                    && let Some(webview) = &state.view
                {
                    state.loads.navigation_settled(webview);
                }
                if let Ok(url) = Url::parse(&request.url) {
                    let cookies = servo
                        .site_data_manager()
                        .cookies_for_url(url, CookieSource::HTTP)
                        .into_iter()
                        .map(|cookie| format!("{}={}", cookie.name(), cookie.value()))
                        .collect::<Vec<_>>();
                    if !cookies.is_empty() {
                        request.cookie = Some(cookies.join("; "));
                    }
                }
                std::thread::spawn(move || {
                    if let Some(webview) = view.webview() {
                        webview.handle_download(request);
                    }
                });
            }
            RuntimeCommand::Wake => {}
        }
        servo.spin_event_loop();
    }
}

fn clear_all_site_data(servo: &Servo) {
    let storage_types = StorageType::Cookies | StorageType::Local | StorageType::Session;
    let manager = servo.site_data_manager();
    let sites = manager
        .site_data(storage_types)
        .into_iter()
        .map(|site| site.name())
        .collect::<Vec<_>>();
    let site_refs = sites.iter().map(String::as_str).collect::<Vec<_>>();
    manager.clear_site_data(&site_refs, storage_types);
    // This also covers cookies whose hosts Servo cannot reduce to a
    // registered domain (for example localhost and IP hosts).
    manager.clear_cookies(None);
}

/// Where the view's current top-level load stands, as far as LingXia has
/// reported it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LoadPhase {
    Idle,
    /// Started and committed under this key; its `Complete` finishes it.
    Loading(NativeKey),
    /// The load failed; Servo's error document lifecycle is not reported.
    Failed,
}

/// Servo reports load status per document, without attempt ids: `Started`
/// when a new top-level document is created (deduplicated away for a view's
/// first load and for reloads), `HeadParsed` once its body exists, and
/// `Complete` at its load event. Each document gets its own key, so the
/// normalizer binds every commit to exactly one start.
struct LoadTracker {
    phase: Cell<LoadPhase>,
    /// The `about:blank` document a view is built with is not a LingXia
    /// navigation; its lifecycle must not reach the page.
    bootstrapping: Cell<bool>,
    /// Servo's constellation drops an embedder load while the view already
    /// has a navigation pending, so the first request would win. Loads wait
    /// here until that navigation produces its document; the latest wins.
    /// The pending navigation and when it began. Only a document at its URL
    /// resolves it: a late event from the previous document must not.
    pending_navigation: RefCell<Option<(Url, Instant)>>,
    queued_load: RefCell<Option<Url>>,
    /// The host-issued trusted load and the unique stamped URL its HTML is
    /// served at. Only the first document at exactly that URL attests it,
    /// the way Android matches its load token at page start.
    trusted_load: RefCell<Option<(String, crate::TrustedLoadIntent)>>,
    /// The pending navigation's URL change, held until its document starts.
    deferred_location: RefCell<Option<NativeSignal>>,
}

impl Default for LoadTracker {
    fn default() -> Self {
        Self {
            phase: Cell::new(LoadPhase::Idle),
            bootstrapping: Cell::new(false),
            pending_navigation: RefCell::new(None),
            queued_load: RefCell::new(None),
            trusted_load: RefCell::new(None),
            deferred_location: RefCell::new(None),
        }
    }
}

impl LoadTracker {
    fn navigation_pending(&self) -> bool {
        self.pending_navigation
            .borrow()
            .as_ref()
            .is_some_and(|(_, since)| since.elapsed() < NAVIGATION_SETTLE_TIMEOUT)
    }

    fn begin_navigation(&self, url: &Url) {
        *self.pending_navigation.borrow_mut() = Some((url.clone(), Instant::now()));
    }

    fn navigate(&self, webview: &WebView, url: Url) {
        if self.navigation_pending() {
            log::debug!("Servo load of {url} waits for the pending navigation");
            *self.queued_load.borrow_mut() = Some(url);
            return;
        }
        self.begin_navigation(&url);
        webview.load(url);
    }

    /// A document at `url` exists; if it is the pending navigation's, issue
    /// the load that waited for it.
    fn document_appeared(&self, webview: &WebView, url: Option<&Url>) {
        let resolves = self
            .pending_navigation
            .borrow()
            .as_ref()
            .is_some_and(|(pending, _)| Some(pending) == url);
        if resolves {
            self.navigation_settled(webview);
        }
    }

    /// The pending navigation is resolved or abandoned.
    fn navigation_settled(&self, webview: &WebView) {
        self.pending_navigation.borrow_mut().take();
        if let Some(url) = self.queued_load.borrow_mut().take() {
            self.navigate(webview, url);
        }
    }

    /// Recover a queued load if Servo never resolved the pending navigation
    /// (a redirect changed its URL, or it was dropped).
    fn flush_stale_navigation(&self, webview: &WebView) {
        if self.pending_navigation.borrow().is_some() && !self.navigation_pending() {
            self.navigation_settled(webview);
        }
    }

    /// Report a new document: supersede a load still in flight, then either
    /// commit it or, if its navigation failed, terminate it.
    fn document_created(&self, view: &ViewKey, webview_id: WebViewId, url: String) {
        if let LoadPhase::Loading(previous) = self.phase.get() {
            view.submit(NativeSignal::NavigationFinished {
                key: Some(previous),
                result: NativeNavigationResult::Cancelled(Some(
                    crate::events::NavigationCancellationReason::Superseded,
                )),
            });
        }
        super::webview::fail_pending_eval_requests_after_navigation(&view.webtag);
        let key = NEXT_LOAD_KEY.fetch_add(1, Ordering::Relaxed);
        let trusted = {
            let mut trusted_load = self.trusted_load.borrow_mut();
            let matches = trusted_load
                .as_ref()
                .is_some_and(|(stamped, _)| *stamped == url);
            matches.then(|| trusted_load.take()).flatten()
        };
        match trusted {
            Some((_, intent)) => {
                if !normalizer::start_trusted_navigation(
                    &view.webtag,
                    view.native_view_id,
                    intent,
                    key,
                    unstamped(&url),
                ) {
                    normalizer::revoke_trusted_load(&view.webtag, view.native_view_id, intent);
                }
            }
            None => view.submit(NativeSignal::NavigationStarted {
                key: Some(key),
                url: unstamped(&url),
            }),
        }
        if let Some(location) = self.deferred_location.borrow_mut().take() {
            view.submit(location);
        }
        if let Some(error) = take_navigation_failure(webview_id) {
            self.phase.set(LoadPhase::Failed);
            view.submit(NativeSignal::NavigationFinished {
                key: Some(key),
                result: NativeNavigationResult::Failed(error),
            });
        } else {
            self.phase.set(LoadPhase::Loading(key));
            view.submit(NativeSignal::DocumentCommitted { key: Some(key) });
        }
    }

    fn document_complete(&self, view: &ViewKey, webview_id: WebViewId, url: String) {
        if self.phase.get() == LoadPhase::Idle {
            self.document_created(view, webview_id, url.clone());
        }
        if let LoadPhase::Loading(key) = self.phase.get() {
            let result = match take_navigation_failure(webview_id) {
                Some(error) => NativeNavigationResult::Failed(error),
                None => NativeNavigationResult::Succeeded {
                    final_url: unstamped(&url),
                },
            };
            view.submit(NativeSignal::NavigationFinished {
                key: Some(key),
                result,
            });
        }
        self.phase.set(LoadPhase::Idle);
    }

    /// End a load the view will never finish (surface loss, crash).
    fn abandon(&self, view: &ViewKey) {
        if let LoadPhase::Loading(key) = self.phase.replace(LoadPhase::Idle) {
            view.submit(NativeSignal::NavigationFinished {
                key: Some(key),
                result: NativeNavigationResult::Cancelled(None),
            });
        }
    }
}

struct EngineState {
    view_key: ViewKey,
    policy: ServoPolicy,
    view: Option<WebView>,
    context: Option<Rc<WindowRenderingContext>>,
    /// The window the context currently renders to. A view outlives its
    /// window: detaching the host view unbinds it, reattaching rebinds.
    native_window: Option<NativeWindow>,
    density: f32,
    size: PhysicalSize<u32>,
    throttled: bool,
    surface_shown: bool,
    pending_load: Option<String>,
    loads: Rc<LoadTracker>,
    probes: Rc<RefCell<Vec<WebView>>>,
    next_embedder_control_token: Rc<Cell<u64>>,
}

impl EngineState {
    fn new(view_key: ViewKey, policy: ServoPolicy) -> Self {
        Self {
            view_key,
            policy,
            view: None,
            context: None,
            native_window: None,
            density: 1.0,
            size: PhysicalSize::new(1, 1),
            throttled: false,
            surface_shown: true,
            pending_load: None,
            loads: Rc::new(LoadTracker::default()),
            probes: Rc::new(RefCell::new(Vec::new())),
            next_embedder_control_token: Rc::new(Cell::new(0)),
        }
    }

    /// Stop rendering to the window without losing the document.
    fn release_window(&mut self) {
        if self.native_window.is_none() {
            return;
        }
        if let Some(view) = &self.view {
            apply_throttle(view, true);
        }
        if let Some(context) = &self.context
            && let Err(error) = context.take_window()
        {
            log::warn!(
                "Failed to unbind the Servo window for {}: {error:?}",
                self.view_key.webtag
            );
        }
        self.native_window = None;
    }

    fn destroy_surface(&mut self) {
        self.release_window();
        self.probes.borrow_mut().clear();
        if let Some(view) = self.view.take() {
            webview_tags()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&view.id());
            navigation_failures()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&view.id());
        }
        // Any load in flight dies with the Servo view.
        self.loads.abandon(&self.view_key);
        self.context = None;
        self.native_window = None;
    }

    fn handle(&mut self, servo: &Servo, command: Command) {
        match command {
            Command::SurfaceCreated {
                native_window,
                width,
                height,
                density,
            } => self.create_surface(servo, native_window, width, height, density),
            Command::SurfaceDestroyed(release_token) => {
                self.release_window();
                confirm_window_released(release_token);
            }
            Command::Resize(width, height) => {
                self.size = PhysicalSize::new(width.max(1), height.max(1));
                if let Some(view) = &self.view {
                    view.resize(self.size);
                }
            }
            Command::Paint => self.paint(),
            Command::SetThrottled(throttled) => {
                self.throttled = throttled;
                self.apply_visibility();
            }
            Command::SetSurfaceShown(shown) => {
                self.surface_shown = shown;
                self.apply_visibility();
            }
            Command::Touch(kind, id, x, y) => {
                if let Some(view) = &self.view {
                    if matches!(&kind, TouchEventType::Down) {
                        view.focus();
                    }
                    view.notify_input_event(InputEvent::Touch(TouchEvent::new(
                        kind,
                        TouchId(id),
                        servo::DevicePoint::new(x, y).into(),
                        TouchPointerType::Touch,
                    )));
                }
            }
            Command::Wheel(dx, dy) => {
                if let Some(view) = &self.view {
                    let center = servo::DevicePoint::new(
                        self.size.width as f32 / 2.0,
                        self.size.height as f32 / 2.0,
                    );
                    view.notify_input_event(InputEvent::Wheel(WheelEvent::new(
                        WheelDelta {
                            x: dx,
                            y: dy,
                            z: 0.0,
                            mode: WheelMode::DeltaPixel,
                        },
                        center.into(),
                    )));
                }
            }
            Command::Input(event) => {
                if let Some(view) = &self.view {
                    view.notify_input_event(event);
                }
            }
            Command::Load(url) => self.load(url),
            Command::LoadData {
                data,
                base_url,
                trusted,
            } => {
                log::info!(
                    "Loading Servo page data for {} ({} bytes, base {base_url})",
                    self.view_key.webtag,
                    data.len()
                );
                let stamp = NEXT_DOCUMENT_STAMP.fetch_add(1, Ordering::Relaxed);
                let Some(url) = stamp_document_url(&base_url, stamp) else {
                    log::error!(
                        "Servo rejected invalid page base URL for {}: {base_url}",
                        self.view_key.webtag
                    );
                    return;
                };
                {
                    let mut documents = documents().lock().unwrap_or_else(|e| e.into_inner());
                    let page = documents
                        .entry(self.view_key.webtag.to_string())
                        .or_insert_with(|| PageDocuments {
                            native_view_id: self.view_key.native_view_id,
                            recent: VecDeque::new(),
                        });
                    if page.native_view_id != self.view_key.native_view_id {
                        page.native_view_id = self.view_key.native_view_id;
                        page.recent.clear();
                    }
                    if page.recent.len() == RETAINED_PAGE_DOCUMENTS {
                        page.recent.pop_front();
                    }
                    page.recent.push_back((url.clone(), data.into_bytes()));
                }
                if let Some(intent) = trusted
                    && let Some((_, replaced)) = self
                        .loads
                        .trusted_load
                        .borrow_mut()
                        .replace((url.clone(), intent))
                {
                    normalizer::revoke_trusted_load(
                        &self.view_key.webtag,
                        self.view_key.native_view_id,
                        replaced,
                    );
                }
                self.load(url);
            }
            Command::ApplyPendingLoad => {
                if let Some(url) = self.pending_load.take() {
                    self.load(url);
                }
            }
            Command::Exec(script) => {
                if let Some(view) = &self.view {
                    view.evaluate_javascript(script, |_| {});
                }
            }
            Command::Evaluate { script, request_id } => {
                if let Some(view) = &self.view {
                    view.evaluate_javascript(script, move |result| {
                        complete_java_evaluation(request_id, result);
                    });
                } else {
                    complete_java_evaluation(
                        request_id,
                        Err(servo::JavaScriptEvaluationError::WebViewNotReady),
                    );
                }
            }
            Command::EvaluateEnvelope {
                scripts,
                request_id,
                token,
            } => {
                let Some(view) = &self.view else {
                    // No document exists before the view has a surface; like
                    // a document mid-replacement, the caller should retry.
                    super::webview::fail_pending_eval_requests_after_navigation(
                        &self.view_key.webtag,
                    );
                    return;
                };
                for script in scripts {
                    let token = token.clone();
                    let webtag = self.view_key.webtag.clone();
                    view.evaluate_javascript(script, move |result| {
                        use servo::JavaScriptEvaluationError as Error;
                        match result {
                            Err(Error::DocumentNotFound | Error::WebViewNotReady) => {
                                super::webview::fail_pending_eval_requests_after_navigation(
                                    &webtag,
                                );
                            }
                            Err(Error::CompilationFailure) => complete_pending_eval_request(
                                request_id,
                                &token,
                                Err("JavaScript evaluation failed to compile".to_string()),
                            ),
                            // The wrapper resolves through the bridge, and its
                            // Promise result is not serializable.
                            _ => {}
                        }
                    });
                }
            }
            Command::CurrentUrl(reply) => {
                let _ = reply.send(
                    self.view
                        .as_ref()
                        .and_then(|view| view.url())
                        .map(|url| unstamped(url.as_str())),
                );
            }
            Command::PostMessage(message) => {
                if let Some(view) = &self.view {
                    let message = serde_json::to_string(&message).unwrap_or_else(|_| "\"\"".into());
                    view.evaluate_javascript(
                        format!(
                            "window.__LingXiaRecvMessage && window.__LingXiaRecvMessage({message})"
                        ),
                        |_| {},
                    );
                }
            }
            Command::ClearBrowsingData => {
                clear_all_site_data(servo);
                servo.network_manager().clear_cache();
            }
            Command::SetUserAgent(user_agent) => {
                let value = match user_agent {
                    UserAgentOverride::Default => String::new(),
                    UserAgentOverride::Custom(value) => value,
                };
                servo.set_preference("user_agent", PrefValue::Str(value));
            }
            Command::Reload => {
                if let Some(view) = &self.view {
                    view.reload()
                }
            }
            Command::Back => {
                if let Some(view) = &self.view {
                    view.go_back(1);
                }
            }
            Command::Forward => {
                if let Some(view) = &self.view {
                    view.go_forward(1);
                }
            }
            Command::ListCookies(reply) => {
                let _ = reply.send(self.list_cookies(servo));
            }
            Command::SetCookie(request, reply) => {
                let _ = reply.send(self.set_cookie(servo, request));
            }
            Command::DeleteCookie {
                name,
                domain,
                path,
                reply,
            } => {
                let _ = reply.send(self.delete_cookie(servo, &name, &domain, &path));
            }
            Command::ClearCookies(reply) => {
                servo.site_data_manager().clear_cookies(None);
                let _ = reply.send(());
            }
            Command::ClearSiteData {
                url,
                options,
                reply,
            } => {
                let _ = reply.send(self.clear_site_data(servo, &url, options));
            }
            Command::NewWindow { probe, url } => {
                self.probes.borrow_mut().retain(|view| view.id() != probe);
                let Some(url) = url else {
                    return;
                };
                let view = self.view_key.clone();
                // A new-window handler is host code (the browser opens a
                // tab); keep it off the engine thread it may call back into.
                std::thread::spawn(move || {
                    let Some(webview) = view.webview() else {
                        return;
                    };
                    if webview.handle_new_window(&url) == NewWindowPolicy::LoadInSelf
                        && let Err(error) = send(&view, Command::Load(url))
                    {
                        log::warn!("Servo new-window load failed for {}: {error}", view.webtag);
                    }
                });
            }
        }
    }

    fn create_surface(
        &mut self,
        servo: &Servo,
        native_window: usize,
        width: u32,
        height: u32,
        density: f32,
    ) {
        let Some(native_window) = NonNull::new(native_window as *mut libc::c_void) else {
            log::error!(
                "Servo received a null ANativeWindow for {}",
                self.view_key.webtag
            );
            return;
        };
        let native_window = NativeWindow(native_window);
        self.release_window();
        self.surface_shown = true;
        self.size = PhysicalSize::new(width.max(1), height.max(1));
        self.density = density.max(0.1);
        let raw_window =
            RawWindowHandle::AndroidNdk(AndroidNdkWindowHandle::new(native_window.0.cast()));
        let window = unsafe { WindowHandle::borrow_raw(raw_window) };

        if let (Some(view), Some(context)) = (&self.view, &self.context) {
            // surfman panics instead of erroring when EGL rejects a window.
            let bound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                context.set_window(window, self.size)
            }));
            if !matches!(bound, Ok(Ok(()))) {
                log::error!(
                    "Failed to rebind the Servo window for {}",
                    self.view_key.webtag
                );
                return;
            }
            view.resize(self.size);
            self.native_window = Some(native_window);
            self.apply_visibility();
            return;
        }

        let display = unsafe {
            DisplayHandle::borrow_raw(RawDisplayHandle::Android(AndroidDisplayHandle::new()))
        };
        let created = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            WindowRenderingContext::new(display, window, self.size)
        }));
        let context = match created {
            Ok(Ok(context)) => Rc::new(context),
            Ok(Err(error)) => {
                log::error!(
                    "Failed to create Servo EGL context for {}: {error:?}",
                    self.view_key.webtag
                );
                return;
            }
            Err(_) => {
                log::error!(
                    "Servo EGL context creation panicked for {}",
                    self.view_key.webtag
                );
                return;
            }
        };
        if let Err(error) = context.make_current() {
            log::error!("Failed to make Servo EGL context current: {error:?}");
        }

        let content = Rc::new(UserContentManager::new(servo));
        content.add_script(Rc::new(UserScript::from(bridge_script(self.policy))));
        let delegate = Rc::new(Delegate {
            view: self.view_key.clone(),
            policy: self.policy,
            context: context.clone(),
            loads: self.loads.clone(),
            probes: self.probes.clone(),
            next_embedder_control_token: self.next_embedder_control_token.clone(),
        });
        let initial_url = self
            .pending_load
            .take()
            .and_then(|url| Url::parse(&url).ok());
        self.loads.bootstrapping.set(initial_url.is_none());
        let view = WebViewBuilder::new(servo, context.clone())
            .delegate(delegate)
            .user_content_manager(content)
            .hidpi_scale_factor(Scale::new(self.density))
            .url(initial_url.unwrap_or_else(|| Url::parse("about:blank").unwrap()))
            .build();
        if self.throttled {
            apply_throttle(&view, true);
        }
        webview_tags()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(view.id(), self.view_key.clone());
        log::info!(
            "Created Servo surface for {} at {}x{}",
            self.view_key.webtag,
            self.size.width,
            self.size.height
        );
        self.native_window = Some(native_window);
        self.context = Some(context);
        self.view = Some(view);
    }

    fn load(&mut self, url: String) {
        let Ok(url) = Url::parse(&url) else {
            log::error!(
                "Servo rejected invalid URL for {}: {url}",
                self.view_key.webtag
            );
            return;
        };
        if let Some(view) = &self.view
            && !self.loads.bootstrapping.get()
        {
            self.loads.navigate(view, url);
        } else {
            log::debug!(
                "Servo load of {url} for {} waits for its view",
                self.view_key.webtag
            );
            self.pending_load = Some(url.to_string());
        }
    }

    fn apply_visibility(&self) {
        if let Some(view) = &self.view
            && self.native_window.is_some()
        {
            apply_throttle(view, self.throttled || !self.surface_shown);
        }
    }

    fn paint(&self) {
        let (Some(view), Some(context), Some(_)) = (&self.view, &self.context, &self.native_window)
        else {
            return;
        };
        self.loads.flush_stale_navigation(view);
        if !self.surface_shown {
            return;
        }
        if context.make_current().is_ok() {
            view.paint();
            context.present();
        }
    }

    fn current_http_url(&self) -> Result<Url, String> {
        self.view
            .as_ref()
            .and_then(|view| view.url())
            .filter(|url| matches!(url.scheme(), "http" | "https"))
            .ok_or_else(|| "Servo cookie operation requires a current HTTP(S) URL".to_string())
    }

    fn list_cookies(&self, servo: &Servo) -> Result<Vec<WebViewCookie>, String> {
        let url = self.current_http_url()?;
        Ok(servo
            .site_data_manager()
            .cookies_for_url(url, CookieSource::HTTP)
            .into_iter()
            .map(cookie_from_servo)
            .collect())
    }

    fn set_cookie(&self, servo: &Servo, request: WebViewCookieSetRequest) -> Result<(), String> {
        let url = if request.url.trim().is_empty() {
            self.current_http_url()?
        } else {
            Url::parse(&request.url).map_err(|error| format!("invalid cookie URL: {error}"))?
        };
        let mut builder = Cookie::build((request.name, request.value)).path(request.path);
        if let Some(domain) = request.domain {
            builder = builder.domain(domain);
        }
        if request.secure {
            builder = builder.secure(true);
        }
        if request.http_only {
            builder = builder.http_only(true);
        }
        if let Some(same_site) = request.same_site {
            builder = builder.same_site(match same_site {
                WebViewCookieSameSite::Lax => SameSite::Lax,
                WebViewCookieSameSite::Strict => SameSite::Strict,
                WebViewCookieSameSite::None => SameSite::None,
            });
        }
        servo
            .site_data_manager()
            .set_cookie_for_url(url, builder.build().into_owned(), None);
        Ok(())
    }

    fn delete_cookie(
        &self,
        servo: &Servo,
        name: &str,
        domain: &str,
        path: &str,
    ) -> Result<(), String> {
        let scheme = if domain.starts_with('.') {
            "https"
        } else {
            "http"
        };
        let host = domain.trim_start_matches('.');
        let url = Url::parse(&format!("{scheme}://{host}{path}"))
            .map_err(|error| format!("invalid cookie domain/path: {error}"))?;
        let cookie = Cookie::build((name.to_string(), String::new()))
            .domain(domain.to_string())
            .path(path.to_string())
            .max_age(cookie::time::Duration::seconds(0))
            .build()
            .into_owned();
        servo
            .site_data_manager()
            .set_cookie_for_url(url, cookie, None);
        Ok(())
    }

    fn clear_site_data(
        &self,
        servo: &Servo,
        url: &str,
        options: ClearSiteDataOptions,
    ) -> Result<ClearSiteDataResult, String> {
        let url = Url::parse(url).map_err(|error| format!("invalid site URL: {error}"))?;
        let host = url
            .host_str()
            .filter(|host| !host.is_empty())
            .ok_or_else(|| "site URL has no host".to_string())?;
        if options.site_data {
            let storage_types = StorageType::Cookies | StorageType::Local | StorageType::Session;
            let manager = servo.site_data_manager();
            let mut sites = manager
                .site_data(storage_types)
                .into_iter()
                .map(|site| site.name())
                .filter(|site| host == site || host.ends_with(&format!(".{site}")))
                .collect::<Vec<_>>();
            if sites.is_empty() {
                sites.push(host.to_string());
            }
            let site_refs = sites.iter().map(String::as_str).collect::<Vec<_>>();
            manager.clear_site_data(&site_refs, storage_types);
        }
        Ok(ClearSiteDataResult {
            cache_cleared: false,
            site_data_cleared: options.site_data,
        })
    }
}

/// Pause/resume: stop Servo painting and throttle the view's timers and
/// animations while the host view is hidden.
fn apply_throttle(view: &WebView, throttled: bool) {
    view.set_throttled(throttled);
    if throttled {
        view.hide();
    } else {
        view.show();
    }
}

/// `file:` and `content:` are never reachable from web content, matching the
/// Android WebView profile settings.
fn is_blocked_local_scheme(url: &Url) -> bool {
    matches!(url.scheme(), "file" | "content")
}

fn encode_png_bytes(
    width: u32,
    height: u32,
    color: png::ColorType,
    data: &[u8],
) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(color);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    writer
        .write_image_data(data)
        .map_err(|error| error.to_string())?;
    writer.finish().map_err(|error| error.to_string())?;
    Ok(bytes)
}

fn encode_favicon(image: &servo::Image) -> Option<Vec<u8>> {
    let (color, data) = match image.format {
        PixelFormat::K8 => (png::ColorType::Grayscale, image.data().to_vec()),
        PixelFormat::KA8 => (png::ColorType::GrayscaleAlpha, image.data().to_vec()),
        PixelFormat::RGB8 => (png::ColorType::Rgb, image.data().to_vec()),
        PixelFormat::RGBA8 => (png::ColorType::Rgba, image.data().to_vec()),
        PixelFormat::BGRA8 => {
            let mut data = image.data().to_vec();
            for pixel in data.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            (png::ColorType::Rgba, data)
        }
    };
    encode_png_bytes(image.width, image.height, color, &data)
        .inspect_err(|error| log::debug!("Dropping undecodable Servo favicon: {error}"))
        .ok()
}

fn cookie_from_servo(cookie: Cookie<'static>) -> WebViewCookie {
    WebViewCookie {
        name: cookie.name().to_string(),
        value: cookie.value().to_string(),
        domain: cookie.domain().unwrap_or_default().to_string(),
        path: cookie.path().unwrap_or("/").to_string(),
        host_only: cookie.domain().is_none(),
        secure: cookie.secure().unwrap_or(false),
        http_only: cookie.http_only().unwrap_or(false),
        session: cookie.expires().is_none(),
        expires_unix_ms: cookie
            .expires_datetime()
            .map(|date| date.unix_timestamp() * 1_000),
        same_site: cookie.same_site().map(|same_site| match same_site {
            SameSite::Strict => WebViewCookieSameSite::Strict,
            SameSite::Lax => WebViewCookieSameSite::Lax,
            SameSite::None => WebViewCookieSameSite::None,
        }),
    }
}

fn complete_embedder_control(view: &ViewKey, token: u64, action: &str, value: &str) -> bool {
    if !is_registered(view) {
        return false;
    }
    let control = embedder_controls()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get_mut(view.webtag.as_str())
        .and_then(|controls| controls.remove(&token));
    let Some(control) = control else {
        return false;
    };
    let confirm = action == "confirm";
    match control {
        EmbedderControl::SelectElement(mut select) => {
            if confirm && let Ok(indices) = serde_json::from_str::<Vec<usize>>(value) {
                select.select(indices);
            }
            select.submit();
        }
        EmbedderControl::ColorPicker(mut picker) => {
            if confirm {
                let color = value.strip_prefix('#').unwrap_or(value);
                if color.len() == 6
                    && let Ok(rgb) = u32::from_str_radix(color, 16)
                {
                    picker.select(Some(RgbColor {
                        red: (rgb >> 16) as u8,
                        green: (rgb >> 8) as u8,
                        blue: rgb as u8,
                    }));
                }
            }
            picker.submit();
        }
        EmbedderControl::FilePicker(mut picker) => {
            if confirm && let Ok(paths) = serde_json::from_str::<Vec<PathBuf>>(value) {
                picker.select(&paths);
                picker.submit();
            } else {
                picker.dismiss();
            }
        }
        EmbedderControl::SimpleDialog(mut dialog) => {
            if confirm {
                if let SimpleDialog::Prompt(prompt) = &mut dialog {
                    prompt.set_current_value(value);
                }
                dialog.confirm();
            } else {
                dialog.dismiss();
            }
        }
        EmbedderControl::ContextMenu(menu) => menu.dismiss(),
        EmbedderControl::InputMethod(_) => {}
    }
    true
}

fn embedder_control_payload(control: &EmbedderControl) -> Option<(&'static str, String)> {
    match control {
        EmbedderControl::SelectElement(select) => {
            let mut index = 0usize;
            let mut options = Vec::new();
            for entry in select.options() {
                match entry {
                    SelectElementOptionOrOptgroup::Option(option) => {
                        options.push(serde_json::json!({
                            "index": index,
                            "label": option.label,
                            "disabled": option.is_disabled,
                            "group": serde_json::Value::Null,
                        }));
                        index += 1;
                    }
                    SelectElementOptionOrOptgroup::Optgroup {
                        label,
                        options: group_options,
                    } => {
                        for option in group_options {
                            options.push(serde_json::json!({
                                "index": index,
                                "label": option.label,
                                "disabled": option.is_disabled,
                                "group": label,
                            }));
                            index += 1;
                        }
                    }
                }
            }
            Some((
                "select",
                serde_json::json!({
                    "options": options,
                    "selected": select.selected_options(),
                    "multiple": select.allow_select_multiple(),
                })
                .to_string(),
            ))
        }
        EmbedderControl::ColorPicker(picker) => {
            let color = picker
                .current_color()
                .map(|color| format!("#{:02x}{:02x}{:02x}", color.red, color.green, color.blue))
                .unwrap_or_else(|| "#000000".into());
            Some(("color", serde_json::json!({ "color": color }).to_string()))
        }
        EmbedderControl::FilePicker(picker) => Some((
            "file",
            serde_json::json!({
                "filters": picker
                    .filter_patterns()
                    .iter()
                    .map(|pattern| pattern.0.clone())
                    .collect::<Vec<_>>(),
                "multiple": picker.allow_select_multiple(),
            })
            .to_string(),
        )),
        EmbedderControl::SimpleDialog(dialog) => {
            let (kind, default_value) = match dialog {
                SimpleDialog::Alert(_) => ("alert", None),
                SimpleDialog::Confirm(_) => ("confirm", None),
                SimpleDialog::Prompt(prompt) => ("prompt", Some(prompt.current_value())),
            };
            Some((
                kind,
                serde_json::json!({
                    "message": dialog.message(),
                    "default": default_value,
                })
                .to_string(),
            ))
        }
        EmbedderControl::InputMethod(_) | EmbedderControl::ContextMenu(_) => None,
    }
}

fn dispatch_registered_file_chooser(
    view: &ViewKey,
    token: u64,
    request: FileChooserRequest,
) -> bool {
    let Some(webview) = view.webview() else {
        return false;
    };
    let callback_view = view.clone();
    webview.handle_file_chooser(request, move |response| {
        let (action, value) = match response {
            FileChooserResponse::Files(files) => {
                let paths = files
                    .into_iter()
                    .filter_map(|file| file.path)
                    .collect::<Vec<_>>();
                if paths.is_empty() {
                    ("cancel".to_string(), String::new())
                } else {
                    (
                        "confirm".to_string(),
                        serde_json::to_string(&paths).unwrap_or_else(|_| "[]".into()),
                    )
                }
            }
            FileChooserResponse::Error(error) => {
                log::warn!(
                    "Servo file chooser failed for {}: {error}",
                    callback_view.webtag
                );
                ("cancel".to_string(), String::new())
            }
            FileChooserResponse::Cancel => ("cancel".to_string(), String::new()),
        };
        complete_embedder_control(&callback_view, token, &action, &value);
    })
}

struct Delegate {
    view: ViewKey,
    policy: ServoPolicy,
    context: Rc<WindowRenderingContext>,
    loads: Rc<LoadTracker>,
    probes: Rc<RefCell<Vec<WebView>>>,
    next_embedder_control_token: Rc<Cell<u64>>,
}

impl WebViewDelegate for Delegate {
    fn notify_url_changed(&self, webview: WebView, url: Url) {
        if self.loads.bootstrapping.get() {
            return;
        }
        publish_browser_state(&self.view, &webview);
        let location = NativeSignal::LocationChanged {
            url: unstamped(url.as_str()),
        };
        // Servo moves the URL before it reports the new document; the page's
        // start must come first, as it does on every other backend.
        if self
            .loads
            .pending_navigation
            .borrow()
            .as_ref()
            .is_some_and(|(pending, _)| *pending == url)
        {
            *self.loads.deferred_location.borrow_mut() = Some(location);
        } else {
            self.view.submit(location);
        }
        self.view.submit(NativeSignal::BackForwardChanged {
            can_go_back: webview.can_go_back(),
            can_go_forward: webview.can_go_forward(),
        });
    }

    fn notify_page_title_changed(&self, webview: WebView, title: Option<String>) {
        if !self.loads.bootstrapping.get() {
            publish_browser_state(&self.view, &webview);
            self.view.submit(NativeSignal::TitleChanged { title });
        }
    }

    fn notify_history_changed(&self, webview: WebView, _entries: Vec<Url>, _current: usize) {
        if !self.loads.bootstrapping.get() {
            publish_browser_state(&self.view, &webview);
            self.view.submit(NativeSignal::BackForwardChanged {
                can_go_back: webview.can_go_back(),
                can_go_forward: webview.can_go_forward(),
            });
        }
    }

    fn notify_favicon_changed(&self, webview: WebView) {
        let png_bytes = webview.favicon().and_then(|image| encode_favicon(&image));
        self.view.submit(NativeSignal::FaviconChanged { png_bytes });
    }

    fn notify_load_status_changed(&self, webview: WebView, status: LoadStatus) {
        let url = webview
            .url()
            .map(|url| url.to_string())
            .unwrap_or_else(|| "about:blank".into());
        log::debug!(
            "Servo load status {status:?} for {} at {url}",
            self.view.webtag
        );
        if self.loads.bootstrapping.get() {
            if url == "about:blank" {
                if status == LoadStatus::Complete {
                    self.loads.bootstrapping.set(false);
                    let _ = send(&self.view, Command::ApplyPendingLoad);
                }
                return;
            }
            self.loads.bootstrapping.set(false);
        }
        if matches!(status, LoadStatus::Started | LoadStatus::HeadParsed) {
            self.loads
                .document_appeared(&webview, webview.url().as_ref());
        }
        match status {
            LoadStatus::Started => self.loads.document_created(&self.view, webview.id(), url),
            // The first document of a view and a reload never report
            // `Started`; their body is the first evidence of the document.
            LoadStatus::HeadParsed => {
                if self.loads.phase.get() == LoadPhase::Idle {
                    self.loads.document_created(&self.view, webview.id(), url);
                }
            }
            LoadStatus::Complete => self.loads.document_complete(&self.view, webview.id(), url),
        }
    }

    fn notify_new_frame_ready(&self, _webview: WebView) {}

    fn notify_crashed(&self, _webview: WebView, reason: String, _backtrace: Option<String>) {
        log::error!("Servo content crashed for {}: {reason}", self.view.webtag);
        // The Servo view survives its crashed pipeline, like a WebView2
        // renderer failure: the document is gone, the native view is not.
        self.loads.abandon(&self.view);
        self.view.submit(NativeSignal::DocumentInvalidated);
        if let Some(delegate) = self
            .view
            .webview()
            .and_then(|webview| webview.get_delegate())
        {
            delegate.on_web_content_process_terminated(self.view.native_view_id);
        }
    }

    fn request_navigation(&self, _webview: WebView, request: servo::NavigationRequest) {
        if is_blocked_local_scheme(&request.url) {
            request.deny();
            return;
        }
        let navigation = NavigationRequest::new(request.url.to_string(), false, true);
        if self.view.webview().is_some_and(|webview| {
            webview.handle_navigation(&navigation) == NavigationPolicy::Cancel
        }) {
            request.deny();
        } else {
            self.loads.begin_navigation(&request.url);
            request.allow();
        }
    }

    fn request_create_new(&self, _parent: WebView, request: CreateNewWebViewRequest) {
        // Servo discloses the target only through the new view's first
        // navigation. Build a never-painted probe to read it, then route the
        // URL through LingXia's new-window policy like Android's popup probe.
        if !self.policy.new_windows {
            return;
        }
        let probe = request
            .builder(self.context.clone() as Rc<dyn RenderingContext>)
            .delegate(Rc::new(NewWindowProbe {
                parent: self.view.clone(),
                captured: Cell::new(false),
            }))
            .build();
        let probe_id = probe.id();
        self.probes.borrow_mut().push(probe);
        let parent = self.view.clone();
        std::thread::spawn(move || {
            std::thread::sleep(NEW_WINDOW_PROBE_TIMEOUT);
            let _ = send(
                &parent,
                Command::NewWindow {
                    probe: probe_id,
                    url: None,
                },
            );
        });
    }

    fn load_web_resource(&self, _webview: WebView, load: WebResourceLoad) {
        let url = load.request().url.clone();
        if is_blocked_local_scheme(&url) {
            let response =
                servo::WebResourceResponse::new(url).status_code(http::StatusCode::FORBIDDEN);
            load.intercept(response).cancel();
            return;
        }
        // `load_data` under an http(s) base URL (a browser error page) must
        // never reach the network: serve its HTML at the stamped URL.
        if load.request().is_for_main_frame
            && let Some(html) = page_document_html(&self.view, url.as_str())
        {
            let mut headers = http::HeaderMap::new();
            headers.insert(
                http::header::CONTENT_TYPE,
                http::HeaderValue::from_static("text/html; charset=utf-8"),
            );
            headers.insert(
                http::header::CACHE_CONTROL,
                http::HeaderValue::from_static("no-store"),
            );
            let response = servo::WebResourceResponse::new(url).headers(headers);
            let mut intercepted = load.intercept(response);
            intercepted.send_body_data(html);
            intercepted.finish();
        }
    }

    fn show_console_message(&self, _webview: WebView, level: ConsoleLogLevel, message: String) {
        // Browser-profile pages may be hostile; like Android WebView, only
        // strict pages log straight to the delegate.
        if !self.policy.strict_profile {
            return;
        }
        let level = match level {
            ConsoleLogLevel::Trace => LogLevel::Verbose,
            ConsoleLogLevel::Debug | ConsoleLogLevel::Dir => LogLevel::Debug,
            ConsoleLogLevel::Log | ConsoleLogLevel::Info => LogLevel::Info,
            ConsoleLogLevel::Warn => LogLevel::Warn,
            ConsoleLogLevel::Error => LogLevel::Error,
        };
        if let Some(delegate) = self
            .view
            .webview()
            .and_then(|webview| webview.get_delegate())
        {
            delegate.log(level, &message);
        }
    }

    fn show_embedder_control(&self, webview: WebView, control: EmbedderControl) {
        if let EmbedderControl::InputMethod(input_method) = &control {
            if input_method.input_method_type() == InputMethodType::Color {
                hide_java_input_method(&self.view);
            } else {
                show_java_input_method(&self.view, input_method);
            }
            return;
        }
        if self.policy.strict_profile
            && let EmbedderControl::SimpleDialog(dialog) = control
        {
            // Strict pages get no JavaScript dialogs; answer like Android.
            log::info!("Suppressed JavaScript dialog in strict profile");
            match dialog {
                SimpleDialog::Alert(_) => dialog.confirm(),
                SimpleDialog::Confirm(_) | SimpleDialog::Prompt(_) => dialog.dismiss(),
            }
            return;
        }
        let Some((kind, payload)) = embedder_control_payload(&control) else {
            return;
        };
        let token = self
            .next_embedder_control_token
            .get()
            .wrapping_add(1)
            .max(1);
        self.next_embedder_control_token.set(token);
        let host_file_request = match &control {
            EmbedderControl::FilePicker(picker) => Some(FileChooserRequest {
                accept_types: picker
                    .filter_patterns()
                    .iter()
                    .map(|pattern| format!(".{}", pattern.0.trim_start_matches('.')))
                    .collect(),
                allow_multiple: picker.allow_select_multiple(),
                allow_directories: false,
                capture: false,
                source_page_url: webview.url().map(|url| unstamped(url.as_str())),
            }),
            _ => None,
        };
        embedder_controls()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .entry(self.view.webtag.to_string())
            .or_default()
            .insert(token, control);
        let handled_by_host = host_file_request
            .is_some_and(|request| dispatch_registered_file_chooser(&self.view, token, request));
        if !handled_by_host {
            show_java_embedder_control(&self.view, token, kind, &payload);
        }
    }

    fn hide_embedder_control(&self, _webview: WebView, control_id: EmbedderControlId) {
        hide_java_input_method(&self.view);
        let mut registry = embedder_controls()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let controls = registry.entry(self.view.webtag.to_string()).or_default();
        let token = controls
            .iter()
            .find_map(|(token, control)| (control.id() == control_id).then_some(*token));
        if let Some(token) = token {
            controls.remove(&token);
            hide_java_embedder_control(&self.view, token);
        }
    }
}

/// Delegate of a `window.open()` probe view: capture the first navigation,
/// never let it load.
struct NewWindowProbe {
    parent: ViewKey,
    captured: Cell<bool>,
}

impl WebViewDelegate for NewWindowProbe {
    fn request_navigation(&self, webview: WebView, request: servo::NavigationRequest) {
        let url = request.url.to_string();
        request.deny();
        if self.captured.replace(true) || url.is_empty() || url == "about:blank" {
            return;
        }
        let _ = send(
            &self.parent,
            Command::NewWindow {
                probe: webview.id(),
                url: Some(url),
            },
        );
    }
}

struct ServoNavigationObserver;

impl servo_net::NavigationObserver for ServoNavigationObserver {
    fn navigation_failed(
        &self,
        webview_id: WebViewId,
        url: &servo::ServoUrl,
        error: &NetworkError,
    ) {
        navigation_failures()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                webview_id,
                LoadError {
                    failing_url: Some(url.to_string()),
                    kind: load_error_kind(error),
                    description: format!("{error:?}"),
                },
            );
    }

    fn claim_download(
        &self,
        webview_id: WebViewId,
        url: &servo::ServoUrl,
        status: u16,
        headers: &http::HeaderMap,
    ) -> bool {
        if !(200..300).contains(&status) {
            return false;
        }
        let Some(view) = view_for_servo_webview(webview_id) else {
            return false;
        };
        let header = |name: http::header::HeaderName| {
            headers
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        };
        let content_disposition = header(http::header::CONTENT_DISPOSITION);
        let mime_type = header(http::header::CONTENT_TYPE);
        if !is_download_response(content_disposition.as_deref(), mime_type.as_deref())
            || !view
                .webview()
                .is_some_and(|webview| webview.has_download_handler())
        {
            return false;
        }
        let request = DownloadRequest {
            url: url.to_string(),
            user_agent: None,
            content_disposition,
            mime_type,
            content_length: header(http::header::CONTENT_LENGTH)
                .and_then(|value| value.parse().ok()),
            suggested_filename: None,
            source_page_url: None,
            cookie: None,
        };
        runtime_sender()
            .send(RuntimeCommand::Download { view, request })
            .is_ok()
    }
}

fn load_error_kind(error: &NetworkError) -> LoadErrorKind {
    match error {
        NetworkError::SslValidation(..)
        | NetworkError::MixedContent
        | NetworkError::ContentSecurityPolicy
        | NetworkError::CorsGeneral
        | NetworkError::CrossOriginResponse
        | NetworkError::CorsCredentials
        | NetworkError::CorsAllowMethods
        | NetworkError::CorsAllowHeaders
        | NetworkError::CorsMethod
        | NetworkError::CorsAuthorization
        | NetworkError::CorsHeaders => LoadErrorKind::Security,
        NetworkError::UnsupportedScheme | NetworkError::InvalidPort => LoadErrorKind::InvalidUrl,
        NetworkError::ConnectionFailure
        | NetworkError::RedirectError
        | NetworkError::TooManyRedirects
        | NetworkError::HttpError(_)
        | NetworkError::WebsocketConnectionFailure(_) => {
            let description = format!("{error:?}").to_ascii_lowercase();
            if description.contains("dns") || description.contains("resolve") {
                LoadErrorKind::Dns
            } else if description.contains("timed out") || description.contains("timeout") {
                LoadErrorKind::Timeout
            } else {
                LoadErrorKind::Network
            }
        }
        NetworkError::ResourceLoadError(description) => {
            let description = description.to_ascii_lowercase();
            if description.contains("not found") || description.contains("no lingxia") {
                LoadErrorKind::NotFound
            } else {
                LoadErrorKind::Unknown
            }
        }
        _ => LoadErrorKind::Unknown,
    }
}

struct ServoNetworkObserver;

impl servo_net::NetworkObserver for ServoNetworkObserver {
    fn request(
        &self,
        request_id: &str,
        request: &servo_net::ObservedNetworkRequest,
        _update: bool,
    ) {
        // Bridge beacons are LingXia's page transport, not page traffic.
        if request.url.as_str().starts_with("lx://bridge/") {
            return;
        }
        let Some(capture) = capture_for_browsing_context(request.browsing_context_id) else {
            return;
        };
        let mut capture = capture.lock().unwrap_or_else(|error| error.into_inner());
        if !capture.enabled {
            return;
        }

        let wall_time = request
            .started_date_time
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        let url = request.url.to_string();
        if let Some(entry) = capture
            .entries
            .iter_mut()
            .rev()
            .find(|entry| entry.request_id == request_id && entry.url == url)
        {
            update_network_request(entry, request, wall_time);
            return;
        }

        let redirect_count = capture
            .entries
            .iter()
            .filter(|entry| {
                entry.request_id == request_id
                    || entry
                        .request_id
                        .starts_with(&format!("{request_id}:redirect:"))
            })
            .count();
        if let Some(entry) = capture
            .entries
            .iter_mut()
            .rev()
            .find(|entry| entry.request_id == request_id)
        {
            entry.request_id = format!("{request_id}:redirect:{redirect_count}");
            entry.finished.get_or_insert(wall_time);
        }

        let mut entry = NetworkEntry {
            request_id: request_id.to_string(),
            url,
            method: request.method.to_string(),
            resource_type: Some(request.destination.as_str().to_string()),
            request_headers: Vec::new(),
            request_body: None,
            status: None,
            response_headers: Vec::new(),
            mime_type: None,
            response_body: NetworkBody::None,
            from_cache: false,
            failed: None,
            wall_time: Some(wall_time),
            started: wall_time,
            finished: None,
        };
        update_network_request(&mut entry, request, wall_time);
        push_network_entry(&mut capture, entry);
    }

    fn response(
        &self,
        request_id: &str,
        response: &servo_net::ObservedNetworkResponse,
        completed: bool,
    ) {
        let Some(capture) = capture_for_browsing_context(response.browsing_context_id) else {
            return;
        };
        let mut capture = capture.lock().unwrap_or_else(|error| error.into_inner());
        if !capture.enabled {
            return;
        }
        let Some(entry) = capture
            .entries
            .iter_mut()
            .rev()
            .find(|entry| entry.request_id == request_id)
        else {
            return;
        };

        entry.status = response.status.try_code().map(|status| status.as_u16());
        entry.response_headers = response
            .headers
            .as_ref()
            .map(network_headers)
            .unwrap_or_default();
        entry.mime_type = response
            .headers
            .as_ref()
            .and_then(|headers| headers.get(http::header::CONTENT_TYPE))
            .map(|value| String::from_utf8_lossy(value.as_bytes()).to_string());
        entry.from_cache = response.from_cache;
        if completed {
            if response.body.is_some() {
                entry.response_body =
                    network_body(response.body.as_ref(), entry.mime_type.as_deref());
            }
            entry.finished = Some(now_epoch_seconds());
        }
    }

    fn failure(
        &self,
        request_id: &str,
        browsing_context_id: servo_net::ObservedBrowsingContextId,
        error: &servo_net::ObservedNetworkError,
    ) {
        let Some(capture) = capture_for_browsing_context(browsing_context_id) else {
            return;
        };
        let mut capture = capture.lock().unwrap_or_else(|error| error.into_inner());
        if !capture.enabled {
            return;
        }
        if let Some(entry) = capture
            .entries
            .iter_mut()
            .rev()
            .find(|entry| entry.request_id == request_id)
        {
            entry.failed = Some(format!("{error:?}"));
            entry.finished = Some(now_epoch_seconds());
            entry.response_body = NetworkBody::None;
        }
    }
}

fn capture_for_browsing_context(
    browsing_context_id: servo_net::ObservedBrowsingContextId,
) -> Option<Arc<Mutex<CaptureState>>> {
    let view = webview_tags()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .iter()
        .find(|(webview_id, _)| **webview_id == browsing_context_id)
        .map(|(_, view)| view.clone())?;
    capture(&view).ok()
}

fn update_network_request(
    entry: &mut NetworkEntry,
    request: &servo_net::ObservedNetworkRequest,
    wall_time: f64,
) {
    entry.url = request.url.to_string();
    entry.method = request.method.to_string();
    entry.resource_type = Some(request.destination.as_str().to_string());
    entry.request_headers = network_headers(&request.headers);
    entry.request_body = request
        .body
        .as_ref()
        .map(|body| String::from_utf8_lossy(&body.0).to_string());
    entry.wall_time = Some(wall_time);
    entry.started = wall_time;
}

fn network_headers(headers: &http::HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .map(|(name, value)| {
            (
                name.to_string(),
                String::from_utf8_lossy(value.as_bytes()).to_string(),
            )
        })
        .collect()
}

fn network_body(body: Option<&servo_net::ObservedNetworkBody>, mime: Option<&str>) -> NetworkBody {
    let Some(body) = body else {
        return NetworkBody::None;
    };
    if body.0.len() > CAPTURE_BODY_LIMIT {
        return NetworkBody::Skipped {
            reason: format!(
                "response body exceeds {} byte capture limit",
                CAPTURE_BODY_LIMIT
            ),
        };
    }
    if is_textual_mime(mime)
        && let Ok(text) = String::from_utf8(body.0.clone())
    {
        return NetworkBody::Text { text };
    }
    NetworkBody::Base64 {
        base64: base64::engine::general_purpose::STANDARD.encode(&body.0),
    }
}

fn is_textual_mime(mime: Option<&str>) -> bool {
    let mime = mime.unwrap_or_default().to_ascii_lowercase();
    mime.starts_with("text/")
        || mime.contains("json")
        || mime.contains("javascript")
        || mime.contains("xml")
        || mime.contains("svg")
        || mime.contains("x-www-form-urlencoded")
}

fn now_epoch_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

fn push_network_entry(capture: &mut CaptureState, entry: NetworkEntry) {
    if capture.entries.len() == CAPTURE_LIMIT {
        capture.entries.pop_front();
        capture.dropped += 1;
    }
    capture.entries.push_back(entry);
}

/// The LingXia view a Servo request belongs to. Only Servo's own request
/// metadata identifies it; nothing the page puts in the URL does.
fn request_view(request: &Request) -> Option<ViewKey> {
    request
        .target_webview_id
        .and_then(view_for_servo_webview)
        .filter(is_registered)
}

fn scheme_request_frame(request: &Request) -> SchemeRequestFrame {
    if request.destination.as_str() == "document" {
        SchemeRequestFrame::TopLevelDocument
    } else {
        SchemeRequestFrame::Subresource
    }
}

/// Routes one of [`ROUTED_SCHEMES`] to the requesting view's LingXia handler.
/// `lx://bridge/*` is the page-to-native beacon transport.
struct SchemeProtocolHandler {
    scheme: &'static str,
}

impl ProtocolHandler for SchemeProtocolHandler {
    fn load<'a>(
        &'a self,
        request: &'a mut Request,
        _done_chan: &mut DoneChannel,
        _context: &FetchContext,
    ) -> Pin<Box<dyn Future<Output = Response> + Send + 'a>> {
        let url = request.current_url();
        let view = request_view(request);
        if self.scheme == "lx" && url.host_str() == Some("bridge") {
            return Box::pin(future::ready(bridge_response(request, url, view)));
        }
        let timing = ResourceFetchTiming::new(request.timing_type());
        let response = view
            .as_ref()
            .and_then(|view| {
                page_document(view, url.as_str()).or_else(|| {
                    let webview = view.webview()?;
                    let mut builder = http::Request::builder()
                        .method(request.method.clone())
                        .uri(url.as_str());
                    if let Some(headers) = builder.headers_mut() {
                        *headers = request.headers.clone();
                    }
                    let http_request = builder.body(Vec::new()).ok()?;
                    webview.handle_contextual_scheme_request(
                        self.scheme,
                        ContextualSchemeRequest::new(
                            http_request,
                            view.native_view_id,
                            scheme_request_frame(request),
                        ),
                    )
                })
            })
            .map(|response| lingxia_response(url.clone(), timing, response))
            .unwrap_or_else(|| {
                Response::network_error(NetworkError::ResourceLoadError(format!(
                    "No LingXia {}:// handler for {url}",
                    self.scheme
                )))
            });
        Box::pin(future::ready(response))
    }

    fn is_fetchable(&self) -> bool {
        true
    }

    fn is_secure(&self) -> bool {
        true
    }
}

/// HTML delivered through `load_data`, served to that view's own navigation.
fn page_document_html(view: &ViewKey, url: &str) -> Option<Vec<u8>> {
    let documents = documents().lock().unwrap_or_else(|e| e.into_inner());
    let page = documents
        .get(view.webtag.as_str())
        .filter(|page| page.native_view_id == view.native_view_id)?;
    page.recent
        .iter()
        .rev()
        .find(|(stamped, _)| stamped == url)
        .map(|(_, html)| html.clone())
}

fn page_document(view: &ViewKey, url: &str) -> Option<WebResourceResponse> {
    page_document_html(view, url).map(|html| {
        WebResourceResponse::bytes(html)
            .mime("text/html; charset=utf-8")
            .header("Cache-Control", "no-store")
    })
}

fn lingxia_response(
    url: servo::ServoUrl,
    timing: ResourceFetchTiming,
    response: WebResourceResponse,
) -> Response {
    let (parts, body) = response.into_parts();
    let mut result = Response::new(url, timing);
    result.status = HttpStatus::new_raw(
        parts.status.as_u16(),
        parts
            .status
            .canonical_reason()
            .unwrap_or_default()
            .as_bytes()
            .to_vec(),
    );
    result.headers = parts.headers;
    let bytes = match body {
        WebResourceBody::Bytes(bytes) => Ok(bytes),
        WebResourceBody::Path(path) => std::fs::read(path),
        WebResourceBody::Pipe(pipe) => {
            let mut file = unsafe { std::fs::File::from_raw_fd(pipe.into_raw_fd()) };
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).map(|_| bytes)
        }
    };
    match bytes {
        Ok(bytes) => *result.body.lock() = ResponseBody::Done(bytes),
        Err(error) => {
            return Response::network_error(NetworkError::ResourceLoadError(error.to_string()));
        }
    }
    result
}

fn bridge_response(request: &Request, url: servo::ServoUrl, view: Option<ViewKey>) -> Response {
    let query: HashMap<String, String> = url.as_url().query_pairs().into_owned().collect();
    let kind = url.path().trim_matches('/');
    match (kind, view) {
        ("post", Some(view)) => {
            if let (Some(message), Some(webview)) = (query.get("message"), view.webview()) {
                if message.len() > MAX_WEB_MESSAGE_BYTES {
                    webview.reject_oversized_web_message();
                } else {
                    // Like Android's JavascriptInterface, a beacon cannot
                    // prove which frame sent it.
                    webview.enqueue_web_message(
                        message.clone(),
                        WebMessageFrame::Unproven,
                        WebMessageTransport::Other,
                        WebMessageSource::diagnostic_url(
                            request
                                .referrer
                                .to_url()
                                .map(|referrer| referrer.as_str().to_string()),
                        ),
                    );
                }
            }
        }
        ("component", Some(view)) => {
            if let Some(message) = query.get("message") {
                dispatch_java_view_message(&view, ViewMessage::NativeComponent, message);
            }
        }
        ("scroll", Some(view)) => {
            if let Some(message) = query.get("message") {
                dispatch_java_view_message(&view, ViewMessage::Scroll, message);
            }
        }
        ("eval", _) => {
            if let (Some(id), Some(token), Some(result)) =
                (query.get("id"), query.get("token"), query.get("result"))
                && let Ok(id) = id.parse()
            {
                complete_pending_eval_request(id, token, Ok(result.clone()));
            }
        }
        _ => {}
    }
    let mut response = Response::new(url, ResourceFetchTiming::new(request.timing_type()));
    response.status = HttpStatus::new_raw(204, b"No Content".to_vec());
    *response.body.lock() = ResponseBody::Done(Vec::new());
    response
}

fn bridge_script(policy: ServoPolicy) -> String {
    let strict_profile_script = if policy.strict_profile {
        // Strict pages have DOM storage and databases disabled, as Android
        // WebView's profile settings do.
        r#"
      for (const name of ['localStorage', 'sessionStorage', 'indexedDB']) {
        try {
          Object.defineProperty(globalThis, name, { configurable: false, get: () => null });
        } catch (_) {}
      }
      globalThis.NativeComponentBridge = {
        postMessage: message => send('component', { message: String(message) })
      };
      let scrollFrame = 0;
      const reportScroll = () => {
        scrollFrame = 0;
        const root = document.scrollingElement || document.documentElement;
        send('scroll', { message: JSON.stringify({
          x: root ? root.scrollLeft : 0,
          y: root ? root.scrollTop : 0,
          dpr: globalThis.devicePixelRatio || 1
        }) });
      };
      addEventListener('scroll', () => {
        if (!scrollFrame) scrollFrame = requestAnimationFrame(reportScroll);
      }, { passive: true, capture: true });"#
    } else {
        ""
    };
    format!(
        r#"(() => {{
      const beacons = new Set();
      let sequence = 0;
      const send = (kind, params) => {{
        const query = new URLSearchParams({{
          ...params,
          sequence: sequence++
        }}).toString();
        const beacon = new Image();
        const done = () => beacons.delete(beacon);
        beacon.onload = done;
        beacon.onerror = done;
        beacons.add(beacon);
        beacon.src = `lx://bridge/${{kind}}?${{query}}`;
      }};
      globalThis.LingXiaProxy = {{
        supportsMessagePort: () => false,
        getPort: () => '',
        postMessage: message => send('post', {{ message: String(message) }}),
        resolveEval: (id, token, result) => send('eval', {{ id, token, result }})
      }};
      {strict_profile_script}
    }})();"#
    )
}

pub(super) fn load_url(view: &WebTag, id: NativeWebViewId, url: &str) -> Result<(), WebViewError> {
    send(&ViewKey::new(view, id), Command::Load(url.to_string()))
}

pub(super) fn load_data(
    webtag: &WebTag,
    id: NativeWebViewId,
    data: &str,
    base_url: &str,
) -> Result<(), WebViewError> {
    send(
        &ViewKey::new(webtag, id),
        Command::LoadData {
            data: data.to_string(),
            base_url: base_url.to_string(),
            trusted: None,
        },
    )
}

pub(super) fn load_trusted_data(
    webtag: &WebTag,
    id: NativeWebViewId,
    intent: crate::TrustedLoadIntent,
    data: &str,
    base_url: &str,
) -> Result<(), WebViewError> {
    send(
        &ViewKey::new(webtag, id),
        Command::LoadData {
            data: data.to_string(),
            base_url: base_url.to_string(),
            trusted: Some(intent),
        },
    )
}

pub(super) fn exec_js(
    webtag: &WebTag,
    id: NativeWebViewId,
    script: &str,
) -> Result<(), WebViewError> {
    send(&ViewKey::new(webtag, id), Command::Exec(script.to_string()))
}

pub(super) fn evaluate(
    webtag: &WebTag,
    id: NativeWebViewId,
    request_id: u64,
    token: &str,
    scripts: [String; 2],
) -> Result<(), WebViewError> {
    send(
        &ViewKey::new(webtag, id),
        Command::EvaluateEnvelope {
            scripts,
            request_id,
            token: token.to_string(),
        },
    )
}

pub(super) async fn current_url(
    webtag: &WebTag,
    id: NativeWebViewId,
) -> Result<Option<String>, WebViewError> {
    let (tx, rx) = oneshot::channel();
    send(&ViewKey::new(webtag, id), Command::CurrentUrl(tx))?;
    rx.await
        .map_err(|_| WebViewError::WebView("Servo current_url was canceled".into()))
}

pub(super) fn post_message(
    webtag: &WebTag,
    id: NativeWebViewId,
    message: &str,
) -> Result<(), WebViewError> {
    send(
        &ViewKey::new(webtag, id),
        Command::PostMessage(message.to_string()),
    )
}

pub(super) fn clear_browsing_data(
    webtag: &WebTag,
    id: NativeWebViewId,
) -> Result<(), WebViewError> {
    send(&ViewKey::new(webtag, id), Command::ClearBrowsingData)
}

pub(super) fn set_user_agent(
    webtag: &WebTag,
    id: NativeWebViewId,
    user_agent: UserAgentOverride,
) -> Result<(), WebViewError> {
    user_agent.validate()?;
    send(&ViewKey::new(webtag, id), Command::SetUserAgent(user_agent))
}

pub(super) fn reload(webtag: &WebTag, id: NativeWebViewId) -> Result<(), WebViewError> {
    send(&ViewKey::new(webtag, id), Command::Reload)
}

pub(super) fn go_back(webtag: &WebTag, id: NativeWebViewId) -> Result<(), WebViewError> {
    send(&ViewKey::new(webtag, id), Command::Back)
}

pub(super) fn go_forward(webtag: &WebTag, id: NativeWebViewId) -> Result<(), WebViewError> {
    send(&ViewKey::new(webtag, id), Command::Forward)
}

pub(super) async fn list_cookies(
    webtag: &WebTag,
    id: NativeWebViewId,
) -> Result<Vec<WebViewCookie>, WebViewError> {
    let (tx, rx) = oneshot::channel();
    send(&ViewKey::new(webtag, id), Command::ListCookies(tx))?;
    rx.await
        .map_err(|_| WebViewError::WebView("Servo list_cookies was canceled".into()))?
        .map_err(WebViewError::WebView)
}

pub(super) async fn set_cookie(
    webtag: &WebTag,
    id: NativeWebViewId,
    request: WebViewCookieSetRequest,
) -> Result<(), WebViewError> {
    let (tx, rx) = oneshot::channel();
    send(&ViewKey::new(webtag, id), Command::SetCookie(request, tx))?;
    rx.await
        .map_err(|_| WebViewError::WebView("Servo set_cookie was canceled".into()))?
        .map_err(WebViewError::WebView)
}

pub(super) async fn delete_cookie(
    webtag: &WebTag,
    id: NativeWebViewId,
    name: &str,
    domain: &str,
    path: &str,
) -> Result<(), WebViewError> {
    let (tx, rx) = oneshot::channel();
    send(
        &ViewKey::new(webtag, id),
        Command::DeleteCookie {
            name: name.to_string(),
            domain: domain.to_string(),
            path: path.to_string(),
            reply: tx,
        },
    )?;
    rx.await
        .map_err(|_| WebViewError::WebView("Servo delete_cookie was canceled".into()))?
        .map_err(WebViewError::WebView)
}

pub(super) async fn clear_cookies(
    webtag: &WebTag,
    id: NativeWebViewId,
) -> Result<(), WebViewError> {
    let (tx, rx) = oneshot::channel();
    send(&ViewKey::new(webtag, id), Command::ClearCookies(tx))?;
    rx.await
        .map_err(|_| WebViewError::WebView("Servo clear_cookies was canceled".into()))
}

pub(super) async fn clear_site_data(
    webtag: &WebTag,
    id: NativeWebViewId,
    url: &str,
    options: ClearSiteDataOptions,
) -> Result<ClearSiteDataResult, WebViewError> {
    let (tx, rx) = oneshot::channel();
    send(
        &ViewKey::new(webtag, id),
        Command::ClearSiteData {
            url: url.to_string(),
            options,
            reply: tx,
        },
    )?;
    rx.await
        .map_err(|_| WebViewError::WebView("Servo clear_site_data was canceled".into()))?
        .map_err(WebViewError::WebView)
}

pub(super) fn apply_http_proxy(
    config: Option<&ProxyConfig>,
) -> Result<ProxyApplyReport, WebViewError> {
    runtime_sender()
        .send(RuntimeCommand::Proxy(config.cloned()))
        .map_err(|_| WebViewError::WebView("Servo proxy update failed".into()))?;
    Ok(if config.is_some() {
        ProxyApplyReport::applied(ProxyActivation::EffectiveNow)
    } else {
        ProxyApplyReport::cleared(ProxyActivation::EffectiveNow)
    })
}

fn capture(view: &ViewKey) -> Result<Arc<Mutex<CaptureState>>, WebViewError> {
    runtimes()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(view.webtag.as_str())
        .filter(|runtime| runtime.native_view_id == view.native_view_id)
        .map(|runtime| runtime.capture.clone())
        .ok_or_else(|| {
            WebViewError::WebView(format!("Servo backend is not ready for {}", view.webtag))
        })
}

pub(super) async fn start_network_capture(
    webtag: &WebTag,
    id: NativeWebViewId,
) -> Result<(), WebViewError> {
    capture(&ViewKey::new(webtag, id))?
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .enabled = true;
    servo_net::set_network_observer(Some(Arc::new(ServoNetworkObserver)));
    Ok(())
}

pub(super) async fn stop_network_capture(
    webtag: &WebTag,
    id: NativeWebViewId,
) -> Result<(), WebViewError> {
    capture(&ViewKey::new(webtag, id))?
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .enabled = false;
    disable_network_observer_if_idle();
    Ok(())
}

fn disable_network_observer_if_idle() {
    let captures = runtimes()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .values()
        .map(|runtime| runtime.capture.clone())
        .collect::<Vec<_>>();
    let any_enabled = captures.iter().any(|capture| {
        capture
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .enabled
    });
    if !any_enabled {
        servo_net::set_network_observer(None);
    }
}

pub(super) async fn network_entries(
    webtag: &WebTag,
    id: NativeWebViewId,
) -> Result<NetworkCaptureSnapshot, WebViewError> {
    let capture = capture(&ViewKey::new(webtag, id))?;
    let capture = capture.lock().unwrap_or_else(|e| e.into_inner());
    Ok(NetworkCaptureSnapshot {
        entries: capture.entries.iter().cloned().collect(),
        dropped: capture.dropped,
    })
}

pub(super) async fn clear_network_capture(
    webtag: &WebTag,
    id: NativeWebViewId,
) -> Result<(), WebViewError> {
    let capture = capture(&ViewKey::new(webtag, id))?;
    let mut capture = capture.lock().unwrap_or_else(|e| e.into_inner());
    capture.entries.clear();
    capture.dropped = 0;
    Ok(())
}

fn browser_states() -> &'static Mutex<HashMap<String, (NativeWebViewId, BrowserState)>> {
    BROWSER_STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn publish_browser_state(view: &ViewKey, webview: &WebView) {
    let state = BrowserState {
        url: webview
            .url()
            .map(|url| unstamped(url.as_str()))
            .unwrap_or_default(),
        title: webview.page_title().unwrap_or_default(),
        can_go_back: webview.can_go_back(),
        can_go_forward: webview.can_go_forward(),
    };
    browser_states()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(view.webtag.to_string(), (view.native_view_id, state));
}

fn browser_state(view: &ViewKey) -> BrowserState {
    browser_states()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(view.webtag.as_str())
        .filter(|(native_view_id, _)| *native_view_id == view.native_view_id)
        .map(|(_, state)| state.clone())
        .unwrap_or_default()
}

fn touch_kind(action: i32) -> Option<TouchEventType> {
    match action {
        0 => Some(TouchEventType::Down),
        1 => Some(TouchEventType::Up),
        2 => Some(TouchEventType::Move),
        3 => Some(TouchEventType::Cancel),
        _ => None,
    }
}

fn android_key(key_code: i32, unicode_code_point: u32) -> (Key, Code) {
    let named = match key_code {
        19 => Some((NamedKey::ArrowUp, Code::ArrowUp)),
        20 => Some((NamedKey::ArrowDown, Code::ArrowDown)),
        21 => Some((NamedKey::ArrowLeft, Code::ArrowLeft)),
        22 => Some((NamedKey::ArrowRight, Code::ArrowRight)),
        61 => Some((NamedKey::Tab, Code::Tab)),
        66 => Some((NamedKey::Enter, Code::Enter)),
        67 => Some((NamedKey::Backspace, Code::Backspace)),
        92 => Some((NamedKey::PageUp, Code::PageUp)),
        93 => Some((NamedKey::PageDown, Code::PageDown)),
        111 => Some((NamedKey::Escape, Code::Escape)),
        112 => Some((NamedKey::Delete, Code::Delete)),
        122 => Some((NamedKey::Home, Code::Home)),
        123 => Some((NamedKey::End, Code::End)),
        _ => None,
    };
    if let Some((key, code)) = named {
        return (Key::Named(key), code);
    }
    char::from_u32(unicode_code_point)
        .filter(|character| *character != '\0')
        .map(|character| (Key::Character(character.to_string()), Code::Unidentified))
        .unwrap_or((Key::Named(NamedKey::Unidentified), Code::Unidentified))
}

fn android_modifiers(meta_state: i32) -> Modifiers {
    let mut modifiers = Modifiers::empty();
    if meta_state & 0x1 != 0 {
        modifiers.insert(Modifiers::SHIFT);
    }
    if meta_state & 0x2 != 0 {
        modifiers.insert(Modifiers::ALT);
    }
    if meta_state & 0x1000 != 0 {
        modifiers.insert(Modifiers::CONTROL);
    }
    if meta_state & 0x1_0000 != 0 {
        modifiers.insert(Modifiers::META);
    }
    modifiers
}

/// Resolve a Java callback to its registered view, or `None` for a view the
/// runtime no longer owns under that tag.
fn java_view(
    env: &mut jni::Env<'_>,
    tag: JString<'_>,
    native_view_id: jlong,
) -> Result<Option<ViewKey>, jni::errors::Error> {
    let tag = tag.try_to_string(env)?;
    if native_view_id <= 0 {
        return Ok(None);
    }
    Ok(Some(ViewKey::new(
        &WebTag::from(tag.as_str()),
        NativeWebViewId::new(native_view_id as u64),
    )))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaServoView_nativeSurfaceCreated(
    mut env: EnvUnowned,
    _this: JObject,
    tag: JString,
    native_view_id: jlong,
    surface: JObject,
    width: jint,
    height: jint,
    density: jfloat,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let Some(view) = java_view(env, tag, native_view_id)? else {
            return Ok(());
        };
        let window = unsafe { ANativeWindow_fromSurface(env.get_raw(), surface.as_raw()) };
        log::info!(
            "Received Servo surface for {} at {}x{}",
            view.webtag,
            width,
            height
        );
        if let Err(error) = send(
            &view,
            Command::SurfaceCreated {
                native_window: window as usize,
                width: width.max(1) as u32,
                height: height.max(1) as u32,
                density,
            },
        ) {
            log::error!(
                "Failed to attach Servo surface for {}: {error}",
                view.webtag
            );
            if !window.is_null() {
                unsafe { ANativeWindow_release(window) };
            }
        }
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaServoView_nativeSurfaceChanged(
    mut env: EnvUnowned,
    _this: JObject,
    tag: JString,
    native_view_id: jlong,
    width: jint,
    height: jint,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        if let Some(view) = java_view(env, tag, native_view_id)? {
            let _ = send(
                &view,
                Command::Resize(width.max(1) as u32, height.max(1) as u32),
            );
        }
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaServoView_nativeSurfaceDestroyed(
    mut env: EnvUnowned,
    _this: JObject,
    tag: JString,
    native_view_id: jlong,
    release_token: jlong,
) -> jboolean {
    env.with_env(|env| -> Result<jboolean, jni::errors::Error> {
        // `false` tells Java nothing renders into the window any more.
        // Bypass the registration check: a view already unregistered by its
        // Rust owner may still have that unregistration queued, and only the
        // engine thread's order proves it no longer renders.
        Ok(java_view(env, tag, native_view_id)?.is_some_and(|view| {
            runtime_sender()
                .send(RuntimeCommand::Dispatch {
                    view,
                    command: Command::SurfaceDestroyed(release_token as u64),
                })
                .is_ok()
        }))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaServoView_nativeCompleteEmbedderControl(
    mut env: EnvUnowned,
    _this: JObject,
    tag: JString,
    native_view_id: jlong,
    token: jlong,
    action: JString,
    value: JString,
) -> jboolean {
    env.with_env(|env| -> Result<jboolean, jni::errors::Error> {
        let Some(view) = java_view(env, tag, native_view_id)? else {
            return Ok(false);
        };
        let action = action.try_to_string(env)?;
        let value = value.try_to_string(env)?;
        Ok(complete_embedder_control(&view, token as u64, &action, &value) as jboolean)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaServoView_nativeFrame(
    mut env: EnvUnowned,
    _this: JObject,
    tag: JString,
    native_view_id: jlong,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        if let Some(view) = java_view(env, tag, native_view_id)? {
            let _ = send(&view, Command::Paint);
        }
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaServoView_nativeSetThrottled(
    mut env: EnvUnowned,
    _this: JObject,
    tag: JString,
    native_view_id: jlong,
    throttled: jboolean,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        if let Some(view) = java_view(env, tag, native_view_id)? {
            let _ = send(&view, Command::SetThrottled(throttled));
        }
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaServoView_nativeSetSurfaceShown(
    mut env: EnvUnowned,
    _this: JObject,
    tag: JString,
    native_view_id: jlong,
    shown: jboolean,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        if let Some(view) = java_view(env, tag, native_view_id)? {
            let _ = send(&view, Command::SetSurfaceShown(shown));
        }
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaServoView_nativeTouch(
    mut env: EnvUnowned,
    _this: JObject,
    tag: JString,
    native_view_id: jlong,
    action: jint,
    id: jint,
    x: jfloat,
    y: jfloat,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        if let Some(view) = java_view(env, tag, native_view_id)?
            && let Some(kind) = touch_kind(action)
        {
            let _ = send(&view, Command::Touch(kind, id, x, y));
        }
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaServoView_nativeWheel(
    mut env: EnvUnowned,
    _this: JObject,
    tag: JString,
    native_view_id: jlong,
    dx: f64,
    dy: f64,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        if let Some(view) = java_view(env, tag, native_view_id)? {
            let _ = send(&view, Command::Wheel(dx, dy));
        }
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaServoView_nativeGetUrl<'a>(
    mut env: EnvUnowned<'a>,
    _this: JObject<'a>,
    tag: JString<'a>,
    native_view_id: jlong,
) -> JString<'a> {
    env.with_env(|env| -> Result<JString<'a>, jni::errors::Error> {
        let value = java_view(env, tag, native_view_id)?
            .map(|view| browser_state(&view))
            .map(|state| state.url)
            .unwrap_or_default();
        env.new_string(value)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaServoView_nativeGetTitle<'a>(
    mut env: EnvUnowned<'a>,
    _this: JObject<'a>,
    tag: JString<'a>,
    native_view_id: jlong,
) -> JString<'a> {
    env.with_env(|env| -> Result<JString<'a>, jni::errors::Error> {
        let value = java_view(env, tag, native_view_id)?
            .map(|view| browser_state(&view))
            .map(|state| state.title)
            .unwrap_or_default();
        env.new_string(value)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaServoView_nativeCanGoBack(
    mut env: EnvUnowned,
    _this: JObject,
    tag: JString,
    native_view_id: jlong,
) -> jboolean {
    env.with_env(|env| -> Result<jboolean, jni::errors::Error> {
        Ok(java_view(env, tag, native_view_id)?
            .map(|view| browser_state(&view))
            .is_some_and(|state| state.can_go_back) as jboolean)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaServoView_nativeCanGoForward(
    mut env: EnvUnowned,
    _this: JObject,
    tag: JString,
    native_view_id: jlong,
) -> jboolean {
    env.with_env(|env| -> Result<jboolean, jni::errors::Error> {
        Ok(java_view(env, tag, native_view_id)?
            .map(|view| browser_state(&view))
            .is_some_and(|state| state.can_go_forward) as jboolean)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaServoView_nativeNavigate(
    mut env: EnvUnowned,
    _this: JObject,
    tag: JString,
    native_view_id: jlong,
    action: jint,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let Some(view) = java_view(env, tag, native_view_id)? else {
            return Ok(());
        };
        let command = match action {
            0 => Command::Reload,
            1 => Command::Back,
            2 => Command::Forward,
            _ => return Ok(()),
        };
        if let Err(error) = send(&view, command) {
            log::warn!(
                "Servo navigation command failed for {}: {error}",
                view.webtag
            );
        }
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaServoView_nativeEvaluate(
    mut env: EnvUnowned,
    _this: JObject,
    tag: JString,
    native_view_id: jlong,
    request_id: jlong,
    script: JString,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let view = java_view(env, tag, native_view_id)?;
        let script = script.try_to_string(env)?;
        let dispatched = view.as_ref().map(|view| {
            send(
                view,
                Command::Evaluate {
                    script,
                    request_id: request_id as u64,
                },
            )
        });
        if !matches!(dispatched, Some(Ok(()))) {
            complete_java_evaluation(
                request_id as u64,
                Err(servo::JavaScriptEvaluationError::WebViewNotReady),
            );
        }
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaServoView_nativeIme(
    mut env: EnvUnowned,
    _this: JObject,
    tag: JString,
    native_view_id: jlong,
    state: jint,
    text: JString,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let Some(view) = java_view(env, tag, native_view_id)? else {
            return Ok(());
        };
        let data = text.try_to_string(env)?;
        let state = match state {
            0 => CompositionState::Start,
            1 => CompositionState::Update,
            2 => CompositionState::End,
            _ => return Ok(()),
        };
        if let Err(error) = send(
            &view,
            Command::Input(InputEvent::Ime(ImeEvent::Composition(CompositionEvent {
                state,
                data,
            }))),
        ) {
            log::warn!("Servo IME dispatch failed for {}: {error}", view.webtag);
        }
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaServoView_nativeKey(
    mut env: EnvUnowned,
    _this: JObject,
    tag: JString,
    native_view_id: jlong,
    action: jint,
    key_code: jint,
    unicode_code_point: jint,
    meta_state: jint,
    repeat_count: jint,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let Some(view) = java_view(env, tag, native_view_id)? else {
            return Ok(());
        };
        let state = match action {
            0 => KeyState::Down,
            1 => KeyState::Up,
            _ => return Ok(()),
        };
        let (key, code) = android_key(key_code, unicode_code_point.max(0) as u32);
        let event = KeyboardEvent::new_without_event(
            state,
            key,
            code,
            Location::Standard,
            android_modifiers(meta_state),
            repeat_count > 0,
            false,
        );
        if let Err(error) = send(&view, Command::Input(InputEvent::Keyboard(event))) {
            log::warn!(
                "Servo keyboard dispatch failed for {}: {error}",
                view.webtag
            );
        }
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaServoView_nativeScreenshotResult(
    mut env: EnvUnowned,
    _this: JObject,
    request_id: jlong,
    png_bytes: jni::objects::JByteArray,
    error: JString,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let error = error.try_to_string(env)?;
        let result = if error.trim().is_empty() {
            Ok(env.convert_byte_array(&png_bytes)?)
        } else {
            Err(error)
        };
        super::webview::complete_pending_screenshot_request(request_id as u64, result);
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}
