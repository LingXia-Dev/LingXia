use crate::events::normalizer::{self, NativeNavigationResult, NativeSignal};
use crate::webview::{
    ProxyActivation, ProxyApplyReport, ProxyConfig, WebTag, find_webview, find_webview_delegate,
};
use crate::{
    ClearSiteDataOptions, ClearSiteDataResult, LogLevel, NavigationPolicy, NavigationRequest,
    NetworkBody, NetworkCaptureSnapshot, NetworkEntry, UserAgentOverride, WebResourceBody,
    WebViewCookie, WebViewCookieSameSite, WebViewCookieSetRequest, WebViewError,
};
use cookie::{Cookie, SameSite};
use dpi::PhysicalSize;
use euclid::Scale;
use jni::objects::{JObject, JString};
use jni::sys::{jfloat, jint};
use jni::{EnvUnowned, errors::ThrowRuntimeExAndDefault};
use raw_window_handle::{
    AndroidDisplayHandle, AndroidNdkWindowHandle, DisplayHandle, RawDisplayHandle, RawWindowHandle,
    WindowHandle,
};
use servo::protocol_handler::{
    DoneChannel, FetchContext, HttpStatus, NetworkError, ProtocolHandler, ProtocolRegistry,
    Request, ResourceFetchTiming, Response, ResponseBody,
};
use servo::{
    ConsoleLogLevel, CookieSource, EventLoopWaker, InputEvent, LoadStatus, PrefValue, Preferences,
    RenderingContext, Servo, ServoBuilder, StorageType, TouchEvent, TouchEventType, TouchId,
    TouchPointerType, UserContentManager, UserScript, WebView, WebViewBuilder, WebViewDelegate,
    WheelDelta, WheelEvent, WheelMode, WindowRenderingContext,
};
use std::collections::{HashMap, VecDeque};
use std::future::{self, Future};
use std::io::Read;
use std::os::fd::FromRawFd;
use std::pin::Pin;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::oneshot;
use url::Url;

use super::webview::complete_pending_eval_request;

const CAPTURE_LIMIT: usize = 1_000;

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
    tx: mpsc::Sender<Command>,
    capture: Arc<Mutex<CaptureState>>,
}

#[derive(Default)]
struct CaptureState {
    enabled: bool,
    entries: VecDeque<NetworkEntry>,
    dropped: u64,
}

static RUNTIMES: OnceLock<Mutex<HashMap<String, RuntimeHandle>>> = OnceLock::new();
static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

fn runtimes() -> &'static Mutex<HashMap<String, RuntimeHandle>> {
    RUNTIMES.get_or_init(|| Mutex::new(HashMap::new()))
}

enum Command {
    SurfaceCreated {
        native_window: usize,
        width: u32,
        height: u32,
        density: f32,
    },
    SurfaceDestroyed,
    Resize(u32, u32),
    Tick,
    Touch(TouchEventType, i32, f32, f32),
    Wheel(f64, f64),
    Load(String),
    LoadData {
        data: String,
        base_url: String,
    },
    Exec(String),
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
    Screenshot(oneshot::Sender<Result<Vec<u8>, String>>),
    Proxy(Option<ProxyConfig>),
    Shutdown,
}

#[derive(Clone)]
struct SenderWaker(mpsc::Sender<Command>);

impl EventLoopWaker for SenderWaker {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(self.clone())
    }

    fn wake(&self) {
        let _ = self.0.send(Command::Tick);
    }
}

pub(super) fn register(webtag: &WebTag) {
    let key = webtag.to_string();
    let mut runtimes = runtimes().lock().unwrap_or_else(|e| e.into_inner());
    if runtimes.contains_key(&key) {
        return;
    }
    let (tx, rx) = mpsc::channel();
    let capture = Arc::new(Mutex::new(CaptureState::default()));
    let thread_tag = webtag.clone();
    let thread_tx = tx.clone();
    let thread_capture = capture.clone();
    std::thread::Builder::new()
        .name(format!("lx-servo-{}", webtag.key()))
        .spawn(move || run(thread_tag, thread_tx, rx, thread_capture))
        .expect("failed to start Servo event thread");
    runtimes.insert(key, RuntimeHandle { tx, capture });
}

pub(super) fn unregister(webtag: &WebTag) {
    if let Some(runtime) = runtimes()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(webtag.as_str())
    {
        let _ = runtime.tx.send(Command::Shutdown);
    }
}

