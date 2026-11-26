//! The Windows window overlay surface: an owned see-through popup over the top-level window that
//! holds the caller's presentation, on one overlay thread, holding a GPU page whose Logic is Rust.
use crate::feedback::Feedback;
use lingxia_view_windows::lingxia_view::platform::{
    HostChrome, OverlayAutomationReply, OverlayInput, OverlayLogic, OverlaySurface, Task, WindowId,
    WindowOverlays,
};
use lingxia_view_windows::lingxia_view::scene::webview::WebViewMount;
use lingxia_view_windows::lingxia_view::session::host::Failure;
use lingxia_view_windows::lingxia_view::session::lifecycle::ConnectionTicket;
use lingxia_view_windows::lingxia_view::session::logic::{ToLogic, ToView};
use lingxia_view_windows::lingxia_view::session::state::{StateSignal, StateWrite};
use lingxia_view_windows::{Dispatcher, Gone, Overlay, Pull, Shell};
use lingxia_webview::platform::windows::find_webview_handler;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

/// Overlays share one View worker, apart from every lxapp's.
const WORKER: &str = "lingxia:overlay";

/// Live overlays by page token, for the View worker's messages.
static OVERLAYS: LazyLock<Mutex<HashMap<i64, OverlayLogic>>> = LazyLock::new(Default::default);
/// Negative, so overlay tokens never meet lxapp page tokens.
static NEXT: AtomicI64 = AtomicI64::new(-1);
static FEEDBACK: OnceLock<Arc<Feedback>> = OnceLock::new();
/// Set once an overlay found no GPU: later opens fail at once instead of showing nothing.
static NO_GPU: OnceLock<String> = OnceLock::new();
/// The overlay thread's queue, once it runs.
static THREAD: OnceLock<Option<Dispatcher>> = OnceLock::new();

thread_local! {
    /// The overlay thread's overlays by token.
    static LIVE: RefCell<HashMap<i64, Overlay>> = RefCell::new(HashMap::new());
}

fn overlays() -> MutexGuard<'static, HashMap<i64, OverlayLogic>> {
    OVERLAYS.lock().unwrap_or_else(|p| p.into_inner())
}

/// Install the window feedback layer over Windows overlays; called once at startup.
pub(super) fn install() {
    let _ = FEEDBACK.set(crate::feedback::install(Arc::new(WinOverlays)));
}

/// The overlay thread, started on first use: it pumps messages for every overlay window.
fn thread() -> Option<Dispatcher> {
    *THREAD.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel();
        let started = std::thread::Builder::new()
            .name("lingxia-gpu-overlay".into())
            .spawn(move || {
                let dispatcher = Dispatcher::current();
                let running = dispatcher.is_ok();
                let _ = tx.send(dispatcher);
                if running {
                    lingxia_view_windows::run_message_loop();
                }
            });
        match started.map_err(|e| e.to_string()).and_then(|_| {
            rx.recv()
                .map_err(|e| e.to_string())
                .and_then(|dispatcher| dispatcher)
        }) {
            Ok(dispatcher) => Some(dispatcher),
            Err(error) => {
                log::warn!("window overlay thread: {error}");
                None
            }
        }
    })
}

fn on_overlay(token: i64, work: impl FnOnce(&Overlay) + Send + 'static) {
    if let Some(dispatcher) = thread() {
        dispatcher.post(Box::new(move || {
            LIVE.with(|live| {
                if let Some(overlay) = live.borrow().get(&token) {
                    work(overlay);
                }
            })
        }));
    }
}

/// The top-level window an lxapp's current page shows in.
fn window_of_app(appid: &str) -> Option<isize> {
    let page = lxapp::try_get(appid)?.current_page().ok()?;
    let view = find_webview_handler(&page.webtag())?;
    lingxia_view_windows::root_window(view.native_view().window)
}

fn closed(window: WindowId) {
    if let Some(feedback) = FEEDBACK.get() {
        feedback.window_closed(window);
    }
}

struct WinOverlays;

impl WindowOverlays for WinOverlays {
    fn window_of(&self, appid: Option<&str>) -> Option<WindowId> {
        appid
            .and_then(window_of_app)
            .or_else(lingxia_view_windows::active_window)
            .map(|window| WindowId(window as u64))
    }

