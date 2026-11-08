//! Windows GPU pages. Each page runs on its own UI thread with its own parent window, as a
//! WebView2 page does, and registers as native content under its webtag: the host presents,
//! moves and reparents it through the same handler it uses for WebViews.
use crate::gpu_page::{self, AutomationReply, Creation, NativeFactory, NativePage, Token};
use lingxia_view_windows::lingxia_view::session::host::Failure;
use lingxia_view_windows::lingxia_view::session::lifecycle::ConnectionTicket;
use lingxia_view_windows::{Dispatcher, Page, Scheme, Shell};
use lingxia_webview::WebTag;
use lingxia_webview::platform::windows::{
    ParkOutcome, WindowsNativeContent, WindowsPreferredColorScheme, WindowsWebViewNativeView,
    create_native_content_parent, destroy_native_content_parent, preferred_color_scheme_for,
    register_native_content, unregister_native_content,
};
use lxapp::{GpuViewGeneration, LxAppError, PageInstance};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicIsize, Ordering};
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
    let made = (|| -> Result<(), String> {
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
        page.host().load(request.source);
        let content: Arc<dyn WindowsNativeContent> = Arc::new(Content {
            token,
            window: page.hwnd(),
            parent: parent.window,
            current: AtomicIsize::new(parent.window),
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
            slot.disposed
        };
        if disposed {
            teardown();
        }
        Ok(())
    })();
    match made {
        Ok(()) => gpu_page::created(token, Ok(Arc::new(WindowsPage { token }))),
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
    destroy_native_content_parent(live.webtag.key(), live.parent);
    lingxia_view_windows::quit_message_loop();
}

/// The host's handle on a page: every call lands on the page thread.
struct Content {
    token: Token,
    /// The page's own window, which takes its input.
    window: isize,
    parent: isize,
    /// The window the content sits in now, or is moving to.
    current: AtomicIsize,
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
        if self
            .current
            .compare_exchange(expected, window, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Ok(ParkOutcome::NotInParent);
        }
        // The host destroys the old parent once this returns: the move must be done by then.
        // It runs off the host's UI thread, which keeps pumping, so waiting cannot deadlock.
        let (tx, rx) = std::sync::mpsc::channel();
        self.post(move |page| {
            page.set_parent(window);
            let _ = tx.send(());
        });
        Ok(match rx.recv_timeout(CAPTURE_TIMEOUT) {
            Ok(()) => ParkOutcome::Parked,
            Err(_) => ParkOutcome::Deferred,
        })
    }

    fn set_rasterization_scale(&self, scale: f64) -> WebResult<()> {
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
    fn message(&self, token: i64, connection: u64, message: &str) {
        gpu_page::message(token, connection, message);
    }
    fn restart(&self, token: i64, connection: u64) -> bool {
        gpu_page::restart(token, connection)
    }
    fn recovery(&self, token: i64, failure: Option<&Failure>) {
        if let Some(failure) = failure {
            log::warn!("GPU page {token} failed: {}", failure.error);
        }
    }
    fn bootstrap_overdue(&self, token: i64, overdue: bool) {
        if overdue {
            log::warn!("GPU page {token}: Logic bootstrap overdue");
        }
    }
    fn host_chrome(&self, _: i64, _: &lingxia_view_windows::lingxia_view::platform::HostChrome) {}
    fn top_layer(&self, _: i64, _: bool) {}
    fn load_image(
        &self,
        token: i64,
        source: String,
        done: Box<dyn FnOnce(Result<Vec<u8>, String>) + Send>,
    ) -> Box<dyn Send> {
        Box::new(gpu_page::load_image(token, source, done))
    }
}

struct WindowsPage {
    token: Token,
}

impl NativePage for WindowsPage {
    fn connect(&self, generation: GpuViewGeneration, nonce: &str) -> Result<(), LxAppError> {
        let nonce = nonce.to_owned();
        on_page(self.token, move |page| {
            page.host()
                .connect(ConnectionTicket::new(generation.get()), nonce)
        });
        Ok(())
    }
    fn receive(&self, generation: GpuViewGeneration, frame: &str) -> Result<(), LxAppError> {
        let frame = frame.to_owned();
        on_page(self.token, move |page| {
            page.host()
                .receive(ConnectionTicket::new(generation.get()), frame)
        });
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
        let dispatcher = {
            let mut threads = threads();
            let Some(slot) = threads.get_mut(&token) else {
                return;
            };
            if slot.dispatcher.is_none() {
                slot.disposed = true;
            }
            slot.dispatcher
        };
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