fn send(webtag: &WebTag, command: Command) -> Result<(), WebViewError> {
    let tx = runtimes()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(webtag.as_str())
        .map(|runtime| runtime.tx.clone())
        .ok_or_else(|| WebViewError::WebView(format!("Servo backend is not ready for {webtag}")))?;
    tx.send(command)
        .map_err(|_| WebViewError::WebView(format!("Servo backend stopped for {webtag}")))
}

fn run(
    webtag: WebTag,
    tx: mpsc::Sender<Command>,
    rx: mpsc::Receiver<Command>,
    capture: Arc<Mutex<CaptureState>>,
) {
    let mut protocols = ProtocolRegistry::default();
    protocols
        .register("lx", LxProtocolHandler(webtag.clone()))
        .expect("lx protocol should only be registered once");
    protocols
        .register("lxbridge", BridgeProtocolHandler(webtag.clone()))
        .expect("lxbridge protocol should only be registered once");

    let servo = ServoBuilder::default()
        .preferences(Preferences::default())
        .protocol_registry(protocols)
        .event_loop_waker(Box::new(SenderWaker(tx)))
        .build();
    let mut state = EngineState {
        webtag,
        servo,
        view: None,
        context: None,
        native_window: None,
        density: 1.0,
        size: PhysicalSize::new(1, 1),
        pending_load: None,
        capture,
    };

    while let Ok(command) = rx.recv() {
        let shutdown = matches!(command, Command::Shutdown);
        state.handle(command);
        state.servo.spin_event_loop();
        if shutdown {
            break;
        }
    }
}

struct EngineState {
    webtag: WebTag,
    servo: Servo,
    view: Option<WebView>,
    context: Option<Rc<dyn RenderingContext>>,
    native_window: Option<NativeWindow>,
    density: f32,
    size: PhysicalSize<u32>,
    pending_load: Option<String>,
    capture: Arc<Mutex<CaptureState>>,
}

