//! `PlatformAdapter` over a page's XComponent. Frames present through wgpu GLES on
//! the node's native window, or rasterize on the CPU pool and are written into it.
use crate::xcomponent::Velocity;
use crate::{Shell, accessibility, dispatch, ime, pages, surface::NativeWindow};
use lingxia_view::DisplayList;
use lingxia_view::interaction::view::{AccessibilityEffect, Dispatch, Effect};
use lingxia_view::platform::*;
use lingxia_view::renderer::gpu::SurfaceSource;
use lingxia_view::renderer::pool::Mailbox;
use lingxia_view::renderer::surfaces::{self, Route, Target};
use lingxia_view::session::host::{Failure, FrameSnapshot, FrameTiming, WeakViewHost};
use lingxia_view::session::lifecycle::{CommitTicket, ConnectionTicket, SurfaceTicket};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

/// ArkUI's long-press delay.
const LONG_PRESS: Duration = Duration::from_millis(500);
/// Fling travel `LINEAR·v + QUADRATIC·v²` (vp, vp/s): fitted to ArkUI `List` under the same touches,
/// where a faster flick travels disproportionately further.
const FLING_LINEAR: f32 = 0.184;
const FLING_QUADRATIC: f32 = 1.49e-4;
/// A fling that would move less than this many device pixels in a frame stops.
const FLING_STOP: f32 = 0.5;
/// The shortest frame a fling step assumes, so a quick step does not end a fling early.
const FLING_FRAME: f32 = 1.0 / 120.0;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|p| p.into_inner())
}

struct Fling {
    region: String,
    velocity: f32,
    /// Exponential decay per second, chosen so the whole fling covers the fitted travel.
    decay: f32,
    last: Instant,
}

/// Display frames that have not ticked for this long have stopped.
const STALLED_MS: u64 = 250;

pub struct HarmonyAdapter {
    pub(crate) token: i64,
    pub(crate) shell: Arc<dyn Shell>,
    started: Instant,
    pub(crate) host: OnceLock<WeakViewHost<HarmonyAdapter>>,
    /// Display frames run for the videos of the kept frame.
    ticking: Arc<AtomicBool>,
    /// When the last display tick ran, ms after `started`.
    ticked: Arc<AtomicU64>,
    /// The page's latest measurement from ArkTS; the surface size overrides its extent.
    pub(crate) facts: Mutex<Option<DeviceFacts>>,
    /// The XComponent surface in device pixels, from its surface callbacks.
    size: Mutex<Option<(u32, u32)>>,
    /// The node's top-left on screen, device pixels, for accessibility bounds.
    pub(crate) origin: Mutex<[f32; 2]>,
    window: Mutex<Option<Arc<NativeWindow>>>,
    /// Bumped when the window goes: an attach queued before then must not bind it.
    windows: Arc<AtomicU64>,
    raster: OnceLock<Arc<Mailbox<(DisplayList, CommitTicket)>>>,
    /// The attached GPU presenter, or 0.
    pub(crate) gpu: AtomicU64,
    target: Mutex<Target>,
    images: Mutex<HashMap<u64, Box<dyn Send>>>,
    highlights: Mutex<[Option<usize>; 2]>,
    modal: AtomicBool,
    /// The static-text selection the handles sit on: its identity, its node as frame JSON, and
    /// the handles last sent to ArkTS.
    selection: Mutex<Option<(String, String, String)>>,
    orientation: Mutex<Option<lingxia_view::scene::video::Orientation>>,
    vsync: AtomicBool,
    long_press: AtomicU64,
    claimed: AtomicBool,
    velocity: Mutex<Velocity>,
    fling: Mutex<Option<Fling>>,
}

