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
mod dispatch;
mod images;
mod pump;
mod worker;

/// Largest input-method request [`ViewHost::input`] accepts, in bytes.
pub use super::input_protocol::MAX_REQUEST_BYTES as MAX_INPUT_REQUEST_BYTES;
pub use bridge::BridgeUpdate;
pub use worker::ViewWorker;

const BOOTSTRAP: Duration = Duration::from_secs(5);
const CARET_BLINK: Duration = Duration::from_millis(500);

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
    /// `{generation, nodes, images, metrics}` for adapters that marshal the frame.
    pub json: String,
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
    /// Vsync to the end of the presented frame's main-thread work.
    pub main_us: u64,
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
    main: Mutex<Main<A::Frame>>,
}

/// Main-thread state. Never held while calling the adapter.
struct Main<F> {
    source: String,
    interaction: ViewInteraction,
    images: ImageRequests,
    environment: Option<FrameEnvironment>,
    snapshot: Option<Arc<FrameSnapshot>>,
    prepared: Option<pump::Prepared<F>>,
    failure: Option<Failure>,
    styled_press: Option<String>,
    caret: Caret,
    visible: bool,
    dirty: bool,
    pending: bool,
    busy: bool,
    number_busy: bool,
    disposed: bool,
    /// Bumped to revoke a pending animation or bootstrap deadline.
    animation_timer: u64,
    bootstrap_timer: u64,
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
        Self {
            inner: Arc::new(Inner {
                adapter,
                worker,
                lifecycle: Lifecycle::new(logic),
                logic,
                page: Mutex::new(None),
                main: Mutex::new(Main {
                    source: String::new(),
                    interaction: ViewInteraction::default(),
                    images: ImageRequests::default(),
                    environment: None,
                    snapshot: None,
                    prepared: None,
                    failure: None,
                    styled_press: None,
                    caret: Caret::default(),
                    visible: false,
                    dirty: true,
                    pending: false,
                    busy: false,
                    number_busy: false,
                    disposed: false,
                    animation_timer: 0,
                    bootstrap_timer: 0,
                }),
            }),
        }
    }

    pub fn adapter(&self) -> &A {
        &self.inner.adapter
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
        self.inner.set_visible(visible);
    }

    /// The presentation target was replaced or resized.
    pub fn surface_changed(&self) {
        self.inner.lifecycle.invalidate_surface();
        self.request_frame();
    }

    /// The presentation target is gone; nothing presents until it returns.
    pub fn surface_lost(&self) {
        self.inner.lifecycle.invalidate_surface();
        self.inner.park_animations();
        let effects = self.inner.main().interaction.cancel_all();
        self.inner.deliver(effects);
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

    /// The display frame requested through `FrameScheduler::request_vsync`.
    pub fn vsync(&self) {
        self.inner.vsync();
    }

    /// Run the recovery action offered with the last failure.
    pub fn retry(&self) {
        self.inner.retry();
    }

    /// A failure the platform detected outside the host, classified like View failures.
    pub fn fail(&self, error: String) {
        self.inner.fail(error);
    }

    pub fn connect(&self, connection: ConnectionTicket, nonce: String) {
        self.inner.connect(connection, nonce);
    }

    pub fn receive(&self, connection: ConnectionTicket, frame: String) {
        self.inner.receive(connection, frame);
    }

    pub fn request_leave(&self, connection: ConnectionTicket, reason: String) {
        self.inner.request_leave(connection, reason);
    }

    /// Returns whether the gesture was consumed.
    pub fn pointer(&self, event: Pointer) -> bool {
        if !self.inner.lifecycle.accepts_input() {
            return false;
        }
        let page = self.inner.lifecycle.page();
        let (handled, effects) = self.inner.main().interaction.pointer(event, page);
        self.inner.deliver(effects);
        handled
    }

    pub fn long_press(&self) {
        let effects = self.inner.main().interaction.long_press();
        self.inner.deliver(effects);
    }

    pub fn key(&self, key: Key, shift: bool, repeat: bool) -> bool {
        let page = self.inner.lifecycle.page();
        let (handled, effects) = self.inner.main().interaction.key(key, shift, repeat, page);
        self.inner.deliver(effects);
        handled
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
        let page = self.inner.lifecycle.page();
        let (handled, effects) = self
            .inner
            .main()
            .interaction
            .accessibility(id, action, page);
        self.inner.deliver(effects);
        handled
    }

    /// Advance a fling; returns false once the region stops moving.
    pub fn scroll_by(&self, region: &str, delta_px: f32) -> bool {
        let changed = self.inner.main().interaction.scroll_by(region, delta_px);
        if changed {
            self.inner.schedule();
        }
        changed
    }

    pub fn can_scroll_vertically(&self, direction: i32) -> bool {
        self.inner
            .main()
            .interaction
            .can_scroll_vertically(direction)
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
    fn main(&self) -> MutexGuard<'_, Main<A::Frame>> {
        self.main
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
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
        let inner = self.clone();
        self.worker.run(move |app| {
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
        });
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
            let page = self.lifecycle.load();
            main.snapshot = None;
            main.prepared = None;
            main.busy = false;
            main.dirty = true;
            main.caret.stop();
            main.styled_press = None;
            let effects = main.interaction.reset();
            main.interaction.set_inert(false);
            let cancelled = main.images.inflight_tokens();
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
                *slot = Some(app.create(&source, logic)?);
                Ok(true)
            }))
            .unwrap_or_else(|_| Err("UI_RENDER: native panic".into()));
            let host = inner.clone();
            inner.adapter.post(Box::new(move || match result {
                Ok(true) if !host.main().disposed && host.lifecycle.loaded(page) => {
                    host.adapter.loaded();
                    host.main().dirty = true;
                    host.schedule();
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
            let page = self.lifecycle.park();
            main.snapshot = None;
            main.prepared = None;
            main.busy = false;
            main.dirty = true;
            main.caret.stop();
            main.styled_press = None;
            let effects = main.interaction.reset();
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
            main.prepared = None;
            main.caret.stop();
            let effects = main.interaction.cancel_all();
            let cancelled = main.images.inflight_tokens();
            main.images.clear();
            (effects, cancelled)
        };
        self.deliver(effects);
        self.lifecycle.dispose();
        for token in cancelled {
            self.adapter.cancel(token);
        }
        let inner = self.clone();
        self.worker.run(move |app| {
            if let Some(id) = inner.page.lock().unwrap_or_else(|p| p.into_inner()).take() {
                app.discard(id);
            }
        });
    }

    fn set_visible(self: &Arc<Self>, visible: bool) {
        let disposed = {
            let mut main = self.main();
            main.visible = visible;
            main.disposed
        };
        self.lifecycle.set_visible(visible && !disposed);
        if visible {
            self.pump_images();
            self.schedule();
        } else {
            self.park_animations();
        }
    }

    fn retry(self: &Arc<Self>) {
        let Some(failure) = self.main().failure.clone() else {
            return;
        };
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
        let (failure, effects) = {
            let mut main = self.main();
            if main.disposed {
                return;
            }
            main.animation_timer += 1;
            main.bootstrap_timer += 1;
            main.pending = false;
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
            (failure, effects)
        };
        if failure.recovery.kind == FailureKind::Session {
            self.adapter.reset();
        }
        self.deliver(effects);
        self.adapter.bootstrap_overdue(false);
        self.adapter.warn(&failure.error);
        self.adapter.recovery(Some(&failure));
    }

    fn caret(self: &Arc<Self>, eligible: bool, state: Option<CaretState>) {
        let (repaint, timer) = {
            let mut main = self.main();
            match state {
                Some(state) => main.caret.update(eligible, state),
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
