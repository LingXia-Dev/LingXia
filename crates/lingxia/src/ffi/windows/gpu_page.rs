//! Windows GPU pages. Each page runs on its own UI thread with its own parent window, as a
//! WebView2 page does, and registers as native content under its webtag: the host presents,
//! moves and reparents it through the same handler it uses for WebViews.
use crate::gpu_page::{self, AutomationReply, Creation, NativeFactory, NativePage, Token};
use lingxia_view_windows::lingxia_view::scene::webview::WebViewMount;
use lingxia_view_windows::lingxia_view::session::host::{Failure, LogicPort, StaleMark};
use lingxia_view_windows::lingxia_view::session::lifecycle::ConnectionTicket;
use lingxia_view_windows::{Dispatcher, Page, Scheme, Shell};
use lingxia_webview::WebTag;
use lingxia_webview::platform::windows::{
    ParentRetirement, ParkOutcome, WindowsNativeContent, WindowsPreferredColorScheme,
    WindowsWebViewNativeView, create_native_content_parent, destroy_native_content_parent,
    preferred_color_scheme_for, register_native_content, retire_native_content_parent,
    unregister_native_content,
};
use lxapp::{GpuViewGeneration, LxAppError, PageInstance};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicIsize, AtomicU64, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use std::time::Duration;

/// How long a host waits for a page thread to capture its frame or leave a parent.
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(3);

/// A page thread, from its start until its teardown.
#[derive(Default)]
struct Slot {
    /// Set once the thread can take tasks.
    dispatcher: Option<Dispatcher>,
    /// Disposed before the thread could take tasks: it tears down as soon as it can.
    disposed: bool,
    /// What the page registered, withdrawn as soon as it is disposed.
    registered: Option<(WebTag, Arc<dyn WindowsNativeContent>)>,
}

static THREADS: LazyLock<Mutex<HashMap<Token, Slot>>> = LazyLock::new(Default::default);

fn threads() -> MutexGuard<'static, HashMap<Token, Slot>> {
    THREADS.lock().unwrap_or_else(|p| p.into_inner())
}

/// What a page thread holds.
struct Live {
    token: Token,
    page: Rc<Page>,
    webtag: WebTag,
    parent: WindowsWebViewNativeView,
    content: Arc<dyn WindowsNativeContent>,
}

thread_local! {
    static LIVE: RefCell<Option<Live>> = const { RefCell::new(None) };
}

fn with_page(token: Token, work: impl FnOnce(&Page)) {
    let page = LIVE.with(|live| {
        live.borrow()
            .as_ref()
            .filter(|live| live.token == token)
            .map(|live| live.page.clone())
    });
    if let Some(page) = page {
        work(&page);
    }
}

/// Runs `work` on the page's thread; dropped when the page is gone.
fn on_page(token: Token, work: impl FnOnce(&Page) + Send + 'static) {
    let dispatcher = threads().get(&token).and_then(|slot| slot.dispatcher);
    if let Some(dispatcher) = dispatcher {
        dispatcher.post(Box::new(move || with_page(token, work)));
    }
}

fn scheme(scheme: WindowsPreferredColorScheme) -> Scheme {
    match scheme {
        WindowsPreferredColorScheme::Auto => Scheme::Auto,
        WindowsPreferredColorScheme::Light => Scheme::Light,
        WindowsPreferredColorScheme::Dark => Scheme::Dark,
    }
}