impl HarmonyAdapter {
    /// Handles follow a static-text selection in each presented frame, or go with it.
    fn place_handles(&self, frame: &FrameSnapshot) {
        use lingxia_view::interaction::chrome::{ChromeNode, endpoint};
        let next = frame
            .frame
            .nodes
            .iter()
            .find(|node| node.input.is_none() && node.text_selection.is_some())
            .and_then(|node| {
                let json = serde_json::to_string(node).ok()?;
                let chrome = ChromeNode::parse(&json).ok()?;
                let ends = [endpoint(&chrome, 0), endpoint(&chrome, 1)];
                let handles = serde_json::json!({
                    "identity": &*node.identity,
                    "color": frame.feedback.selection_handle,
                    "ends": ends,
                })
                .to_string();
                Some((node.identity.to_string(), json, handles))
            });
        let mut selection = lock(&self.selection);
        let sent = |s: &Option<(String, String, String)>| s.as_ref().map(|(.., h)| h.clone());
        if sent(&selection) != sent(&next) {
            let handles = sent(&next).unwrap_or_default();
            self.shell.selection_handles(self.token, &handles);
        }
        *selection = next;
    }

    /// The selection's identity and node JSON, for a handle drag.
    pub(crate) fn selection(&self) -> Option<(String, String)> {
        lock(&self.selection)
            .as_ref()
            .map(|(identity, json, _)| (identity.clone(), json.clone()))
    }

    pub(crate) fn new(token: i64, shell: Arc<dyn Shell>) -> Self {
        Self {
            token,
            shell,
            started: Instant::now(),
            host: OnceLock::new(),
            ticking: Default::default(),
            ticked: Default::default(),
            facts: Mutex::new(None),
            size: Mutex::new(None),
            origin: Mutex::new([0.0; 2]),
            window: Mutex::new(None),
            windows: Default::default(),
            raster: OnceLock::new(),
            gpu: AtomicU64::new(0),
            // Frames wait for the first attach; a failed one falls back to the CPU.
            target: Mutex::new(Target::Waiting),
            images: Mutex::new(HashMap::new()),
            highlights: Mutex::new([None; 2]),
            modal: AtomicBool::new(false),
            selection: Mutex::new(None),
            orientation: Mutex::new(None),
            vsync: AtomicBool::new(false),
            long_press: AtomicU64::new(0),
            claimed: AtomicBool::new(false),
            velocity: Mutex::new(Velocity::default()),
            fling: Mutex::new(None),
        }
    }

    /// Device pixels per virtual pixel, and the node's screen origin.
    pub(crate) fn placement(&self) -> (f32, [f32; 2]) {
        let scale = lock(&self.facts).as_ref().map_or(1.0, |f| f.scale);
        (scale, *lock(&self.origin))
    }

    /// The surface in device pixels, `(0, 0)` before it is measured.
    pub(crate) fn extent(&self) -> (f32, f32) {
        lock(&self.size).map_or((0.0, 0.0), |(w, h)| (w as f32, h as f32))
    }