    fn open(
        &self,
        window: WindowId,
        module: &'static str,
        logic: OverlayLogic,
    ) -> Result<Box<dyn OverlaySurface>, String> {
        if let Some(error) = NO_GPU.get() {
            return Err(format!("the window overlay needs the GPU: {error}"));
        }
        let dispatcher = thread().ok_or("no window overlay thread")?;
        let token = NEXT.fetch_sub(1, Ordering::Relaxed);
        overlays().insert(token, logic);
        dispatcher.post(Box::new(move || {
            let owner = window.0 as isize;
            let gone = Box::new(move |gone: Gone| {
                if let Gone::NoGpu(error) = gone
                    && NO_GPU.set(error.clone()).is_ok()
                {
                    log::warn!("window overlay off: no GPU presenter ({error})");
                }
                closed(window);
            });
            match Overlay::new(token, Arc::new(OverlayShell), WORKER, owner, gone) {
                Ok(overlay) => {
                    let host = overlay.page().host();
                    host.load(module.to_owned());
                    host.connect(ConnectionTicket::new(1));
                    LIVE.with(|live| live.borrow_mut().insert(token, overlay));
                }
                Err(error) => {
                    log::warn!("window overlay: {error}");
                    closed(window);
                }
            }
        }));
        Ok(Box::new(WinSurface { token }))
    }

    fn after(&self, delay: Duration, task: Task) {
        match thread() {
            Some(dispatcher) => dispatcher.post_after(delay, task),
            None => log::warn!("window overlay timer dropped: no overlay thread"),
        }
    }
}

struct WinSurface {
    token: i64,
}

impl OverlaySurface for WinSurface {
    fn write_state(&self, write: StateWrite) {
        on_overlay(self.token, move |overlay| {
            overlay
                .page()
                .host()
                .write_state(ConnectionTicket::new(1), write)
        });
    }

    fn send(&self, message: ToView) {
        on_overlay(self.token, move |overlay| {
            overlay
                .page()
                .host()
                .deliver(ConnectionTicket::new(1), message)
        });
    }

    fn set_input(&self, input: OverlayInput) {
        on_overlay(self.token, move |overlay| {
            overlay.block(input == OverlayInput::Block)
        });
    }

    fn set_focus(&self, focused: bool) {
        on_overlay(self.token, move |overlay| overlay.focus(focused));
    }

    fn automation(&self, request: serde_json::Value, reply: OverlayAutomationReply) {
        let token = self.token;
        let Some(dispatcher) = thread() else {
            return reply(Err("page is not active".into()));
        };
        dispatcher.post(Box::new(move || {
            let answer = LIVE.with(|live| {
                live.borrow()
                    .get(&token)
                    .map(|overlay| super::gpu_page::automation_answer(overlay.page(), &request))
            });
            reply(answer.unwrap_or_else(|| Err("page is not active".into())));
        }));
    }
}

impl Drop for WinSurface {
    fn drop(&mut self) {
        let token = self.token;
        overlays().remove(&token);
        if let Some(dispatcher) = thread() {
            dispatcher.post(Box::new(move || {
                let overlay = LIVE.with(|live| live.borrow_mut().remove(&token));
                drop(overlay);
            }));
        }
    }
}

/// The overlay page's host: its Logic is the layer; it has no chrome, images or web views.
struct OverlayShell;

impl Shell for OverlayShell {
    fn presented(&self, _: i64, _: u64) {}
    fn logic(&self, token: i64, _: u64, messages: Vec<ToLogic>) {
        let logic = overlays().get(&token).cloned();
        if let Some(logic) = logic {
            logic(messages);
        }
    }
    fn state(&self, _: i64, _: u64, _: StateSignal) {}
    fn restart(&self, _: i64, _: u64) -> bool {
        false
    }
    fn recovery(&self, token: i64, failure: Option<&Failure>) {
        if let Some(failure) = failure {
            log::warn!("window overlay {token} failed: {}", failure.error);
        }
    }
    fn text(&self, key: &str, fallback: &str) -> String {
        lingxia_platform::i18n::text(key, fallback)
    }
    fn host_chrome(&self, _: i64, _: &HostChrome) {}
    fn top_layer(&self, _: i64, _: bool) {}
    fn load_image(
        &self,
        _: i64,
        _: String,
        done: Box<dyn FnOnce(Result<Vec<u8>, String>) + Send>,
    ) -> Box<dyn Send> {
        // The layer's page shows no images; dropping `done` is a cancelled load.
        drop(done);
        Box::new(())
    }
    fn video_url(&self, _: i64, _: &str) -> Result<String, String> {
        Err("the window overlay plays no video".into())
    }
    fn sync_webviews(&self, _: i64, _: &[WebViewMount]) {}
    fn picture_webviews(&self, _: i64, _: &[String], _: f32) {}
    fn exit_webview_fullscreen(&self, _: i64, _: &str) {}
    fn clear_webviews(&self, _: i64) {}
    fn can_pull_refresh(&self, _: i64) -> bool {
        false
    }
    fn pull_refresh(&self, _: i64, _: Pull) {}
}