/// The page thread: make the page, pump its messages, tear it down.
fn run(token: Token, webtag: WebTag, request: Creation) {
    let made = (|| -> Result<(LogicPort, StaleMark), String> {
        let dispatcher = Dispatcher::current()?;
        let parent = create_native_content_parent(&webtag).map_err(|e| e.to_string())?;
        let page = match Page::new(
            token,
            Arc::new(Bridge),
            &request.worker,
            request.logic,
            parent.window,
            scheme(preferred_color_scheme_for(&webtag)),
        ) {
            Ok(page) => page,
            Err(error) => {
                destroy_native_content_parent(webtag.key(), parent);
                return Err(error);
            }
        };
        let ports = {
            let host = page.host();
            host.load(request.source);
            (host.logic_port(), host.stale_mark())
        };
        let content: Arc<dyn WindowsNativeContent> = Arc::new(Content {
            token,
            window: page.hwnd(),
            parent: parent.window,
            current: AtomicIsize::new(parent.window),
            scale: AtomicU64::new(0),
        });
        register_native_content(&webtag, content.clone());
        LIVE.with(|live| {
            *live.borrow_mut() = Some(Live {
                token,
                page,
                webtag,
                parent,
                content,
            })
        });
        let disposed = {
            let mut threads = threads();
            let slot = threads.entry(token).or_default();
            slot.dispatcher = Some(dispatcher);
            if !slot.disposed {
                slot.registered = LIVE.with(|live| {
                    live.borrow()
                        .as_ref()
                        .map(|live| (live.webtag.clone(), live.content.clone()))
                });
            }
            slot.disposed
        };
        if disposed {
            teardown();
        }
        Ok(ports)
    })();
    match made {
        Ok((port, stale)) => {
            gpu_page::created(token, Ok(Arc::new(WindowsPage { token, port, stale })))
        }
        Err(error) => {
            threads().remove(&token);
            return gpu_page::created(token, Err(error));
        }
    }
    lingxia_view_windows::run_message_loop();
}

/// Drop the page and its windows, then end the thread's loop. On the page thread.
fn teardown() {
    let Some(live) = LIVE.with(|live| live.borrow_mut().take()) else {
        return;
    };
    threads().remove(&live.token);
    unregister_native_content(&live.webtag, &live.content);
    drop(live.page);
    let key = live.webtag.key().to_owned();
    let parent = live.parent;
    let finish = move || {
        destroy_native_content_parent(&key, parent);
        lingxia_view_windows::quit_message_loop();
    };
    let Ok(dispatcher) = Dispatcher::current() else {
        return finish();
    };
    // Other pages the host presented into this parent move out first; that may wait on this
    // thread, which keeps pumping meanwhile.
    std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + RETIRE_DEADLINE;
        loop {
            match retire_native_content_parent(parent) {
                Ok(ParentRetirement::Ready) => break,
                Ok(ParentRetirement::Deferred) => {}
                Err(error) => {
                    log::warn!("GPU page parent retirement could not park pages: {error}")
                }
            }
            if std::time::Instant::now() >= deadline {
                log::warn!("GPU page parent retirement timed out; destroying the parent anyway");
                break;
            }
            std::thread::sleep(Duration::from_millis(16));
        }
        dispatcher.post(Box::new(finish));
    });
}

const RETIRE_DEADLINE: Duration = Duration::from_secs(10);

/// The host's handle on a page: every call lands on the page thread.
struct Content {
    token: Token,
    /// The page's own window, which takes its input.
    window: isize,
    parent: isize,
    /// The window the content sits in now, or is moving to.
    current: AtomicIsize,
    /// `f64` bits of the rasterization scale; 0 while the window's DPI applies.
    scale: AtomicU64,
}

impl Content {
    fn post(&self, work: impl FnOnce(&Page) + Send + 'static) {
        on_page(self.token, work);
    }
}

type WebResult<T> = Result<T, lingxia_webview::WebViewError>;

impl WindowsNativeContent for Content {
    fn native_view(&self) -> WindowsWebViewNativeView {
        WindowsWebViewNativeView {
            window: self.parent,
        }
    }

    fn has_parent_window(&self, window: isize) -> bool {
        self.current.load(Ordering::Acquire) == window
    }

    fn content_window(&self) -> Option<isize> {
        Some(self.window)
    }

    fn content_scale(&self) -> Option<f64> {
        Some(f64::from_bits(self.scale.load(Ordering::Acquire))).filter(|scale| *scale > 0.0)
    }

    fn viewport(&self) -> Option<[f64; 2]> {
        lingxia_view_windows::simulated_viewport().map(|size| size.map(f64::from))
    }

    fn set_content_geometry(
        &self,
        left: i32,
        top: i32,
        width: i32,
        height: i32,
        corner_radii: [i32; 4],
    ) -> WebResult<()> {
        self.post(move |page| page.set_bounds(left, top, width, height, corner_radii));
        Ok(())
    }