    pub(crate) fn velocity(&self) -> MutexGuard<'_, Velocity> {
        lock(&self.velocity)
    }

    /// The core claimed the current gesture: ancestors (edge back, pull to refresh) lose it.
    pub(crate) fn claimed(&self) -> bool {
        self.claimed.load(Ordering::Relaxed)
    }

    /// A new touch sequence: no claim yet, a fresh velocity window.
    pub(crate) fn begin_gesture(&self) {
        self.claimed.store(false, Ordering::Relaxed);
        *lock(&self.velocity) = Velocity::default();
    }

    fn snapshot(&self) -> Option<Arc<FrameSnapshot>> {
        self.host.get()?.upgrade()?.snapshot()
    }

    pub(crate) fn surface_created(&self, host: &pages::Host, window: NativeWindow) {
        log::debug!(target: "lingxia_view", "surface created token={}", self.token);
        let window = Arc::new(window);
        *lock(&self.window) = Some(window.clone());
        if let Some((width, height)) = *lock(&self.size) {
            attach(host, window, width, height);
        }
    }

    pub(crate) fn surface_resized(&self, host: &pages::Host, width: u32, height: u32) {
        log::debug!(target: "lingxia_view", "surface {width}x{height} token={}", self.token);
        *lock(&self.size) = Some((width, height));
        if let Some(window) = lock(&self.window).clone() {
            window.set_geometry(width, height);
            // A GLES window surface keeps its creation size: a resize rebinds it.
            attach(host, window, width, height);
        }
        host.surface_changed();
    }

    fn elapsed_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    /// The page is gone: no queued attach binds its window again; returns its presenter.
    pub(crate) fn release_gpu(&self) -> u64 {
        self.windows.fetch_add(1, Ordering::AcqRel);
        self.gpu.swap(0, Ordering::AcqRel)
    }

    /// The window goes away; its size is kept, a recreated surface usually has the same.
    pub(crate) fn surface_destroyed(&self, host: &pages::Host) {
        log::debug!(target: "lingxia_view", "surface destroyed token={}", self.token);
        let gpu = self.gpu.load(Ordering::Acquire);
        self.windows.fetch_add(1, Ordering::AcqRel);
        // The window goes when this returns: release its surface on the thread that owns GL.
        crate::render::run_now(move || surfaces::suspend(gpu));
        *lock(&self.window) = None;
        host.surface_lost();
    }

    /// The node's surface came back on screen (window foregrounded, screen unlocked): its
    /// buffers may be gone, so the current frame is drawn again.
    pub(crate) fn surface_shown(&self, host: &pages::Host) {
        log::debug!(target: "lingxia_view", "surface shown token={}", self.token);
        host.window_visible(true);
        host.redraw();
    }

    pub(crate) fn surface_hidden(&self, host: &pages::Host) {
        log::debug!(target: "lingxia_view", "surface hidden token={}", self.token);
        host.window_visible(false);
    }

    /// A display frame: step any fling, run the requested vsync. `display` is the refresh
    /// this frame shows on and the refresh period, in monotonic nanoseconds.
    pub(crate) fn display_frame(&self, host: &pages::Host, display: Option<(i64, i64)>) {
        if self.step_fling(host) {
            crate::xcomponent::request_frame(self.token);
        }
        if self.vsync.swap(false, Ordering::AcqRel) {
            match display {
                Some((present, period)) => host.vsync_at(
                    (present - crate::xcomponent::monotonic_ns()) / 1000,
                    (period / 1000) as u64,
                ),
                None => host.vsync(),
            }
        }
    }

    fn step_fling(&self, host: &pages::Host) -> bool {
        let mut fling = lock(&self.fling);
        let Some(state) = fling.as_mut() else {
            return false;
        };
        let now = Instant::now();
        let dt = now.duration_since(state.last).as_secs_f32();
        state.last = now;
        let delta = state.velocity * dt;
        state.velocity *= (-state.decay * dt).exp();
        let moved = host.scroll_by(&state.region, delta);
        if !moved || state.velocity.abs() * dt.max(FLING_FRAME) < FLING_STOP {
            *fling = None;
            return false;
        }
        true
    }

    fn highlight(&self, which: usize, node: Option<usize>, frame: Option<&FrameSnapshot>) {
        lock(&self.highlights)[which] = node;
        let shape = node
            .zip(frame)
            .and_then(|(index, frame)| {
                let node = serde_json::to_string(frame.frame.nodes.get(index)?).ok()?;
                lingxia_view::interaction::chrome::feedback_shape(
                    &lingxia_view::interaction::chrome::ChromeNode::parse(&node).ok()?,
                )
            })
            .unwrap_or_default();
        let style = frame.map(|f| f.feedback);
        let (color, width) = match (which, style) {
            (0, Some(style)) => (style.press_overlay, 0.0),
            (_, Some(style)) => (style.focus_ring, style.focus_ring_width),
            (_, None) => (0, 0.0),
        };
        let flat: Vec<f32> = shape.into_iter().flatten().collect();
        self.shell
            .feedback(self.token, which == 1, &flat, color, width);
    }

    fn route(&self) -> Route {
        surfaces::route(self.gpu.load(Ordering::Acquire), *lock(&self.target))
    }
}

