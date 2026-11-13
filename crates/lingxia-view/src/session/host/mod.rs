//! One page hosted on a [`PlatformAdapter`]. The host owns the View worker job
//! queue, the frame pump (environment, prepare, present, presented effects),
//! dispatch, images, the Logic bridge, recovery and caret timing. Adapters call
//! the public methods on their main thread unless a method says otherwise.
use super::ViewSession;
use super::image_requests::ImageRequests;
use super::lifecycle::{
    CommitTicket, ConnectionTicket, FailureKind, Lifecycle, PageTicket, Recovery, Retry,
};
use crate::interaction::frame::PresentedFrame;
use crate::interaction::view::{AccessibilityAction, Key, Pointer, ViewInteraction};
use crate::platform::{FrameEnvironment, PlatformAdapter};
use crate::semantics::projection::Projection;
use crate::{ImageSource, ViewPageId};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

mod bridge;
mod clipboard;
pub(crate) mod decode;
mod dispatch;
mod drag;
mod edits;
mod environment;
mod fonts;
mod frame_stats;
mod glide;
#[cfg(feature = "hot-swap")]
mod hot_swap;
#[cfg(feature = "hot-swap")]
pub use hot_swap::{SwapReporter, set_swap_reporter};
mod autoclose;
mod images;
mod interest;
mod pump;
mod reveal;
mod snap;
mod telemetry;
mod text_menu;
mod video;
mod virtual_rows;
mod worker;

/// Largest input-method request [`ViewHost::input`] accepts, in bytes.
pub use super::input_protocol::MAX_REQUEST_BYTES as MAX_INPUT_REQUEST_BYTES;
pub use bridge::BridgeUpdate;
pub(crate) use frame_stats::SPARK as HUD_SPARK;
pub use reveal::{FIRST_FRAME_WATCHDOG, REVEAL_DEADLINE};
pub use telemetry::TransitionEvent;
pub use text_menu::{MenuAction, MenuTarget, TextMenu};
pub use worker::ViewWorker;

const BOOTSTRAP: Duration = Duration::from_secs(5);
const CARET_BLINK: Duration = Duration::from_millis(500);
/// The same failure this many times in a row stops rendering until a retry or a new size: a
/// deterministic error would otherwise re-render and fail at frame rate.
const STALL_AFTER: u32 = 3;

/// A presented frame as the adapter sees it.
pub struct FrameSnapshot {
    pub generation: CommitTicket,
    /// Device pixels.
    pub width: u16,
    pub height: u16,
    pub scale: f32,
    pub loaded_images: usize,
    pub frame: Arc<PresentedFrame>,
    pub images: Vec<ImageSource>,
    /// Each source's decode size in this frame.
    pub image_buckets: std::collections::HashMap<String, crate::scene::image::Bucket>,
    pub metrics: crate::session::metrics::FrameMetrics,
    /// The Logic state revision the frame was built from.
    pub data_revision: u64,
    pub feedback: crate::interaction::chrome::FeedbackStyle,
    /// The metrics as reported, with the presenter's own counters.
    pub reported: serde_json::Value,
    json: std::sync::OnceLock<String>,
}

impl FrameSnapshot {
    /// `{nodes, generation, images, metrics, feedback}`, serialized on first use: most frames
    /// are never read as text.
    pub fn json(&self) -> &str {
        self.json.get_or_init(|| {
            let nodes = serde_json::to_string(&self.frame.nodes).unwrap_or_else(|_| "[]".into());
            let rest = serde_json::json!({
                "generation": self.generation,
                "images": self.images,
                "metrics": self.reported,
                "feedback": self.feedback,
            })
            .to_string();
            format!("{{\"nodes\":{nodes},{}", &rest[1..])
        })
    }
}

/// What drawing and presenting a deferred frame took, as the adapter that drew it measured.
#[derive(Clone, Debug, Default)]
pub struct DrawStats {
    /// Scene, encode, submit and present µs.
    pub gpu_us: [u64; 4],
    /// The presenter's counters for the draw: `composite`, `backdropCache`, `passes`.
    pub presenter: Option<serde_json::Value>,
}

/// How one presented frame was produced, for diagnostics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameTiming {
    pub rebuild: bool,
    pub caret_visible: bool,
    /// Scheduling to the worker starting the frame.
    pub queue_us: u64,
    pub worker_us: u64,
    /// Prepared to vsync.
    pub wait_us: u64,
    /// Vsync to the end of the presented frame's main-thread work; a deferred frame's includes
    /// its wait for the adapter to show it.
    pub main_us: u64,
    /// The main thread's own work in that span: the vsync handler and the presented frame.
    pub main_work_us: u64,
    /// An adapter that draws off the worker: scene, encode, submit and present µs.
    pub gpu_us: [u64; 4],
}

/// Which host message recovery chrome shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryMessage {
    /// The bundle needs a newer runtime.
    UpdateRequired,
    /// The page's isolate was interrupted.
    SessionFailed,
    /// An update failed over a still-presented frame.
    UpdateFailed,
    /// Nothing could be presented.
    DisplayFailed,
}

/// The recovery button; it calls [`ViewHost::retry`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryAction {
    Retry,
    Reload,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Failure {
    pub error: String,
    pub recovery: Recovery,
    pub message: RecoveryMessage,
    pub action: Option<RecoveryAction>,
    /// A presented frame stays under the chrome.
    pub retained: bool,
}

impl Failure {
    /// Recovery chrome covers the whole page: nothing was presented, or what was is an
    /// interrupted page that no longer takes input and must not pass for a live one.
    pub fn covers_page(&self) -> bool {
        !self.retained || self.message == RecoveryMessage::SessionFailed
    }

    fn new(error: String, recovery: Recovery, retained: bool) -> Self {
        let message = match recovery.kind {
            FailureKind::Compatibility => RecoveryMessage::UpdateRequired,
            FailureKind::Session => RecoveryMessage::SessionFailed,
            FailureKind::Candidate if retained => RecoveryMessage::UpdateFailed,
            FailureKind::Candidate => RecoveryMessage::DisplayFailed,
        };
        let action = (recovery.kind != FailureKind::Compatibility && recovery.retry != Retry::None)
            .then_some(if recovery.kind == FailureKind::Session {
                RecoveryAction::Reload
            } else {
                RecoveryAction::Retry
            });
        Self {
            error,
            recovery,
            message,
            action,
            retained,
        }
    }
}

/// The focused editor's text state, reported by the input method.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CaretState {
    pub text: String,
    pub anchor: i32,
    pub focus: i32,
    pub composition: (i32, i32),
}

/// One blink phase; unchanged text across presentations does not restart it.
#[derive(Default)]
struct Caret {
    running: bool,
    hidden: bool,
    state: Option<CaretState>,
    timer: u64,
}

impl Caret {
    /// Returns whether to repaint now and the timer to start.
    fn update(&mut self, eligible: bool, state: CaretState) -> (bool, Option<u64>) {
        let changed = self.state.as_ref() != Some(&state);
        let running = eligible && state.anchor == state.focus;
        self.state = Some(state);
        if running == self.running && !changed {
            return (false, None);
        }
        self.timer += 1;
        self.running = running;
        let repaint = std::mem::take(&mut self.hidden);
        (repaint, running.then_some(self.timer))
    }

    fn stop(&mut self) {
        self.timer += 1;
        self.running = false;
        self.hidden = false;
        self.state = None;
    }
}

pub struct ViewHost<A: PlatformAdapter> {
    inner: Arc<Inner<A>>,
}

/// Hands Logic's state and messages to one host from any thread; see [`ViewHost::logic_port`].
#[derive(Clone)]
pub struct LogicPort(Arc<dyn Fn(ConnectionTicket, FromLogic) -> bool + Send + Sync>);

enum FromLogic {
    State(crate::session::state::StateWrite),
    Message(crate::session::logic::ToView),
}

impl LogicPort {
    /// False once the page is gone.
    pub fn write_state(
        &self,
        connection: ConnectionTicket,
        write: crate::session::state::StateWrite,
    ) -> bool {
        (self.0)(connection, FromLogic::State(write))
    }

    /// False once the page is gone.
    pub fn deliver(
        &self,
        connection: ConnectionTicket,
        message: crate::session::logic::ToView,
    ) -> bool {
        (self.0)(connection, FromLogic::Message(message))
    }
}

/// Marks from any thread that the page's presented frame is no longer its own; see
/// [`ViewHost::stale_mark`].
#[derive(Clone)]
pub struct StaleMark(Arc<std::sync::atomic::AtomicBool>);