    fn set_content_visible(&self, visible: bool) -> WebResult<()> {
        self.post(move |page| page.set_visible(visible));
        Ok(())
    }

    fn bring_content_to_front(&self) -> WebResult<()> {
        self.post(|page| page.bring_to_front());
        Ok(())
    }

    fn move_focus(&self) -> WebResult<()> {
        self.post(|page| page.focus());
        Ok(())
    }

    fn set_parent_window(&self, window: isize) -> WebResult<()> {
        self.current.store(window, Ordering::Release);
        self.post(move |page| page.set_parent(window));
        Ok(())
    }

    fn park_from_parent(&self, expected: isize, window: isize) -> WebResult<ParkOutcome> {
        let claimed = self
            .current
            .compare_exchange(expected, window, Ordering::AcqRel, Ordering::Acquire)
            .is_ok();
        // A page leaving the screen moves to its new parent late, so it can still sit in
        // `expected` after the host moved it on.
        let target = if claimed {
            window
        } else {
            self.current.load(Ordering::Acquire)
        };
        // The host destroys the old parent once this returns: the move must be done by then.
        // It runs off the host's UI thread, which keeps pumping, so waiting cannot deadlock.
        let (tx, rx) = std::sync::mpsc::channel();
        self.post(move |page| {
            let inside = claimed || page.parent_window() == expected;
            if inside {
                page.park(target);
            }
            let _ = tx.send(inside);
        });
        Ok(match rx.recv_timeout(CAPTURE_TIMEOUT) {
            Ok(true) => ParkOutcome::Parked,
            // A page whose thread is gone holds no window.
            Ok(false) | Err(RecvTimeoutError::Disconnected) => ParkOutcome::NotInParent,
            Err(RecvTimeoutError::Timeout) => ParkOutcome::Deferred,
        })
    }

    fn set_rasterization_scale(&self, scale: f64) -> WebResult<()> {
        let kept = if scale > 0.0 { scale } else { 0.0 };
        self.scale.store(kept.to_bits(), Ordering::Release);
        let scale = (scale > 0.0).then_some(scale as f32);
        self.post(move |page| page.set_scale(scale));
        Ok(())
    }

    fn set_preferred_color_scheme(&self, preferred: WindowsPreferredColorScheme) {
        let preferred = scheme(preferred);
        self.post(move |page| page.set_scheme(preferred));
    }

    fn capture_png(&self) -> WebResult<Vec<u8>> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.post(move |page| {
            let _ = tx.send(page.screenshot_png());
        });
        let fail = |message: String| lingxia_webview::WebViewError::WebView(message);
        match rx.recv_timeout(CAPTURE_TIMEOUT) {
            Ok(Ok(Some(png))) => Ok(png),
            Ok(Ok(None)) => Err(fail("GPU page has no presented frame".into())),
            Ok(Err(error)) => Err(fail(error)),
            Err(_) => Err(fail("GPU page did not answer the capture".into())),
        }
    }
}

struct Bridge;