/// Bind `window` to a GLES presenter on the render thread; frames wait for it, then use
/// the CPU when it fails.
fn attach(host: &pages::Host, window: Arc<NativeWindow>, width: u32, height: u32) {
    let adapter = host.adapter();
    let existing = adapter.gpu.load(Ordering::Acquire);
    *lock(&adapter.target) = Target::Waiting;
    let windows = adapter.windows.clone();
    let seen = windows.load(Ordering::Acquire);
    let host = host.downgrade();
    crate::render::run(move || {
        // The window went while this waited; binding it would swap into a dead window.
        if windows.load(Ordering::Acquire) != seen {
            return;
        }
        let attached = surfaces::attach(
            SurfaceSource::window(Arc::new(SharedWindow(window))),
            width,
            height,
            existing,
        );
        dispatch::post_main(Box::new(move || {
            let Some(host) = host.upgrade() else { return };
            let adapter = host.adapter();
            *lock(&adapter.target) = match &attached {
                Ok(id) => {
                    adapter.gpu.store(*id, Ordering::Release);
                    let (record, backend) = surfaces::describe(*id).unwrap_or_default();
                    log::info!(target: "lingxia_view", "backend={backend} surface={id} adapter={record}");
                    Target::Ready
                }
                Err(error) => {
                    log::warn!(target: "lingxia_view", "GPU unavailable; using CPU renderer: {error}");
                    Target::Unavailable
                }
            };
            host.surface_changed();
        }));
    });
}

/// The retained window, shared between the CPU blit and the GLES surface.
struct SharedWindow(Arc<NativeWindow>);

impl raw_window_handle::HasWindowHandle for SharedWindow {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        self.0.window_handle()
    }
}

impl raw_window_handle::HasDisplayHandle for SharedWindow {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        self.0.display_handle()
    }
}

pub enum Frame {
    /// Drawn on the render thread while the worker goes on; presented there too.
    Gpu {
        surface: u64,
        revision: u64,
        drawn: Arc<crate::render::Drawn>,
    },
    /// Rasterized off the View worker when presented.
    Cpu(DisplayList, CommitTicket),
}

impl Presenter for HarmonyAdapter {
    type Frame = Frame;

    fn prepare(
        &self,
        display: DisplayList,
        _: SurfaceTicket,
        generation: CommitTicket,
    ) -> Result<Option<Frame>, String> {
        match self.route() {
            Route::Wait => Ok(None),
            Route::Gpu(surface) => Ok(Some(Frame::Gpu {
                surface,
                revision: generation.get(),
                drawn: crate::render::draw(
                    crate::render::SURFACES,
                    surface,
                    generation.get(),
                    display,
                ),
            })),
            Route::Cpu => Ok(Some(Frame::Cpu(display, generation))),
        }
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
        match frame {
            Frame::Gpu {
                surface,
                revision,
                drawn,
            } => {
                let Some(host) = self.host.get().cloned() else {
                    return Ok(false);
                };
                let ticking = self.ticking.clone();
                // A tick on a hidden window runs nothing and leaves `ticking` set: frames that
                // stopped that way restart on the next present.
                let stalled = self
                    .elapsed_ms()
                    .saturating_sub(self.ticked.load(Ordering::Acquire))
                    > STALLED_MS;
                crate::render::present(surface, revision, drawn, move |shown, gpu| {
                    let Some(strong) = host.upgrade() else { return };
                    // The first frame that draws video starts display frames for its players.
                    if surfaces::keeps_video(surface)
                        && (!ticking.swap(true, Ordering::AcqRel) || stalled)
                    {
                        let weak = host.clone();
                        strong.adapter().post(Box::new(move || {
                            if let Some(host) = weak.upgrade() {
                                host.tick();
                            }
                        }));
                    }
                    let host = strong;
                    match shown {
                        Ok(shown) => host.deferred_drawn(CommitTicket::new(revision), shown, gpu),
                        Err(error) => {
                            host.deferred_presented(CommitTicket::new(revision), false);
                            host.fail(error);
                        }
                    }
                });
                Ok(true)
            }
            Frame::Cpu(display, generation) => {
                let Some(host) = self.host.get().cloned() else {
                    return Ok(false);
                };
                self.raster
                    .get_or_init(|| {
                        Mailbox::new(move |(display, generation)| {
                            rasterize(&host, display, generation)
                        })
                    })
                    .submit((display, generation));
                Ok(true)
            }
        }
    }

