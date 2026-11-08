//! `PlatformAdapter` over a page's content window. Frames present through wgpu on the window,
//! or rasterize on the CPU pool and reach it as pixels for `WM_PAINT`.
use crate::Shell;
use crate::dispatch::Dispatcher;
use lingxia_view::DisplayList;
use lingxia_view::interaction::view::{AccessibilityEffect, Dispatch, Effect};
use lingxia_view::platform::*;
use lingxia_view::renderer::pool::Mailbox;
use lingxia_view::renderer::surfaces::{self, Route, Target};
use lingxia_view::session::host::{Failure, FrameSnapshot, FrameTiming, WeakViewHost};
use lingxia_view::session::lifecycle::{CommitTicket, ConnectionTicket, SurfaceTicket};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Windows' default long-press delay for a held mouse button.
const LONG_PRESS: Duration = Duration::from_millis(500);
/// Fling travel `LINEAR·v + QUADRATIC·v²` (logical px, px/s), the curve the HarmonyOS adapter
/// fitted: a faster flick travels disproportionately further.
const FLING_LINEAR: f32 = 0.184;
const FLING_QUADRATIC: f32 = 1.49e-4;
/// A fling that would move less than this many device pixels in a frame stops.
const FLING_STOP: f32 = 0.5;
/// The shortest frame a fling step assumes, so a quick step does not end a fling early.
const FLING_FRAME: f32 = 1.0 / 120.0;

struct Fling {
    region: String,
    velocity: f32,
    /// Exponential decay per second, chosen so the whole fling covers the fitted travel.
    decay: f32,
    last: Instant,
}

/// A CPU frame for `WM_PAINT`: BGRA rows, top-down.
pub(crate) struct CpuFrame {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

pub struct WindowsAdapter {
    pub(crate) token: i64,
    pub(crate) shell: Arc<dyn Shell>,
    pub(crate) dispatcher: Dispatcher,
    /// The content window, once made.
    pub(crate) hwnd: AtomicIsize,
    started: Instant,
    pub(crate) host: OnceLock<WeakViewHost<WindowsAdapter>>,
    /// The window's latest measurement; `None` until it has a size.
    pub(crate) facts: Mutex<Option<DeviceFacts>>,
    raster: OnceLock<Arc<Mailbox<(DisplayList, CommitTicket)>>>,
    /// The attached GPU presenter, or 0.
    pub(crate) gpu: AtomicU64,
    pub(crate) target: Mutex<Target>,
    pub(crate) cpu_frame: Mutex<Option<CpuFrame>>,
    images: Mutex<HashMap<u64, Box<dyn Send>>>,
    /// Bumped to disarm a pending long press.
    long_press: AtomicU64,
    /// A dialog of the top layer is open.
    modal: AtomicBool,
    /// Bumped for every text menu shown or closed: a popup returning for an older one is stale.
    pub(crate) menu: AtomicU64,
    pub(crate) ime: crate::ime::Ime,
    fling: Mutex<Option<Fling>>,
    /// The nodes showing press (0) and focus-ring (1) feedback, drawn into each frame: a
    /// window over the swap chain cannot blend with it.
    highlights: Mutex<[Option<usize>; 2]>,
    /// The presented frame's feedback colours.
    feedback: Mutex<Option<lingxia_view::interaction::chrome::FeedbackStyle>>,
}

impl WindowsAdapter {
    pub(crate) fn new(token: i64, shell: Arc<dyn Shell>, dispatcher: Dispatcher) -> Self {
        Self {
            token,
            shell,
            dispatcher,
            hwnd: AtomicIsize::new(0),
            started: Instant::now(),
            host: OnceLock::new(),
            facts: Mutex::new(None),
            raster: OnceLock::new(),
            gpu: AtomicU64::new(0),
            // CPU frames until the window attaches a presenter.
            target: Mutex::new(Target::Unavailable),
            cpu_frame: Mutex::new(None),
            images: Mutex::new(HashMap::new()),
            long_press: AtomicU64::new(0),
            modal: AtomicBool::new(false),
            menu: AtomicU64::new(0),
            ime: Default::default(),
            fling: Mutex::new(None),
            highlights: Mutex::new([None; 2]),
            feedback: Mutex::new(None),
        }
    }

