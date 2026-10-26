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
pub(crate) mod decode;
mod dispatch;
mod edits;
mod environment;
#[cfg(feature = "hot-swap")]
mod hot_swap;
mod images;
mod pump;
mod reveal;
mod telemetry;
mod text_menu;
mod worker;

/// Largest input-method request [`ViewHost::input`] accepts, in bytes.
pub use super::input_protocol::MAX_REQUEST_BYTES as MAX_INPUT_REQUEST_BYTES;
pub use bridge::BridgeUpdate;
pub use reveal::{FIRST_FRAME_WATCHDOG, REVEAL_DEADLINE};
pub use text_menu::{MenuAction, MenuTarget, TextMenu};
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
    /// Each source's decode size in this frame.
    pub image_buckets: std::collections::HashMap<String, crate::scene::image::Bucket>,
    pub metrics: crate::session::metrics::FrameMetrics,
    /// `{generation, nodes, images, metrics, feedback}` for adapters that marshal the
    /// frame; `feedback` is the [`crate::interaction::chrome::FeedbackStyle`] to draw.
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
    main: Mutex<Main<A::Frame>>,
}

/// Main-thread state. Never held while calling the adapter.
struct Main<F> {
    source: String,
    interaction: ViewInteraction,
    images: ImageRequests,
    environment: Option<FrameEnvironment>,
    /// The environment of the last frame the target accepted; cleared when the target changes.
    presented_environment: Option<FrameEnvironment>,
    host_environment: environment::HostEnvironment,
    snapshot: Option<Arc<FrameSnapshot>>,
    prepared: Option<pump::Prepared<F>>,
    /// A frame handed to the adapter that is not on screen yet.
    deferred: Option<pump::Deferred>,
    /// A frame prepared ahead, while `prepared` waits for its vsync, is in flight.
    ahead: bool,
    /// An ahead frame that finished before the frame under it presented.
    next_prepared: Option<pump::Prepared<F>>,
    /// Bumped to drop ahead frames built on a frame that will not present.
    ahead_epoch: u64,
    /// Host time of the last vsync and the display interval estimate.
    last_vsync: Option<Duration>,
    vsync_interval: Duration,
    /// Consecutive refusals of a frame that had a newer one prepared behind it.
    stale_declines: u32,
    /// The presenter keeps only its newest frame; nothing is prepared ahead any more.
    single_target: bool,
    #[cfg(test)]
    no_frames_ahead: bool,
    failure: Option<Failure>,
    styled_press: Option<String>,
    styled_focus: Option<String>,
    caret: Caret,
    visible: bool,
    /// A hidden page whose target changed renders once, so returning to it needs no wait.
    catch_up: bool,
    dirty: bool,
    pending: bool,
    busy: bool,
    number_busy: bool,
    disposed: bool,
    /// Bumped to revoke a pending animation or bootstrap deadline.
    animation_timer: u64,
    /// The last presented frame asked for the next display frame.
    animating: bool,
    /// When the last frame was queued behind an acknowledgement; one per instant.
    animation_queued_at: Duration,
    bootstrap_timer: u64,
    reveal: reveal::Reveal,
    telemetry: telemetry::Telemetry,
    text_menu: text_menu::MenuState,
    edits: edits::Edits,
}

impl<F> Main<F> {
    /// Forget frames prepared ahead: what they build on will not present.
    fn drop_ahead(&mut self) {
        self.next_prepared = None;
        self.ahead = false;
        self.ahead_epoch += 1;
    }

    /// Input waits only for frames that change the View: animation frames still queued for
    /// the presented layout are dropped so the activation runs now. Their commits belong
    /// to that layout, which the session still accepts.
    fn yield_to_input(&mut self) {
        let layout = self.snapshot.as_ref().and_then(|s| s.frame.layout);
        let same = |frame: &Option<pump::Prepared<F>>| {
            frame
                .as_ref()
                .is_none_or(|frame| layout.is_some() && frame.layout() == layout)
        };
        if self.deferred.is_none()
            && (self.prepared.is_some() || self.ahead)
            && same(&self.prepared)
            && same(&self.next_prepared)
        {
            self.prepared = None;
            self.drop_ahead();
            self.busy = false;
        }
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
                    presented_environment: None,
                    host_environment: Default::default(),
                    snapshot: None,
                    prepared: None,
                    deferred: None,
                    ahead: false,
                    next_prepared: None,
                    ahead_epoch: 0,
                    last_vsync: None,
                    vsync_interval: Duration::from_micros(16_667),
                    stale_declines: 0,
                    single_target: false,
                    #[cfg(test)]
                    no_frames_ahead: false,
                    failure: None,
                    styled_press: None,
                    styled_focus: None,
                    caret: Caret::default(),
                    visible: false,
                    catch_up: false,
                    dirty: true,
                    pending: false,
                    busy: false,
                    number_busy: false,
                    disposed: false,
                    animation_timer: 0,
                    animating: false,
                    animation_queued_at: Duration::ZERO,
                    bootstrap_timer: 0,
                    reveal: Default::default(),
                    telemetry: Default::default(),
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

    /// Dev sessions: replace the page module in place, keeping its state.
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
        self.inner.set_visible(visible);
    }