    fn deferred(&self, _: &Frame) -> bool {
        true
    }
}

/// Raster thread: native window buffers take writes from any thread, so the frame goes
/// straight in. Every frame is reported once, shown or dropped.
fn rasterize(host: &WeakViewHost<HarmonyAdapter>, display: DisplayList, generation: CommitTicket) {
    let Some(host) = host.upgrade() else {
        return;
    };
    let frame = match display.rasterize() {
        Ok(frame) => frame,
        Err(error) => {
            host.deferred_presented(generation, false);
            return host.fail(error);
        }
    };
    let window = lock(&host.adapter().window).clone();
    let shown = window.is_some_and(|window| {
        crate::surface::blit(
            &window,
            frame.pixels.data_as_u8_slice(),
            u32::from(frame.pixels.width()),
            u32::from(frame.pixels.height()),
        )
    });
    host.deferred_presented(generation, shown);
}

impl FrameScheduler for HarmonyAdapter {
    fn request_vsync(&self) {
        self.vsync.store(true, Ordering::Release);
        crate::xcomponent::request_frame(self.token);
    }

    fn tick(&self) -> bool {
        self.ticked.store(self.elapsed_ms(), Ordering::Release);
        let ticking = surfaces::tick(self.gpu.load(Ordering::Acquire));
        self.ticking.store(ticking, Ordering::Release);
        ticking
    }
}

impl MainThread for HarmonyAdapter {
    fn post(&self, task: Task) {
        dispatch::post_main(task);
    }
    fn post_after(&self, delay: Duration, task: Task) {
        dispatch::post_after(delay, task);
    }
}

impl EnvironmentSource for HarmonyAdapter {
    fn environment(&self) -> Option<FrameEnvironment> {
        let mut facts = lock(&self.facts).clone()?;
        if let Some((width, height)) = *lock(&self.size) {
            (facts.width, facts.height) = (width, height);
        }
        facts.environment()
    }
}

impl ImageProvider for HarmonyAdapter {
    fn fetch(&self, id: u64, source: &str) {
        let Some(host) = self.host.get().cloned() else {
            return;
        };
        let finished = Arc::new(AtomicBool::new(false));
        let done = {
            let finished = finished.clone();
            Box::new(move |result: Result<Vec<u8>, String>| {
                finished.store(true, Ordering::Release);
                if let Some(host) = host.upgrade() {
                    lock(&host.adapter().images).remove(&id);
                    host.image_loaded(id, result);
                }
            })
        };
        let handle = self.shell.load_image(self.token, source.into(), done);
        // A loader may finish synchronously; then there is nothing left to cancel.
        if !finished.load(Ordering::Acquire) {
            lock(&self.images).insert(id, handle);
        }
    }
    fn cancel(&self, id: u64) {
        let handle = lock(&self.images).remove(&id);
        drop(handle);
    }
    fn video_resolver(&self) -> Option<lingxia_view::scene::video::VideoResolver> {
        let (shell, token) = (self.shell.clone(), self.token);
        Some(Arc::new(move |source: &str| {
            let url = shell.video_url(token, source)?;
            if lingxia_view::renderer::gpu::hls::is_playlist(&url)
                && !crate::video::has_manifest(&url)
            {
                read_manifest(&*shell, token, &url);
            }
            Ok(url)
        }))
    }
}