    fn route(&self) -> Route {
        let target = *self.target.lock().unwrap_or_else(|p| p.into_inner());
        surfaces::route(self.gpu.load(Ordering::Acquire), target)
    }

    fn images(&self) -> std::sync::MutexGuard<'_, HashMap<u64, Box<dyn Send>>> {
        self.images.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn scale(&self) -> f32 {
        self.facts
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .map_or(1.0, |facts| facts.scale)
    }

    /// The vsync key fling steps use, apart from the host's own frame requests.
    pub(crate) fn fling_key(&self) -> i64 {
        !self.token
    }

    /// Step the fling at the next display refresh.
    fn fling_frame(&self) {
        let Some(host) = self.weak() else {
            return;
        };
        crate::vsync::request(
            self.fling_key(),
            self.dispatcher,
            Box::new(move |_, _| {
                if let Some(host) = host.upgrade()
                    && host.adapter().step_fling(&host)
                {
                    host.adapter().fling_frame();
                }
            }),
        );
    }

    /// Move the flung region by one frame of its decaying velocity; false once it stops.
    fn step_fling(&self, host: &lingxia_view::session::host::ViewHost<WindowsAdapter>) -> bool {
        let (region, delta) = {
            let mut fling = self.fling.lock().unwrap_or_else(|p| p.into_inner());
            let Some(state) = fling.as_mut() else {
                return false;
            };
            let now = Instant::now();
            let dt = now.duration_since(state.last).as_secs_f32();
            state.last = now;
            let delta = state.velocity * dt;
            state.velocity *= (-state.decay * dt).exp();
            let region = state.region.clone();
            if state.velocity.abs() * dt.max(FLING_FRAME) < FLING_STOP {
                *fling = None;
            }
            (region, delta)
        };
        let moved = host.scroll_by(&region, delta);
        let mut fling = self.fling.lock().unwrap_or_else(|p| p.into_inner());
        if !moved {
            *fling = None;
        }
        fling.is_some()
    }

    fn weak(&self) -> Option<WeakViewHost<WindowsAdapter>> {
        self.host.get().cloned()
    }
}

pub(crate) fn log(message: &str) {
    log::info!(target: "lingxia_view", "{message}");
}

pub enum Frame {
    /// Drawn on the render thread while the worker goes on; presented there too.
    Gpu {
        surface: u64,
        revision: u64,
        drawn: Arc<lingxia_view::renderer::render_thread::Drawn>,
    },
    /// Rasterized off the View worker when presented.
    Cpu(DisplayList, CommitTicket),
}

impl WindowsAdapter {
    fn highlight(&self, which: usize, node: Option<usize>) {
        let changed = {
            let mut highlights = self.highlights.lock().unwrap_or_else(|p| p.into_inner());
            std::mem::replace(&mut highlights[which], node) != node
        };
        if changed && let Some(host) = self.weak() {
            // Effects arrive while the core delivers them; redraw once it is done.
            self.dispatcher.post(Box::new(move || {
                if let Some(host) = host.upgrade() {
                    host.redraw();
                }
            }));
        }
    }