impl EngineState {
    fn handle(&mut self, command: Command) {
        match command {
            Command::SurfaceCreated {
                native_window,
                width,
                height,
                density,
            } => self.create_surface(native_window, width, height, density),
            Command::SurfaceDestroyed => {
                self.pending_load = self
                    .view
                    .as_ref()
                    .and_then(|view| view.url())
                    .map(|u| u.to_string());
                self.view = None;
                self.context = None;
                self.native_window = None;
            }
            Command::Resize(width, height) => {
                self.size = PhysicalSize::new(width.max(1), height.max(1));
                if let Some(view) = &self.view {
                    view.resize(self.size);
                }
            }
            Command::Tick => self.paint(),
            Command::Touch(kind, id, x, y) => {
                if let Some(view) = &self.view {
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
            Command::Load(url) => self.load(url),
            Command::LoadData { data, base_url } => {
                let encoded = base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    data.as_bytes(),
                );
                let url = format!("data:text/html;charset=utf-8;base64,{encoded}#{base_url}");
                self.load(url);
            }
            Command::Exec(script) => {
                if let Some(view) = &self.view {
                    view.evaluate_javascript(script, |_| {});
                }
            }
            Command::CurrentUrl(reply) => {
                let _ = reply.send(
                    self.view
                        .as_ref()
                        .and_then(|view| view.url())
                        .map(|u| u.to_string()),
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
                let storage_types =
                    StorageType::Cookies | StorageType::Local | StorageType::Session;
                let manager = self.servo.site_data_manager();
                let sites = manager
                    .site_data(storage_types)
                    .into_iter()
                    .map(|site| site.name())
                    .collect::<Vec<_>>();
                let site_refs = sites.iter().map(String::as_str).collect::<Vec<_>>();
                manager.clear_site_data(&site_refs, storage_types);
                // This also covers cookies whose hosts Servo cannot reduce to
                // a registered domain (for example localhost and IP hosts).
                manager.clear_cookies(None);
                self.servo.network_manager().clear_cache();
            }
            Command::SetUserAgent(user_agent) => {
                let value = match user_agent {
                    UserAgentOverride::Default => String::new(),
                    UserAgentOverride::Custom(value) => value,
                };
                self.servo
                    .set_preference("user_agent", PrefValue::Str(value));
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
                let _ = reply.send(self.list_cookies());
            }
            Command::SetCookie(request, reply) => {
                let _ = reply.send(self.set_cookie(request));
            }
            Command::DeleteCookie {
                name,
                domain,
                path,
                reply,
            } => {
                let _ = reply.send(self.delete_cookie(&name, &domain, &path));
            }
            Command::ClearCookies(reply) => {
                self.servo.site_data_manager().clear_cookies(None);
                let _ = reply.send(());
            }
            Command::ClearSiteData {
                url,
                options,
                reply,
            } => {
                let _ = reply.send(self.clear_site_data(&url, options));
            }
            Command::Screenshot(reply) => {
                if let Some(view) = &self.view {
                    view.take_screenshot(None, move |result| {
                        let result = result
                            .map_err(|error| format!("Servo screenshot failed: {error:?}"))
                            .and_then(encode_png);
                        let _ = reply.send(result);
                    });
                } else {
                    let _ = reply.send(Err("Servo surface is not ready".into()));
                }
            }
            Command::Proxy(config) => {
                let (http, https, bypass) = config
                    .map(|config| {
                        let proxy = format!("http://{}:{}", config.host, config.port);
                        (proxy.clone(), proxy, config.bypass.join(","))
                    })
                    .unwrap_or_default();
                self.servo
                    .set_preference("network_http_proxy_uri", PrefValue::Str(http));
                self.servo
                    .set_preference("network_https_proxy_uri", PrefValue::Str(https));
                self.servo
                    .set_preference("network_http_no_proxy", PrefValue::Str(bypass));
            }
            Command::Shutdown => {
                self.view = None;
                self.context = None;
                self.native_window = None;
            }
        }
    }

    fn create_surface(&mut self, native_window: usize, width: u32, height: u32, density: f32) {
        let Some(native_window) = NonNull::new(native_window as *mut libc::c_void) else {
            log::error!("Servo received a null ANativeWindow for {}", self.webtag);
            return;
        };
        self.size = PhysicalSize::new(width.max(1), height.max(1));
        self.density = density.max(0.1);
        let raw_display = RawDisplayHandle::Android(AndroidDisplayHandle::new());
        let raw_window = RawWindowHandle::AndroidNdk(AndroidNdkWindowHandle::new(native_window));
        let display = unsafe { DisplayHandle::borrow_raw(raw_display) };
        let window = unsafe { WindowHandle::borrow_raw(raw_window) };
        let context = match WindowRenderingContext::new(display, window, self.size) {
            Ok(context) => Rc::new(context) as Rc<dyn RenderingContext>,
            Err(error) => {
                log::error!(
                    "Failed to create Servo EGL context for {}: {error:?}",
                    self.webtag
                );
                unsafe { ANativeWindow_release(native_window.as_ptr()) };
                return;
            }
        };
        if let Err(error) = context.make_current() {
            log::error!("Failed to make Servo EGL context current: {error:?}");
        }

        let content = Rc::new(UserContentManager::new(&self.servo));
        content.add_script(Rc::new(UserScript::from(bridge_script(&self.webtag))));
        let delegate = Rc::new(Delegate {
            webtag: self.webtag.clone(),
            capture: self.capture.clone(),
        });
        let initial_url = self
            .pending_load
            .take()
            .and_then(|url| Url::parse(&url).ok())
            .unwrap_or_else(|| Url::parse("about:blank").unwrap());
        let view = WebViewBuilder::new(&self.servo, context.clone())
            .delegate(delegate)
            .user_content_manager(content)
            .hidpi_scale_factor(Scale::new(self.density))
            .url(initial_url)
            .build();
        self.native_window = Some(NativeWindow(native_window));
        self.context = Some(context);
        self.view = Some(view);
    }

    fn load(&mut self, url: String) {
        let Ok(url) = Url::parse(&url) else {
            log::error!("Servo rejected invalid URL for {}: {url}", self.webtag);
            return;
        };
        if let Some(view) = &self.view {
            view.load(url);
        } else {
            self.pending_load = Some(url.to_string());
        }
    }

    fn paint(&self) {
        let (Some(view), Some(context)) = (&self.view, &self.context) else {
            return;
        };
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

    fn list_cookies(&self) -> Result<Vec<WebViewCookie>, String> {
        let url = self.current_http_url()?;
        Ok(self
            .servo
            .site_data_manager()
            .cookies_for_url(url, CookieSource::HTTP)
            .into_iter()
            .map(cookie_from_servo)
            .collect())
    }

    fn set_cookie(&self, request: WebViewCookieSetRequest) -> Result<(), String> {
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
        self.servo
            .site_data_manager()
            .set_cookie_for_url(url, builder.build().into_owned(), None);
        Ok(())
    }

    fn delete_cookie(&self, name: &str, domain: &str, path: &str) -> Result<(), String> {
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
        self.servo
            .site_data_manager()
            .set_cookie_for_url(url, cookie, None);
        Ok(())
    }

    fn clear_site_data(
        &self,
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
            let manager = self.servo.site_data_manager();
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

fn encode_png(image: servo::RgbaImage) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, image.width(), image.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    writer
        .write_image_data(image.as_raw())
        .map_err(|error| error.to_string())?;
    writer.finish().map_err(|error| error.to_string())?;
    Ok(bytes)
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

struct Delegate {
    webtag: WebTag,
    capture: Arc<Mutex<CaptureState>>,
}

impl WebViewDelegate for Delegate {
    fn notify_url_changed(&self, webview: WebView, url: Url) {
        normalizer::submit(
            &self.webtag,
            NativeSignal::LocationChanged {
                url: url.to_string(),
            },
        );
        normalizer::submit(
            &self.webtag,
            NativeSignal::BackForwardChanged {
                can_go_back: webview.can_go_back(),
                can_go_forward: webview.can_go_forward(),
            },
        );
    }

    fn notify_page_title_changed(&self, _webview: WebView, title: Option<String>) {
        normalizer::submit(&self.webtag, NativeSignal::TitleChanged { title });
    }

    fn notify_load_status_changed(&self, webview: WebView, status: LoadStatus) {
        let url = webview
            .url()
            .map(|url| url.to_string())
            .unwrap_or_else(|| "about:blank".into());
        match status {
            LoadStatus::Started => normalizer::submit(
                &self.webtag,
                NativeSignal::NavigationStarted { key: None, url },
            ),
            LoadStatus::HeadParsed => {
                normalizer::submit(&self.webtag, NativeSignal::DocumentCommitted)
            }
            LoadStatus::Complete => normalizer::submit(
                &self.webtag,
                NativeSignal::NavigationFinished {
                    key: None,
                    result: NativeNavigationResult::Succeeded { final_url: url },
                },
            ),
        }
    }

    fn notify_new_frame_ready(&self, webview: WebView) {
        webview.paint();
    }

    fn request_navigation(&self, _webview: WebView, request: servo::NavigationRequest) {
        let navigation = NavigationRequest::new(request.url.to_string(), false, true);
        if find_webview(&self.webtag).is_some_and(|webview| {
            webview.handle_navigation(&navigation) == NavigationPolicy::Cancel
        }) {
            request.deny();
        } else {
            request.allow();
        }
    }

    fn show_console_message(&self, _webview: WebView, level: ConsoleLogLevel, message: String) {
        let level = match level {
            ConsoleLogLevel::Trace => LogLevel::Verbose,
            ConsoleLogLevel::Debug | ConsoleLogLevel::Dir => LogLevel::Debug,
            ConsoleLogLevel::Log | ConsoleLogLevel::Info => LogLevel::Info,
            ConsoleLogLevel::Warn => LogLevel::Warn,
            ConsoleLogLevel::Error => LogLevel::Error,
        };
        if let Some(delegate) = find_webview_delegate(&self.webtag) {
            delegate.log(level, &message);
        }
    }

    fn load_web_resource(&self, _webview: WebView, load: servo::WebResourceLoad) {
        let request = load.request();
        let mut capture = self.capture.lock().unwrap_or_else(|e| e.into_inner());
        if !capture.enabled {
            return;
        }
        let request_id = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed).to_string();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        let entry = NetworkEntry {
            request_id,
            url: request.url.to_string(),
            method: request.method.to_string(),
            resource_type: Some(format!("{:?}", request.destination).to_ascii_lowercase()),
            request_headers: request
                .headers
                .iter()
                .map(|(name, value)| {
                    (
                        name.to_string(),
                        value.to_str().unwrap_or_default().to_string(),
                    )
                })
                .collect(),
            request_body: None,
            status: None,
            response_headers: Vec::new(),
            mime_type: None,
            response_body: NetworkBody::Skipped {
                reason: "Servo embedding API exposes request interception but no response callback"
                    .into(),
            },
            from_cache: false,
            failed: None,
            wall_time: Some(now),
            started: now,
            finished: None,
        };
        if capture.entries.len() == CAPTURE_LIMIT {
            capture.entries.pop_front();
            capture.dropped += 1;
        }
        capture.entries.push_back(entry);
    }
}

struct LxProtocolHandler(WebTag);

impl ProtocolHandler for LxProtocolHandler {
    fn load<'a>(
        &'a self,
        request: &'a mut Request,
        _done_chan: &mut DoneChannel,
        _context: &FetchContext,
    ) -> Pin<Box<dyn Future<Output = Response> + Send + 'a>> {
        let url = request.current_url();
        if url.host_str() == Some("bridge") {
            let webtag = bridge_request_webtag(request, &url);
            return Box::pin(future::ready(bridge_response(request, url, webtag)));
        }
        let webtag = request_webtag(request).or_else(|| initial_lx_webtag(&url));
        let response = webtag
            .as_ref()
            .and_then(|webtag| {
                let document = documents().lock().unwrap_or_else(|e| e.into_inner());
                let document = document.get(webtag.as_str())?;
                (document.url == url.as_str()).then(|| {
                    WebResourceResponse::bytes(document.html.clone())
                        .mime("text/html; charset=utf-8")
                })
            })
            .or_else(|| {
                webtag.as_ref().and_then(find_webview).and_then(|webview| {
                    let mut builder = http::Request::builder()
                        .method(request.method.clone())
                        .uri(url.as_str());
                    if let Some(headers) = builder.headers_mut() {
                        *headers = request.headers.clone();
                    }
                    builder
                        .body(Vec::new())
                        .ok()
                        .and_then(|request| webview.handle_scheme_request("lx", request))
                })
            })
            .map(|response| {
                lingxia_response(
                    url.clone(),
                    ResourceFetchTiming::new(request.timing_type()),
                    response,
                )
            })
            .unwrap_or_else(|| {
                Response::network_error(NetworkError::ResourceLoadError(format!(
                    "No LingXia lx:// handler for {url}"
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

fn lingxia_response(
    url: servo::ServoUrl,
    timing: ResourceFetchTiming,
    response: crate::WebResourceResponse,
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

struct BridgeProtocolHandler(WebTag);

impl ProtocolHandler for BridgeProtocolHandler {
    fn load<'a>(
        &'a self,
        request: &'a mut Request,
        _done_chan: &mut DoneChannel,
        _context: &FetchContext,
    ) -> Pin<Box<dyn Future<Output = Response> + Send + 'a>> {
        let url = request.current_url();
        let webtag = bridge_request_webtag(request, &url);
        Box::pin(future::ready(bridge_response(request, url, webtag)))
    }

    fn is_fetchable(&self) -> bool {
        true
    }
    fn is_secure(&self) -> bool {
        true
    }
}

fn bridge_request_webtag(request: &Request, url: &servo::ServoUrl) -> Option<WebTag> {
    request_webtag(request).or_else(|| {
        let query: HashMap<String, String> = url.as_url().query_pairs().into_owned().collect();
        let tag = WebTag::from(query.get("tag")?.as_str());
        runtimes()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains_key(tag.as_str())
            .then_some(tag)
    })
}

fn bridge_response(request: &Request, url: servo::ServoUrl, webtag: Option<WebTag>) -> Response {
    let query: HashMap<String, String> = url.as_url().query_pairs().into_owned().collect();
    let kind = if url.scheme() == "lx" {
        url.path().trim_matches('/')
    } else {
        url.host_str().unwrap_or_default()
    };
    match kind {
        "post" => {
            if let Some(message) = query.get("message")
                && let Some(delegate) = webtag.as_ref().and_then(find_webview_delegate)
            {
                delegate.handle_post_message(message.clone());
            }
        }
        "component" => {
            if let Some(message) = query.get("message")
                && let Some(delegate) = webtag.as_ref().and_then(find_webview_delegate)
            {
                delegate.handle_native_component_message(message.clone());
            }
        }
        "eval" => {
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

fn bridge_script(webtag: &WebTag) -> String {
    let webtag = serde_json::to_string(webtag.as_str()).unwrap_or_else(|_| "\"\"".into());
    format!(
        r#"(() => {{
      const webtag = {webtag};
      const beacons = new Set();
      const send = (kind, params) => {{
        const query = new URLSearchParams({{ ...params, tag: webtag }}).toString();
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
      globalThis.NativeComponentBridge = {{
        postMessage: message => send('component', {{ message: String(message) }})
      }};
    }})();"#
    )
}

pub(super) fn load_url(webtag: &WebTag, url: &str) -> Result<(), WebViewError> {
    send(webtag, Command::Load(url.to_string()))
}

pub(super) fn load_data(webtag: &WebTag, data: &str, base_url: &str) -> Result<(), WebViewError> {
    send(
        webtag,
        Command::LoadData {
            data: data.to_string(),
            base_url: base_url.to_string(),
        },
    )
}

pub(super) fn exec_js(webtag: &WebTag, script: &str) -> Result<(), WebViewError> {
    send(webtag, Command::Exec(script.to_string()))
}

pub(super) async fn current_url(webtag: &WebTag) -> Result<Option<String>, WebViewError> {
    let (tx, rx) = oneshot::channel();
    send(webtag, Command::CurrentUrl(tx))?;
    rx.await
        .map_err(|_| WebViewError::WebView("Servo current_url was canceled".into()))
}

pub(super) fn post_message(webtag: &WebTag, message: &str) -> Result<(), WebViewError> {
    send(webtag, Command::PostMessage(message.to_string()))
}

pub(super) fn clear_browsing_data(webtag: &WebTag) -> Result<(), WebViewError> {
    send(webtag, Command::ClearBrowsingData)
}

pub(super) fn set_user_agent(
    webtag: &WebTag,
    user_agent: UserAgentOverride,
) -> Result<(), WebViewError> {
    user_agent.validate()?;
    send(webtag, Command::SetUserAgent(user_agent))
}

pub(super) fn reload(webtag: &WebTag) -> Result<(), WebViewError> {
    send(webtag, Command::Reload)
}
pub(super) fn go_back(webtag: &WebTag) -> Result<(), WebViewError> {
    send(webtag, Command::Back)
}
pub(super) fn go_forward(webtag: &WebTag) -> Result<(), WebViewError> {
    send(webtag, Command::Forward)
}

pub(super) async fn list_cookies(webtag: &WebTag) -> Result<Vec<WebViewCookie>, WebViewError> {
    let (tx, rx) = oneshot::channel();
    send(webtag, Command::ListCookies(tx))?;
    rx.await
        .map_err(|_| WebViewError::WebView("Servo list_cookies was canceled".into()))?
        .map_err(WebViewError::WebView)
}

pub(super) async fn set_cookie(
    webtag: &WebTag,
    request: WebViewCookieSetRequest,
) -> Result<(), WebViewError> {
    let (tx, rx) = oneshot::channel();
    send(webtag, Command::SetCookie(request, tx))?;
    rx.await
        .map_err(|_| WebViewError::WebView("Servo set_cookie was canceled".into()))?
        .map_err(WebViewError::WebView)
}

pub(super) async fn delete_cookie(
    webtag: &WebTag,
    name: &str,
    domain: &str,
    path: &str,
) -> Result<(), WebViewError> {
    let (tx, rx) = oneshot::channel();
    send(
        webtag,
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

pub(super) async fn clear_cookies(webtag: &WebTag) -> Result<(), WebViewError> {
    let (tx, rx) = oneshot::channel();
    send(webtag, Command::ClearCookies(tx))?;
    rx.await
        .map_err(|_| WebViewError::WebView("Servo clear_cookies was canceled".into()))
}

pub(super) async fn clear_site_data(
    webtag: &WebTag,
    url: &str,
    options: ClearSiteDataOptions,
) -> Result<ClearSiteDataResult, WebViewError> {
    let (tx, rx) = oneshot::channel();
    send(
        webtag,
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

pub(super) async fn take_screenshot(webtag: &WebTag) -> Result<Vec<u8>, WebViewError> {
    let (tx, rx) = oneshot::channel();
    send(webtag, Command::Screenshot(tx))?;
    rx.await
        .map_err(|_| WebViewError::WebView("Servo screenshot was canceled".into()))?
        .map_err(WebViewError::WebView)
}

pub(super) fn apply_http_proxy(
    config: Option<&ProxyConfig>,
) -> Result<ProxyApplyReport, WebViewError> {
    let runtimes = runtimes().lock().unwrap_or_else(|e| e.into_inner());
    for runtime in runtimes.values() {
        runtime
            .tx
            .send(Command::Proxy(config.cloned()))
            .map_err(|_| WebViewError::WebView("Servo proxy update failed".into()))?;
    }
    Ok(if config.is_some() {
        ProxyApplyReport::applied(ProxyActivation::EffectiveNow)
    } else {
        ProxyApplyReport::cleared(ProxyActivation::EffectiveNow)
    })
}

fn capture(webtag: &WebTag) -> Result<Arc<Mutex<CaptureState>>, WebViewError> {
    runtimes()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(webtag.as_str())
        .map(|runtime| runtime.capture.clone())
        .ok_or_else(|| WebViewError::WebView(format!("Servo backend is not ready for {webtag}")))
}

pub(super) async fn start_network_capture(webtag: &WebTag) -> Result<(), WebViewError> {
    capture(webtag)?
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .enabled = true;
    Ok(())
}

pub(super) async fn stop_network_capture(webtag: &WebTag) -> Result<(), WebViewError> {
    capture(webtag)?
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .enabled = false;
    Ok(())
}

pub(super) async fn network_entries(
    webtag: &WebTag,
) -> Result<NetworkCaptureSnapshot, WebViewError> {
    let capture = capture(webtag)?;
    let capture = capture.lock().unwrap_or_else(|e| e.into_inner());
    Ok(NetworkCaptureSnapshot {
        entries: capture.entries.iter().cloned().collect(),
        dropped: capture.dropped,
    })
}

pub(super) async fn clear_network_capture(webtag: &WebTag) -> Result<(), WebViewError> {
    let capture = capture(webtag)?;
    let mut capture = capture.lock().unwrap_or_else(|e| e.into_inner());
    capture.entries.clear();
    capture.dropped = 0;
    Ok(())
}

pub(super) fn wheel(webtag: &WebTag, dx: f64, dy: f64) -> Result<(), WebViewError> {
    send(webtag, Command::Wheel(dx, dy))
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

fn jstring(env: &mut jni::Env<'_>, value: JString<'_>) -> Result<String, jni::errors::Error> {
    value.try_to_string(env)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaServoView_nativeSurfaceCreated(
    mut env: EnvUnowned,
    _this: JObject,
    tag: JString,
    surface: JObject,
    width: jint,
    height: jint,
    density: jfloat,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let tag = jstring(env, tag)?;
        let tag = WebTag::from(tag.as_str());
        let window = unsafe { ANativeWindow_fromSurface(env.get_raw(), surface.as_raw()) };
        if let Some(runtime) = runtimes()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(tag.as_str())
        {
            let _ = runtime.tx.send(Command::SurfaceCreated {
                native_window: window as usize,
                width: width.max(1) as u32,
                height: height.max(1) as u32,
                density,
            });
        } else if !window.is_null() {
            unsafe { ANativeWindow_release(window) };
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
    width: jint,
    height: jint,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let tag = jstring(env, tag)?;
        let tag = WebTag::from(tag.as_str());
        let _ = send(
            &tag,
            Command::Resize(width.max(1) as u32, height.max(1) as u32),
        );
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaServoView_nativeSurfaceDestroyed(
    mut env: EnvUnowned,
    _this: JObject,
    tag: JString,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let tag = jstring(env, tag)?;
        let tag = WebTag::from(tag.as_str());
        let _ = send(&tag, Command::SurfaceDestroyed);
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaServoView_nativeFrame(
    mut env: EnvUnowned,
    _this: JObject,
    tag: JString,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let tag = jstring(env, tag)?;
        let tag = WebTag::from(tag.as_str());
        let _ = send(&tag, Command::Tick);
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaServoView_nativeTouch(
    mut env: EnvUnowned,
    _this: JObject,
    tag: JString,
    action: jint,
    id: jint,
    x: jfloat,
    y: jfloat,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let tag = jstring(env, tag)?;
        let tag = WebTag::from(tag.as_str());
        if let Some(kind) = touch_kind(action) {
            let _ = send(&tag, Command::Touch(kind, id, x, y));
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
    dx: f64,
    dy: f64,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let tag = jstring(env, tag)?;
        let tag = WebTag::from(tag.as_str());
        let _ = wheel(&tag, dx, dy);
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}