/// Playlist reads in flight by URL; dropping a handle cancels its read.
static MANIFEST_READS: std::sync::LazyLock<Mutex<HashMap<String, Box<dyn Send>>>> =
    std::sync::LazyLock::new(Default::default);

/// Reads `url`'s master playlist under the page's authority, so its player can list and pin
/// renditions. A failed read is retried at the next resolve.
fn read_manifest(shell: &dyn Shell, token: i64, url: &str) {
    if lock(&MANIFEST_READS).contains_key(url) {
        return;
    }
    let finished = Arc::new(AtomicBool::new(false));
    let done = {
        let (url, finished) = (url.to_owned(), finished.clone());
        Box::new(move |result: Result<Vec<u8>, String>| {
            finished.store(true, Ordering::Release);
            match result {
                Ok(bytes) => crate::video::store_manifest(&url, &String::from_utf8_lossy(&bytes)),
                Err(error) => log::debug!(target: "lingxia_view", "video playlist {url}: {error}"),
            }
            let handle = lock(&MANIFEST_READS).remove(&url);
            drop(handle);
        })
    };
    let handle = shell.load_image(token, url.to_owned(), done);
    if !finished.load(Ordering::Acquire) {
        lock(&MANIFEST_READS).insert(url.to_owned(), handle);
        // The read may have finished between the check and the insert.
        if finished.load(Ordering::Acquire) {
            let handle = lock(&MANIFEST_READS).remove(url);
            drop(handle);
        }
    }
}

impl AccessibilitySink for HarmonyAdapter {
    fn announce(&self, events: &[AccessibilityEffect]) {
        if let Some(provider) = pages::provider(self.token) {
            accessibility::announce(provider, events);
        }
    }
    fn clicked(&self) {}
}

impl InputMethod for HarmonyAdapter {
    fn reconcile(&self, frame: &FrameSnapshot) {
        ime::reconcile(self.token, frame, *lock(&self.origin));
    }
    fn focus(&self, editor: &Dispatch, frame: &FrameSnapshot) {
        let Dispatch::Editor { node, commit, .. } = editor else {
            return;
        };
        if let Some(node) = frame.frame.nodes.get(*node) {
            ime::focus_editor(self.token, &node.identity, commit.get());
        }
    }
    fn activation(&self) {
        ime::disconnect(self.token);
    }
    fn reset(&self) {
        ime::reset(self.token);
    }
    fn edit_text(&self, node: usize, text: Option<&str>, start: i32, end: i32) -> EditOutcome {
        let Some(frame) = self.snapshot() else {
            return EditOutcome::NotReady;
        };
        ime::edit_text(self.token, &frame, node, text, start, end)
    }
}