    /// Press and focus-ring feedback over the page, following its node into this frame.
    fn decorate(&self, display: &mut DisplayList) {
        use lingxia_view::interaction::chrome::{ChromeNode, feedback_shape};
        let highlights = *self.highlights.lock().unwrap_or_else(|p| p.into_inner());
        if highlights.iter().all(Option::is_none) {
            return;
        }
        let Some(style) = *self.feedback.lock().unwrap_or_else(|p| p.into_inner()) else {
            return;
        };
        for (which, node) in highlights.into_iter().enumerate() {
            let shape = node
                .and_then(|index| serde_json::to_string(display.nodes.get(index)?).ok())
                .and_then(|json| feedback_shape(&ChromeNode::parse(&json).ok()?));
            let Some(shape) = shape else { continue };
            let (color, ring) = if which == 0 {
                (style.press_overlay, 0.0)
            } else {
                (style.focus_ring, style.focus_ring_width)
            };
            display.push_feedback(
                if which == 0 {
                    "lingxia:press"
                } else {
                    "lingxia:focus"
                },
                &shape,
                color,
                ring,
            );
        }
    }
}

impl Presenter for WindowsAdapter {
    type Frame = Frame;

    fn prepare(
        &self,
        mut display: DisplayList,
        _: SurfaceTicket,
        generation: CommitTicket,
    ) -> Result<Option<Frame>, String> {
        use lingxia_view::renderer::render_thread;
        self.decorate(&mut display);
        Ok(match self.route() {
            Route::Wait => None,
            Route::Gpu(surface) => Some(Frame::Gpu {
                surface,
                revision: generation.get(),
                drawn: render_thread::draw(
                    render_thread::SURFACES,
                    surface,
                    generation.get(),
                    display,
                ),
            }),
            Route::Cpu => Some(Frame::Cpu(display, generation)),
        })
    }

    fn metrics(&self, frame: &Frame) -> Option<serde_json::Value> {
        Some(match frame {
            Frame::Gpu { .. } => {
                serde_json::json!({"renderer": "sparse-strips", "renderThread": true})
            }
            Frame::Cpu(..) => serde_json::json!({"backend": "vello-cpu"}),
        })
    }

    fn present(&self, frame: Frame) -> Result<bool, String> {
        let Some(host) = self.weak() else {
            return Ok(false);
        };
        match frame {
            Frame::Gpu {
                surface,
                revision,
                drawn,
            } => {
                lingxia_view::renderer::render_thread::present(
                    surface,
                    revision,
                    drawn,
                    move |shown, gpu| {
                        let Some(host) = host.upgrade() else { return };
                        match shown {
                            Ok(shown) => {
                                host.deferred_drawn(CommitTicket::new(revision), shown, gpu)
                            }
                            Err(error) => {
                                host.deferred_presented(CommitTicket::new(revision), false);
                                host.fail(error);
                            }
                        }
                    },
                );
            }
            Frame::Cpu(display, generation) => {
                self.raster
                    .get_or_init(|| {
                        Mailbox::new(move |(display, generation)| {
                            rasterize(&host, display, generation)
                        })
                    })
                    .submit((display, generation));
            }
        }
        Ok(true)
    }