    /// The presentation target was replaced or resized; a size change still reruns the View.
    pub fn surface_changed(&self) {
        self.inner.lifecycle.invalidate_surface();
        let hidden = {
            let mut main = self.inner.main();
            main.presented_environment = None;
            main.catch_up = !main.visible;
            main.catch_up
        };
        // A hidden page's catch-up frame runs as background work, behind the shown page's frame.
        let _ = hidden;
        self.redraw();
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

    /// The window was stopped or started; a pending reveal waits while it is stopped.
    pub fn window_visible(&self, visible: bool) {
        self.inner.window_visible(visible);
    }

    /// The presentation target is gone; nothing presents until it returns.
    pub fn surface_lost(&self) {
        self.inner.lifecycle.invalidate_surface();
        self.inner.main().presented_environment = None;
        self.inner.park_animations();
        let effects = self.inner.main().interaction.cancel_all();
        self.inner.deliver(effects);
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

    /// The display frame requested through `FrameScheduler::request_vsync`.
    pub fn vsync(&self) {
        self.inner.vsync();
    }

    /// A deferred frame of `generation` reached the screen, or `shown: false` when the
    /// target dropped it. Any thread.
    pub fn deferred_presented(&self, generation: CommitTicket, shown: bool) {
        let inner = self.inner.clone();
        self.inner.adapter.post(Box::new(move || {
            inner.deferred_presented(generation, shown)
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
        if event.phase == crate::interaction::view::PointerPhase::Down {
            self.inner.dismiss_editor_menu();
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

    /// `false` leaves the key to the platform, as after a session failure.
    pub fn key(&self, key: Key, shift: bool, repeat: bool) -> bool {
        if !self.inner.lifecycle.accepts_input() {
            return false;
        }
        let page = self.inner.lifecycle.page();
        let (handled, effects) = self.inner.main().interaction.key(key, shift, repeat, page);
        self.inner.deliver(effects);
        handled
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
        }
    }

    /// A selection handle drag started or ended; the menu hides while dragging and
    /// an editor's menu returns on release.
    pub fn text_handle_drag(&self, active: bool) {
        self.inner.set_handle_drag(active);
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
        if let Some(handled) = self.static_text_action(id, &action) {
            return handled;
        }
        let page = self.inner.lifecycle.page();
        let (handled, effects) = self
            .inner
            .main()
            .interaction
            .accessibility(id, action, page);
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
                self.set_text_selection(identity, offset(start), offset(end));
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
        let main = self.inner.main();
        main.interaction.automation_query(css, index, all, max_text)
    }

    /// One automation request from the page driver:
    /// `{op: "query", css, index, all, maxText}` or
    /// `{op: "act", css, index, action: {kind: click|dblclick|fill|type|scroll|press|select, text, key, value, force}}`,
    /// `{op: "tree"}` or `{op: "frame"}` (presentation counters).
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
                    other => return Err(format!("E_UNSUPPORTED: automation action {other:?}")),
                };
                let force = action["force"].as_bool().unwrap_or(false);
                self.automate(css, index, act, force)
                    .map(|()| serde_json::Value::Null)
            }
            Some("tree") => self.inner.main().interaction.automation_tree(),
            Some("frame") => Ok(self.inner.frame_metrics()),
            other => Err(format!("E_UNSUPPORTED: automation op {other:?}")),
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
        let effects = self
            .inner
            .main()
            .interaction
            .automate(css, index, act, force, page)?;
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

    fn frame_is_current(&self) -> bool {
        let Some(mut environment) = self.adapter.environment() else {
            return false;
        };
        let main = self.main();
        main.host_environment.apply(&mut environment.value);
        main.presented_environment.as_ref() == Some(&environment)
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
            let page = self.lifecycle.load();
            main.telemetry.opened(self.adapter.now());
            main.snapshot = None;
            main.prepared = None;
            main.deferred = None;
            main.drop_ahead();
            main.busy = false;
            main.dirty = true;
            main.caret.stop();
            main.styled_press = None;
            main.styled_focus = None;
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
            main.deferred = None;
            main.drop_ahead();
            main.busy = false;
            main.dirty = true;
            main.caret.stop();
            main.styled_press = None;
            main.styled_focus = None;
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
            main.deferred = None;
            main.drop_ahead();
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