impl Shell for Bridge {
    fn presented(&self, token: i64, connection: u64) {
        gpu_page::presented(token, connection);
    }
    fn logic(
        &self,
        token: i64,
        connection: u64,
        messages: Vec<lingxia_view_windows::lingxia_view::session::logic::ToLogic>,
    ) {
        gpu_page::view_events(token, connection, messages);
    }
    fn state(
        &self,
        token: i64,
        connection: u64,
        signal: lingxia_view_windows::lingxia_view::session::state::StateSignal,
    ) {
        gpu_page::state_signal(token, connection, gpu_page::state::from_view(signal));
    }
    fn console(&self, token: i64, level: log::Level, message: &str) {
        gpu_page::console(token, level, message);
    }
    fn restart(&self, token: i64, connection: u64) -> bool {
        gpu_page::restart(token, connection)
    }
    fn recovery(&self, token: i64, failure: Option<&Failure>) {
        if let Some(failure) = failure {
            log::warn!("GPU page {token} failed: {}", failure.error);
        }
    }
    fn text(&self, key: &str, fallback: &str) -> String {
        lingxia_platform::i18n::text(key, fallback)
    }
    fn host_chrome(&self, _: i64, _: &lingxia_view_windows::lingxia_view::platform::HostChrome) {}
    fn top_layer(&self, token: i64, modal: bool) {
        if let Some(page) = gpu_page::page_of(token) {
            lingxia_windows_contract::set_webview_top_layer(&page.webtag(), modal);
        }
    }
    fn bars(&self, token: i64) -> [bool; 2] {
        let Some(page) = gpu_page::page_of(token) else {
            return [false, false];
        };
        let navigation = page
            .get_navbar_state()
            .is_some_and(|state| state.show_navbar);
        let tabs = lxapp::try_get(&page.appid())
            .and_then(|app| app.get_tabbar())
            .is_some_and(|tabbar| tabbar.is_visible_on(&page.path()));
        [navigation, tabs]
    }
    fn load_image(
        &self,
        token: i64,
        source: String,
        done: Box<dyn FnOnce(Result<Vec<u8>, String>) + Send>,
    ) -> Box<dyn Send> {
        Box::new(gpu_page::load_image(token, source, done))
    }
    fn video_url(&self, token: i64, source: &str) -> Result<String, String> {
        gpu_page::video_url(token, source)
    }
    fn sync_webviews(&self, token: i64, mounts: &[WebViewMount]) {
        gpu_page::webviews::sync_webviews(token, mounts, embedded::deliver, embedded::place);
    }
    fn picture_webviews(&self, token: i64, web_ids: &[String], _scale: f32) {
        embedded::picture(token, web_ids);
    }
    fn exit_webview_fullscreen(&self, token: i64, web_id: &str) {
        gpu_page::webviews::exit_webview_fullscreen(token, web_id);
    }
    fn clear_webviews(&self, token: i64) {
        gpu_page::webviews::clear_webviews(token, Some(embedded::clear));
    }
    fn can_pull_refresh(&self, token: i64) -> bool {
        let Some(page) = gpu_page::page_of(token) else {
            return false;
        };
        page.is_pull_down_refresh_enabled()
            && lingxia_platform::windows::windows_pull_refresh_indicator(page.webtag().key())
                .ok()
                .flatten()
                .is_some_and(|indicator| indicator.phase != "refreshing")
    }
    fn pull_refresh(&self, token: i64, pull: lingxia_view_windows::Pull) {
        use lingxia_platform::windows::PagePull;
        let Some(page) = gpu_page::page_of(token) else {
            return;
        };
        let pull = match pull {
            lingxia_view_windows::Pull::Moved { travel, armed } => PagePull::Moved {
                travel: f64::from(travel),
                armed,
            },
            lingxia_view_windows::Pull::Cancelled => PagePull::Cancelled,
            lingxia_view_windows::Pull::Released => PagePull::Released,
        };
        lingxia_platform::windows::report_windows_page_pull(page.webtag().key(), pull);
    }
}

/// Embedded `<webview>`s: each one's composition surface window becomes a child of its page's
/// window, which composes its frames above it and leaves it a transparent cutout.
mod embedded {
    use super::{Token, WebViewMount, on_page};
    use crate::gpu_page::webviews::child_webtag;
    use lingxia_view_windows::lingxia_view::WebViewInbound;
    use lingxia_webview::WebTag;
    use lingxia_webview::platform::windows::find_webview_handler;
    use std::collections::HashMap;
    use std::sync::mpsc::{Receiver, Sender, channel};
    use std::sync::{LazyLock, Mutex};

    pub(super) fn deliver(token: Token, web_id: String, event: WebViewInbound) {
        on_page(token, move |page| page.host().webview_event(web_id, event));
    }

    /// Where a page's web views go, in its window's pixels.
    struct Placement {
        token: Token,
        window: isize,
        views: Vec<(WebTag, [i32; 4], bool)>,
    }

    /// Placing waits on each web view's own thread, so it runs off the page's.
    static PLACER: LazyLock<Mutex<Sender<Placement>>> = LazyLock::new(|| {
        let (sender, receiver) = channel();
        let _ = std::thread::Builder::new()
            .name("lingxia-gpu-webviews".into())
            .spawn(move || run(receiver));
        Mutex::new(sender)
    });