impl Feedback for HarmonyAdapter {
    fn apply(&self, effects: &[Effect]) {
        for effect in effects {
            match effect {
                // A node with a pressed style repaints itself.
                Effect::Press {
                    node,
                    styled: false,
                    ..
                } => self.highlight(0, Some(*node), self.snapshot().as_deref()),
                Effect::Press { .. } | Effect::ClearPress => self.highlight(0, None, None),
                Effect::FocusRing { node } => {
                    self.highlight(1, Some(*node), self.snapshot().as_deref())
                }
                Effect::ClearFocusRing => self.highlight(1, None, None),
                Effect::Haptic => self.shell.haptic(self.token),
                Effect::Claim { .. } => self.claimed.store(true, Ordering::Relaxed),
                Effect::ArmLongPress => {
                    let armed = self.long_press.fetch_add(1, Ordering::AcqRel) + 1;
                    let host = self.host.get().cloned();
                    dispatch::post_after(
                        LONG_PRESS,
                        Box::new(move || {
                            let Some(host) = host.and_then(|h| h.upgrade()) else {
                                return;
                            };
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
                    let speed = velocity.abs() / self.placement().0;
                    *lock(&self.fling) = Some(Fling {
                        region: region.clone(),
                        velocity: *velocity,
                        decay: 1.0 / (FLING_LINEAR + FLING_QUADRATIC * speed),
                        last: Instant::now(),
                    });
                    crate::xcomponent::request_frame(self.token);
                }
                Effect::StopFling => *lock(&self.fling) = None,
                Effect::Clicked => {}
                // The host runs these itself.
                Effect::Redraw
                | Effect::Dispatch
                | Effect::Number
                | Effect::Accessibility(_)
                | Effect::EditText { .. } => {}
            }
        }
    }
}

impl HostEvents for HarmonyAdapter {
    fn loaded(&self) {
        self.shell.loaded(self.token);
    }
    fn presented(&self, frame: &FrameSnapshot, timing: &FrameTiming) {
        let modal = frame.metrics.top_layer_open;
        if self.modal.swap(modal, Ordering::Relaxed) != modal {
            self.shell.top_layer(self.token, modal);
        }
        self.place_handles(frame);
        let orientation = frame.metrics.video_orientation;
        if std::mem::replace(&mut *lock(&self.orientation), orientation) != orientation {
            self.shell.video_orientation(self.token, orientation);
        }
        let highlights = *lock(&self.highlights);
        for (which, node) in highlights.into_iter().enumerate() {
            if node.is_some() {
                self.highlight(which, node, Some(frame));
            }
        }
        if log::log_enabled!(target: "lingxia_view", log::Level::Debug) {
            let metrics = &frame.reported;
            log::debug!(
                target: "lingxia_view",
                "present token={} generation={} size={}x{} workerMs={} nodes={} metrics={metrics}",
                self.token,
                frame.generation.get(),
                frame.width,
                frame.height,
                timing.worker_us / 1000,
                frame.frame.nodes.len(),
            );
        }
        let connection = self
            .host
            .get()
            .and_then(WeakViewHost::upgrade)
            .map_or(0, |host| host.lifecycle().connection().get());
        self.shell.presented(self.token, connection);
    }
    fn logic(
        &self,
        connection: ConnectionTicket,
        messages: Vec<lingxia_view::session::logic::ToLogic>,
    ) {
        self.shell.logic(self.token, connection.get(), messages);
    }
    fn state(
        &self,
        connection: ConnectionTicket,
        signal: lingxia_view::session::state::StateSignal,
    ) {
        self.shell.state(self.token, connection.get(), signal);
    }
    fn restart(&self, connection: ConnectionTicket) -> bool {
        self.shell.restart(self.token, connection.get())
    }
    fn recovery(&self, failure: Option<&Failure>) {
        self.shell.recovery(self.token, failure);
    }
    fn bootstrap_overdue(&self, overdue: bool) {
        self.shell.bootstrap_overdue(self.token, overdue);
    }
    fn copy_text(&self, text: &str) {
        self.shell.copy_text(text);
    }
    fn text_menu(&self, menu: Option<&lingxia_view::session::host::TextMenu>) {
        let json = menu
            .and_then(|menu| serde_json::to_string(menu).ok())
            .unwrap_or_default();
        self.shell.text_menu(self.token, &json);
    }
    fn revealed(&self, hold_surface: bool) {
        self.shell.revealed(self.token, hold_surface);
    }
    fn host_chrome(&self, chrome: &HostChrome) {
        self.shell.host_chrome(self.token, chrome);
    }
    fn warn(&self, message: &str) {
        log::warn!(target: "lingxia_view", "{message}");
    }
    fn console(&self, level: log::Level, message: &str) {
        self.shell.console(self.token, level, message);
    }
    fn debug(&self, message: &str) {
        log::debug!(target: "lingxia_view", "{message}");
    }
}

impl Clock for HarmonyAdapter {
    fn now(&self) -> Duration {
        self.started.elapsed()
    }
}