    fn deferred(&self, _: &Frame) -> bool {
        true
    }
}

/// Raster thread; the pixels reach the window on its thread. Every frame is reported once,
/// shown or dropped; the host re-prepares a dropped one.
fn rasterize(host: &WeakViewHost<WindowsAdapter>, display: DisplayList, generation: CommitTicket) {
    let Some(strong) = host.upgrade() else {
        return;
    };
    let frame = match display.rasterize() {
        Ok(frame) => frame,
        Err(error) => {
            strong.deferred_presented(generation, false);
            return strong.fail(error);
        }
    };
    let (width, height) = (
        u32::from(frame.pixels.width()),
        u32::from(frame.pixels.height()),
    );
    let mut bgra = frame.pixels.data_as_u8_slice().to_vec();
    for pixel in bgra.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    let dispatcher = strong.adapter().dispatcher;
    drop(strong);
    let host = host.clone();
    dispatcher.post(Box::new(move || {
        let Some(host) = host.upgrade() else {
            return;
        };
        let adapter = host.adapter();
        *adapter.cpu_frame.lock().unwrap_or_else(|p| p.into_inner()) = Some(CpuFrame {
            width,
            height,
            bgra,
        });
        let shown = crate::view::repaint(adapter.hwnd.load(Ordering::Acquire));
        host.deferred_presented(generation, shown);
    }));
}

impl FrameScheduler for WindowsAdapter {
    fn request_vsync(&self) {
        let Some(host) = self.weak() else {
            return;
        };
        crate::vsync::request(
            self.token,
            self.dispatcher,
            Box::new(move |present_in, interval| {
                if let Some(host) = host.upgrade() {
                    host.vsync_at(present_in, interval);
                }
            }),
        );
    }
}

impl MainThread for WindowsAdapter {
    fn post(&self, task: Task) {
        self.dispatcher.post(task);
    }
    fn post_after(&self, delay: Duration, task: Task) {
        self.dispatcher.post_after(delay, task);
    }
}

impl EnvironmentSource for WindowsAdapter {
    fn environment(&self) -> Option<FrameEnvironment> {
        self.facts
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()?
            .environment()
    }
}

impl ImageProvider for WindowsAdapter {
    fn fetch(&self, id: u64, source: &str) {
        let Some(host) = self.weak() else {
            return;
        };
        let finished = Arc::new(AtomicBool::new(false));
        let done = {
            let finished = finished.clone();
            Box::new(move |result: Result<Vec<u8>, String>| {
                finished.store(true, Ordering::Release);
                if let Some(host) = host.upgrade() {
                    host.adapter().images().remove(&id);
                    host.image_loaded(id, result);
                }
            })
        };
        let handle = self.shell.load_image(self.token, source.into(), done);
        // A loader may finish synchronously; then there is nothing left to cancel.
        if !finished.load(Ordering::Acquire) {
            self.images().insert(id, handle);
        }
    }
    fn cancel(&self, id: u64) {
        let handle = self.images().remove(&id);
        drop(handle);
    }
}

impl AccessibilitySink for WindowsAdapter {
    fn announce(&self, events: &[AccessibilityEffect]) {
        let (hwnd, events) = (self.hwnd.load(Ordering::Acquire), events.to_vec());
        self.dispatcher
            .post(Box::new(move || crate::uia::announce(hwnd, &events)));
    }
    fn clicked(&self) {}
}

impl InputMethod for WindowsAdapter {
    fn reconcile(&self, frame: &FrameSnapshot) {
        crate::ime::reconcile(self, frame);
    }
    fn focus(&self, editor: &Dispatch, frame: &FrameSnapshot) {
        let Dispatch::Editor { node, commit, .. } = editor else {
            return;
        };
        if let Some(node) = frame.frame.nodes.get(*node) {
            crate::ime::focus(self, &node.identity, commit.get(), None);
        }
    }
    fn activation(&self) {
        crate::ime::disconnect(self);
    }
    fn reset(&self) {
        crate::ime::reset(self);
    }
    fn edit_text(&self, node: usize, text: Option<&str>, start: i32, end: i32) -> EditOutcome {
        let Some(frame) = self
            .host
            .get()
            .and_then(|h| h.upgrade())
            .and_then(|h| h.snapshot())
        else {
            return EditOutcome::NotReady;
        };
        crate::ime::edit_text(self, &frame, node, text, start, end)
    }
}

impl Feedback for WindowsAdapter {
    fn apply(&self, effects: &[Effect]) {
        for effect in effects {
            match effect {
                Effect::ArmLongPress => {
                    let armed = self.long_press.fetch_add(1, Ordering::AcqRel) + 1;
                    let Some(host) = self.weak() else { continue };
                    self.dispatcher.post_after(
                        LONG_PRESS,
                        Box::new(move || {
                            let Some(host) = host.upgrade() else { return };
                            if host.adapter().long_press.load(Ordering::Acquire) == armed {
                                host.long_press();
                            }
                        }),
                    );
                }
                Effect::CancelLongPress => {
                    self.long_press.fetch_add(1, Ordering::AcqRel);
                }
                Effect::Fling {
                    region, velocity, ..
                } => {
                    let scale = self.scale();
                    let speed = velocity.abs() / scale;
                    *self.fling.lock().unwrap_or_else(|p| p.into_inner()) = Some(Fling {
                        region: region.clone(),
                        velocity: *velocity,
                        decay: 1.0 / (FLING_LINEAR + FLING_QUADRATIC * speed),
                        last: Instant::now(),
                    });
                    self.fling_frame();
                }
                Effect::StopFling => {
                    *self.fling.lock().unwrap_or_else(|p| p.into_inner()) = None;
                }
                // A node with a pressed style repaints itself.
                Effect::Press {
                    node,
                    styled: false,
                    ..
                } => self.highlight(0, Some(*node)),
                Effect::Press { .. } | Effect::ClearPress => self.highlight(0, None),
                Effect::FocusRing { node } => self.highlight(1, Some(*node)),
                Effect::ClearFocusRing => self.highlight(1, None),
                _ => {}
            }
        }
    }
}

impl HostEvents for WindowsAdapter {
    fn loaded(&self) {}
    fn presented(&self, frame: &FrameSnapshot, timing: &FrameTiming) {
        *self.feedback.lock().unwrap_or_else(|p| p.into_inner()) = Some(frame.feedback);
        // An exiting dialog still covers the page until its exit ends.
        let open = frame.frame.nodes.iter().any(|node| {
            node.role.as_deref() == Some("dialog")
                && lingxia_view::interaction::frame::visible(node)
        });
        let modal = open || (self.modal.load(Ordering::Relaxed) && frame.metrics.ghosts > 0);
        if self.modal.swap(modal, Ordering::Relaxed) != modal {
            self.shell.top_layer(self.token, modal);
        }
        log::debug!(
            target: "lingxia_view",
            "present token={} generation={} size={}x{} workerMs={} nodes={}",
            self.token,
            frame.generation.get(),
            frame.width,
            frame.height,
            timing.worker_us / 1000,
            frame.frame.nodes.len(),
        );
        let connection = self
            .weak()
            .and_then(|host| host.upgrade())
            .map_or(0, |host| host.lifecycle().connection().get());
        self.shell.presented(self.token, connection);
    }
    fn message(&self, connection: ConnectionTicket, message: &str) {
        self.shell.message(self.token, connection.get(), message);
    }
    fn restart(&self, connection: ConnectionTicket) -> bool {
        self.shell.restart(self.token, connection.get())
    }
    fn recovery(&self, failure: Option<&Failure>) {
        if let Some(failure) = failure {
            log(&format!("recovery token={}: {}", self.token, failure.error));
        }
        self.shell.recovery(self.token, failure);
        let page = self.hwnd.load(Ordering::Acquire);
        if page == 0 {
            return;
        }
        let dark = self
            .facts
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .is_some_and(|facts| facts.dark);
        let (shell, failure) = (self.shell.clone(), failure.cloned());
        self.dispatcher.post(Box::new(move || {
            crate::recovery::show(page, failure.as_ref(), dark, &|key, fallback| {
                shell.text(key, fallback)
            });
        }));
    }
    fn bootstrap_overdue(&self, overdue: bool) {
        self.shell.bootstrap_overdue(self.token, overdue);
    }
    fn copy_text(&self, text: &str) {
        if let Err(error) = crate::clipboard::set_text(text) {
            log::warn!(target: "lingxia_view", "copy failed: {error}");
        }
    }
    fn text_menu(&self, menu: Option<&lingxia_view::session::host::TextMenu>) {
        crate::text_menu::show(self, menu);
    }
    fn host_chrome(&self, chrome: &HostChrome) {
        self.shell.host_chrome(self.token, chrome);
    }
    fn warn(&self, message: &str) {
        log::warn!(target: "lingxia_view", "{message}");
    }
    fn debug(&self, message: &str) {
        log::debug!(target: "lingxia_view", "{message}");
    }
}

impl Clock for WindowsAdapter {
    fn now(&self) -> Duration {
        self.started.elapsed()
    }
}