    pub(super) fn place(token: Token, mounts: &[WebViewMount]) {
        let mounts: Vec<_> = mounts
            .iter()
            .map(|m| (m.web_id.clone(), m.bounds, !m.degraded))
            .collect();
        on_page(token, move |page| {
            let scale = page.window_scale();
            let px = |v: f32| (v * scale).round() as i32;
            let views = mounts
                .into_iter()
                .filter_map(|(web_id, [x, y, w, h], shown)| {
                    let webtag = child_webtag(token, &web_id)?;
                    Some((webtag, [px(x), px(y), px(w), px(h)], shown))
                })
                .collect();
            send(Placement {
                token,
                window: page.hwnd(),
                views,
            });
        });
    }

    pub(super) fn clear(token: Token) {
        on_page(token, move |page| {
            send(Placement {
                token,
                window: page.hwnd(),
                views: Vec::new(),
            })
        });
    }

    fn send(placement: Placement) {
        let _ = PLACER
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .send(placement);
    }

    fn run(receiver: Receiver<Placement>) {
        let mut placed: HashMap<Token, Vec<WebTag>> = HashMap::new();
        while let Ok(first) = receiver.recv() {
            // Only each page's latest placement matters.
            let mut latest = vec![first];
            while let Ok(next) = receiver.try_recv() {
                match latest.iter_mut().find(|p| p.token == next.token) {
                    Some(slot) => *slot = next,
                    None => latest.push(next),
                }
            }
            for placement in latest {
                apply(&mut placed, placement);
            }
        }
    }

    fn apply(placed: &mut HashMap<Token, Vec<WebTag>>, placement: Placement) {
        let Placement {
            token,
            window,
            views,
        } = placement;
        for gone in placed.remove(&token).unwrap_or_default() {
            if views.iter().all(|(webtag, ..)| *webtag != gone)
                && let Some(view) = find_webview_handler(&gone)
                && view.has_parent_window(window)
            {
                let _ = view.set_content_visible(false);
            }
        }
        for (webtag, [x, y, width, height], shown) in &views {
            let Some(view) = find_webview_handler(webtag) else {
                continue;
            };
            if !view.has_parent_window(window)
                && let Err(error) = view.set_parent_window(window)
            {
                log::warn!("embedded webview {webtag:?}: {error}");
                continue;
            }
            let _ = view.set_content_geometry(*x, *y, *width, *height, [0; 4], 0);
            let _ = view.set_content_visible(*shown);
        }
        lingxia_view_windows::adopt_web_surfaces(window);
        if !views.is_empty() {
            placed.insert(
                token,
                views.into_iter().map(|(webtag, ..)| webtag).collect(),
            );
        }
    }

    /// What each web view shows, for the boxes that stand in for it.
    pub(super) fn picture(token: Token, web_ids: &[String]) {
        let views: Vec<_> = web_ids
            .iter()
            .filter_map(|web_id| Some((web_id.clone(), child_webtag(token, web_id)?)))
            .collect();
        let _ = std::thread::Builder::new()
            .name("lingxia-gpu-webview-picture".into())
            .spawn(move || {
                for (web_id, webtag) in views {
                    let Some(png) = find_webview_handler(&webtag)
                        .and_then(|view| view.capture_png().ok())
                        .filter(|png| !png.is_empty())
                    else {
                        continue;
                    };
                    on_page(token, move |page| page.host().webview_picture(web_id, png));
                }
            });
    }
}

struct WindowsPage {
    token: Token,
    port: LogicPort,
    stale: StaleMark,
}