impl StaleMark {
    pub fn set(&self) {
        self.0.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

/// A host handle that does not keep the page alive, e.g. for an adapter's
/// completion callbacks.
pub struct WeakViewHost<A: PlatformAdapter> {
    inner: std::sync::Weak<Inner<A>>,
}

impl<A: PlatformAdapter> Clone for WeakViewHost<A> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<A: PlatformAdapter> WeakViewHost<A> {
    pub fn upgrade(&self) -> Option<ViewHost<A>> {
        self.inner.upgrade().map(|inner| ViewHost { inner })
    }
}

impl<A: PlatformAdapter> Clone for ViewHost<A> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

struct Inner<A: PlatformAdapter> {
    adapter: A,
    worker: Arc<ViewWorker>,
    lifecycle: Lifecycle,
    logic: bool,
    /// Worker only: this page's context in the worker's isolate.
    page: Mutex<Option<ViewPageId>>,
    /// Input resolved on the presented frame, oldest first; every worker frame runs it before
    /// it renders.
    input: Mutex<std::collections::VecDeque<dispatch::Queued<A>>>,
    /// Set off the main thread when a park or reload is requested; the request itself runs later.
    stale: StaleMark,
    main: Mutex<Main<A::Frame>>,
}

/// Main-thread state. Never held while calling the adapter.
struct Main<F> {
    source: String,
    interaction: ViewInteraction,
    images: ImageRequests,
    /// `@font-face` fetches in flight, by token.
    fonts: std::collections::HashMap<u64, super::fonts::FontFace>,
    next_font: u64,
    /// When the players' states were last turned into events.
    video_flushed: Option<Duration>,
    /// When playback time last reached the page, and the scroll revision seen then.
    video_time_flushed: Option<Duration>,
    video_scroll_revision: u64,
    environment: Option<FrameEnvironment>,
    /// The environment of the last frame the target accepted; cleared when the target changes.
    presented_environment: Option<FrameEnvironment>,
    host_environment: environment::HostEnvironment,
    snapshot: Option<Arc<FrameSnapshot>>,
    /// State acknowledgements held until a frame built from their revision is queued to present.
    acks: Vec<(ConnectionTicket, u64)>,
    prepared: Option<pump::Prepared<F>>,
    /// A frame handed to the adapter that is not on screen yet.
    deferred: Option<pump::Deferred>,
    /// The frame the worker is preparing; a completion of any other is stale.
    in_flight: Option<pump::Flight>,
    /// A finished flight's result is being stored: a frame started meanwhile would queue
    /// behind it uncounted. `landing.1` records a start that was held back.
    landing: (bool, bool),
    #[cfg(test)]
    ahead_refusals: u32,
    next_flight: u64,
    /// The flight of the last presented frame, and of the last whose animation state the
    /// session acknowledged.
    presented_flight: u64,
    acked_flight: u64,
    /// Input-method requests on the worker; their View changes land when they return.
    inputs_in_flight: u32,
    /// Bumped per queued input: a frame started before the latest one misses it.
    input_seq: u64,
    /// A frame prepared ahead that finished before the frame under it presented.
    next_prepared: Option<pump::Prepared<F>>,
    /// Host time of the last vsync and the display interval estimate.
    last_vsync: Option<Duration>,
    vsync_interval: Duration,
    /// A covered window's next frame tick is posted.
    covered_tick: bool,
    /// The platform's timing for the display frame being served: `(shows at, interval)`.
    display: Option<(Duration, Duration)>,
    /// When a frame was first asked for since the last request took the demand.
    demand_at: Option<Duration>,
    /// The frame-statistics HUD is drawn over the page.
    hud: bool,
    /// The last generation [`ViewHost::set_chrome_snapshot`] handed out.
    chrome_generation: u64,
    /// `measureFrames` windows a View step asked for: `(token, milliseconds)`.
    frame_measures: Vec<(u64, u64)>,
    /// Queued activations whose result has not come back; automation waits for them.
    activating: u32,
    /// Automation `action`/`data` calls by id; `Some` once answered.
    automation_calls: std::collections::BTreeMap<u64, Option<Result<serde_json::Value, String>>>,
    /// An answered call settles once a frame started after its answer was seen presents.
    settle_after: std::collections::HashMap<u64, u64>,
    next_automation_call: u64,
    /// The automation act being delivered; the first activation it starts reports to it.
    act_call: Option<(u64, String)>,
    virtual_rows: virtual_rows::VirtualRows,
    snaps: snap::Snaps,
    drags: drag::Drags,
    /// User activations reported dropped, and clicks that ran a callback, for frame metrics.
    dropped_activations: u64,
    activations: u64,
    /// The host chrome last pushed to the adapter.
    chrome: Option<crate::platform::HostChrome>,
    /// A frame was declined while others still held the presenter's targets; presenting
    /// one frees a target, so the next vsync retries.
    declined: bool,
    /// Consecutive refusals of a frame that had a newer one prepared behind it.
    stale_declines: u32,
    /// The presenter keeps only its newest frame; nothing is prepared ahead any more.
    single_target: bool,
    /// A pointer is pressed on the page.
    pointer_down: bool,
    /// The worker time of the last frame built.
    last_build: Duration,
    #[cfg(test)]
    no_frames_ahead: bool,
    /// A fake presenter's modelled build time, which the worker clock does not see.
    #[cfg(test)]
    modelled_build: Option<Duration>,
    failure: Option<Failure>,
    /// How many times in a row the current failure's error repeated.
    failure_repeats: u32,
    /// The target size rendering stalled at after repeated failures; another size renders again.
    stalled_at: Option<(u16, u16)>,
    styled_press: Option<String>,
    styled_hover: Option<String>,
    interest: interest::Interest,
    autoclose: autoclose::Autoclose,
    styled_focus: Option<String>,
    caret: Caret,
    visible: bool,
    /// A hidden page whose target changed renders once, so returning to it needs no wait.
    catch_up: bool,
    dirty: bool,
    pending: bool,
    /// The target changed under a frame in flight; if that frame comes back declined, the
    /// page still owes one for the new target.
    retarget: bool,
    number_busy: bool,
    disposed: bool,
    /// Bumped to revoke a pending animation or bootstrap deadline.
    animation_timer: u64,
    /// The last presented frame asked for the next display frame.
    animating: bool,
    /// Animating with motion that ends; endless keyframe animations leave it false.
    moving: bool,
    /// When the last frame was queued behind an acknowledgement; one per instant.
    animation_queued_at: Duration,
    bootstrap_timer: u64,
    reveal: reveal::Reveal,
    telemetry: telemetry::Telemetry,
    /// The presenter's counters for the last frame drawn off the worker; a frame drawn on
    /// the worker carries them in its snapshot.
    drawn: Option<serde_json::Value>,
    text_menu: text_menu::MenuState,
    edits: edits::Edits,
}

impl<F> Main<F> {
    /// A frame is in flight, prepared or on its way to the screen.
    fn busy(&self) -> bool {
        self.in_flight.is_some()
            || self.prepared.is_some()
            || self.next_prepared.is_some()
            || self.deferred.is_some()
    }

    /// Forget frames prepared ahead, with the frame they build on: the session builds ahead
    /// only on frames that present. What they rendered must render again.
    fn drop_ahead(&mut self) {
        let ahead = self.in_flight.is_some_and(|flight| flight.ahead);
        let next = self.next_prepared.take();
        if let Some(next) = &next {
            self.release_input(next.input());
        }
        if next.is_some() || ahead {
            self.drop_prepared();
            self.dirty = true;
        }
        if ahead {
            self.in_flight = None;
        }
    }

    /// A frame that will not present hands its input to the next one, which then reports
    /// the whole wait.
    fn release_input(&mut self, input: Option<Duration>) {
        if let Some(at) = input {
            self.telemetry.stats.input(at);
        }
    }

    fn drop_prepared(&mut self) {
        if let Some(prepared) = self.prepared.take() {
            self.release_input(prepared.input());
        }
    }

    /// Forget every frame: the page or its connection was replaced.
    fn forget_frames(&mut self) {
        self.prepared = None;
        self.next_prepared = None;
        self.deferred = None;
        self.in_flight = None;
    }

    /// A frame may start while another waits for its vsync.
    fn ahead_allowed(&self) -> bool {
        #[cfg(test)]
        if self.no_frames_ahead {
            return false;
        }
        self.visible && !self.single_target
    }

    /// Under a finger a frame queued behind one waiting for vsync shows input a refresh
    /// late; only a build too slow to start after vsync and make the next one needs it.
    fn finger_holds_ahead(&self) -> bool {
        self.prepared.is_some() && self.pointer_down && self.build_time() * 2 <= self.vsync_interval
    }

    fn build_time(&self) -> Duration {
        #[cfg(test)]
        if let Some(build) = self.modelled_build {
            return build;
        }
        self.last_build
    }

    /// When a frame queued behind `queued` others should show: sampling there keeps
    /// pipelined frames evenly spaced.
    fn expected_vsync(&self, now: Duration, queued: u32) -> Duration {
        let Some(last) = self.last_vsync else {
            return now;
        };
        let interval = self.vsync_interval.max(Duration::from_millis(1));
        let mut next = last + interval;
        if next <= now {
            let behind = (now - next).as_nanos() / interval.as_nanos() + 1;
            next += interval * u32::try_from(behind).unwrap_or(u32::MAX);
        }
        next + interval * queued
    }
}

impl<A: PlatformAdapter> Drop for Inner<A> {
    fn drop(&mut self) {
        if let Some(id) = self.page.get_mut().ok().and_then(Option::take) {
            self.worker.run(move |app| {
                app.discard(id);
            });
        }
    }
}

/// Which ticket must still be current for queued work to run.
#[derive(Clone, Copy)]
enum Guard {
    Page(PageTicket),
    Connection(ConnectionTicket),
}

impl<A: PlatformAdapter> ViewHost<A> {
    pub fn new(adapter: A, worker: Arc<ViewWorker>, logic: bool) -> Self {
        // The app's pack is part of the first environment, so no frame shows the default palette.
        let host_environment = environment::HostEnvironment::for_worker(worker.key());
        Self {
            inner: Arc::new(Inner {
                adapter,
                worker,
                lifecycle: Lifecycle::new(logic),
                logic,
                page: Mutex::new(None),
                input: Mutex::new(Default::default()),
                stale: StaleMark(Default::default()),
                main: Mutex::new(Main {
                    source: String::new(),
                    interaction: ViewInteraction::default(),
                    images: ImageRequests::default(),
                    fonts: Default::default(),
                    next_font: 0,
                    video_flushed: None,
                    video_time_flushed: None,
                    video_scroll_revision: 0,
                    environment: None,
                    presented_environment: None,
                    host_environment,
                    snapshot: None,
                    acks: Vec::new(),
                    prepared: None,
                    deferred: None,
                    in_flight: None,
                    landing: (false, false),
                    #[cfg(test)]
                    ahead_refusals: 0,
                    next_flight: 0,
                    presented_flight: 0,
                    acked_flight: 0,
                    inputs_in_flight: 0,
                    input_seq: 0,
                    next_prepared: None,
                    last_vsync: None,
                    vsync_interval: Duration::from_micros(16_667),
                    covered_tick: false,
                    display: None,
                    demand_at: None,
                    hud: false,
                    chrome_generation: 0,
                    frame_measures: Vec::new(),
                    activating: 0,
                    automation_calls: Default::default(),
                    settle_after: Default::default(),
                    next_automation_call: 0,
                    act_call: None,
                    virtual_rows: Default::default(),
                    snaps: Default::default(),
                    drags: Default::default(),
                    dropped_activations: 0,
                    activations: 0,
                    chrome: None,
                    declined: false,
                    stale_declines: 0,
                    single_target: false,
                    pointer_down: false,
                    last_build: Duration::ZERO,
                    #[cfg(test)]
                    no_frames_ahead: false,
                    #[cfg(test)]
                    modelled_build: None,
                    failure: None,
                    failure_repeats: 0,
                    stalled_at: None,
                    styled_press: None,
                    styled_hover: None,
                    interest: Default::default(),
                    autoclose: Default::default(),
                    styled_focus: None,
                    caret: Caret::default(),
                    visible: false,
                    catch_up: false,
                    dirty: true,
                    pending: false,
                    retarget: false,
                    number_busy: false,
                    disposed: false,
                    animation_timer: 0,
                    animating: false,
                    moving: false,
                    animation_queued_at: Duration::ZERO,
                    bootstrap_timer: 0,
                    reveal: Default::default(),
                    telemetry: Default::default(),
                    drawn: None,
                    text_menu: Default::default(),
                    edits: Default::default(),
                }),
            }),
        }
    }

    pub fn adapter(&self) -> &A {
        &self.inner.adapter
    }

    pub fn downgrade(&self) -> WeakViewHost<A> {
        WeakViewHost {
            inner: Arc::downgrade(&self.inner),
        }
    }

    pub fn lifecycle(&self) -> &Lifecycle {
        &self.inner.lifecycle
    }

    /// The worker this page runs on, for adapter work that must serialize with its frames.
    pub fn worker(&self) -> &Arc<ViewWorker> {
        &self.inner.worker
    }

    /// Evaluate a new module instance; earlier page work becomes stale.
    pub fn load(&self, source: String) {
        self.inner.load(Some(source));
    }

    /// Dev sessions: replace the page module in place, keeping its state; a module that cannot
    /// swap in loads afresh. [`set_swap_reporter`] hears which.
    #[cfg(feature = "hot-swap")]
    pub fn swap_module(&self, source: String) {
        self.inner.swap_module(source);
    }

    /// Evaluate the current module again.
    pub fn reload(&self) {
        self.inner.load(None);
    }

    /// Release local state but keep the module for the next connection.
    pub fn park(&self) {
        self.inner.park();
    }

    pub fn dispose(&self) {
        self.inner.dispose();
    }

    pub fn set_visible(&self, visible: bool) {
        if !visible {
            self.inner.set_hovered(None);
            self.inner.lose_interest();
            self.inner.pointer_crossing(None);
        }
        self.inner.set_visible(visible);
        self.inner.sync_autoclose();
    }

    /// The presentation target was replaced or resized. A frame being built for the same size,
    /// scale and environment still presents; one built for other inputs is rebuilt.
    pub fn surface_changed(&self) {
        // Back on screen: a visible page's videos play again where they held.
        let visible = self.inner.main().visible && !self.inner.window_stopped();
        self.inner.hide_videos(!visible);
        let fits = {
            let measured = self.inner.adapter.environment();
            let main = self.inner.main();
            main.host_environment
                .still_fits(main.environment.as_ref(), measured)
        };
        if !fits {
            self.inner.lifecycle.invalidate_surface();
        }
        let (hidden, kept) = {
            let mut main = self.inner.main();
            main.presented_environment = None;
            main.catch_up = !main.visible && !main.interaction.parked();
            let kept = fits && (main.prepared.is_some() || main.in_flight.is_some());
            main.retarget |= kept && main.in_flight.is_some();
            (main.catch_up, kept)
        };
        // A hidden page's catch-up frame runs as background work, behind the shown page's frame.
        let _ = hidden;
        // A kept frame on its way presents to the new target; a redraw would only repeat it.
        if !kept {
            self.redraw();
        }
    }

    /// The target holds an accepted frame for the current size and environment; a pending
    /// data update still follows as an ordinary frame.
    pub fn frame_is_current(&self) -> bool {
        self.inner.frame_is_current()
    }

    /// Show this page in place of the one on screen once it has a current frame:
    /// [`crate::platform::HostEvents::revealed`] says when. `surface_holds_frame`:
    /// the target still shows a frame this page presented before.
    pub fn reveal(&self, surface_holds_frame: bool) {
        self.inner.begin_reveal(surface_holds_frame);
    }

    /// The window was stopped or started (app backgrounded, screen locked); a pending
    /// reveal waits while it is stopped.
    pub fn window_visible(&self, visible: bool) {
        self.inner.window_visible(visible);
        self.inner.sync_autoclose();
    }

    /// The platform slides this page in on a push or pop; `frame_metrics` reports the slide.
    pub fn transition(&self, event: TransitionEvent) {
        let now = self.inner.adapter.now();
        self.inner.main().telemetry.transition(now, event);
    }

    /// Other windows cover this one completely, or no longer do; its surface stays live.
    pub fn window_covered(&self, covered: bool) {
        self.inner.window_covered(covered);
        self.inner.sync_autoclose();
    }

    /// The presentation target is gone; nothing presents until it returns.
    pub fn surface_lost(&self) {
        // Off screen like a hidden page: its videos pause and keep their place.
        self.inner.hide_videos(true);
        self.inner.lifecycle.invalidate_surface();
        self.inner.main().presented_environment = None;
        self.inner.park_animations();
        let effects = self.inner.main().interaction.cancel_all();
        self.inner.deliver(effects);
        // The bars are the host's again; a returning surface captures them anew.
        self.decline_chrome_snapshot();
    }

    /// A runtime environment push (`language`, `surface` or `theme` kind, JSON).
    pub fn update_environment(&self, update: &str) -> Result<(), String> {
        if self.inner.main().host_environment.update(update)? {
            self.request_frame();
        }
        Ok(())
    }

    /// Rebuild the View for the next frame.
    pub fn request_frame(&self) {
        self.inner.main().dirty = true;
        self.inner.schedule();
    }

    /// Present again without rerunning the View.
    pub fn redraw(&self) {
        self.inner.schedule();
    }

    /// Prepare at most one frame at a time, as before frames could be prepared ahead.
    #[cfg(test)]
    pub(crate) fn disable_frames_ahead(&self) {
        self.inner.main().no_frames_ahead = true;
    }

    /// Ahead frames the session refused because two frames already waited.
    #[cfg(test)]
    pub(crate) fn ahead_refusals(&self) -> u32 {
        self.inner.main().ahead_refusals
    }

    /// Ask for a frame while a finished flight lands: whether a flight started then, and
    /// once the landing ended.
    #[cfg(test)]
    pub(crate) fn schedule_while_landing(&self) -> (bool, bool) {
        {
            let mut main = self.inner.main();
            main.landing = (true, false);
            main.dirty = true;
        }
        self.inner.schedule();
        let during = self.inner.main().in_flight.is_some();
        drop(pump::Landed(self.inner.clone()));
        let after = self.inner.main().in_flight.is_some();
        (during, after)
    }

    /// Every build takes `build`, as a device's worker would report it.
    #[cfg(test)]
    pub(crate) fn model_build_time(&self, build: Duration) {
        self.inner.main().modelled_build = Some(build);
    }

    /// The display frame requested through `FrameScheduler::request_vsync`.
    pub fn vsync(&self) {
        self.tick();
        self.inner.vsync();
    }

    /// Runs the adapter's per-frame renderer work, keeping display frames coming while it has
    /// some. A hidden window shows nothing, so it has none.
    pub fn tick(&self) {
        if !self.inner.window_hidden() && self.inner.adapter.tick() {
            self.inner.adapter.request_vsync();
            self.inner.video_tick();
        }
    }

    /// [`Self::vsync`] with the platform's frame timing: the refresh this frame shows on is
    /// `present_in_us` from now (negative once past), and refreshes come every `interval_us`.
    pub fn vsync_at(&self, present_in_us: i64, interval_us: u64) {
        let now = self.inner.adapter.now();
        let delta = Duration::from_micros(present_in_us.unsigned_abs());
        let target = if present_in_us >= 0 {
            now + delta
        } else {
            now.saturating_sub(delta)
        };
        let interval = Duration::from_micros(interval_us);
        self.inner.main().display = Some((target, interval));
        self.tick();
        self.inner.vsync();
    }

    /// The next pointer, wheel or key event happened `age_us` ago on the platform's input
    /// clock; without it the host takes the time it receives the event.
    pub fn input_age(&self, age_us: u64) {
        crate::session::probe::sample(crate::session::probe::Probe::InputDispatch, true, age_us);
        let at = self
            .inner
            .adapter
            .now()
            .saturating_sub(Duration::from_micros(age_us));
        self.inner.main().telemetry.stats.input(at);
    }

    /// Draw the frame-statistics HUD over the page, or stop.
    pub fn set_hud(&self, on: bool) {
        let changed = {
            let mut main = self.inner.main();
            std::mem::replace(&mut main.hud, on) != on
        };
        if changed {
            self.redraw();
        }
    }

    /// While the page's top layer is open (`top_layer_open` in frame metrics), draw a
    /// snapshot of the host's `edge` bar under it so the backdrop dims the bar too.
    /// `rgba` is premultiplied, `width` × `height`; `rect` places it in surface pixels.
    /// Returns its generation: once a frame reporting `chrome_snapshot` at or past it is on
    /// screen, the host may hide the real bar. The page drops the snapshots when the top layer
    /// closes or the surface changes, and frames then report 0: show the bar again.
    pub fn set_chrome_snapshot(
        &self,
        edge: crate::scene::display::ChromeEdge,
        rgba: Vec<u8>,
        width: u32,
        height: u32,
        rect: [f32; 4],
    ) -> u64 {
        let generation = {
            let mut main = self.inner.main();
            main.chrome_generation += 1;
            main.chrome_generation
        };
        self.inner.work(
            Guard::Page(self.inner.lifecycle.page()),
            move |_, session| {
                session.set_chrome_snapshot(edge, generation, &rgba, width, height, rect)
            },
            |inner, result| match result {
                Some(Ok(())) => inner.schedule(),
                Some(Err(error)) => log::warn!(target: "lingxia_view", "{error}"),
                None => {}
            },
        );
        generation
    }

    /// The host shows its bars again before the top layer closed: drop the snapshots.
    pub fn decline_chrome_snapshot(&self) {
        self.inner.work(
            Guard::Page(self.inner.lifecycle.page()),
            |_, session| {
                session.clear_chrome_snapshots();
                Ok(())
            },
            |inner, result| {
                if result.is_some() {
                    inner.schedule();
                }
            },
        );
    }

    /// A deferred frame of `generation` reached the screen, or `shown: false` when the
    /// target dropped it. Any thread.
    pub fn deferred_presented(&self, generation: CommitTicket, shown: bool) {
        self.deferred_drawn(generation, shown, DrawStats::default());
    }

    /// [`Self::deferred_presented`] from an adapter that drew the frame itself.
    pub fn deferred_drawn(&self, generation: CommitTicket, shown: bool, stats: DrawStats) {
        // Stamped here, not on main: the post back is not part of presenting.
        let done = self.inner.adapter.now();
        let inner = self.inner.clone();
        self.inner.adapter.post(Box::new(move || {
            inner.deferred_presented(generation, shown, stats, done)
        }));
    }

    /// Run the recovery action offered with the last failure.
    pub fn retry(&self) {
        self.inner.retry();
    }

    /// A failure the platform detected outside the host, classified like View failures.
    pub fn fail(&self, error: String) {
        self.inner.fail(error);
    }

    /// A fresh Logic connection; the View answers with `ToLogic::Connected` from its worker.
    pub fn connect(&self, connection: ConnectionTicket) {
        self.inner.connect(connection);
    }

    /// Any thread: a typed Logic state write, applied in write order with the others.
    pub fn write_state(
        &self,
        connection: ConnectionTicket,
        write: crate::session::state::StateWrite,
    ) {
        self.inner.write_state(connection, write);
    }

    /// Any thread: a typed Logic message, applied in order with state writes.
    pub fn deliver(&self, connection: ConnectionTicket, message: crate::session::logic::ToView) {
        self.inner.deliver_logic(connection, message);
    }

    /// Set it where a park or reload is requested and posted to the main thread, so a reveal
    /// that runs before the posted call does not take the old instance's frame as current.
    pub fn stale_mark(&self) -> StaleMark {
        self.inner.stale.clone()
    }

    /// A handle for [`Self::write_state`] and [`Self::deliver`] that does not keep the page alive.
    pub fn logic_port(&self) -> LogicPort {
        let host = self.downgrade();
        LogicPort(Arc::new(move |connection, from| {
            host.upgrade()
                .map(|host| match from {
                    FromLogic::State(write) => host.write_state(connection, write),
                    FromLogic::Message(message) => host.deliver(connection, message),
                })
                .is_some()
        }))
    }

    pub fn request_leave(&self, connection: ConnectionTicket, reason: String) {
        self.inner.request_leave(connection, reason);
    }

    /// Returns whether the gesture was consumed.
    pub fn pointer(&self, event: Pointer) -> bool {
        if !self.inner.lifecycle.accepts_input() {
            return false;
        }
        if event.phase == crate::interaction::view::PointerPhase::Down {
            self.inner.dismiss_editor_menu();
            self.inner.dismiss_hints_at([event.x, event.y]);
        }
        self.inner.input_now();
        let pressed = matches!(
            event.phase,
            crate::interaction::view::PointerPhase::Down
                | crate::interaction::view::PointerPhase::Move
        );
        let at = self.inner.adapter.now();
        {
            let mut main = self.inner.main();
            main.pointer_down = pressed;
            let press = main.interaction.frame().map(|frame| {
                let scale = if frame.scale > 0.0 { frame.scale } else { 1.0 };
                [event.x / scale, event.y / scale]
            });
            // A press holds a popover by where it went down, wherever the finger moves.
            if !pressed {
                main.autoclose.set_press(None);
            } else if event.phase == crate::interaction::view::PointerPhase::Down {
                main.autoclose.set_press(press);
            }
            if pressed {
                main.telemetry.stats.pointer(at, [event.x, event.y]);
            }
        }
        let page = self.inner.lifecycle.page();
        let (handled, effects) = self.inner.main().interaction.pointer(event, page);
        self.inner.deliver(effects);
        self.inner.snap_later();
        handled
    }

    /// A fine pointer moved to `point` (device pixels), or left the page: `hover:` deltas
    /// repaint without running the View.
    pub fn hover(&self, point: Option<[f32; 2]>) {
        if !self.inner.lifecycle.accepts_input() {
            return;
        }
        let (identity, trigger) = {
            let main = self.inner.main();
            let shown = main.interest.shown().map(str::to_owned);
            point
                .and_then(|[x, y]| {
                    let frame = main.interaction.frame()?;
                    let scale = if frame.scale > 0.0 { frame.scale } else { 1.0 };
                    let point = [x / scale, y / scale];
                    let identity = frame
                        .hover_hit(point)
                        .and_then(|index| frame.nodes.get(index))
                        .map(|node| node.identity.to_string());
                    // Resting on the shown hint keeps its trigger's interest.
                    let target = shown.as_deref().and_then(|trigger| {
                        frame
                            .nodes
                            .iter()
                            .find(|node| &*node.identity == trigger)?
                            .interest_for
                            .clone()
                    });
                    let trigger = frame
                        .interest_hit(point, target.as_deref())
                        .and_then(|index| frame.nodes.get(index))
                        .and_then(|node| match node.interest_for {
                            Some(_) => Some(node.identity.to_string()),
                            None => shown.clone(),
                        });
                    Some((identity, trigger))
                })
                .unwrap_or_default()
        };
        self.inner.set_hovered(identity);
        self.inner.interest_hovered(trigger);
        let logical = point.and_then(|[x, y]| {
            let main = self.inner.main();
            let scale = main.interaction.frame()?.scale;
            let scale = if scale > 0.0 { scale } else { 1.0 };
            Some([x / scale, y / scale])
        });
        self.inner.main().autoclose.set_pointer(logical);
        self.inner.sync_autoclose();
        self.inner.pointer_crossing(logical);
    }

    /// A wheel or trackpad scroll; returns whether a region moved.
    pub fn wheel(&self, event: crate::interaction::view::Wheel) -> bool {
        if !self.inner.lifecycle.accepts_input() {
            return false;
        }
        self.inner.input_now();
        let (moved, effects) = self.inner.main().interaction.wheel(event);
        self.inner.deliver(effects);
        self.inner.snap_later();
        moved
    }

    pub fn long_press(&self) {
        let effects = self.inner.main().interaction.long_press();
        self.inner.deliver(effects);
    }

    /// `false` leaves the key to the platform, as after a session failure.
    pub fn key(&self, key: Key, shift: bool, repeat: bool) -> bool {
        if !self.inner.lifecycle.accepts_input() {
            return false;
        }
        self.inner.input_now();
        let page = self.inner.lifecycle.page();
        let (handled, effects) = self.inner.main().interaction.key(key, shift, repeat, page);
        self.inner.deliver(effects);
        handled
    }

    /// A platform close request (Android back): the topmost dialog or light-dismiss popover
    /// closes and the request is consumed; false when none is open.
    pub fn close_request(&self) -> bool {
        if !self.inner.lifecycle.accepts_input() {
            return false;
        }
        let Some(effects) = self.inner.main().interaction.close_request() else {
            return false;
        };
        self.inner.deliver(effects);
        true
    }

    /// After a session failure the page takes no input at all, platform gestures
    /// (refresh, selection handles) included, until it reloads.
    pub fn input_blocked(&self) -> bool {
        self.inner.lifecycle.session_failed()
    }

    /// The window gained or lost input focus; losing it ends every gesture in flight.
    pub fn window_focus(&self, focused: bool) {
        if !focused {
            self.cancel();
            // Not every platform reports the pointer leaving a window that lost focus.
            self.inner.set_hovered(None);
            self.inner.lose_interest();
            self.inner.pointer_crossing(None);
        }
    }

    /// A selection handle drag started or ended; the menu hides while dragging and
    /// an editor's menu returns on release.
    pub fn text_handle_drag(&self, active: bool) {
        self.inner.set_handle_drag(active);
    }

    /// A double click at a device-pixel point selects the word of selectable text there.
    pub fn select_word_at(&self, point: [f32; 2]) {
        let effects = self
            .inner
            .main()
            .interaction
            .select_word_at(point[0], point[1]);
        self.inner.deliver(effects);
    }

    /// A secondary click at a device-pixel point opens the text menu of the static
    /// selection or focused editor there; false when there is none.
    pub fn context_menu(&self, point: [f32; 2]) -> bool {
        let scale = self
            .snapshot()
            .map_or(1.0, |s| s.scale)
            .max(f32::MIN_POSITIVE);
        self.inner
            .context_menu([point[0] / scale, point[1] / scale])
    }

    /// The user closed the text menu; a static selection is cleared with it.
    pub fn text_menu_dismissed(&self) {
        if self.inner.text_menu_dismissed() {
            self.clear_text_selection();
        }
    }

    /// A text menu item was chosen. Static-text actions run here and return true;
    /// an editor's run in the platform input connection.
    pub fn text_menu_action(&self, action: MenuAction) -> bool {
        let shown = self.inner.main().text_menu_shown();
        match shown {
            Some(TextMenu {
                target: MenuTarget::Text { identity },
                ..
            }) => match action {
                MenuAction::Copy => {
                    let host = self.downgrade();
                    self.copy_text_selection(move |text| {
                        if let (Some(host), Some(text)) = (host.upgrade(), text)
                            && !text.is_empty()
                        {
                            host.adapter().copy_text(&text);
                        }
                    });
                    true
                }
                MenuAction::SelectAll => {
                    let length = self
                        .snapshot()
                        .and_then(|s| {
                            s.frame
                                .nodes
                                .iter()
                                .find(|n| n.identity == identity)
                                .map(|n| n.text.encode_utf16().count())
                        })
                        .unwrap_or(0);
                    self.set_text_selection(identity, 0, length as u32);
                    true
                }
                _ => false,
            },
            _ => {
                if action != MenuAction::SelectAll {
                    self.inner.dismiss_editor_menu();
                }
                false
            }
        }
    }

    /// End pointer, press and long-press state, e.g. when focus or the window goes away.
    pub fn cancel(&self) {
        let effects = self.inner.main().interaction.cancel_all();
        self.inner.deliver(effects);
    }

    pub fn focus_entered(&self, backward: bool) {
        let page = self.inner.lifecycle.page();
        let effects = self.inner.main().interaction.focus_entered(backward, page);
        self.inner.deliver(effects);
    }

    pub fn focus_left(&self) {
        let effects = self.inner.main().interaction.focus_left();
        self.inner.deliver(effects);
    }

    /// Touch exploration at a device-pixel point, or its end with `None`.
    pub fn explore(&self, point: Option<[f32; 2]>) -> bool {
        let (handled, effects) = self.inner.main().interaction.explore(point);
        self.inner.deliver(effects);
        handled
    }

    pub fn accessibility(&self, id: u32, action: AccessibilityAction) -> bool {
        if let Some(handled) = self
            .static_text_action(id, &action)
            .or_else(|| self.editor_clipboard_action(id, &action))
        {
            return handled;
        }
        let page = self.inner.lifecycle.page();
        let label = format!("{action:?}");
        let (handled, effects, refusal) = {
            let mut main = self.inner.main();
            let (handled, effects) = main.interaction.accessibility(id, action, page);
            let refusal = (!handled).then(|| main.interaction.accessibility_refusal(id));
            (handled, effects, refusal)
        };
        if let Some(reason) = refusal {
            self.adapter().debug(&format!(
                "UI_A11Y: refused page={} id={id} action={label} reason={reason}",
                page.get()
            ));
        }
        self.inner.deliver(effects);
        handled && !self.inner.take_edit_failure()
    }

    /// Selection and Copy on selectable static text act on the session's text
    /// selection, not on an editor.
    fn static_text_action(&self, id: u32, action: &AccessibilityAction) -> Option<bool> {
        use crate::semantics::projection::Action;
        if !matches!(
            action,
            AccessibilityAction::SetSelection(..) | AccessibilityAction::Copy
        ) {
            return None;
        }
        let node = self
            .semantics(|projection, _| projection.and_then(|projection| projection.node(id)))
            .filter(|node| node.text_field.is_none())?;
        match *action {
            AccessibilityAction::SetSelection(start, end)
                if node.actions.contains(&Action::SetSelection) =>
            {
                let identity = self
                    .snapshot()?
                    .frame
                    .nodes
                    .get(node.index)?
                    .identity
                    .clone();
                let offset = |n: i32| u32::try_from(n).unwrap_or(0);
                self.set_text_selection(identity.to_string(), offset(start), offset(end));
                Some(true)
            }
            AccessibilityAction::Copy if node.text_selection.is_some() => {
                let host = self.downgrade();
                self.copy_text_selection(move |text| {
                    if let (Some(host), Some(text)) = (host.upgrade(), text)
                        && !text.is_empty()
                    {
                        host.adapter().copy_text(&text);
                    }
                });
                Some(true)
            }
            AccessibilityAction::Copy => Some(false),
            _ => None,
        }
    }

    /// The WebView-shaped automation query over the presented frame.
    pub fn automation_query(
        &self,
        css: &str,
        index: Option<usize>,
        all: bool,
        max_text: Option<usize>,
    ) -> Result<serde_json::Value, String> {
        let found = {
            let main = self.inner.main();
            // A page that left the stack reads as the blank document a parked WebView loads.
            if main.interaction.parked() {
                return Ok(if all {
                    serde_json::json!({ "count": 0, "renderer": "gpu", "items": [] })
                } else {
                    serde_json::json!({
                        "exists": false, "renderer": "gpu", "index": index.unwrap_or(0), "count": 0,
                        "visible": false, "inViewport": false, "enabled": false, "editable": false,
                    })
                });
            }
            main.interaction
                .automation_query(css, index, all, max_text)?
        };
        // An off-screen List row is attached and visible outside the viewport, as in a WebView
        // DOM, so locators reveal it; its content waits until it draws.
        let missed = found["count"] == 0 && (all || index.unwrap_or(0) == 0);
        if missed && self.inner.virtual_row(css).is_some() {
            let row = serde_json::json!({
                "exists": true, "virtual": true, "renderer": "gpu", "index": 0, "count": 1,
                "visible": true, "inViewport": false, "enabled": false, "editable": false,
            });
            return Ok(if all {
                serde_json::json!({ "count": 1, "renderer": "gpu", "items": [row] })
            } else {
                row
            });
        }
        Ok(found)
    }

    /// One automation request from the page driver:
    /// `{op: "query", css, index, all, maxText}` or
    /// `{op: "act", css, index, action: {kind: click|dblclick|fill|type|scroll|press|select, text, key, value, force}}`,
    /// `{op: "tree"}`, `{op: "frame", windowMs?}` (presentation counters and frame pacing over
    /// the last `windowMs`) or `{op: "hud", on}` (draw frame statistics over the page).
    /// Screenshots belong to the adapter's presenter.
    pub fn automation_request(
        &self,
        request: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        use crate::interaction::view::AutomationAct;
        let css = request["css"].as_str().unwrap_or_default();
        let index = request["index"].as_u64().map(|i| i as usize);
        match request["op"].as_str() {
            Some("query") => self.automation_query(
                css,
                index,
                request["all"].as_bool().unwrap_or(false),
                request["maxText"].as_u64().map(|n| n as usize),
            ),
            Some("act") => {
                let action = &request["action"];
                let string = |key: &str| action[key].as_str().unwrap_or_default().to_owned();
                let act = match action["kind"].as_str() {
                    Some("click") => AutomationAct::Click,
                    Some("fill") => AutomationAct::Fill(string("text")),
                    Some("type") => AutomationAct::Type(string("text")),
                    Some("scroll") => AutomationAct::ScrollIntoView,
                    Some("press") => AutomationAct::Press(string("key")),
                    Some("select") => AutomationAct::SelectOption(string("value")),
                    Some("dblclick") => AutomationAct::DoubleClick,
                    Some("action") => AutomationAct::Action(string("name")),
                    other => return Err(format!("E_UNSUPPORTED: automation action {other:?}")),
                };
                let force = action["force"].as_bool().unwrap_or(false);
                // The reply names a call; `result` reports once the worker applied the act.
                let call = self.automation_call()?;
                self.inner.main().act_call = Some((call, css.to_owned()));
                let acted = self.automate(css, index, act, force);
                let mut main = self.inner.main();
                let unclaimed = main.act_call.take().is_some();
                match acted {
                    Err(error) => {
                        main.automation_calls.remove(&call);
                        Err(error)
                    }
                    Ok(()) => {
                        // Nothing reached the worker (fill, scroll, a local key): applied already.
                        if unclaimed && let Some(slot) = main.automation_calls.get_mut(&call) {
                            *slot = Some(Ok(serde_json::Value::Null));
                        }
                        Ok(serde_json::json!({ "call": call }))
                    }
                }
            }
            Some("tree") => self.inner.main().interaction.automation_tree(),
            Some("action") => self.automation_action(
                request["name"].as_str().unwrap_or_default(),
                request.get("args").filter(|args| !args.is_null()).cloned(),
            ),
            Some("data") => self.automation_data(),
            Some("result") => self.automation_result(request["call"].as_u64().unwrap_or(u64::MAX)),
            Some("frame") => Ok(self
                .inner
                .frame_metrics(request["windowMs"].as_u64().map(Duration::from_millis))),
            Some("hud") => {
                let on = request["on"]
                    .as_bool()
                    .ok_or("E_INVALID_ARG: hud takes `on: boolean`")?;
                self.set_hud(on);
                Ok(serde_json::json!({ "hud": on }))
            }
            other => Err(format!("E_UNSUPPORTED: automation op {other:?}")),
        }
    }

    fn automation_call(&self) -> Result<u64, String> {
        let mut main = self.inner.main();
        if main.disposed {
            return Err("E_PAGE_NOT_READY: the page is disposed".into());
        }
        if main.interaction.parked() {
            return Err("E_PAGE_NOT_READY: the page is parked (it left the stack)".into());
        }
        // A caller that gave up never polls again: past the cap the oldest call expires.
        while main.automation_calls.len() >= 64 {
            if let Some((old, _)) = main.automation_calls.pop_first() {
                main.settle_after.remove(&old);
            }
        }
        main.next_automation_call += 1;
        let call = main.next_automation_call;
        main.automation_calls.insert(call, None);
        Ok(call)
    }

    /// Call page action `name` as `view.actions[name](args)` would; poll `result` for its outcome.
    fn automation_action(
        &self,
        name: &str,
        args: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        if !self.inner.logic {
            return Err("UI_ACTION: this page has no Logic".into());
        }
        let call = self.automation_call()?;
        let name = name.to_owned();
        self.inner.activate(
            self.inner.lifecycle.page(),
            false,
            Box::new(move |session| session.automation_action(call, &name, args.as_ref())),
        );
        Ok(serde_json::json!({ "call": call }))
    }

    /// Read the page's current `view.data`; poll `result` for it.
    fn automation_data(&self) -> Result<serde_json::Value, String> {
        let call = self.automation_call()?;
        self.inner.work(
            Guard::Page(self.inner.lifecycle.page()),
            |_, session| Ok(session.automation_data()),
            move |inner, result| {
                if let Some(slot) = inner.main().automation_calls.get_mut(&call) {
                    *slot = Some(
                        result
                            .unwrap_or_else(|| Err("E_PAGE_NOT_READY: the page went away".into())),
                    );
                }
            },
        );
        Ok(serde_json::json!({ "call": call }))
    }

    /// `done` once the call answered and the frame its changes produced is committed.
    fn automation_result(&self, call: u64) -> Result<serde_json::Value, String> {
        let mut main = self.inner.main();
        // Work the answer started that has not reached the main thread yet.
        let quiet = main.activating == 0 && main.inputs_in_flight == 0;
        let after = if quiet && matches!(main.automation_calls.get(&call), Some(Some(_))) {
            let next = main.next_flight + 1;
            *main.settle_after.entry(call).or_insert(next)
        } else {
            u64::MAX
        };
        // A page that left (parked) or is not on screen commits no further frame: what its
        // worker already ran is the answer, as when a click navigates away or a dialog covers it.
        // Otherwise nothing is coming, or a frame started after the answer presented and the
        // session saw no animation still running; newer Logic state meanwhile does not count.
        let shown = (!main.dirty && !main.busy()) || (main.acked_flight >= after && !main.moving);
        let settled = quiet && (shown || main.interaction.parked() || !main.visible);
        if settled || matches!(main.automation_calls.get(&call), Some(Some(Err(_))) | None) {
            main.settle_after.remove(&call);
        }
        match main.automation_calls.get(&call) {
            None => Err(format!(
                "E_UNSUPPORTED: automation call {call} expired or was never made"
            )),
            Some(Some(Err(_))) => Err(main
                .automation_calls
                .remove(&call)
                .flatten()
                .unwrap()
                .unwrap_err()),
            Some(Some(_)) if settled => match main.automation_calls.remove(&call).flatten() {
                Some(Ok(value)) => Ok(serde_json::json!({ "done": true, "value": value })),
                Some(Err(error)) => Err(error),
                None => unreachable!(),
            },
            Some(_) => Ok(serde_json::json!({ "done": false })),
        }
    }

    /// Run a locator action through the assistive input paths.
    pub fn automate(
        &self,
        css: &str,
        index: Option<usize>,
        act: crate::interaction::view::AutomationAct,
        force: bool,
    ) -> Result<(), String> {
        let page = self.inner.lifecycle.page();
        let scroll = act == crate::interaction::view::AutomationAct::ScrollIntoView;
        // Bound first: the guard must drop before the miss path locks again.
        let result = self
            .inner
            .main()
            .interaction
            .automate(css, index, act, force, page);
        let effects = match result {
            Ok(effects) => effects,
            Err(error) if error.starts_with("Element not found") && index.unwrap_or(0) == 0 => {
                // An off-screen List row: scroll its list there; the action retries once it draws.
                let Some((list, offset)) = self.inner.virtual_row(css) else {
                    return Err(error);
                };
                self.inner.main().interaction.scroll_list_to(&list, offset);
                self.inner.deliver(vec![
                    crate::interaction::view::Effect::StopFling,
                    crate::interaction::view::Effect::Redraw,
                ]);
                return if scroll {
                    Ok(())
                } else {
                    Err(format!(
                        "Element not interactable: {css} is outside the viewport"
                    ))
                };
            }
            Err(error) => return Err(error),
        };
        self.inner.deliver(effects);
        if self.inner.take_edit_failure() {
            return Err(format!("Element not editable: {css} refused the edit"));
        }
        Ok(())
    }

    /// Drag the static-text selection's anchor (0) or focus (1) end to a logical point.
    pub fn move_text_selection(&self, identity: String, which: u8, point: [f32; 2]) {
        self.inner.change_text_selection(move |session| {
            session.move_text_selection(&identity, which as usize, point)
        });
    }

    /// Select UTF-16 `start..end` of a selectable text node (accessibility, Select all).
    pub fn set_text_selection(&self, identity: String, start: u32, end: u32) {
        self.inner.change_text_selection(move |session| {
            session.set_text_selection(&identity, start, end)
        });
    }

    /// The selected text for Copy, then the selection is dropped.
    pub fn copy_text_selection(&self, reply: impl FnOnce(Option<String>) + Send + 'static) {
        let page = self.inner.lifecycle.page();
        self.inner.work(
            Guard::Page(page),
            |_, session| {
                let text = session.selected_text();
                Ok((text, session.clear_text_selection()))
            },
            move |inner, result| {
                let (text, cleared) = match result {
                    Some(Ok(done)) => done,
                    _ => (None, false),
                };
                if cleared && inner.lifecycle.is_current_page(page) {
                    inner.main().dirty = true;
                    inner.schedule();
                }
                reply(text);
            },
        );
    }

    /// Drop the static-text selection, e.g. when its action mode is dismissed.
    pub fn clear_text_selection(&self) {
        self.inner
            .change_text_selection(|session| Ok(session.clear_text_selection()));
    }

    /// Advance a fling; returns false once the region stops moving.
    pub fn scroll_by(&self, region: &str, delta_px: f32) -> bool {
        let changed = self.inner.main().interaction.scroll_by(region, delta_px);
        if changed {
            self.inner.schedule();
            self.inner.snap_later();
        }
        changed
    }

    pub fn can_scroll_vertically(&self, direction: i32) -> bool {
        self.inner
            .main()
            .interaction
            .can_scroll_vertically(direction)
    }

    /// [`Self::can_scroll_vertically`] for the regions under a device-pixel point, when
    /// no touch is down (wheel and trackpad input).
    pub fn can_scroll_vertically_at(&self, point: [f32; 2], direction: i32) -> bool {
        self.inner
            .main()
            .interaction
            .can_scroll_vertically_at(point[0], point[1], direction)
    }

    pub fn can_scroll_horizontally_at(&self, point: [f32; 2], direction: i32) -> bool {
        self.inner
            .main()
            .interaction
            .can_scroll_horizontally_at(point[0], point[1], direction)
    }

    pub fn keyboard_focus(&self) -> Option<usize> {
        self.inner.main().interaction.keyboard_focus()
    }

    /// Read the presented semantic projection and the accessibility focus.
    /// `read` must not call back into the host.
    pub fn semantics<T>(&self, read: impl FnOnce(Option<&Projection<'_>>, Option<u32>) -> T) -> T {
        let main = self.inner.main();
        let projection = main.interaction.semantics();
        read(projection.as_ref(), main.interaction.accessibility_focus())
    }

    pub fn snapshot(&self) -> Option<Arc<FrameSnapshot>> {
        self.inner.main().snapshot.clone()
    }

    /// The last failure still shown, if any.
    pub fn failure(&self) -> Option<Failure> {
        self.inner.main().failure.clone()
    }

    /// An input-method protocol request; `reply` runs on the main thread with
    /// the session's JSON reply, or `None` after a failure. Stale requests never reply.
    pub fn input(&self, request: String, reply: impl FnOnce(Option<String>) + Send + 'static) {
        self.inner.input(request, Box::new(reply));
    }

    /// The focused editor's text state; `None` when no editor is connected.
    /// `eligible` is whether the caret may blink (window focused, page active).
    pub fn caret(&self, eligible: bool, state: Option<CaretState>) {
        self.inner.caret(eligible, state);
    }

    /// Whether the caret paints in the next frame.
    pub fn caret_visible(&self) -> bool {
        !self.inner.main().caret.hidden
    }

    /// Image bytes for a request from `ImageProvider::fetch`. Any thread.
    pub fn image_loaded(&self, id: u64, bytes: Result<Vec<u8>, String>) {
        let inner = self.inner.clone();
        self.inner
            .adapter
            .post(Box::new(move || inner.image_loaded(id, bytes)));
    }
}

impl<A: PlatformAdapter> Inner<A> {
    /// An input reached the page now; a platform timestamp given just before wins.
    fn input_now(&self) {
        let at = self.adapter.now();
        self.main().telemetry.stats.input(at);
    }

    fn main(&self) -> MutexGuard<'_, Main<A::Frame>> {
        self.main
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    fn frame_is_current(&self) -> bool {
        if self.stale.0.load(std::sync::atomic::Ordering::SeqCst) {
            return false;
        }
        let Some(mut environment) = self.adapter.environment() else {
            return false;
        };
        let main = self.main();
        main.host_environment.apply(&mut environment.value);
        main.presented_environment
            .as_ref()
            .is_some_and(|presented| main.host_environment.same(presented, &environment))
    }

    fn current(&self, guard: Guard) -> bool {
        match guard {
            Guard::Page(page) => self.lifecycle.is_current_page(page),
            Guard::Connection(connection) => self.lifecycle.is_current_connection(connection),
        }
    }

    /// Run `job` on the worker against this page's session, then `done` on the
    /// main thread with its result, or `None` when `guard` went stale first.
    fn work<R: Send + 'static>(
        self: &Arc<Self>,
        guard: Guard,
        job: impl FnOnce(&Arc<Self>, &mut ViewSession) -> Result<R, String> + Send + 'static,
        done: impl FnOnce(&Arc<Self>, Option<Result<R, String>>) + Send + 'static,
    ) {
        self.work_at(false, guard, job, done);
    }

    /// `background` work waits until no shown page has work queued on the shared worker.
    fn work_at<R: Send + 'static>(
        self: &Arc<Self>,
        background: bool,
        guard: Guard,
        job: impl FnOnce(&Arc<Self>, &mut ViewSession) -> Result<R, String> + Send + 'static,
        done: impl FnOnce(&Arc<Self>, Option<Result<R, String>>) + Send + 'static,
    ) {
        let inner = self.clone();
        let run = move |app: &mut crate::ViewApp| {
            let result = inner.current(guard).then(|| {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let id = (*inner.page.lock().unwrap_or_else(|p| p.into_inner()))
                        .ok_or("UI_SESSION: page not loaded")?;
                    job(&inner, app.page(id)?)
                }))
                .unwrap_or_else(|_| Err("UI_RENDER: native panic".into()))
            });
            let host = inner.clone();
            inner.adapter.post(Box::new(move || done(&host, result)));
        };
        if background {
            self.worker.run_background(run);
        } else {
            self.worker.run(run);
        }
    }

    fn load(self: &Arc<Self>, source: Option<String>) {
        let (page, source, effects, cancelled) = {
            let mut main = self.main();
            if main.disposed {
                return;
            }
            if let Some(source) = source {
                main.source = source;
            }
            main.animation_timer += 1;
            main.bootstrap_timer += 1;
            main.failure = None;
            main.failure_repeats = 0;
            main.stalled_at = None;
            let page = self.lifecycle.load();
            main.telemetry.opened(self.adapter.now());
            main.snapshot = None;
            // The surface still holds the previous instance's frame; it is not this page's.
            main.presented_environment = None;
            self.stale
                .0
                .store(false, std::sync::atomic::Ordering::SeqCst);
            main.forget_frames();
            main.dirty = true;
            main.caret.stop();
            main.styled_press = None;
            main.styled_hover = None;
            main.styled_focus = None;
            main.interest.reset();
            main.autoclose.reset();
            let effects = main.interaction.reset();
            main.interaction.set_inert(false);
            let mut cancelled = main.images.inflight_tokens();
            cancelled.extend(main.fonts.drain().map(|(token, _)| token));
            main.images.clear();
            (page, main.source.clone(), effects, cancelled)
        };
        self.adapter.recovery(None);
        self.adapter.bootstrap_overdue(false);
        self.adapter.reset();
        self.deliver(effects);
        for token in cancelled {
            self.adapter.cancel(token);
        }
        let inner = self.clone();
        let logic = self.logic;
        self.worker.run(move |app| {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut slot = inner.page.lock().unwrap_or_else(|p| p.into_inner());
                if let Some(old) = slot.take() {
                    app.discard(old);
                }
                if !inner.lifecycle.is_current_page(page) {
                    return Ok(false);
                }
                let host = std::sync::Arc::downgrade(&inner);
                let console: crate::engine::console::ConsoleSink =
                    std::sync::Arc::new(move |level, message: &str| {
                        if let Some(host) = host.upgrade() {
                            host.adapter.console(level, message);
                        }
                    });
                *slot = Some(app.create_with_console(&source, logic, Some(console))?);
                Ok(true)
            }))
            .unwrap_or_else(|_| Err("UI_RENDER: native panic".into()));
            let host = inner.clone();
            inner.adapter.post(Box::new(move || match result {
                Ok(true) if !host.main().disposed && host.lifecycle.loaded(page) => {
                    host.adapter.loaded();
                    host.main().dirty = true;
                    host.schedule();
                    host.load_fonts(page);
                }
                Ok(_) => {}
                Err(error) if host.lifecycle.is_current_page(page) => host.fail(error),
                Err(_) => {}
            }));
        });
    }

    fn park(self: &Arc<Self>) {
        let (page, effects, cancelled) = {
            let mut main = self.main();
            if main.disposed {
                return;
            }
            main.animation_timer += 1;
            main.bootstrap_timer += 1;
            main.failure = None;
            main.failure_repeats = 0;
            main.stalled_at = None;
            main.telemetry.stats.abandon_reveal();
            let page = self.lifecycle.park();
            main.snapshot = None;
            main.presented_environment = None;
            self.stale
                .0
                .store(false, std::sync::atomic::Ordering::SeqCst);
            main.forget_frames();
            main.dirty = true;
            main.caret.stop();
            main.styled_press = None;
            main.styled_hover = None;
            main.styled_focus = None;
            main.interest.reset();
            main.autoclose.reset();
            // Input stops; automation reads a parked page as blank.
            let effects = main.interaction.park();
            for slot in main.automation_calls.values_mut() {
                slot.get_or_insert_with(|| Err("E_PAGE_NOT_READY: the page was parked".into()));
            }
            let cancelled = main.images.inflight_tokens();
            main.images.clear();
            (page, effects, cancelled)
        };
        self.adapter.recovery(None);
        self.adapter.bootstrap_overdue(false);
        self.deliver(effects);
        for token in cancelled {
            self.adapter.cancel(token);
        }
        self.work(
            Guard::Page(page),
            |_, session| session.reset(),
            move |inner, result| {
                if let Some(Err(error)) = result
                    && inner.lifecycle.is_current_page(page)
                {
                    inner.fail(error);
                }
            },
        );
    }

    fn dispose(self: &Arc<Self>) {
        let (effects, cancelled) = {
            let mut main = self.main();
            if main.disposed {
                return;
            }
            main.disposed = true;
            main.animation_timer += 1;
            main.bootstrap_timer += 1;
            main.forget_frames();
            main.caret.stop();
            let effects = main.interaction.cancel_all();
            let cancelled = main.images.inflight_tokens();
            main.images.clear();
            (effects, cancelled)
        };
        self.deliver(effects);
        self.lifecycle.dispose();
        for token in cancelled.into_iter().chain(self.take_font_fetches()) {
            self.adapter.cancel(token);
        }
        let inner = self.clone();
        self.worker.run(move |app| {
            if let Some(id) = inner.page.lock().unwrap_or_else(|p| p.into_inner()).take() {
                app.discard(id);
            }
        });
    }

    /// A hidden page's videos hold still: no decoding, no frames, the same place when it shows.
    fn hide_videos(self: &Arc<Self>, hidden: bool) {
        let inner = self.clone();
        self.worker.run(move |app| {
            let Some(id) = *inner.page.lock().unwrap_or_else(|p| p.into_inner()) else {
                return;
            };
            if let Ok(session) = app.page(id) {
                crate::renderer::video_hidden(session.video_scope(), hidden);
            }
        });
    }

    fn set_visible(self: &Arc<Self>, visible: bool) {
        let (disposed, shown) = {
            let mut main = self.main();
            let shown = visible && !main.visible;
            if !visible && main.visible {
                // The page coming in builds first; this page's catch-up waits for its frame.
                self.worker.hold();
            }
            main.visible = visible;
            (main.disposed, shown)
        };
        // Hosts without the reveal protocol show a page by making it visible.
        if shown && !disposed {
            let at = self.adapter.now();
            let current = self.frame_is_current();
            let mut main = self.main();
            main.telemetry.stats.reveal_started(at);
            if current {
                main.telemetry.stats.reveal_current(at);
            }
        }
        self.lifecycle.set_visible(visible && !disposed);
        self.hide_videos(!visible || self.window_stopped());
        if visible {
            self.pump_images();
            self.schedule();
        } else {
            // Nothing presents while hidden: held writes are acknowledged now.
            self.release_acks(u64::MAX);
            self.park_animations();
            self.clear_text_selection();
            self.adapter.release_transient();
        }
    }

    /// Run a selection change on the worker; a change repaints.
    fn change_text_selection(
        self: &Arc<Self>,
        change: impl FnOnce(&mut ViewSession) -> Result<bool, String> + Send + 'static,
    ) {
        let page = self.lifecycle.page();
        self.work(
            Guard::Page(page),
            move |_, session| change(session),
            move |inner, result| {
                if matches!(result, Some(Ok(true))) && inner.lifecycle.is_current_page(page) {
                    inner.main().dirty = true;
                    inner.schedule();
                }
            },
        );
    }

    /// A hidden page keeps no selection: its kept frame catches up without the handles.
    fn clear_text_selection(self: &Arc<Self>) {
        let page = self.lifecycle.page();
        self.work(
            Guard::Page(page),
            |_, session| Ok(session.clear_text_selection()),
            move |inner, result| {
                if !matches!(result, Some(Ok(true))) || !inner.lifecycle.is_current_page(page) {
                    return;
                }
                {
                    let mut main = inner.main();
                    main.dirty = true;
                    main.catch_up |= !main.visible;
                }
                inner.schedule();
            },
        );
    }

    fn retry(self: &Arc<Self>) {
        let Some(failure) = self.main().failure.clone() else {
            return;
        };
        {
            let mut main = self.main();
            main.stalled_at = None;
            main.failure_repeats = 0;
        }
        match failure.recovery.retry {
            Retry::None => {}
            Retry::Redraw => {
                self.main().dirty = true;
                self.schedule();
            }
            Retry::Reload => self.load(None),
            Retry::Restart => {
                if !self.adapter.restart(self.lifecycle.connection()) {
                    let failure = Failure {
                        action: None,
                        ..failure
                    };
                    self.main().failure = Some(failure.clone());
                    self.adapter.recovery(Some(&failure));
                }
            }
        }
    }

    /// Record a failure, stop scheduling and show recovery chrome.
    fn fail(self: &Arc<Self>, error: String) {
        let (failure, effects, repeats) = {
            let mut main = self.main();
            if main.disposed {
                return;
            }
            main.animation_timer += 1;
            main.bootstrap_timer += 1;
            main.pending = false;
            main.telemetry.stats.abandon_reveal();
            let repeated = main
                .failure
                .as_ref()
                .is_some_and(|last| last.error == error);
            main.failure_repeats = if repeated {
                main.failure_repeats + 1
            } else {
                1
            };
            if main.failure_repeats >= STALL_AFTER {
                main.stalled_at = self
                    .adapter
                    .environment()
                    .map(|environment| (environment.width, environment.height));
            }
            let recovery = self.lifecycle.fail(&error);
            let effects = if recovery.kind == FailureKind::Session {
                main.interaction.set_inert(true);
                main.caret.stop();
                main.interaction.cancel_all()
            } else {
                Vec::new()
            };
            let failure = Failure::new(error, recovery, main.snapshot.is_some());
            main.failure = Some(failure.clone());
            (failure, effects, main.failure_repeats)
        };
        if failure.recovery.kind == FailureKind::Session {
            self.adapter.reset();
        }
        self.deliver(effects);
        self.adapter.bootstrap_overdue(false);
        // A repeat is already shown and logged; the stall is said once.
        if repeats == 1 {
            self.adapter.warn(&failure.error);
            self.adapter.recovery(Some(&failure));
        } else if repeats == STALL_AFTER {
            self.adapter.warn(&format!(
                "{} (failed {STALL_AFTER} times in a row; rendering stops until a retry)",
                failure.error
            ));
        }
        self.revealed_by_failure();
    }

    fn caret(self: &Arc<Self>, eligible: bool, state: Option<CaretState>) {
        let (repaint, timer) = {
            let mut main = self.main();
            match state {
                Some(state) if eligible => main.caret.update(eligible, state),
                Some(state) => {
                    drop(main);
                    self.dismiss_editor_menu();
                    self.main().caret.update(eligible, state)
                }
                None => {
                    main.caret.stop();
                    return;
                }
            }
        };
        if let Some(timer) = timer {
            self.blink(timer);
        }
        if repaint {
            self.schedule();
        }
    }

    fn blink(self: &Arc<Self>, timer: u64) {
        let inner = self.clone();
        self.adapter.post_after(
            CARET_BLINK,
            Box::new(move || {
                {
                    let mut main = inner.main();
                    if main.caret.timer != timer || !main.caret.running {
                        return;
                    }
                    main.caret.hidden = !main.caret.hidden;
                }
                inner.schedule();
                inner.blink(timer);
            }),
        );
    }
}