impl NativePage for WindowsPage {
    fn connect(&self, generation: GpuViewGeneration) -> Result<(), LxAppError> {
        on_page(self.token, move |page| {
            page.host().connect(ConnectionTicket::new(generation.get()))
        });
        Ok(())
    }
    fn deliver(
        &self,
        generation: GpuViewGeneration,
        message: lxapp::GpuViewMessage,
    ) -> Result<(), LxAppError> {
        self.port.deliver(
            ConnectionTicket::new(generation.get()),
            gpu_page::state::message_to_view(message),
        );
        Ok(())
    }
    fn write_state(
        &self,
        generation: GpuViewGeneration,
        write: lxapp::GpuStateWrite,
    ) -> Result<(), LxAppError> {
        self.port.write_state(
            ConnectionTicket::new(generation.get()),
            gpu_page::state::to_view(write),
        );
        Ok(())
    }
    fn request_leave(&self, generation: GpuViewGeneration, reason: &str) -> Result<(), LxAppError> {
        let reason = reason.to_owned();
        on_page(self.token, move |page| {
            page.host()
                .request_leave(ConnectionTicket::new(generation.get()), reason)
        });
        Ok(())
    }
    fn update_environment(&self, update: &serde_json::Value) -> Result<(), LxAppError> {
        let update = update.to_string();
        on_page(self.token, move |page| {
            if let Err(error) = page.host().update_environment(&update) {
                log::warn!("GPU environment update rejected: {error}");
            }
        });
        Ok(())
    }
    fn reload(&self, source: &str) -> Result<(), LxAppError> {
        // Marked before the post: a frame presented meanwhile is of the old document.
        self.stale.set();
        let source = source.to_owned();
        on_page(self.token, move |page| page.host().load(source));
        Ok(())
    }
    #[cfg(feature = "devtool")]
    fn swap_module(&self, source: &str) -> Result<(), LxAppError> {
        let source = source.to_owned();
        on_page(self.token, move |page| page.host().swap_module(source));
        Ok(())
    }
    fn park(&self) -> Result<(), LxAppError> {
        self.stale.set();
        on_page(self.token, |page| page.host().park());
        Ok(())
    }
    fn retain_surface(&self, _: bool) -> Result<(), LxAppError> {
        Ok(())
    }
    fn automation(&self, request: serde_json::Value, reply: AutomationReply) {
        let token = self.token;
        let dispatcher = threads().get(&token).and_then(|slot| slot.dispatcher);
        let Some(dispatcher) = dispatcher else {
            return reply(Err("page is not active".into()));
        };
        dispatcher.post(Box::new(move || {
            let mut answer = None;
            with_page(token, |page| {
                answer = Some(if request["op"] != "screenshot" {
                    page.host().automation_request(&request)
                } else {
                    use base64::Engine;
                    match page.screenshot_png() {
                        Ok(Some(png)) => Ok(serde_json::json!({
                            "png": base64::engine::general_purpose::STANDARD.encode(png)
                        })),
                        Ok(None) => Err("E_PAGE_NOT_READY: no presented frame".into()),
                        Err(error) => Err(error),
                    }
                });
            });
            reply(answer.unwrap_or_else(|| Err("page is not active".into())));
        }));
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct Factory;

impl NativeFactory for Factory {
    fn create(
        &self,
        token: Token,
        page: &PageInstance,
        request: Creation,
    ) -> Result<(), LxAppError> {
        let webtag = page.webtag();
        threads().insert(token, Slot::default());
        std::thread::Builder::new()
            .name(format!("lingxia-gpu-page-{token}"))
            .spawn(move || run(token, webtag, request))
            .map(|_| ())
            .map_err(|error| {
                threads().remove(&token);
                LxAppError::Runtime(format!("GPU page thread: {error}"))
            })
    }

    fn dispose(&self, token: Token) {
        let (dispatcher, registered) = {
            let mut threads = threads();
            let Some(slot) = threads.get_mut(&token) else {
                return;
            };
            if slot.dispatcher.is_none() {
                slot.disposed = true;
            }
            (slot.dispatcher, slot.registered.take())
        };
        // Withdraw now, not when the page thread gets to it: until then the host still counts
        // the page's window as a live workspace and can present the next page into it, just
        // before teardown destroys that window.
        if let Some((webtag, content)) = registered {
            unregister_native_content(&webtag, &content);
        }
        if let Some(dispatcher) = dispatcher {
            dispatcher.post(Box::new(teardown));
        }
    }
}

/// Register the GPU page factory; called once at startup.
pub(crate) fn install() {
    if lxapp::register_gpu_page_view_factory(Arc::new(gpu_page::Binding(Arc::new(Factory))))
        .is_err()
    {
        log::warn!("GPU page factory already registered");
    }
}
