//! The shared core driven through a fake platform adapter on a test "main" thread.
use crate::DisplayList;
use crate::interaction::view::{AccessibilityEffect, Dispatch, Effect, Pointer, PointerPhase};
use crate::platform::*;
use crate::semantics::projection::Role;
use crate::session::host::{
    CaretState, Failure, FrameSnapshot, FrameTiming, RecoveryAction, RecoveryMessage, ViewHost,
    ViewWorker,
};
use crate::session::lifecycle::{
    CommitTicket, ConnectionTicket, FailureKind, Retry, SurfaceTicket,
};
use crate::session::state::StateSignal;
use crate::session::tests::{bounded_jobs, source, state_write};
use image::ImageEncoder;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::ThreadId;
use std::time::Duration;

#[path = "test_environment.rs"]
mod test_environment;

#[derive(Default)]
struct Log {
    prepared: Vec<ThreadId>,
    /// The focused editor's identity in each frame the input method reconciled with.
    reconciled: Vec<Option<String>>,
    /// Display sizes of frames the fake target accepted.
    accepted: Vec<(u16, u16)>,
    presented: Vec<Vec<String>>,
    /// Display sizes of the frames presented, in order.
    presented_sizes: Vec<(u16, u16)>,
    presented_at: Vec<std::time::Instant>,
    fetched: Vec<(u64, String)>,
    cancelled: Vec<u64>,
    events: Vec<AccessibilityEffect>,
    effects: Vec<Effect>,
    editors: Vec<Dispatch>,
    /// Text input ended by a non-editor activation or a hidden page.
    ended_input: usize,
    messages: Vec<(u64, String)>,
    signals: Vec<(u64, StateSignal)>,
    recovery: Vec<Option<Failure>>,
    warnings: Vec<String>,
    loaded: usize,
    clicked: usize,
    vsync: bool,
    carets: Vec<bool>,
    /// `hold_surface` of each reveal.
    revealed: Vec<bool>,
    copied: Vec<String>,
    /// Host chrome pushes, in order, with the frames presented before each.
    chrome: Vec<(usize, crate::platform::HostChrome)>,
    /// Edits offered to the input method and what it answered.
    edits: Vec<(Effect, EditOutcome)>,
    menus: Vec<Option<crate::session::host::TextMenu>>,
    /// Threads that ran main-thread callbacks.
    main: Vec<ThreadId>,
    /// Generations of frames the fake target accepted.
    generations: Vec<CommitTicket>,
    /// The last prepared frame's filled rectangles: colour and bounds.
    rects: Vec<([u8; 4], [f32; 4])>,
    /// The last prepared frame's strokes: their points and width.
    strokes: Vec<(Vec<[f32; 2]>, f32)>,
    /// Generations the host reported presented (`HostEvents::presented`).
    shown: Vec<CommitTicket>,
    /// The feedback style each presented frame carried.
    feedback: Vec<serde_json::Value>,
    /// Deferred frames handed to the fake raster thread, in order.
    handed: Vec<CommitTicket>,
    debug: Vec<String>,
    /// Lines the page's `console` wrote.
    console: Vec<String>,
    released_transient: usize,
    /// Whether each prepared frame carried the HUD overlay.
    hud: Vec<bool>,
    webviews: Vec<Vec<crate::scene::webview::WebViewMount>>,
    exited_webview_fs: Vec<String>,
    cleared_webviews: usize,
}

/// What a phone adapter reports, built as adapters build it, so the fake never lacks a key a
/// real host sends.
fn fake_environment() -> FrameEnvironment {
    let mut environment = crate::platform::DeviceFacts {
        width: 200,
        height: 400,
        scale: 2.0,
        os: "test".into(),
        smallest_width: 100.0,
        locale: "en".into(),
        animation_scale: 1.0,
        ..Default::default()
    }
    .environment()
    .unwrap();
    environment.value["canDock"] = false.into();
    environment
}

struct Fake {
    log: Mutex<Log>,
    tasks: Mutex<Sender<Task>>,
    timers: Arc<Mutex<Vec<(Duration, Task)>>>,
    clock: Mutex<Duration>,
    environment: Mutex<Option<FrameEnvironment>>,
    restart: bool,
    /// A target still attaching: frames are declined, never produced some other way.
    pending: std::sync::atomic::AtomicBool,
    /// Presenting only queues the frame, as a CPU raster thread does.
    defer: std::sync::atomic::AtomicBool,
    /// Simulated worker and UI-thread costs; frames present only once ready.
    costs: Mutex<Option<Costs>>,
    /// Answers to the next `edit_text` calls; `Applied` once they run out.
    edit_outcomes: Mutex<std::collections::VecDeque<EditOutcome>>,
}

impl Fake {
    fn main(&self) -> std::sync::MutexGuard<'_, Log> {
        let mut log = self.log.lock().unwrap();
        log.main.push(std::thread::current().id());
        log
    }
}

/// A frame encoded on the worker: its texts and whether the caret painted.
struct Encoded {
    texts: Vec<String>,
    caret: bool,
    generation: CommitTicket,
    size: (u16, u16),
    /// The offscreen target this frame holds until it is presented or dropped.
    target: Option<Arc<Mutex<Vec<CommitTicket>>>>,
}

impl Drop for Encoded {
    fn drop(&mut self) {
        if let Some(held) = &self.target {
            held.lock().unwrap().retain(|g| *g != self.generation);
        }
    }
}

impl Presenter for Fake {
    type Frame = Encoded;
    fn prepare(
        &self,
        display: DisplayList,
        _: SurfaceTicket,
        generation: CommitTicket,
    ) -> Result<Option<Encoded>, String> {
        {
            let mut log = self.log.lock().unwrap();
            log.generations.push(generation);
            log.rects = display
                .commands
                .iter()
                .filter_map(|paint| match paint {
                    crate::scene::paint::Paint::Rect { color, bounds, .. } => {
                        Some((*color, *bounds))
                    }
                    _ => None,
                })
                .collect();
            log.strokes = display
                .commands
                .iter()
                .filter_map(|paint| match paint {
                    crate::scene::paint::Paint::Stroke { points, width, .. } => {
                        Some((points.clone(), *width))
                    }
                    _ => None,
                })
                .collect();
            log.hud.push(
                display
                    .animated_spans()
                    .iter()
                    .any(|span| span.identity == "lingxia:hud"),
            );
        }
        let texts: Vec<String> = display
            .nodes
            .iter()
            .map(|node| node.text.clone())
            .filter(|t| !t.is_empty())
            .collect();
        if let Some(costs) = self.costs.lock().unwrap().as_mut() {
            if costs.decline_new_views && costs.declined_view.as_ref() != Some(&texts) {
                // The first frame of each new View finds no target free.
                costs.declined_view = Some(texts);
                costs.declined += 1;
                return Ok(None);
            }
            if costs
                .targets
                .is_some_and(|targets| costs.held.lock().unwrap().len() >= targets)
            {
                // Every offscreen target holds a frame not yet presented or dropped.
                costs.declined += 1;
                return Ok(None);
            }
            if costs.targets.is_some() {
                costs.held.lock().unwrap().push(generation);
            }
            // A worker job starts when the worker and the main thread that queued it are free.
            let start = costs.worker_free.max(costs.main_free);
            costs.worker_free = start + costs.worker;
            costs
                .ready
                .push_back(costs.worker_free.max(costs.main_free));
        }
        self.log
            .lock()
            .unwrap()
            .prepared
            .push(std::thread::current().id());
        if self.pending.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(None);
        }
        self.log
            .lock()
            .unwrap()
            .accepted
            .push((display.width, display.height));
        Ok(Some(Encoded {
            texts: display
                .nodes
                .iter()
                .map(|node| node.text.clone())
                .filter(|t| !t.is_empty())
                .collect(),
            caret: display.caret_paint.is_some(),
            generation,
            size: (display.width, display.height),
            target: self
                .costs
                .lock()
                .unwrap()
                .as_ref()
                .filter(|costs| costs.targets.is_some())
                .map(|costs| costs.held.clone()),
        }))
    }
    fn present(&self, frame: Encoded) -> Result<bool, String> {
        if self.defer.load(std::sync::atomic::Ordering::Acquire) {
            self.log.lock().unwrap().handed.push(frame.generation);
        }
        if let Some(costs) = self.costs.lock().unwrap().as_mut() {
            costs.ready.pop_front();
            costs.main_free = *self.clock.lock().unwrap() + costs.ui;

            let newest = self.log.lock().unwrap().generations.last().copied();
            if costs.single_target && newest.is_some_and(|newest| newest != frame.generation) {
                // One offscreen target: a later prepare overwrote this frame.
                costs.ready.clear();
                return Ok(false);
            }
            if costs.decline > 0 {
                costs.decline -= 1;
                // Frames prepared on top of a refused one are dropped with it.
                costs.ready.clear();
                return Ok(false);
            }
        }
        let mut log = self.main();
        log.carets.push(frame.caret);
        log.presented.push(frame.texts.clone());
        log.presented_sizes.push(frame.size);
        log.presented_at.push(std::time::Instant::now());
        drop(log);
        if let Some(costs) = self.costs.lock().unwrap().as_mut()
            && costs.targets.is_some()
        {
            // The frame on screen keeps its target until the next one replaces it.
            costs.on_screen = Some(frame);
        }
        Ok(true)
    }
    fn deferred(&self, _: &Encoded) -> bool {
        self.defer.load(std::sync::atomic::Ordering::Acquire)
    }
    fn release_transient(&self) {
        self.log.lock().unwrap().released_transient += 1;
    }
}
impl FrameScheduler for Fake {
    fn request_vsync(&self) {
        self.main().vsync = true;
    }
}
impl MainThread for Fake {
    fn post(&self, task: Task) {
        let _ = self.tasks.lock().unwrap().send(task);
    }
    fn post_after(&self, delay: Duration, task: Task) {
        self.timers.lock().unwrap().push((delay, task));
    }
}
impl EnvironmentSource for Fake {
    fn environment(&self) -> Option<FrameEnvironment> {
        self.environment.lock().unwrap().clone()
    }
}
impl ImageProvider for Fake {
    fn fetch(&self, id: u64, source: &str) {
        self.main().fetched.push((id, source.into()));
    }
    fn cancel(&self, id: u64) {
        self.main().cancelled.push(id);
    }
}
impl AccessibilitySink for Fake {
    fn announce(&self, events: &[AccessibilityEffect]) {
        self.main().events.extend_from_slice(events);
    }
    fn clicked(&self) {
        self.main().clicked += 1;
    }
}
impl InputMethod for Fake {
    fn reconcile(&self, frame: &FrameSnapshot) {
        let focused = frame
            .frame
            .nodes
            .iter()
            .find(|node| {
                node.input
                    .as_ref()
                    .is_some_and(|input| input.enabled && input.edit_token.is_some_and(|t| t > 0))
            })
            .map(|node| node.identity.to_string());
        self.main().reconciled.push(focused);
    }
    fn focus(&self, editor: &Dispatch, _: &FrameSnapshot) {
        self.main().editors.push(editor.clone());
    }
    fn activation(&self) {
        self.main().ended_input += 1;
    }
    fn reset(&self) {}
    fn edit_text(&self, node: usize, text: Option<&str>, start: i32, end: i32) -> EditOutcome {
        let outcome = self
            .edit_outcomes
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(EditOutcome::Applied);
        self.main().edits.push((
            Effect::EditText {
                node,
                text: text.map(str::to_owned),
                start,
                end,
            },
            outcome,
        ));
        outcome
    }
}
impl Feedback for Fake {
    fn apply(&self, effects: &[Effect]) {
        self.main().effects.extend_from_slice(effects);
    }
}
impl HostEvents for Fake {
    fn loaded(&self) {
        self.main().loaded += 1;
    }
    fn presented(&self, frame: &FrameSnapshot, _: &FrameTiming) {
        let json: serde_json::Value = serde_json::from_str(frame.json()).unwrap();
        self.main().feedback.push(json["feedback"].clone());
        self.main().shown.push(frame.generation);
    }
    fn logic(&self, connection: ConnectionTicket, messages: Vec<crate::session::logic::ToLogic>) {
        // The worker calls this; it must not count as a main-thread call.
        let mut log = self.log.lock().unwrap();
        for message in messages {
            log.messages
                .push((connection.get(), message.frame().to_string()));
        }
    }
    fn state(&self, connection: ConnectionTicket, signal: StateSignal) {
        self.main().signals.push((connection.get(), signal));
    }
    fn restart(&self, _: ConnectionTicket) -> bool {
        self.restart
    }
    fn recovery(&self, failure: Option<&Failure>) {
        self.main().recovery.push(failure.cloned());
    }
    fn revealed(&self, hold_surface: bool) {
        self.main().revealed.push(hold_surface);
    }
    fn copy_text(&self, text: &str) {
        self.main().copied.push(text.into());
    }
    fn host_chrome(&self, chrome: &crate::platform::HostChrome) {
        let mut log = self.main();
        let presented = log.presented.len();
        log.chrome.push((presented, *chrome));
    }
    fn sync_webviews(&self, mounts: &[crate::scene::webview::WebViewMount]) {
        self.main().webviews.push(mounts.to_vec());
    }
    fn exit_webview_fullscreen(&self, web_id: &str) {
        self.main().exited_webview_fs.push(web_id.to_owned());
    }
    fn clear_webviews(&self) {
        self.main().cleared_webviews += 1;
    }
    fn text_menu(&self, menu: Option<&crate::session::host::TextMenu>) {
        self.main().menus.push(menu.cloned());
    }
    // Warnings may come from the worker, so they skip the main-thread record.
    fn warn(&self, message: &str) {
        self.log.lock().unwrap().warnings.push(message.to_owned());
    }
    fn console(&self, _: log::Level, message: &str) {
        self.log.lock().unwrap().console.push(message.to_owned());
    }
    fn debug(&self, message: &str) {
        self.log.lock().unwrap().debug.push(message.to_owned());
    }
}
impl Clock for Fake {
    fn now(&self) -> Duration {
        *self.clock.lock().unwrap()
    }
}

/// The test thread plays the platform main thread for one or more hosts.
struct Platform {
    tasks: Receiver<Task>,
    sender: Sender<Task>,
    timers: Arc<Mutex<Vec<(Duration, Task)>>>,
    main: ThreadId,
    workers: Vec<Arc<ViewWorker>>,
}

impl Platform {
    fn new(workers: &[&Arc<ViewWorker>]) -> Self {
        let (sender, tasks) = channel();
        Self {
            tasks,
            sender,
            timers: Default::default(),
            main: std::thread::current().id(),
            workers: workers.iter().map(|worker| Arc::clone(worker)).collect(),
        }
    }

    fn host(&self, worker: &Arc<ViewWorker>, page: &str, logic: bool) -> ViewHost<Fake> {
        let host = ViewHost::new(
            Fake {
                log: Default::default(),
                tasks: Mutex::new(self.sender.clone()),
                timers: self.timers.clone(),
                clock: Mutex::new(Duration::ZERO),
                environment: Mutex::new(Some(fake_environment())),
                restart: false,
                pending: Default::default(),
                defer: Default::default(),
                costs: Default::default(),
                edit_outcomes: Default::default(),
            },
            worker.clone(),
            logic,
        );
        host.load(page.into());
        host
    }

    /// Run posted work and vsyncs until the worker and every host are quiet.
    fn settle(&self, hosts: &[&ViewHost<Fake>]) {
        self.settle_with(hosts, true);
    }

    /// `decodes: false` leaves image decodes in flight, as a stalled decode would.
    fn settle_with(&self, hosts: &[&ViewHost<Fake>], decodes: bool) {
        let mut vsyncs = 0;
        loop {
            if decodes {
                crate::session::host::decode::wait_idle();
            }
            // Each queue is FIFO and background work runs after foreground work: once a
            // background marker runs, earlier jobs of both levels have posted their results.
            for worker in &self.workers {
                let (done, wait) = channel();
                worker.run_background(move |_| {
                    let _ = done.send(());
                });
                wait.recv_timeout(Duration::from_secs(30)).unwrap();
            }
            let mut ran = false;
            while let Ok(task) = self.tasks.try_recv() {
                task();
                ran = true;
            }
            for host in hosts {
                if std::mem::take(&mut host.adapter().log.lock().unwrap().vsync) {
                    vsyncs += 1;
                    // The clock never moves here, so a timed animation would ask for frames forever.
                    assert!(
                        vsyncs < 1_000,
                        "settle: still asking for frames after 1000 vsyncs on a frozen clock; \
                         a timed animation is running (advance the clock, or settle without it)"
                    );
                    host.vsync();
                    ran = true;
                }
            }
            if !ran {
                break;
            }
        }
        for host in hosts {
            let log = host.adapter().log.lock().unwrap();
            assert!(log.main.iter().all(|thread| *thread == self.main));
            assert!(log.prepared.iter().all(|thread| *thread != self.main));
        }
    }

    /// Fire every timer due within `elapsed`; returns how many ran.
    fn advance(&self, elapsed: Duration) -> usize {
        let due: Vec<Task> = {
            let mut timers = self.timers.lock().unwrap();
            let (due, later): (Vec<_>, Vec<_>) =
                timers.drain(..).partition(|(delay, _)| *delay <= elapsed);
            *timers = later
                .into_iter()
                .map(|(delay, task)| (delay - elapsed, task))
                .collect();
            due.into_iter().map(|(_, task)| task).collect()
        };
        let count = due.len();
        for task in due {
            task();
        }
        count
    }
}

fn tap(host: &ViewHost<Fake>, x: f32, y: f32) {
    for phase in [PointerPhase::Down, PointerPhase::Up] {
        host.pointer(Pointer {
            phase,
            x,
            y,
            slop: 8.0,
            velocity: [0.0; 2],
            min_fling: 50.0,
        });
    }
}

fn last(host: &ViewHost<Fake>) -> Vec<String> {
    host.adapter()
        .log
        .lock()
        .unwrap()
        .presented
        .last()
        .cloned()
        .unwrap_or_default()
}

fn png() -> Vec<u8> {
    png_of(0)
}

/// Distinct bytes per `seed`: decodes are cached process-wide by content.
fn png_of(seed: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(&[255, seed, 0, 255], 1, 1, image::ExtendedColorType::Rgba8)
        .unwrap();
    bytes
}

fn loaded_images(host: &ViewHost<Fake>) -> usize {
    host.snapshot().map_or(0, |snapshot| snapshot.loaded_images)
}

/// A Logic frame as it now reaches the host: state as a typed write, the rest as a message.
fn deliver(host: &ViewHost<Fake>, connection: ConnectionTicket, frame: serde_json::Value) {
    if frame["kind"]
        .as_str()
        .is_some_and(|kind| kind.starts_with("state."))
    {
        host.write_state(connection, state_write(&frame));
    } else {
        host.deliver(
            connection,
            crate::session::logic::ToView::from_frame(&frame),
        );
    }
}

#[test]
fn a_fake_platform_hosts_frames_input_semantics_and_images() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){const n=useLocal('n',0);return h('view',{},h('button',{'data-testid':'add',onClick:()=>n.set(n.value+1),style:{height:60}},`Count ${n.value}`),n.value<2?h('image',{src:'lx://pic.png',alt:'Picture',style:{width:20,height:20}}):h('text',{},'done'))}",
        ),
        false,
    );
    platform.settle(&[&host]);
    assert_eq!(host.adapter().log.lock().unwrap().loaded, 1);
    assert!(
        host.adapter().log.lock().unwrap().presented.is_empty(),
        "hidden pages do not present"
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    assert_eq!(last(&host), ["Count 0"]);
    let roles = host.semantics(|projection, _| {
        let projection = projection.unwrap();
        projection
            .roots()
            .iter()
            .filter_map(|id| projection.node(*id))
            .map(|node| node.role)
            .collect::<Vec<_>>()
    });
    assert_eq!(roles, [Role::Button, Role::Image]);
    let (image, source) = host.adapter().log.lock().unwrap().fetched[0].clone();
    assert_eq!(source, "lx://pic.png");
    tap(&host, 40.0, 40.0);
    platform.settle(&[&host]);
    assert!(
        host.adapter()
            .log
            .lock()
            .unwrap()
            .effects
            .contains(&Effect::Clicked)
    );
    assert_eq!(host.adapter().log.lock().unwrap().clicked, 1);
    assert_eq!(last(&host), ["Count 1"]);
    // Delivered from another thread, as a platform loader completes.
    let loader = host.clone();
    std::thread::spawn(move || loader.image_loaded(image, Ok(png())))
        .join()
        .unwrap();
    platform.settle(&[&host]);
    let presented = host.adapter().log.lock().unwrap().presented.len();
    host.image_loaded(image, Ok(png()));
    platform.settle(&[&host]);
    assert_eq!(
        host.adapter().log.lock().unwrap().presented.len(),
        presented,
        "a settled request cannot bind twice"
    );
    tap(&host, 40.0, 40.0);
    platform.settle(&[&host]);
    assert_eq!(last(&host), ["Count 2", "done"]);
    assert!(
        host.adapter()
            .log
            .lock()
            .unwrap()
            .events
            .iter()
            .any(|e| matches!(e, AccessibilityEffect::Change(_)))
    );
}

#[test]
fn removed_images_cancel_their_transport_and_failures_stop_input() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){const show=useLocal('show',true);return h('view',{},h('button',{onClick:()=>show.set(false),style:{height:60}},'hide'),show.value?h('image',{src:'lx://a.png',style:{width:20,height:20}}):h('text',{},'gone'))}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let (image, _) = host.adapter().log.lock().unwrap().fetched[0].clone();
    tap(&host, 20.0, 20.0);
    platform.settle(&[&host]);
    assert_eq!(host.adapter().log.lock().unwrap().cancelled, [image]);
    assert_eq!(last(&host), ["hide", "gone"]);
    // A failed candidate keeps the presented frame and offers a redraw.
    host.load(source(
        "function render(){const n=useLocal('n',0);if(globalThis.boom===1){globalThis.boom=2;throw Error('boom')}return h('button',{onClick:()=>{globalThis.boom=1;n.set(n.value+1)},style:{height:60}},`n ${n.value}`)}",
    ));
    platform.settle(&[&host]);
    tap(&host, 20.0, 20.0);
    platform.settle(&[&host]);
    let failure = host.failure().unwrap();
    assert_eq!(
        (failure.recovery.kind, failure.recovery.retry),
        (FailureKind::Candidate, Retry::Redraw)
    );
    assert_eq!(failure.message, RecoveryMessage::UpdateFailed);
    assert_eq!(failure.action, Some(RecoveryAction::Retry));
    // Only a loud build (dev, or under automation) shows the error itself on the panel.
    assert_eq!(failure.detail.is_some(), cfg!(feature = "dev"));
    assert!(
        failure
            .detail
            .as_deref()
            .is_none_or(|detail| detail.contains("boom"))
    );
    assert_eq!(last(&host), ["n 0"]);
    let frames = host.adapter().log.lock().unwrap().presented.len();
    host.retry();
    platform.settle(&[&host]);
    assert!(host.failure().is_none());
    assert_eq!(
        host.adapter().log.lock().unwrap().presented.len(),
        frames + 1
    );
    assert_eq!(
        host.adapter().log.lock().unwrap().recovery.last(),
        Some(&None),
        "a rebuilt frame clears recovery chrome"
    );
    if !bounded_jobs() {
        return;
    }
    // A timed-out View job poisons the session; the host offers a reload.
    host.load(source(
        "function render(){return h('button',{onClick:()=>{while(true){}},style:{height:60}},'spin')}",
    ));
    platform.settle(&[&host]);
    tap(&host, 20.0, 20.0);
    platform.settle(&[&host]);
    let failure = host.failure().unwrap();
    assert_eq!(
        (failure.recovery.kind, failure.recovery.retry),
        (FailureKind::Session, Retry::Reload)
    );
    assert_eq!(failure.message, RecoveryMessage::SessionFailed);
    assert_eq!(failure.action, Some(RecoveryAction::Reload));
    assert!(!host.pointer(Pointer {
        phase: PointerPhase::Down,
        x: 20.0,
        y: 20.0,
        slop: 8.0,
        velocity: [0.0; 2],
        min_fling: 0.0
    }));
    host.retry();
    platform.settle(&[&host]);
    assert!(host.failure().is_none());
    assert_eq!(last(&host), ["spin"]);
}

/// A retry whose attempt fails the same way shows the panel again, its action live, never a
/// stuck "Retrying…".
#[test]
fn a_retry_that_fails_again_shows_the_panel_again() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render(){return h('text',{},'ok')}"),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let error = "UI_LAYOUT: the platform declined the frame".to_string();
    host.fail(error.clone());
    assert_eq!(
        host.failure().map(|f| f.action),
        Some(Some(RecoveryAction::Retry))
    );
    let shown = |host: &ViewHost<Fake>| {
        host.adapter()
            .log
            .lock()
            .unwrap()
            .recovery
            .iter()
            .filter(|r| r.is_some())
            .count()
    };
    let before = shown(&host);
    host.retry();
    host.fail(error);
    assert_eq!(
        shown(&host),
        before + 1,
        "the failed retry shows the panel again"
    );
}

#[test]
fn pages_share_one_worker_and_every_platform_call_lands_on_the_main_thread() {
    let worker = ViewWorker::shared("test:app-session").unwrap();
    let platform = Platform::new(&[&worker]);
    assert!(Arc::ptr_eq(
        &worker,
        &ViewWorker::shared("test:app-session").unwrap()
    ));
    let counter = |name: &str| {
        source(&format!(
            "function render(){{const n=useLocal('n',0);return h('button',{{onClick:()=>n.set(n.value+1),style:{{height:60}}}},`{name} ${{n.value}}`)}}"
        ))
    };
    let first = platform.host(&worker, &counter("first"), false);
    let second = platform.host(&worker, &counter("second"), false);
    first.set_visible(true);
    second.set_visible(true);
    platform.settle(&[&first, &second]);
    assert_eq!(last(&first), ["first 0"]);
    for round in 1..=3 {
        tap(&first, 20.0, 20.0);
        tap(&second, 20.0, 20.0);
        tap(&second, 20.0, 20.0);
        // Ingress keeps arriving while earlier frames are still on the worker.
        platform.settle(&[&first, &second]);
        assert_eq!(last(&first), [format!("first {round}")]);
        assert_eq!(last(&second), [format!("second {}", round * 2)]);
    }
    let frames = first.adapter().log.lock().unwrap().presented.len();
    assert!(frames >= 4, "{frames}");
    first.dispose();
    tap(&first, 20.0, 20.0);
    tap(&second, 20.0, 20.0);
    platform.settle(&[&first, &second]);
    assert_eq!(first.adapter().log.lock().unwrap().presented.len(), frames);
    assert_eq!(last(&second), ["second 7"]);
}

#[test]
fn a_page_whose_target_is_attaching_waits_without_holding_up_its_worker() {
    let worker = ViewWorker::shared("test:attach").unwrap();
    let platform = Platform::new(&[&worker]);
    // Page A's image transport never completes.
    let stalled = platform.host(
        &worker,
        &source("function render(){return h('view',{},h('text',{},'A'),h('image',{src:'lx://slow.png',alt:'',style:{width:20,height:20}}))}"),
        false,
    );
    let attaching = platform.host(
        &worker,
        &source("function render(){return h('text',{},'B')}"),
        false,
    );
    attaching
        .adapter()
        .pending
        .store(true, std::sync::atomic::Ordering::Release);
    stalled.set_visible(true);
    attaching.set_visible(true);
    platform.settle(&[&stalled, &attaching]);
    assert_eq!(last(&stalled), ["A"]);
    assert_eq!(stalled.adapter().log.lock().unwrap().fetched.len(), 1);
    assert!(attaching.adapter().log.lock().unwrap().presented.is_empty());
    // The attach completes off the worker; the adapter asks for the frame it declined.
    attaching
        .adapter()
        .pending
        .store(false, std::sync::atomic::Ordering::Release);
    attaching.request_frame();
    platform.settle(&[&stalled, &attaching]);
    assert_eq!(last(&attaching), ["B"]);
}

#[test]
fn a_target_attached_while_its_declined_frame_is_in_flight_still_gets_a_frame() {
    let worker = ViewWorker::spawn("test-attach-race").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render(){return h('text',{},'B')}"),
        false,
    );
    host.adapter()
        .pending
        .store(true, std::sync::atomic::Ordering::Release);
    host.set_visible(true);
    platform.settle(&[&host]);
    assert!(host.adapter().log.lock().unwrap().presented.is_empty());
    // A frame for the attaching target is in flight when the attach lands: the worker
    // declines it, but its result has not reached the main thread yet.
    host.request_frame();
    let (done, ran) = std::sync::mpsc::channel();
    worker.run_background(move |_| {
        let _ = done.send(());
    });
    ran.recv_timeout(Duration::from_secs(30)).unwrap();
    // The attach lands first and reports the target as changed.
    host.adapter()
        .pending
        .store(false, std::sync::atomic::Ordering::Release);
    host.surface_changed();
    platform.settle(&[&host]);
    assert_eq!(
        last(&host),
        ["B"],
        "the declined frame is built again for the new target"
    );
}

#[test]
fn a_stalled_image_decode_never_holds_up_another_page_on_the_worker() {
    let worker = ViewWorker::shared("test:decode").unwrap();
    let platform = Platform::new(&[&worker]);
    let photo = platform.host(
        &worker,
        &source("function render(){return h('view',{},h('text',{},'A'),h('image',{src:'lx://photo.png',alt:'',style:{width:20,height:20}}))}"),
        false,
    );
    photo.set_visible(true);
    platform.settle(&[&photo]);
    let (image, _) = photo.adapter().log.lock().unwrap().fetched[0].clone();
    let (release, stalled) = channel();
    crate::session::host::decode::occupy(stalled);
    photo.image_loaded(image, Ok(png_of(7)));
    let other = platform.host(
        &worker,
        &source("function render(){return h('text',{},'B')}"),
        false,
    );
    other.set_visible(true);
    platform.settle_with(&[&photo, &other], false);
    assert_eq!(
        last(&other),
        ["B"],
        "B presents while A's decode is stalled"
    );
    assert_eq!(loaded_images(&photo), 0, "A's image is not bound yet");
    drop(release);
    platform.settle(&[&photo, &other]);
    assert_eq!(loaded_images(&photo), 1);
}

#[test]
fn images_past_the_visible_budget_fail_to_their_author_instead_of_the_page() {
    let worker = ViewWorker::shared("test:visible-budget").unwrap();
    let platform = Platform::new(&[&worker]);
    // Five stacked 2048px images: 16 MiB each decoded, past the 64 MiB a frame binds.
    let page = platform.host(
        &worker,
        &source("function render(){const s=[0,1,2,3,4].map(i=>useLocal('s'+i,'-'));return h('view',{},h('text',{},s.map(x=>x.value).join('')),...s.map((x,i)=>h('image',{src:'lx://big'+i+'.png',alt:'',onLoad:()=>x.set('L'),onError:()=>x.set('E'),style:{position:'absolute',top:0,left:0,width:2048,height:2048}})))}"),
        false,
    );
    page.set_visible(true);
    platform.settle(&[&page]);
    let fetched = page.adapter().log.lock().unwrap().fetched.clone();
    assert_eq!(fetched.len(), 4, "four loads run at once");
    let big = |seed: u8| {
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(
                &vec![seed; 2048 * 2048 * 4],
                2048,
                2048,
                image::ExtendedColorType::Rgba8,
            )
            .unwrap();
        bytes
    };
    for (n, (token, _)) in fetched.iter().enumerate() {
        page.image_loaded(*token, Ok(big(40 + n as u8)));
    }
    platform.settle(&[&page]);
    let (token, _) = page.adapter().log.lock().unwrap().fetched[4].clone();
    page.image_loaded(token, Ok(big(44)));
    platform.settle(&[&page]);
    assert!(page.failure().is_none(), "the page keeps running");
    assert_eq!(last(&page)[0], "LLLLE");
    assert_eq!(loaded_images(&page), 4);
}

const SHARED_IMAGE: &str = "function render(){return h('image',{src:'lx://shared.png',alt:'',style:{width:20,height:20}})}";

#[test]
fn a_source_shared_by_two_pages_is_decoded_once() {
    let worker = ViewWorker::shared("test:shared-decode").unwrap();
    let platform = Platform::new(&[&worker]);
    let pages = [0, 1].map(|_| {
        let page = platform.host(&worker, &source(SHARED_IMAGE), false);
        page.set_visible(true);
        page
    });
    platform.settle(&[&pages[0], &pages[1]]);
    let bytes = png_of(11);
    let (release, stalled) = channel();
    crate::session::host::decode::occupy(stalled);
    for page in &pages {
        let (image, _) = page.adapter().log.lock().unwrap().fetched[0].clone();
        page.image_loaded(image, Ok(bytes.clone()));
    }
    drop(release);
    platform.settle(&[&pages[0], &pages[1]]);
    assert_eq!(crate::session::host::decode::decodes_of(&bytes), 1);
    assert_eq!(pages.each_ref().map(loaded_images), [1, 1]);
}

#[test]
fn an_image_cancelled_mid_decode_is_never_bound_and_its_pixels_are_reused() {
    let worker = ViewWorker::shared("test:cancel-decode").unwrap();
    let platform = Platform::new(&[&worker]);
    let closing = platform.host(&worker, &source(SHARED_IMAGE), false);
    closing.set_visible(true);
    platform.settle(&[&closing]);
    let bytes = png_of(13);
    let (release, stalled) = channel();
    crate::session::host::decode::occupy(stalled);
    let (image, _) = closing.adapter().log.lock().unwrap().fetched[0].clone();
    closing.image_loaded(image, Ok(bytes.clone()));
    closing.dispose();
    drop(release);
    platform.settle(&[&closing]);
    assert_eq!(loaded_images(&closing), 0, "a disposed page binds nothing");
    let next = platform.host(&worker, &source(SHARED_IMAGE), false);
    next.set_visible(true);
    platform.settle(&[&next]);
    let (image, _) = next.adapter().log.lock().unwrap().fetched[0].clone();
    next.image_loaded(image, Ok(bytes.clone()));
    platform.settle(&[&next]);
    assert_eq!(loaded_images(&next), 1);
    assert_eq!(
        crate::session::host::decode::decodes_of(&bytes),
        1,
        "the cancelled decode's pixels serve the next request"
    );
}

#[test]
fn only_an_accepted_frame_for_the_current_environment_is_current() {
    let worker = ViewWorker::shared("test:current").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render(){return h('text',{},'A')}"),
        false,
    );
    assert!(!host.frame_is_current(), "nothing presented yet");
    host.set_visible(true);
    platform.settle(&[&host]);
    assert!(host.frame_is_current());
    host.surface_changed();
    assert!(
        !host.frame_is_current(),
        "a changed target holds no accepted frame"
    );
    platform.settle(&[&host]);
    assert!(host.frame_is_current());
    host.adapter()
        .environment
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .width = 300;
    assert!(
        !host.frame_is_current(),
        "a frame for another size is not current"
    );
    host.surface_changed();
    platform.settle(&[&host]);
    assert!(
        host.frame_is_current(),
        "presenting at the new size makes it current again"
    );
}

/// Runs the worker until the host holds a prepared frame whose vsync has not come.
fn prepare_one(platform: &Platform, worker: &Arc<ViewWorker>, host: &ViewHost<Fake>) {
    for _ in 0..50 {
        let (done, wait) = channel();
        worker.run_background(move |_| {
            let _ = done.send(());
        });
        wait.recv_timeout(Duration::from_secs(30)).unwrap();
        while let Ok(task) = platform.tasks.try_recv() {
            task();
        }
        if host.adapter().log.lock().unwrap().vsync {
            break;
        }
    }
    assert_eq!(presents(host), 0);
}

#[test]
fn a_frame_prepared_before_a_same_size_target_change_still_presents() {
    let worker = ViewWorker::shared("test:replaced-target").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render(){return h('text',{},'A')}"),
        false,
    );
    host.set_visible(true);
    prepare_one(&platform, &worker, &host);
    // The target is replaced at the same size before that vsync, as Android reports a new
    // surface's size right after creating it.
    host.surface_changed();
    platform.settle(&[&host]);
    let log = host.adapter().log.lock().unwrap();
    assert_eq!(
        log.accepted.len(),
        1,
        "the frame built first is not thrown away"
    );
    assert_eq!(log.presented, [["A"]]);
    drop(log);
    assert!(host.frame_is_current());
}

#[test]
fn a_frame_prepared_for_another_size_is_rebuilt_and_never_presented() {
    let worker = ViewWorker::shared("test:resized-target").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render(){return h('text',{},'A')}"),
        false,
    );
    host.set_visible(true);
    prepare_one(&platform, &worker, &host);
    let built = host.adapter().log.lock().unwrap().accepted[0];
    host.adapter()
        .environment
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .width = 300;
    host.surface_changed();
    platform.settle(&[&host]);
    let log = host.adapter().log.lock().unwrap();
    assert!(log.accepted.len() > 1 && log.accepted[1..].iter().all(|size| *size == (300, built.1)));
    assert!(
        !log.presented_sizes.is_empty()
            && log
                .presented_sizes
                .iter()
                .all(|size| *size == (300, built.1)),
        "only frames laid out for the new size present: {:?}",
        log.presented_sizes
    );
}

/// A surface push lands while the page's first frame waits for its vsync.
/// Returns the presents at that vsync, then every frame presented.
fn first_frame_across_a_surface_push(name: &str, render: &str) -> (usize, Vec<Vec<String>>) {
    let worker = ViewWorker::shared(name).unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(&worker, &source(render), false);
    host.set_visible(true);
    prepare_one(&platform, &worker, &host);
    let surface = serde_json::json!({"kind": "surface", "revision": 1,
        "context": {"sizeClass": "regular", "canDock": true, "width": 900.0, "height": 700.0}});
    host.update_environment(&surface.to_string()).unwrap();
    host.vsync();
    let at_vsync = host.adapter().log.lock().unwrap().presented.len();
    platform.settle(&[&host]);
    let presented = host.adapter().log.lock().unwrap().presented.clone();
    (at_vsync, presented)
}

#[test]
fn a_frame_presents_across_a_push_its_view_did_not_read() {
    let (at_vsync, presented) = first_frame_across_a_surface_push(
        "test:push-unread",
        "function render(){ return h('text',{},'Static') }",
    );
    assert_eq!(
        at_vsync, 1,
        "the frame built before the push presents at its vsync"
    );
    assert_eq!(presented.first(), Some(&vec!["Static".to_string()]));
}

#[test]
fn a_frame_whose_view_read_a_pushed_key_is_rebuilt_before_it_presents() {
    let (at_vsync, presented) = first_frame_across_a_surface_push(
        "test:push-read",
        "function render(view){ return h('text',{},String(view.env.sizeClass)) }",
    );
    assert!(
        !presented.is_empty() && presented.iter().all(|p| p == &vec!["regular".to_string()]),
        "only frames with the pushed value present: {presented:?}"
    );
    assert_eq!(at_vsync, 0);
}

#[test]
fn a_declined_frame_leaves_the_session_as_it_was() {
    let worker = ViewWorker::shared("test:declined").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render(){return h('text',{},'A')}"),
        false,
    );
    host.adapter()
        .pending
        .store(true, std::sync::atomic::Ordering::Release);
    host.set_visible(true);
    platform.settle(&[&host]);
    assert!(host.snapshot().is_none(), "nothing was accepted");
    // The final sample differs from the one the declined attempt laid out.
    host.adapter()
        .environment
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .width = 300;
    host.adapter()
        .pending
        .store(false, std::sync::atomic::Ordering::Release);
    host.surface_changed();
    platform.settle(&[&host]);
    let log = host.adapter().log.lock().unwrap();
    assert_eq!(
        log.accepted,
        [(300, 400)],
        "laid out once, for the final sample"
    );
    drop(log);
    assert_eq!(
        host.snapshot().unwrap().generation.get(),
        1,
        "declined candidates take no generation"
    );
    assert_eq!(last(&host), ["A"]);
}

#[test]
fn a_hidden_page_catches_up_when_its_environment_changes_without_a_new_target() {
    let worker = ViewWorker::shared("test:catch-up-environment").unwrap();
    let platform = Platform::new(&[&worker]);
    let hidden = platform.host(
        &worker,
        &source("function render(){return h('text',{},'H')}"),
        false,
    );
    hidden.set_visible(true);
    platform.settle(&[&hidden]);
    hidden.set_visible(false);
    let frames = hidden.adapter().log.lock().unwrap().presented.len();
    // Insets move with the window's bars (a rotation), the surface stays the same size.
    hidden
        .adapter()
        .environment
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .value["insets"] = serde_json::json!({"top": 0, "left": 0, "right": 0, "bottom": 40});
    assert!(!hidden.frame_is_current());
    hidden.request_frame();
    platform.settle(&[&hidden]);
    assert_eq!(
        hidden.adapter().log.lock().unwrap().presented.len(),
        frames + 1,
        "one catch-up frame"
    );
    assert!(hidden.frame_is_current(), "returning needs no new frame");
    hidden.request_frame();
    platform.settle(&[&hidden]);
    assert_eq!(
        hidden.adapter().log.lock().unwrap().presented.len(),
        frames + 1,
        "an unchanged environment stays frozen"
    );
}

#[test]
fn a_parked_page_renders_nothing_after_it_left() {
    let worker = ViewWorker::spawn("test-park-renders").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render({env}){return h('text',{'data-testid':'t'},`page ${env.theme}`)}"),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let presents = || host.adapter().log.lock().unwrap().presented.len();
    let before = presents();
    // Leaving: hidden, then parked; the host keeps pushing context (appearance, surface).
    host.set_visible(false);
    host.park();
    platform.settle(&[&host]);
    host.adapter()
        .environment
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .value["theme"] = serde_json::json!("dark");
    host.surface_changed();
    host.request_frame();
    platform.settle(&[&host]);
    platform.advance(Duration::from_secs(2));
    platform.settle(&[&host]);
    assert_eq!(presents(), before, "parking renders nothing");
    let text = host
        .automation_request(&serde_json::json!({"op": "query", "css": "[data-testid=\"t\"]"}))
        .unwrap();
    assert_eq!(text["exists"], false, "a parked page reads as blank");
}

#[test]
fn a_click_that_changes_the_look_logs_one_input_line() {
    let worker = ViewWorker::spawn("test-click-look").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render({env}){const n=useLocal('n',0),l=useLocal('l','-');return h('view',{},h('button',{'data-testid':'look',onClick:()=>n.set(n.value+1),style:{width:100,height:40}},`${env.theme} ${n.value} ${l.value}`),h('image',{src:'lx://look.png',alt:'',onLoad:()=>l.set('L'),style:{width:10,height:10}}))}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    tap(&host, 20.0, 20.0);
    // The click's handler switches the appearance, as the Home look segment does: the
    // environment changes while that click's frame is still in flight.
    host.adapter()
        .environment
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .value["theme"] = serde_json::json!("dark");
    host.surface_changed();
    platform.settle(&[&host]);
    // An image load lands afterwards: its callback is not an input and logs nothing.
    let (token, _) = host.adapter().log.lock().unwrap().fetched[0].clone();
    host.image_loaded(token, Ok(png_of(3)));
    platform.settle(&[&host]);
    tap(&host, 20.0, 20.0);
    platform.settle(&[&host]);
    assert_eq!(last(&host)[0], "dark 2 L");
    let lines: Vec<String> = host
        .adapter()
        .log
        .lock()
        .unwrap()
        .debug
        .iter()
        .filter(|line| line.starts_with("UI_INPUT: click"))
        .cloned()
        .collect();
    assert_eq!(lines.len(), 2, "one line per tap: {lines:?}");
}

/// One rule on every platform: a field focuses on a completed tap only.
#[test]
fn a_held_field_focuses_on_release_and_never_when_cancelled_or_slid_off() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){const v=useLocal('v','');return h('page',null,h('scroll',{style:{flexGrow:1}},h('view',{style:{padding:16,gap:16}},h('input',{'data-testid':'f',bind:v,'aria-label':'Name',style:{height:48,backgroundColor:'#ffffff'}}),h('view',{style:{height:200}}))))}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let rect = host
        .automation_request(&serde_json::json!({"op": "query", "css": "[data-testid=\"f\"]"}))
        .unwrap()["rect"]
        .clone();
    let edge = |key: &str| rect[key].as_f64().unwrap() as f32;
    // Rects are in points; touches arrive in device pixels.
    let scale = host.snapshot().unwrap().scale;
    let at = [
        (edge("left") + edge("width") / 2.0) * scale,
        (edge("top") + edge("height") / 2.0) * scale,
    ];
    let pointer = |phase, [x, y]: [f32; 2]| {
        host.pointer(Pointer {
            phase,
            x,
            y,
            slop: 8.0,
            velocity: [0.0; 2],
            min_fling: 50.0,
        });
    };
    let editors = || host.adapter().log.lock().unwrap().editors.len();
    // Held, then cancelled (the screen took the touch): no focus.
    pointer(PointerPhase::Down, at);
    host.long_press();
    platform.settle(&[&host]);
    assert_eq!(editors(), 0, "a held field does not focus mid-press");
    host.cancel();
    platform.settle(&[&host]);
    assert_eq!(editors(), 0, "a cancelled touch never focuses");
    // Held, then slid off and lifted: no focus.
    pointer(PointerPhase::Down, at);
    host.long_press();
    pointer(PointerPhase::Move, [at[0], at[1] + 150.0 * scale]);
    pointer(PointerPhase::Up, [at[0], at[1] + 150.0 * scale]);
    platform.settle(&[&host]);
    assert_eq!(editors(), 0, "lifted off the field");
    // Held, then lifted on the field: focuses, as a tap.
    pointer(PointerPhase::Down, at);
    host.long_press();
    pointer(PointerPhase::Up, at);
    platform.settle(&[&host]);
    let editors = host.adapter().log.lock().unwrap().editors.clone();
    assert!(
        matches!(&editors[..], [Dispatch::Editor { word: false, .. }]),
        "released on the field: {editors:?}"
    );
}

#[test]
fn hiding_a_page_ends_its_text_input() {
    let worker = ViewWorker::shared("test:hide-input").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render(){return h('page',{})}"),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    host.set_visible(false);
    platform.settle(&[&host]);
    assert_eq!(
        host.adapter().log.lock().unwrap().ended_input,
        1,
        "a page pushed over it"
    );
}

/// A click whose update throws keeps the shown page under a banner: the click settles.
#[test]
fn a_click_whose_update_fails_settles() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){const n=useLocal('n',0);if(n.value>0)throw Error('broken update');return h('page',null,h('button',{'data-testid':'go',onClick:()=>n.set(1),style:{height:60}},'Go'))}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let call = host
        .automation_request(&serde_json::json!({"op": "act", "css": "[data-testid=\"go\"]", "action": {"kind": "click"}}))
        .unwrap()["call"]
        .clone();
    let mut answer = None;
    for _ in 0..20 {
        host.request_frame();
        platform.settle(&[&host]);
        match host.automation_request(&serde_json::json!({"op": "result", "call": call})) {
            Ok(reply) if reply["done"] == true => {
                answer = Some(Ok(reply));
                break;
            }
            Ok(_) => {}
            Err(error) => {
                answer = Some(Err(error));
                break;
            }
        }
    }
    assert!(answer.is_some(), "the click settles once its update failed");
}

/// A combobox's option: its click keeps the field editing, so no blur closes the list first.
#[test]
fn clicking_an_option_of_a_focused_combobox_keeps_the_field_editing() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){const v=useLocal('v','');const last=useLocal('last','none');return h('page',null,h('input',{'data-testid':'f',bind:v,role:'combobox','aria-label':'Fruit','aria-controls':'list',onBlur:()=>last.set('blur'),style:{height:48}}),h('view',{id:'list',role:'listbox','aria-label':'Fruits'},h('button',{'data-testid':'cherry',role:'option',onClick:()=>{v.set('Cherry');last.set('picked')}},'Cherry')),h('text',{'data-testid':'last'},last.value))}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let current: serde_json::Value = serde_json::from_str(host.snapshot().unwrap().json()).unwrap();
    let identity = current["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["test_id"] == "f")
        .unwrap()["identity"]
        .clone();
    host.input(
        serde_json::json!({"op": "focus", "identity": identity, "generation": current["generation"]})
            .to_string(),
        |_| {},
    );
    platform.settle(&[&host]);
    run_frames(&platform, &host, 2);
    host.automation_request(&serde_json::json!({"op": "act", "css": "[data-testid=\"cherry\"]", "action": {"kind": "click"}}))
        .unwrap();
    platform.settle(&[&host]);
    run_frames(&platform, &host, 2);
    assert_eq!(
        host.adapter().log.lock().unwrap().ended_input,
        0,
        "the field keeps editing"
    );
    let last = host
        .automation_request(&serde_json::json!({"op": "query", "css": "[data-testid=\"last\"]"}))
        .unwrap();
    assert_eq!(last["text"], "picked", "{last}");
}

#[test]
fn hiding_a_page_drops_its_text_selection_from_the_kept_frame() {
    let worker = ViewWorker::shared("test:hide-selection").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){return h('text',{'data-testid':'p',style:{userSelect:'text',fontSize:16}},'hello brave world')}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let text = host
        .snapshot()
        .unwrap()
        .frame
        .nodes
        .iter()
        .find(|n| n.test_id == "p")
        .unwrap()
        .bounds;
    host.pointer(Pointer {
        phase: PointerPhase::Down,
        x: text[0] + 8.0,
        y: text[1] + text[3] / 2.0,
        slop: 8.0,
        velocity: [0.0; 2],
        min_fling: 50.0,
    });
    host.long_press();
    platform.settle(&[&host]);
    assert!(
        host.snapshot().unwrap().frame.has_text_selection(),
        "long press selects"
    );
    host.cancel();
    host.set_visible(false);
    platform.settle(&[&host]);
    assert!(
        !host.snapshot().unwrap().frame.has_text_selection(),
        "the kept frame carries no handles"
    );
    assert_eq!(
        host.adapter().log.lock().unwrap().released_transient,
        1,
        "hiding releases memory kept for coming frames"
    );
    assert!(host.frame_is_current(), "returning needs no new frame");
}

#[test]
fn a_hidden_page_catches_up_after_the_shown_page_and_stays_frozen() {
    let worker = ViewWorker::shared("test:catch-up").unwrap();
    let platform = Platform::new(&[&worker]);
    let shown = platform.host(
        &worker,
        &source("function render(){return h('text',{},'S')}"),
        false,
    );
    let hidden = platform.host(
        &worker,
        &source("function render(){return h('text',{},'H')}"),
        false,
    );
    shown.set_visible(true);
    hidden.set_visible(true);
    platform.settle(&[&shown, &hidden]);
    hidden.set_visible(false);
    let frames = hidden.adapter().log.lock().unwrap().presented.len();
    // Both targets change while the worker is busy; the hidden one asks first.
    let (release, busy) = channel::<()>();
    worker.run(move |_| {
        let _ = busy.recv();
    });
    for host in [&hidden, &shown] {
        host.adapter()
            .environment
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .width = 300;
        host.surface_changed();
    }
    drop(release);
    platform.settle(&[&shown, &hidden]);
    let shown_at = *shown
        .adapter()
        .log
        .lock()
        .unwrap()
        .presented_at
        .last()
        .unwrap();
    let hidden_at = *hidden
        .adapter()
        .log
        .lock()
        .unwrap()
        .presented_at
        .last()
        .unwrap();
    assert!(shown_at < hidden_at, "the shown page's frame goes first");
    assert_eq!(
        hidden.adapter().log.lock().unwrap().presented.len(),
        frames + 1
    );
    assert!(hidden.frame_is_current(), "returning needs no new frame");
    hidden.request_frame();
    platform.settle(&[&shown, &hidden]);
    assert_eq!(
        hidden.adapter().log.lock().unwrap().presented.len(),
        frames + 1,
        "still frozen"
    );
}

#[test]
fn the_caret_blinks_on_a_host_timer_and_text_changes_restart_its_phase() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){const v=useLocal('v','');return h('input',{bind:v,autoFocus:true,style:{width:100,height:30}})}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let state = |text: &str| CaretState {
        text: text.into(),
        anchor: 0,
        focus: 0,
        composition: (-1, -1),
    };
    host.caret(true, Some(state("")));
    assert!(host.caret_visible());
    assert_eq!(platform.advance(Duration::from_millis(499)), 0);
    assert_eq!(platform.advance(Duration::from_millis(1)), 1);
    assert!(!host.caret_visible());
    platform.settle(&[&host]);
    host.caret(true, Some(state("a")));
    assert!(host.caret_visible(), "an edit shows the caret immediately");
    platform.settle(&[&host]);
    host.caret(false, Some(state("a")));
    platform.advance(Duration::from_secs(5));
    assert!(host.caret_visible(), "an ineligible caret stays solid");
    host.caret(true, None);
    assert!(platform.advance(Duration::from_secs(5)) <= 1);
    assert!(host.caret_visible());
}

#[test]
fn connections_report_connected_and_an_overdue_bootstrap() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        // Compiled Logic pages always publish their action catalog, even an empty one.
        &(source("function render({data}){return h('text',{},`count ${data.count}`)}")
            + "\nglobalThis.__lingxiaActionCatalog={__names:[],__modes:{}};"),
        true,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let connection = ConnectionTicket::new(1);
    host.connect(connection);
    platform.settle(&[&host]);
    if bounded_jobs() {
        let connected: serde_json::Value =
            serde_json::from_str(&host.adapter().log.lock().unwrap().messages[0].1).unwrap();
        assert_eq!(connected["kind"], "connected");
    }
    assert!(
        host.adapter().log.lock().unwrap().presented.is_empty(),
        "a Logic page waits for its state"
    );
    platform.advance(Duration::from_secs(5));
    if bounded_jobs() {
        // Never a blank page: the slow-load panel covers it, on the page's own canvas.
        let failure = host.failure().expect("an overdue bootstrap shows recovery");
        assert!(failure.error.starts_with("UI_BOOTSTRAP_TIMEOUT:"));
        assert!(failure.covers_page());
        assert_eq!(failure.message, RecoveryMessage::SlowLoad);
        assert_eq!(failure.action, Some(RecoveryAction::Wait));
        assert!(
            !host.adapter().log.lock().unwrap().chrome.is_empty(),
            "the panel has the page's colours before any frame"
        );
        // Keep waiting closes the panel and waits as long again, on the same connection.
        host.retry();
        assert!(host.failure().is_none());
        assert_eq!(
            host.adapter().log.lock().unwrap().recovery.last(),
            Some(&None)
        );
        platform.advance(Duration::from_secs(4));
        assert!(host.failure().is_none(), "the wait starts over");
        platform.advance(Duration::from_secs(1));
        assert_eq!(
            host.failure().map(|failure| failure.message),
            Some(RecoveryMessage::SlowLoad)
        );
    }
    deliver(
        &host,
        connection,
        serde_json::json!({"v":2,"kind":"state.snapshot","rev":0,"state":{"count":3}}),
    );
    platform.settle(&[&host]);
    if bounded_jobs() {
        assert_eq!(last(&host), ["count 3"]);
        assert!(
            host.failure().is_none(),
            "a late first state clears recovery"
        );
        // A superseded connection's frames are ignored.
        host.connect(ConnectionTicket::new(2));
        deliver(
            &host,
            connection,
            serde_json::json!({"v":2,"kind":"state.snapshot","rev":1,"state":{"count":4}}),
        );
        platform.settle(&[&host]);
        assert_eq!(last(&host), ["count 3"]);
        // The new connection has no state of its own, whatever the old one drew.
        platform.advance(Duration::from_secs(5));
        let failure = host
            .failure()
            .expect("a reconnect without state shows recovery");
        assert!(failure.error.starts_with("UI_BOOTSTRAP_TIMEOUT:"));
    } else {
        // Engines without bounded jobs refuse Logic actions; the failure is recoverable chrome.
        let failure = host.failure().unwrap();
        assert!(failure.error.contains("UI_ACTION"), "{}", failure.error);
    }
}

#[test]
fn a_state_write_is_acknowledged_once_a_frame_showing_it_is_built() {
    if !bounded_jobs() {
        return;
    }
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &(source("function render({data}){return h('text',{},`count ${data.count}`)}")
            + "\nglobalThis.__lingxiaActionCatalog={__names:[],__modes:{}};"),
        true,
    );
    host.set_visible(true);
    let connection = ConnectionTicket::new(1);
    host.connect(connection);
    deliver(
        &host,
        connection,
        serde_json::json!({"v":2,"kind":"state.snapshot","rev":0,"state":{"count":3}}),
    );
    platform.settle(&[&host]);
    assert_eq!(last(&host), ["count 3"]);
    let acks = |host: &ViewHost<Fake>| {
        host.adapter()
            .log
            .lock()
            .unwrap()
            .signals
            .iter()
            .filter(|(_, s)| matches!(s, StateSignal::Acked(_)))
            .count()
    };
    // A target that declines frames: the write reaches the View but no frame shows it.
    host.adapter()
        .pending
        .store(true, std::sync::atomic::Ordering::Release);
    deliver(
        &host,
        connection,
        serde_json::json!({"v":2,"kind":"state.patch","baseRev":0,"rev":1,"ack":true,
            "ops":[{"op":"replace","path":"/count","value":4}]}),
    );
    platform.settle(&[&host]);
    assert_eq!(acks(&host), 0, "held until a frame showing it is built");
    host.adapter()
        .pending
        .store(false, std::sync::atomic::Ordering::Release);
    host.surface_changed();
    platform.settle(&[&host]);
    assert_eq!(last(&host), ["count 4"]);
    assert_eq!(acks(&host), 1);
    // With no frame to show it, a write is acknowledged at the deadline.
    host.adapter()
        .pending
        .store(true, std::sync::atomic::Ordering::Release);
    deliver(
        &host,
        connection,
        serde_json::json!({"v":2,"kind":"state.patch","baseRev":1,"rev":2,"ack":true,
            "ops":[{"op":"replace","path":"/count","value":5}]}),
    );
    platform.settle(&[&host]);
    assert_eq!(acks(&host), 1);
    platform.advance(Duration::from_millis(250));
    assert_eq!(acks(&host), 2);
}

// Motion: wakeups, hidden silence and the animation frame counter.

const SPIN: &str = r#"{"name":"spin","durationMs":1000,"delayMs":0,"iterations":null,"whenReduced":"base","easing":"linear","opacity":null,"transforms":[{"kind":"rotate","angle":[{"offset":0,"value":0},{"offset":1,"value":360}]}]}"#;

fn presents(host: &ViewHost<Fake>) -> usize {
    host.adapter().log.lock().unwrap().presented.len()
}

/// Advance the fake clock by `frames` display frames, firing due wakeups and at most
/// one vsync per frame, as a display does.
fn run_frames(platform: &Platform, host: &ViewHost<Fake>, frames: u32) {
    let frame = Duration::from_micros(16_667);
    for _ in 0..frames {
        *host.adapter().clock.lock().unwrap() += frame;
        platform.advance(frame);
        let mut vsynced = false;
        loop {
            for worker in &platform.workers {
                let (done, wait) = channel();
                worker.run_background(move |_| {
                    let _ = done.send(());
                });
                wait.recv_timeout(Duration::from_secs(30)).unwrap();
            }
            let mut ran = false;
            while let Ok(task) = platform.tasks.try_recv() {
                task();
                ran = true;
            }
            if !vsynced && std::mem::take(&mut host.adapter().log.lock().unwrap().vsync) {
                host.vsync();
                vsynced = true;
                ran = true;
            }
            if !ran {
                break;
            }
        }
    }
}

fn animation_frames(host: &ViewHost<Fake>) -> u64 {
    let snapshot = host.snapshot().unwrap();
    let json: serde_json::Value = serde_json::from_str(snapshot.json()).unwrap();
    json["metrics"]["animationFrames"].as_u64().unwrap()
}

#[test]
fn a_spinner_presents_at_display_rate_only_while_visible_and_present() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(&format!(
            "function render(){{const on=useLocal('on',true);return h('view',{{}},h('button',{{onClick:()=>on.set(false),style:{{height:60}}}},'stop'),on.value?h('view',{{style:{{width:20,height:20,backgroundColor:'#ff0000',animation:{SPIN}}}}}):null)}}"
        )),
        false,
    );
    platform.settle(&[&host]);
    run_frames(&platform, &host, 10);
    assert_eq!(presents(&host), 0, "hidden from the start");
    host.set_visible(true);
    platform.settle(&[&host]);
    let shown = presents(&host);
    run_frames(&platform, &host, 60);
    let spun = presents(&host) - shown;
    assert!((58..=61).contains(&spun), "{spun} presents in one second");
    // Each frame queues the next behind its acknowledgement; no wakeup timer round trip.
    assert!(platform.timers.lock().unwrap().is_empty());
    assert!(animation_frames(&host) >= 55, "{}", animation_frames(&host));

    host.set_visible(false);
    platform.settle(&[&host]);
    let hidden = presents(&host);
    run_frames(&platform, &host, 60);
    assert_eq!(presents(&host), hidden, "a hidden page presents nothing");

    host.set_visible(true);
    platform.settle(&[&host]);
    run_frames(&platform, &host, 10);
    assert!(presents(&host) - hidden >= 10);

    tap(&host, 20.0, 20.0);
    platform.settle(&[&host]);
    run_frames(&platform, &host, 2);
    let removed = presents(&host);
    run_frames(&platform, &host, 60);
    assert_eq!(
        presents(&host),
        removed,
        "a removed spinner schedules nothing"
    );
    assert!(platform.timers.lock().unwrap().is_empty());
}

#[test]
fn frame_pacing_reports_display_rate_frames_and_the_hud_never_becomes_page_content() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(&format!(
            "function render(){{return h('view',{{}},h('text',{{}},'spin'),h('view',{{style:{{width:20,height:20,backgroundColor:'#ff0000',animation:{SPIN}}}}}))}}"
        )),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    run_frames(&platform, &host, 90);
    let frame = host
        .automation_request(&serde_json::json!({"op": "frame", "windowMs": 1000}))
        .unwrap();
    let pacing = &frame["pacing"];
    assert!(pacing["samples"].as_u64().unwrap() >= 55, "{pacing}");
    assert_eq!(pacing["frameTimeMs"]["p50"], 16.67, "{pacing}");
    assert_eq!(pacing["dropped"], 0, "{pacing}");
    assert_eq!(pacing["jank"], 0, "{pacing}");
    assert!(pacing["cpuMs"]["p95"].is_number(), "{pacing}");
    assert_eq!(frame["hud"], false);
    assert!(!host.adapter().log.lock().unwrap().hud.iter().any(|on| *on));

    let nodes = host.snapshot().unwrap().frame.nodes.len();
    let reply = host
        .automation_request(&serde_json::json!({"op": "hud", "on": true}))
        .unwrap();
    assert_eq!(reply["hud"], true);
    run_frames(&platform, &host, 5);
    assert_eq!(host.adapter().log.lock().unwrap().hud.last(), Some(&true));
    assert_eq!(
        host.snapshot().unwrap().frame.nodes.len(),
        nodes,
        "not page content"
    );
    assert!(
        !host
            .automation_request(&serde_json::json!({"op": "tree"}))
            .unwrap()
            .to_string()
            .contains("frame "),
        "the HUD is not in the semantics tree"
    );
    host.set_hud(false);
    run_frames(&platform, &host, 2);
    assert_eq!(host.adapter().log.lock().unwrap().hud.last(), Some(&false));
}

#[test]
fn measure_frames_resolves_with_the_window_it_waited_out() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(&format!(
            "function render(){{const n=useLocal('n','none');return h('view',{{}},h('button',{{onClick:()=>{{measureFrames(500).then(s=>n.set(s.windowMs+':'+(s.samples>=25)+':'+(s.frameTimeMs.p50<17)))}},style:{{height:60}}}},'measured '+n.value),h('view',{{style:{{width:20,height:20,backgroundColor:'#ff0000',animation:{SPIN}}}}}))}}"
        )),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    run_frames(&platform, &host, 3);
    tap(&host, 20.0, 20.0);
    run_frames(&platform, &host, 25);
    assert_eq!(last(&host)[0], "measured none", "the window has not ended");
    run_frames(&platform, &host, 10);
    assert_eq!(last(&host)[0], "measured 500:true:true");
}

#[test]
fn an_input_is_charged_to_the_frame_that_shows_it() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){const n=useLocal('n',0);return h('button',{onClick:()=>n.set(n.value+1),style:{height:60}},'tap '+n.value)}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    run_frames(&platform, &host, 3);
    host.input_age(8_000);
    tap(&host, 20.0, 20.0);
    platform.settle(&[&host]);
    run_frames(&platform, &host, 3);
    let pacing = &host
        .automation_request(&serde_json::json!({"op": "frame"}))
        .unwrap()["pacing"];
    assert_eq!(pacing["inputToPresent"]["count"], 1, "{pacing}");
    assert!(
        pacing["inputToPresent"]["lastMs"].as_f64().unwrap() >= 8.0,
        "{pacing}"
    );
}

#[test]
fn a_one_shot_animation_stops_presenting_once_complete() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let fade = r#"{"name":"fade","durationMs":100,"delayMs":0,"iterations":1,"whenReduced":"base","easing":"linear","opacity":[{"offset":0,"value":0},{"offset":1,"value":1}],"transforms":[]}"#;
    let host = platform.host(
        &worker,
        &source(&format!(
            "function render(){{return h('view',{{style:{{width:20,height:20,backgroundColor:'#ff0000',animation:{fade}}}}})}}"
        )),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    run_frames(&platform, &host, 12);
    let done = presents(&host);
    assert!(done >= 7, "{done}");
    run_frames(&platform, &host, 60);
    assert_eq!(presents(&host), done);
    assert!(platform.timers.lock().unwrap().is_empty());
}

#[test]
fn a_reduced_motion_change_while_hidden_catches_up_once_and_stays_silent() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(&format!(
            "function render(){{return h('view',{{style:{{width:20,height:20,backgroundColor:'#ff0000',animation:{SPIN}}}}})}}"
        )),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    run_frames(&platform, &host, 5);
    host.set_visible(false);
    platform.settle(&[&host]);
    assert!(host.frame_is_current());
    let hidden = presents(&host);
    host.adapter()
        .environment
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .value["reducedMotion"] = serde_json::json!(true);
    host.redraw();
    platform.settle(&[&host]);
    assert_eq!(presents(&host), hidden + 1, "one catch-up frame");
    assert!(host.frame_is_current());
    run_frames(&platform, &host, 30);
    assert_eq!(presents(&host), hidden + 1);
    // Shown again with reduced motion, the spinner shows its base style and never wakes.
    host.set_visible(true);
    platform.settle(&[&host]);
    let shown = presents(&host);
    run_frames(&platform, &host, 30);
    assert_eq!(presents(&host), shown);
}

fn revealed(host: &ViewHost<Fake>) -> Vec<bool> {
    host.adapter().log.lock().unwrap().revealed.clone()
}

/// A shown page whose next frame will not match what it last presented.
fn resize(host: &ViewHost<Fake>, width: u16) {
    host.adapter()
        .environment
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .width = width;
}

fn revealing_page(platform: &Platform, worker: &Arc<ViewWorker>) -> ViewHost<Fake> {
    let host = platform.host(
        worker,
        &source("function render(){return h('text',{},'shown')}"),
        false,
    );
    host.set_visible(true);
    host
}

#[test]
fn a_first_visit_reveals_with_its_first_frame_and_a_current_page_at_once() {
    let worker = ViewWorker::spawn("test-reveal-first").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = revealing_page(&platform, &worker);
    host.reveal(false);
    assert!(revealed(&host).is_empty(), "nothing to show yet");
    platform.settle(&[&host]);
    assert_eq!(last(&host), ["shown"]);
    assert_eq!(revealed(&host), [false]);
    host.reveal(true);
    assert_eq!(
        revealed(&host),
        [false, false],
        "a current frame shows at once"
    );
    platform.advance(crate::session::host::FIRST_FRAME_WATCHDOG);
    assert_eq!(
        revealed(&host),
        [false, false],
        "a completed reveal's deadline is spent"
    );
}

#[test]
fn a_returning_page_with_a_stale_frame_is_shown_held_at_the_deadline_then_released() {
    let worker = ViewWorker::spawn("test-reveal-stale").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = revealing_page(&platform, &worker);
    platform.settle(&[&host]);
    resize(&host, 180);
    host.reveal(true);
    let deadline = crate::session::host::REVEAL_DEADLINE;
    assert_eq!(platform.advance(deadline - Duration::from_millis(1)), 0);
    platform.advance(Duration::from_millis(1));
    assert_eq!(
        revealed(&host),
        [true],
        "shown, but its stale surface stays hidden"
    );
    let reveal = host
        .automation_request(&serde_json::json!({"op": "frame"}))
        .unwrap()["reveal"]
        .clone();
    assert_eq!(reveal["deadlines"], 1);
    assert_eq!(reveal["lastReveal"]["reason"], "deadline");
    host.surface_changed();
    platform.settle(&[&host]);
    assert_eq!(
        revealed(&host),
        [true, false],
        "the current frame releases the surface"
    );
}

#[test]
fn a_stopped_window_pauses_the_deadline_and_a_failure_reveals_at_once() {
    let worker = ViewWorker::spawn("test-reveal-paused").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = revealing_page(&platform, &worker);
    platform.settle(&[&host]);
    resize(&host, 180);
    host.reveal(true);
    host.window_visible(false);
    platform.advance(crate::session::host::REVEAL_DEADLINE);
    assert!(revealed(&host).is_empty());
    host.window_visible(true);
    platform.advance(crate::session::host::REVEAL_DEADLINE);
    assert_eq!(revealed(&host), [true]);

    let first = revealing_page(&platform, &worker);
    first.reveal(false);
    first.fail("UI_RENDER: broken".into());
    assert_eq!(
        revealed(&first),
        [false],
        "recovery chrome shows without waiting"
    );
    platform.advance(crate::session::host::FIRST_FRAME_WATCHDOG);
    assert_eq!(revealed(&first), [false]);
}

#[test]
fn the_same_render_failure_three_times_stops_rendering_until_a_retry_or_a_new_size() {
    // The View's console drops lines past the logger's maximum level.
    log::set_max_level(log::LevelFilter::Trace);
    let worker = ViewWorker::spawn("test-failure-stall").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("let n=0;function render(){n++;console.log('render');if(n>1)throw Error('steady failure');return h('text',{},'ok')}"),
        false,
    );
    let renders = || host.adapter().log.lock().unwrap().console.len();
    let shown = || {
        let log = host.adapter().log.lock().unwrap();
        let recoveries = log.recovery.iter().filter(|r| r.is_some()).count();
        let warned = log
            .warnings
            .iter()
            .filter(|w| w.contains("steady failure"))
            .count();
        (recoveries, warned)
    };
    host.set_visible(true);
    platform.settle(&[&host]);
    assert_eq!(last(&host), ["ok"]);
    for _ in 0..6 {
        host.request_frame();
        platform.settle(&[&host]);
    }
    assert_eq!(renders(), 1 + 3, "rendering stalled");
    assert_eq!(
        shown(),
        (1, 2),
        "recovery shown once; logged once and at the stall"
    );
    // Another size may render: it tries again.
    resize(&host, 180);
    host.surface_changed();
    platform.settle(&[&host]);
    assert_eq!(renders(), 2 + 3);
    for _ in 0..6 {
        host.request_frame();
        platform.settle(&[&host]);
    }
    let stalled = renders();
    host.retry();
    platform.settle(&[&host]);
    assert_eq!(renders(), stalled + 1, "a retry renders again");
    for _ in 0..6 {
        host.request_frame();
        platform.settle(&[&host]);
    }
    // Another page at the stalled size renders.
    host.load(source("function render(){return h('text',{},'fresh')}"));
    platform.settle(&[&host]);
    assert_eq!(last(&host), ["fresh"]);
}

#[test]
fn a_frame_past_its_budget_is_dropped_without_recovery_and_counted() {
    let worker = ViewWorker::spawn("test-budget-overrun").unwrap();
    let platform = Platform::new(&[&worker]);
    // The budget charges CPU time, so the page spins two budgets of CPU work, calibrated on this
    // thread's clock: a wall-clock spin falls short of the budget under host load.
    let Some(clock) = crate::engine::thread_clock::ThreadClock::current() else {
        return;
    };
    let probe = 200_000u64;
    let mut calibration = crate::session::ViewSession::new(&source(&format!(
        "function render(){{let s=0;for(let i=0;i<{probe};i++)s+=i;return h('text',{{}},'probe')}}"
    )))
    .unwrap();
    let before = clock.elapsed().unwrap();
    calibration.layout(300, 500, 1.0).unwrap();
    let per_ms = probe as f64
        / (clock
            .elapsed()
            .unwrap()
            .saturating_sub(before)
            .as_secs_f64()
            * 1000.0)
            .max(0.001);
    let budget_ms = crate::session::eval_budget().as_secs_f64() * 1000.0;
    // The page's second render runs two budgets once; every other render is quick.
    let host = platform.host(
        &worker,
        &source(&format!(
            "let n=0;function render(){{n++;if(n===2){{let s=0;for(let i=0;i<{};i++)s+=i;}}return h('text',{{}},'frame '+n)}}",
            (per_ms * budget_ms * 2.0) as u64
        )),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    assert_eq!(last(&host), ["frame 1"]);
    resize(&host, 180);
    host.surface_changed();
    platform.settle(&[&host]);
    assert_ne!(
        last(&host),
        ["frame 1"],
        "the dropped frame is rendered again"
    );
    {
        let log = host.adapter().log.lock().unwrap();
        assert!(
            log.recovery.iter().all(Option::is_none),
            "a dropped frame shows no recovery"
        );
        assert!(
            log.warnings
                .iter()
                .any(|w| w.starts_with(crate::session::OVERRUN)),
            "{:?}",
            log.warnings
        );
    }
    let frame = host
        .automation_request(&serde_json::json!({"op": "frame"}))
        .unwrap();
    assert_eq!(frame["budget"]["overruns"], 1, "{frame}");
}

#[test]
fn a_parked_or_reloaded_page_is_not_shown_on_its_old_frame() {
    let worker = ViewWorker::spawn("test-reveal-reused").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = revealing_page(&platform, &worker);
    platform.settle(&[&host]);
    assert_eq!(last(&host), ["shown"]);
    // Popped into the cache, then pushed again: the surface holds the old instance's frame.
    host.set_visible(false);
    host.park();
    platform.settle(&[&host]);
    host.set_visible(true);
    host.reveal(true);
    assert!(
        revealed(&host).is_empty(),
        "a parked page's old frame is not current"
    );
    platform.advance(crate::session::host::REVEAL_DEADLINE);
    assert_eq!(
        revealed(&host),
        [true],
        "shown at the deadline with its surface held"
    );
    // A reload of a shown page waits for its own first frame too.
    host.load(source("function render(){return h('text',{},'again')}"));
    host.reveal(true);
    assert_eq!(revealed(&host), [true], "nothing new at once");
    platform.settle(&[&host]);
    assert_eq!(last(&host), ["again"]);
    assert_eq!(
        revealed(&host),
        [true, false],
        "the new frame releases the surface"
    );
}

#[test]
fn a_reveal_before_a_requested_park_runs_is_not_shown_on_the_old_frame() {
    let worker = ViewWorker::spawn("test-reveal-stale-mark").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = revealing_page(&platform, &worker);
    platform.settle(&[&host]);
    assert_eq!(last(&host), ["shown"]);
    // The park is requested and posted; the push's reveal runs before the posted park.
    host.stale_mark().set();
    host.reveal(true);
    assert!(
        revealed(&host).is_empty(),
        "the old instance's frame is not current once its park is requested"
    );
    host.park();
    host.load(source("function render(){return h('text',{},'again')}"));
    platform.settle(&[&host]);
    assert_eq!(last(&host), ["again"]);
    assert_eq!(revealed(&host), [false], "the new frame reveals it");
}

#[test]
fn a_hidden_page_waiting_to_be_revealed_builds_its_first_frame_without_the_deadline() {
    let worker = ViewWorker::spawn("test-reveal-repush").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = revealing_page(&platform, &worker);
    platform.settle(&[&host]);
    // Popped into the cache, then pushed again: hidden until its reveal completes.
    host.set_visible(false);
    host.park();
    platform.settle(&[&host]);
    host.load(source("function render(){return h('text',{},'again')}"));
    host.reveal(true);
    platform.settle(&[&host]);
    assert_eq!(last(&host), ["again"]);
    assert_eq!(
        revealed(&host),
        [false],
        "revealed by its own frame, not the deadline"
    );
}

#[test]
fn a_failed_reveal_does_not_charge_its_wait_to_the_next_load() {
    let worker = ViewWorker::spawn("test-reveal-failed-reload").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = revealing_page(&platform, &worker);
    host.reveal(false);
    host.fail("UI_TIMEOUT: the render ran out of time".into());
    host.set_visible(false);
    host.park();
    platform.settle(&[&host]);
    // Pushed again much later: the wait starts with this reveal, not the failed one.
    *host.adapter().clock.lock().unwrap() += Duration::from_secs(5);
    host.load(source("function render(){return h('text',{},'again')}"));
    host.set_visible(true);
    host.reveal(false);
    platform.settle(&[&host]);
    assert_eq!(last(&host), ["again"]);
    let pacing = host
        .automation_request(&serde_json::json!({"op": "frame"}))
        .unwrap()["pacing"]
        .clone();
    let waited = pacing["firstPresentMs"]
        .as_f64()
        .expect("the reload presented");
    assert!(waited < 1000.0, "{pacing}");
}

#[test]
fn a_window_nothing_of_which_shows_holds_its_frame_until_it_returns() {
    let worker = ViewWorker::spawn("test-window-hidden").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = revealing_page(&platform, &worker);
    platform.settle(&[&host]);
    let presented = || host.adapter().log.lock().unwrap().presented.len();
    let frames = presented();
    host.window_visible(false);
    resize(&host, 180);
    host.surface_changed();
    platform.settle(&[&host]);
    assert_eq!(presented(), frames, "an occluded window presents nothing");
    host.window_visible(true);
    platform.settle(&[&host]);
    assert!(
        presented() > frames,
        "the held frame shows when the window returns"
    );
}

#[test]
fn a_covered_window_completes_frames_on_the_refresh_tick_and_counts_them() {
    let worker = ViewWorker::spawn("test-window-covered").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = revealing_page(&platform, &worker);
    platform.settle(&[&host]);
    let frames = || {
        host.automation_request(&serde_json::json!({"op": "frame"}))
            .unwrap()["pacing"]["frames"]
            .as_u64()
            .unwrap()
    };
    let before = frames();
    host.window_covered(true);
    resize(&host, 180);
    host.surface_changed();
    platform.settle(&[&host]);
    assert!(
        !host.adapter().log.lock().unwrap().vsync,
        "a covered window has no display link to ask"
    );
    assert_eq!(frames(), before, "the frame waits for the refresh tick");
    assert_eq!(platform.advance(Duration::from_millis(17)), 1);
    platform.settle(&[&host]);
    assert_eq!(frames(), before + 1, "a covered frame completes and counts");
}

#[test]
fn a_window_reported_covered_still_presents_and_a_stopped_one_counts_held_frames() {
    let worker = ViewWorker::spawn("test-window-covered-presents").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = revealing_page(&platform, &worker);
    platform.settle(&[&host]);
    let frame = |key: &str| {
        host.automation_request(&serde_json::json!({"op": "frame"}))
            .unwrap()[key]
            .as_u64()
            .unwrap()
    };
    // macOS reports a window on screen as occluded while its app is not active.
    host.window_covered(true);
    let shown = presents(&host);
    let frames = frame("frames");
    resize(&host, 180);
    host.surface_changed();
    platform.settle(&[&host]);
    platform.advance(Duration::from_millis(17));
    platform.settle(&[&host]);
    assert!(
        presents(&host) > shown,
        "a covered window's frame reaches the screen"
    );
    assert!(frame("frames") > frames);
    assert_eq!(frame("held"), 0);

    // A stopped window holds: its frames complete but neither show nor count as presented.
    host.window_covered(false);
    host.window_visible(false);
    let shown = presents(&host);
    let frames = frame("frames");
    resize(&host, 160);
    host.surface_changed();
    platform.settle(&[&host]);
    assert_eq!(presents(&host), shown);
    assert_eq!(
        frame("frames"),
        frames,
        "a held frame is not a presented one"
    );
    assert!(frame("held") > 0);
}

#[test]
fn a_covered_window_whose_surface_refuses_a_frame_holds_until_it_shows_again() {
    let worker = ViewWorker::spawn("test-window-covered-refused").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = costly_spinner(&platform, &worker, false);
    host.window_covered_watched(true);
    run_timed(&platform, &host, 10);
    let shown = presents(&host);
    assert!(shown > 0, "a watched covered window presents");
    host.adapter()
        .costs
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .decline = 1;
    run_timed(&platform, &host, 20);
    let held = presents(&host);
    run_timed(&platform, &host, 20);
    assert_eq!(
        presents(&host),
        held,
        "after a refusal the covered window holds"
    );
    host.window_covered_watched(false);
    run_timed(&platform, &host, 10);
    assert!(presents(&host) > held, "shown again, it presents");
}

#[test]
fn a_covered_window_parks_a_looping_animation_unless_it_is_watched() {
    let worker = ViewWorker::spawn("test-window-watched").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(&format!(
            "function render(){{return h('view',{{style:{{width:20,height:20,backgroundColor:'#ff0000',animation:{SPIN}}}}})}}"
        )),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let frames = || {
        host.automation_request(&serde_json::json!({"op": "frame"}))
            .unwrap()["pacing"]["frames"]
            .as_u64()
            .unwrap()
    };
    let ticks = |count: usize| {
        for _ in 0..count {
            platform.advance(Duration::from_millis(17));
            platform.settle(&[&host]);
        }
    };
    host.window_covered(true);
    platform.settle(&[&host]);
    // Frames already on their way still complete.
    ticks(10);
    let parked = frames();
    ticks(10);
    assert_eq!(frames(), parked, "nobody sees the spinner turn");
    host.window_covered(false);
    platform.settle(&[&host]);
    host.window_covered_watched(true);
    platform.settle(&[&host]);
    let watched = frames();
    ticks(10);
    assert!(
        frames() >= watched + 2,
        "a watched page keeps turning on the refresh tick: {} then {}",
        watched,
        frames()
    );
    assert!(
        !host.adapter().log.lock().unwrap().vsync,
        "still no display link to ask"
    );
}

#[test]
fn a_stopped_window_ticks_no_frames_of_its_own_and_counts_none() {
    let worker = ViewWorker::spawn("test-window-stopped").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = revealing_page(&platform, &worker);
    platform.settle(&[&host]);
    let frames = || {
        host.automation_request(&serde_json::json!({"op": "frame"}))
            .unwrap()["pacing"]["frames"]
            .as_u64()
            .unwrap()
    };
    let before = frames();
    host.window_visible(false);
    resize(&host, 180);
    host.surface_changed();
    platform.settle(&[&host]);
    assert_eq!(
        platform.advance(Duration::from_secs(20)),
        0,
        "a backgrounded app keeps no frame timer"
    );
    assert_eq!(frames(), before, "nothing it completes counts");
}

#[test]
fn a_page_that_never_presents_is_revealed_held_by_the_watchdog() {
    let worker = ViewWorker::spawn("test-reveal-watchdog").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = revealing_page(&platform, &worker);
    host.adapter()
        .pending
        .store(true, std::sync::atomic::Ordering::Release);
    host.reveal(true);
    platform.settle(&[&host]);
    platform.advance(crate::session::host::REVEAL_DEADLINE);
    assert!(
        revealed(&host).is_empty(),
        "never presented: not a returning page"
    );
    platform.advance(crate::session::host::FIRST_FRAME_WATCHDOG);
    assert_eq!(revealed(&host), [true]);
}

#[test]
fn automation_requests_query_and_act_through_the_core_driver() {
    let worker = ViewWorker::spawn("test-automation-request").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){const n=useLocal('n',0);return h('button',{'data-testid':'add',onClick:()=>n.set(n.value+1)},`Count ${n.value}`)}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let query = serde_json::json!({"op": "query", "css": "[data-testid=\"add\"]"});
    let found = host.automation_request(&query).unwrap();
    assert!(found.to_string().contains("Count 0"), "{found}");
    let click = serde_json::json!({"op": "act", "css": "[data-testid=\"add\"]", "action": {"kind": "click"}});
    // An act names a call; `result` reports once the worker applied it and the frame settled.
    let call = host.automation_request(&click).unwrap()["call"].clone();
    platform.settle(&[&host]);
    assert_eq!(last(&host), ["Count 1"]);
    let result = serde_json::json!({"op": "result", "call": call});
    assert_eq!(
        host.automation_request(&result).unwrap(),
        serde_json::json!({"done": true, "value": null})
    );
    let tree = host
        .automation_request(&serde_json::json!({"op": "tree"}))
        .unwrap();
    assert!(tree.to_string().contains("Count 1"), "{tree}");
    let frame = host
        .automation_request(&serde_json::json!({"op": "frame"}))
        .unwrap();
    assert_eq!(frame["renderer"], "gpu");
    assert!(frame["frames"].as_u64().unwrap() >= 2, "{frame}");
    assert_eq!(frame["firstPresentAt"], 0, "the fake clock never moves");
    assert!(frame["commit"].as_u64().unwrap() > 0);
    assert!(frame["page"].is_string(), "PageUiFrame.page is a string");
    for key in [
        "workerQueueUs",
        "evalMs",
        "imageCacheBytes",
        "imageCacheEntries",
        "animationFrames",
        "wakeups",
    ] {
        assert!(!frame[key].is_null(), "{key} in {frame}");
    }
    assert_eq!(frame["reveal"]["deadlines"], 0);
    assert!(frame["reveal"]["lastReveal"].is_null());
    for (request, error) in [
        (
            serde_json::json!({"op": "act", "css": "button", "action": {"kind": "hover"}}),
            "automation action",
        ),
        (serde_json::json!({"op": "screenshot"}), "automation op"),
    ] {
        let failed = host.automation_request(&request).unwrap_err();
        assert!(
            failed.starts_with("E_UNSUPPORTED") && failed.contains(error),
            "{failed}"
        );
    }
}

#[test]
fn automation_calls_page_actions_and_reads_data_once_the_frame_commits() {
    if !bounded_jobs() {
        return;
    }
    let worker = ViewWorker::spawn("test-automation-action").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &(source(
            "function render({data}){return h('text',{'data-testid':'count'},`count ${data.count}`)}",
        ) + "\nglobalThis.__lingxiaActionCatalog={__names:['bump'],__modes:{bump:'call'}};"),
        true,
    );
    host.set_visible(true);
    let connection = ConnectionTicket::new(1);
    host.connect(connection);
    deliver(
        &host,
        connection,
        serde_json::json!({"v":2,"kind":"state.snapshot","rev":0,"state":{"count":1}}),
    );
    platform.settle(&[&host]);
    let ask = |request: serde_json::Value| host.automation_request(&request);
    let poll = |call: &serde_json::Value| {
        platform.settle(&[&host]);
        ask(serde_json::json!({"op": "result", "call": call}))
    };
    let data = ask(serde_json::json!({"op": "data"})).unwrap();
    assert_eq!(
        poll(&data["call"]).unwrap(),
        serde_json::json!({"done": true, "value": {"count": 1}})
    );
    let started =
        ask(serde_json::json!({"op": "action", "name": "bump", "args": {"by": 2}})).unwrap();
    platform.settle(&[&host]);
    // The View sent the request as `view.actions.bump({by: 2})` would.
    let request: serde_json::Value = {
        let log = host.adapter().log.lock().unwrap();
        serde_json::from_str(&log.messages.last().unwrap().1).unwrap()
    };
    assert_eq!(
        (request["method"].as_str(), &request["params"]),
        (Some("bump"), &serde_json::json!({"by": 2}))
    );
    assert_eq!(
        poll(&started["call"]).unwrap(),
        serde_json::json!({"done": false})
    );
    // Logic sets data, then answers; the result waits for the frame that shows it.
    deliver(
        &host,
        connection,
        serde_json::json!({"v":2,"kind":"state.snapshot","rev":1,"state":{"count":3}}),
    );
    deliver(
        &host,
        connection,
        serde_json::json!({"v":2,"kind":"res","id":request["id"],"ok":true,"result":{"count":3}}),
    );
    assert_eq!(
        poll(&started["call"]).unwrap(),
        serde_json::json!({"done": true, "value": {"count": 3}})
    );
    assert_eq!(last(&host), ["count 3"]);
    let data = ask(serde_json::json!({"op": "data"})).unwrap();
    assert_eq!(
        poll(&data["call"]).unwrap()["value"],
        serde_json::json!({"count": 3})
    );
    // Unknown actions and Logic failures come back as errors.
    let missing = ask(serde_json::json!({"op": "action", "name": "nope"})).unwrap();
    let error = poll(&missing["call"]).unwrap_err();
    assert!(
        error.starts_with("UI_ACTION") && error.contains("nope"),
        "{error}"
    );
    let failing = ask(serde_json::json!({"op": "action", "name": "bump"})).unwrap();
    platform.settle(&[&host]);
    let id = {
        let log = host.adapter().log.lock().unwrap();
        serde_json::from_str::<serde_json::Value>(&log.messages.last().unwrap().1).unwrap()["id"]
            .clone()
    };
    deliver(
        &host,
        connection,
        serde_json::json!({"v":2,"kind":"res","id":id,"ok":false,"error":{"code":"E_LIMIT","message":"too many"}}),
    );
    assert_eq!(poll(&failing["call"]).unwrap_err(), "E_LIMIT: too many");
}

#[test]
fn platform_edits_and_picks_reach_the_bound_locals() {
    fill_and_pick(false, false, false);
}

#[test]
fn platform_edits_and_picks_reach_the_bound_locals_in_a_dev_module() {
    fill_and_pick(true, false, false);
}

#[cfg(feature = "hot-swap")]
#[test]
fn platform_edits_and_picks_reach_the_bound_locals_after_a_hot_swap() {
    fill_and_pick(true, true, false);
    fill_and_pick(true, true, true);
}

/// The automation fill and pick flow; `dev` loads the module as `lingxia dev` compiles it, `swap`
/// replaces it in place first, and `logic` connects a Logic page.
fn fill_and_pick(dev: bool, swap: bool, logic: bool) {
    let worker = ViewWorker::spawn("test-fill-select").unwrap();
    let platform = Platform::new(&[&worker]);
    let mut page = source(
        "function render(){const name=useLocal('name',''),plan=useLocal('plan','');return h('view',{},h('input',{'data-testid':'name',bind:name,'aria-label':'Name'}),h('select',{'data-testid':'plan',bind:plan,'aria-label':'Plan',options:[{value:'a',label:'A'},{value:'b',label:'B'}]}),h('text',{'data-testid':'out'},`name=${name.value} plan=${plan.value}`))}",
    );
    if dev {
        page.push_str("\nconfigureSourceMap({sources:[null],mappings:''});\nenableHotSwap();\n");
    }
    if logic {
        page.push_str("\nglobalThis.__lingxiaActionCatalog={__names:[],__modes:{}};");
    }
    let host = platform.host(&worker, &page, logic);
    host.adapter()
        .environment
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .value["reducedMotion"] = serde_json::json!(true);
    host.set_visible(true);
    if logic {
        let connection = ConnectionTicket::new(1);
        host.connect(connection);
        deliver(
            &host,
            connection,
            serde_json::json!({"v":2,"kind":"state.snapshot","rev":0,"state":{"n":0}}),
        );
    }
    platform.settle(&[&host]);
    #[cfg(feature = "hot-swap")]
    if swap {
        host.swap_module(page.clone());
        platform.settle(&[&host]);
    }
    let _ = swap;
    // What a platform editor does for an automation fill: focus and replace; the select is the View's.
    let ask = |request: serde_json::Value| -> serde_json::Value {
        let (tx, rx) = channel();
        host.input(request.to_string(), move |reply| {
            let _ = tx.send(reply);
        });
        platform.settle(&[&host]);
        serde_json::from_str(&rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap()).unwrap()
    };
    let snapshot: serde_json::Value =
        serde_json::from_str(host.snapshot().unwrap().json()).unwrap();
    let node = |test_id: &str| {
        snapshot["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["test_id"] == test_id)
            .unwrap()
            .clone()
    };
    let name = node("name");
    let generation = snapshot["generation"].clone();
    let token =
        ask(serde_json::json!({"op":"focus","identity":name["identity"],"generation":generation}))
            ["token"]
            .clone();
    ask(
        serde_json::json!({"op":"edit","identity":name["identity"],"token":token,"submit":false,
        "edits":[{"op":"replace","text":"Ada","cursor":1,"composing":false}]}),
    );
    host.request_frame();
    platform.settle(&[&host]);
    let generation: serde_json::Value = serde_json::from_str::<serde_json::Value>(
        host.snapshot().unwrap().json(),
    )
    .unwrap()["generation"]
        .clone();
    let _ = generation;
    // A select's choice comes from the View's own picker, as automation drives it.
    host.automation_request(&serde_json::json!({"op":"act","css":"[data-testid=\"plan\"]","action":{"kind":"select","value":"b"}}))
        .unwrap();
    platform.settle(&[&host]);
    host.request_frame();
    platform.settle(&[&host]);
    let out = host
        .automation_request(&serde_json::json!({"op": "query", "css": "[data-testid=\"out\"]"}))
        .unwrap();
    assert_eq!(out["text"], "name=Ada plan=b");
}

#[test]
fn logic_feedback_calls_draw_in_the_view_and_answer_like_the_webview() {
    if !bounded_jobs() {
        return;
    }
    let worker = ViewWorker::spawn("test-view-calls").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &(source(
            "function render({data}){return h('page',{},h('text',{'data-testid':'body'},`n ${data.n}`))}",
        ) + "\nglobalThis.__lingxiaActionCatalog={__names:[],__modes:{}};"),
        true,
    );
    // The fake clock stands still between settles; reduced motion lands presence at once.
    host.adapter()
        .environment
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .value["reducedMotion"] = serde_json::json!(true);
    host.set_visible(true);
    let connection = ConnectionTicket::new(1);
    host.connect(connection);
    deliver(
        &host,
        connection,
        serde_json::json!({"v":2,"kind":"state.snapshot","rev":0,"state":{"n":0}}),
    );
    platform.settle(&[&host]);
    let replies = || -> Vec<serde_json::Value> {
        host.adapter()
            .log
            .lock()
            .unwrap()
            .messages
            .iter()
            .filter_map(|(_, m)| serde_json::from_str::<serde_json::Value>(m).ok())
            .filter(|m| m["kind"] == "res")
            .collect()
    };
    let call = |id: &str, method: &str, params: serde_json::Value| {
        deliver(
            &host,
            connection,
            serde_json::json!({"v":2,"kind":"req","id":id,"method":method,"params":params}),
        );
        platform.settle(&[&host]);
    };
    let query = |css: &str| {
        host.automation_request(&serde_json::json!({"op":"query","css":css}))
            .unwrap()
    };
    // A toast answers on presentation and leaves after its duration.
    call(
        "lv_2",
        "ui.showToast",
        serde_json::json!({"title":"Saved","icon":"success","duration":1500}),
    );
    assert_eq!(
        replies().into_iter().find(|r| r["id"] == "lv_2").unwrap()["ok"],
        true
    );
    let toast = query("[data-testid=\"lx-toast\"]");
    assert_eq!(
        (toast["exists"].as_bool(), toast["text"].as_str()),
        (Some(true), Some("Saved"))
    );
    assert_eq!(toast["role"], "status");
    platform.advance(Duration::from_millis(1600));
    platform.settle(&[&host]);
    assert_eq!(query("[data-testid=\"lx-toast\"]")["exists"], false);
    // A modal answers when a button is chosen.
    call(
        "lv_3",
        "ui.showModal",
        serde_json::json!({"title":"Delete?","content":"This cannot be undone","confirmText":"Delete"}),
    );
    assert!(
        replies().iter().all(|r| r["id"] != "lv_3"),
        "waits for a choice"
    );
    let dialog = query("[role=\"alertdialog\"]");
    assert_eq!(dialog["aria_label"], "Delete?");
    host.automation_request(&serde_json::json!({"op":"act","css":"[data-testid=\"lx-modal-confirm\"]","action":{"kind":"click"}})).unwrap();
    platform.settle(&[&host]);
    let chosen = replies()
        .into_iter()
        .find(|r| r["id"] == "lv_3")
        .expect("answered after the tap");
    assert_eq!(
        chosen["result"],
        serde_json::json!({"confirm": true, "cancel": false})
    );
    assert_eq!(query("[data-testid=\"lx-modal\"]")["exists"], false);
    // An action sheet answers with the tapped index, or -1 for cancel.
    call(
        "lv_4",
        "ui.showActionSheet",
        serde_json::json!({"itemList":["Copy","Share"]}),
    );
    host.automation_request(&serde_json::json!({"op":"act","css":"[data-testid=\"lx-action-sheet-item-1\"]","action":{"kind":"click"}})).unwrap();
    platform.settle(&[&host]);
    assert_eq!(
        replies().into_iter().find(|r| r["id"] == "lv_4").unwrap()["result"],
        serde_json::json!({"tapIndex": 1})
    );
    call(
        "lv_5",
        "ui.showActionSheet",
        serde_json::json!({"itemList":["Copy"]}),
    );
    host.automation_request(&serde_json::json!({"op":"act","css":"[data-testid=\"lx-action-sheet-cancel\"]","action":{"kind":"click"}})).unwrap();
    platform.settle(&[&host]);
    assert_eq!(
        replies().into_iter().find(|r| r["id"] == "lv_5").unwrap()["result"],
        serde_json::json!({"tapIndex": -1})
    );
    // Escape dismisses the innermost overlay as its cancel would.
    call(
        "lv_6",
        "ui.showActionSheet",
        serde_json::json!({"itemList":["Copy"]}),
    );
    assert!(
        host.key(crate::interaction::view::Key::Escape, false, false),
        "a dialog takes Escape"
    );
    platform.settle(&[&host]);
    assert_eq!(
        replies().into_iter().find(|r| r["id"] == "lv_6").unwrap()["result"],
        serde_json::json!({"tapIndex": -1})
    );
    call(
        "lv_7",
        "ui.showModal",
        serde_json::json!({"title":"Leave?"}),
    );
    host.key(crate::interaction::view::Key::Escape, false, false);
    platform.settle(&[&host]);
    assert_eq!(
        replies().into_iter().find(|r| r["id"] == "lv_7").unwrap()["result"],
        serde_json::json!({"confirm": false, "cancel": true})
    );
    // Logic's dismiss closes the dialogs on screen, each settling as cancelled.
    call("lv_9", "ui.showModal", serde_json::json!({"title":"Wait"}));
    call(
        "lv_10",
        "ui.showActionSheet",
        serde_json::json!({"itemList":["Copy"]}),
    );
    call("lv_11", "ui.dismissDialog", serde_json::json!({}));
    platform.settle(&[&host]);
    let replies_now = replies();
    let answer = |id: &str| replies_now.iter().find(|r| r["id"] == id).cloned().unwrap();
    assert_eq!(
        answer("lv_9")["result"],
        serde_json::json!({"confirm": false, "cancel": true})
    );
    assert_eq!(
        answer("lv_10")["result"],
        serde_json::json!({"tapIndex": -1})
    );
    assert_eq!(answer("lv_11")["ok"], true);
    assert_eq!(query("[data-testid=\"lx-modal\"]")["exists"], false);
    assert_eq!(query("[data-testid=\"lx-action-sheet\"]")["exists"], false);
    call(
        "lv_8",
        "ui.showToast",
        serde_json::json!({"title":"Busy","duration":0}),
    );
    assert!(
        !host.key(crate::interaction::view::Key::Escape, false, false),
        "a toast is no dialog"
    );
    platform.settle(&[&host]);
    assert_eq!(
        query("[data-testid=\"lx-toast\"]")["exists"],
        false,
        "but Escape still hides it"
    );
    assert_eq!(
        query("[data-testid=\"body\"]")["text"],
        "n 0",
        "the page underneath is untouched"
    );
}

#[test]
fn abandoned_automation_calls_expire_instead_of_exhausting_the_slots() {
    if !bounded_jobs() {
        return;
    }
    let worker = ViewWorker::spawn("test-automation-expiry").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &(source("function render({data}){return h('text',{},`n ${data.n}`)}")
            + "\nglobalThis.__lingxiaActionCatalog={__names:['slow'],__modes:{slow:'call'}};"),
        true,
    );
    host.set_visible(true);
    let connection = ConnectionTicket::new(1);
    host.connect(connection);
    deliver(
        &host,
        connection,
        serde_json::json!({"v":2,"kind":"state.snapshot","rev":0,"state":{"n":0}}),
    );
    platform.settle(&[&host]);
    // Logic never answers and no caller polls: 100 timed-out calls in a row.
    let mut first = None;
    for _ in 0..100 {
        let started = host
            .automation_request(&serde_json::json!({"op": "action", "name": "slow"}))
            .unwrap();
        first.get_or_insert(started["call"].clone());
    }
    let expired = host
        .automation_request(&serde_json::json!({"op": "result", "call": first}))
        .unwrap_err();
    assert!(
        expired.starts_with("E_UNSUPPORTED") && expired.contains("expired"),
        "{expired}"
    );
    let data = host
        .automation_request(&serde_json::json!({"op": "data"}))
        .unwrap();
    platform.settle(&[&host]);
    assert_eq!(
        host.automation_request(&serde_json::json!({"op": "result", "call": data["call"]}))
            .unwrap()["value"],
        serde_json::json!({"n": 0}),
        "a new call still gets a slot"
    );
}

#[test]
fn a_page_parked_while_still_shown_stays_blank() {
    let worker = ViewWorker::spawn("test-park-shown").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render(){return h('text',{'data-testid':'t'},'page')}"),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let presents = || host.adapter().log.lock().unwrap().presented.len();
    let before = presents();
    // Parked before the next page hides it, as a desktop host can.
    host.park();
    host.surface_changed();
    host.request_frame();
    platform.settle(&[&host]);
    platform.advance(Duration::from_secs(2));
    platform.settle(&[&host]);
    assert_eq!(presents(), before, "a shown parked page renders nothing");
    let found = host
        .automation_request(&serde_json::json!({"op": "query", "css": "[data-testid=\"t\"]"}))
        .unwrap();
    assert_eq!(found["exists"], false, "no old frame un-parks it");
}

#[test]
fn a_parked_page_reads_blank_and_refuses_actions() {
    let worker = ViewWorker::spawn("test-automation-parked").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){const n=useLocal('n',0);return h('view',{'data-testid':'card','data-mode':'live'},h('text',{},'Total'),h('button',{'data-testid':'add',onClick:()=>n.set(n.value+1)},`Count ${n.value}`))}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    // Navigating away: the page leaves the stack and is parked.
    host.set_visible(false);
    host.park();
    platform.settle(&[&host]);
    let card = host
        .automation_request(&serde_json::json!({"op": "query", "css": "[data-mode=\"live\"]"}))
        .unwrap();
    assert_eq!(card["exists"], false);
    let all = host
        .automation_request(
            &serde_json::json!({"op": "query", "css": "[data-testid=\"add\"]", "all": true}),
        )
        .unwrap();
    assert_eq!(
        (all["count"].clone(), all["items"].clone()),
        (serde_json::json!(0), serde_json::json!([]))
    );
    assert!(
        host.automation_request(&serde_json::json!({"op": "tree"}))
            .is_ok()
    );
    let click = serde_json::json!({"op": "act", "css": "[data-testid=\"add\"]", "action": {"kind": "click"}});
    let error = host.automation_request(&click).unwrap_err();
    assert!(
        error.starts_with("E_PAGE_NOT_READY") && error.contains("parked"),
        "{error}"
    );
}

#[test]
fn accessibility_selects_and_copies_static_text_through_the_session_selection() {
    use crate::interaction::view::AccessibilityAction;
    let worker = ViewWorker::spawn("test-static-copy").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render(){return h('text',{'data-testid':'p',style:{userSelect:'text'}},'hello brave world')}"),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let id = host.semantics(|projection, _| projection.unwrap().ids.id(0).unwrap());
    assert!(
        !host.accessibility(id, AccessibilityAction::Copy),
        "nothing is selected yet"
    );
    assert!(host.accessibility(id, AccessibilityAction::SetSelection(6, 11)));
    platform.settle(&[&host]);
    let selection =
        host.semantics(|projection, _| projection.unwrap().node(id).unwrap().text_selection);
    assert_eq!(selection, Some([6, 11]));
    assert!(host.accessibility(id, AccessibilityAction::Copy));
    platform.settle(&[&host]);
    assert_eq!(host.adapter().log.lock().unwrap().copied, ["brave"]);
    let selection =
        host.semantics(|projection, _| projection.unwrap().node(id).unwrap().text_selection);
    assert_eq!(selection, None, "Copy drops the selection");
}

#[test]
fn a_failed_session_leaves_touches_and_keys_to_the_platform() {
    use crate::interaction::view::Key;
    let worker = ViewWorker::spawn("test-failed-input").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render(){return h('button',{onClick:()=>{}},'tap')}"),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let down = Pointer {
        phase: PointerPhase::Down,
        x: 10.0,
        y: 10.0,
        slop: 8.0,
        velocity: [0.0; 2],
        min_fling: 50.0,
    };
    assert!(host.pointer(down));
    host.window_focus(false);
    assert!(
        host.adapter()
            .log
            .lock()
            .unwrap()
            .effects
            .contains(&Effect::ClearPress),
        "losing window focus ends the press"
    );
    assert!(host.key(Key::Tab, false, false));
    host.fail("UI_TIMEOUT: View JavaScript exceeded its execution budget".into());
    assert!(!host.pointer(down));
    assert!(!host.key(Key::Tab, false, false));
}

#[test]
fn the_frame_says_how_to_draw_press_and_focus_feedback_for_its_theme() {
    use crate::interaction::chrome::FeedbackStyle;
    let worker = ViewWorker::spawn("test-feedback-style").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render(){return h('button',{onClick:()=>{}},'tap')}"),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let snapshot: serde_json::Value =
        serde_json::from_str(host.snapshot().unwrap().json()).unwrap();
    let feedback = |dark| FeedbackStyle::for_palette(&crate::theme::Palette::builtin(dark), dark);
    assert_eq!(
        snapshot["feedback"],
        serde_json::to_value(feedback(false)).unwrap()
    );
    assert_eq!(snapshot["feedback"]["pressOverlay"], 0x2217_2033u32);
    assert_eq!(snapshot["feedback"]["selectionHandle"], 0xFF36_5CDDu32);
    assert_eq!(snapshot["feedback"]["noticeBackground"], 0xFFFF_FFFFu32);
    assert_ne!(dark_notice(), feedback(false).notice_text);
    assert_ne!(feedback(true).focus_ring, feedback(false).focus_ring);
}

fn menu(host: &ViewHost<Fake>) -> Option<crate::session::host::TextMenu> {
    host.adapter()
        .log
        .lock()
        .unwrap()
        .menus
        .last()
        .cloned()
        .flatten()
}

#[test]
fn a_static_selection_shows_its_menu_which_hides_while_dragging_and_clears_on_dismiss() {
    use crate::session::host::{MenuAction, MenuTarget};
    let worker = ViewWorker::spawn("test-static-menu").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render(){return h('text',{'data-testid':'p',style:{userSelect:'text'}},'hello brave world')}"),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    assert!(menu(&host).is_none());
    let identity = host.snapshot().unwrap().frame.nodes[0].identity.clone();
    host.set_text_selection(identity.to_string(), 0, 5);
    platform.settle(&[&host]);
    let shown = menu(&host).unwrap();
    assert_eq!(
        shown.target,
        MenuTarget::Text {
            identity: identity.to_string()
        }
    );
    assert_eq!(shown.actions, [MenuAction::Copy, MenuAction::SelectAll]);
    host.text_handle_drag(true);
    assert!(menu(&host).is_none(), "hidden while a handle moves");
    host.text_handle_drag(false);
    assert!(menu(&host).is_some());
    assert!(host.text_menu_action(MenuAction::SelectAll));
    platform.settle(&[&host]);
    let selection = host.snapshot().unwrap().frame.nodes[0].selection_range;
    assert_eq!(selection, Some([0, 17]));
    let events = host.adapter().log.lock().unwrap().menus.len();
    host.text_menu_dismissed();
    platform.settle(&[&host]);
    assert_eq!(
        host.adapter().log.lock().unwrap().menus.len(),
        events,
        "the adapter closed it already"
    );
    assert_eq!(
        host.snapshot().unwrap().frame.nodes[0].selection_range,
        None
    );
}

#[test]
fn copying_a_static_selection_clears_it_and_closes_its_menu() {
    use crate::session::host::MenuAction;
    let worker = ViewWorker::spawn("test-static-copy").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render(){return h('text',{'data-testid':'p',style:{userSelect:'text'}},'hello brave world')}"),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let identity = host.snapshot().unwrap().frame.nodes[0].identity.clone();
    host.set_text_selection(identity.to_string(), 0, 5);
    platform.settle(&[&host]);
    assert!(menu(&host).is_some());
    assert!(host.text_menu_action(MenuAction::Copy));
    platform.settle(&[&host]);
    assert_eq!(
        host.snapshot().unwrap().frame.nodes[0].selection_range,
        None
    );
    assert!(menu(&host).is_none(), "the menu goes with its selection");
}

fn fine_pointer_text_host(platform: &Platform, worker: &Arc<ViewWorker>) -> ViewHost<Fake> {
    let host = platform.host(
        worker,
        &source("function render(){return h('text',{'data-testid':'p',style:{userSelect:'text'}},'hello brave world')}"),
        false,
    );
    if let Some(environment) = host.adapter().environment.lock().unwrap().as_mut() {
        environment.value["pointer"] = "fine".into();
    }
    host.set_visible(true);
    platform.settle(&[&host]);
    host
}

#[test]
fn a_mouse_drag_selects_static_text_and_its_menu_waits_for_a_secondary_click() {
    let worker = ViewWorker::spawn("test-mouse-select").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = fine_pointer_text_host(&platform, &worker);
    // The text wraps in its 100-point box; points on its first line, device pixels at scale 2.
    let [x, y, w, h] = host.snapshot().unwrap().frame.nodes[0].bounds;
    let line = (y + 8.0) * 2.0;
    let at = |px: f32, phase| Pointer {
        phase,
        x: px * 2.0,
        y: line,
        slop: 8.0,
        velocity: [0.0; 2],
        min_fling: 50.0,
    };
    assert!(host.pointer(at(x + 1.0, PointerPhase::Down)));
    assert!(host.pointer(at(x + w / 2.0, PointerPhase::Move)));
    assert!(host.pointer(at(x + w - 1.0, PointerPhase::Move)));
    assert!(host.pointer(at(x + w - 1.0, PointerPhase::Up)));
    platform.settle(&[&host]);
    let range = host.snapshot().unwrap().frame.nodes[0].selection_range;
    assert!(
        range.is_some_and(|[a, b]| a == 0 && b >= 10),
        "the drag selects what it crossed: {range:?}"
    );
    assert!(menu(&host).is_none(), "a mouse selects without a menu");
    assert!(
        !host.context_menu([0.0, (y + h + 40.0) * 2.0]),
        "nothing under the click"
    );
    assert!(host.context_menu([(x + 4.0) * 2.0, line]));
    assert!(
        menu(&host).is_some_and(|m| m.actions.contains(&crate::session::host::MenuAction::Copy))
    );
    host.text_menu_dismissed();
    platform.settle(&[&host]);
    assert_eq!(
        host.snapshot().unwrap().frame.nodes[0].selection_range,
        range,
        "closing a context menu keeps the selection"
    );
}

#[test]
fn a_double_click_selects_the_word_under_it() {
    let worker = ViewWorker::spawn("test-double-click").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = fine_pointer_text_host(&platform, &worker);
    let [x, y, ..] = host.snapshot().unwrap().frame.nodes[0].bounds;
    host.select_word_at([(x + 2.0) * 2.0, (y + 8.0) * 2.0]);
    platform.settle(&[&host]);
    assert_eq!(
        host.snapshot().unwrap().frame.nodes[0].selection_range,
        Some([0, 5])
    );
    assert!(menu(&host).is_none());
}

#[test]
fn an_editor_menu_offers_what_the_editor_allows_and_goes_when_its_text_changes() {
    use crate::session::host::{MenuAction, MenuTarget};
    let worker = ViewWorker::spawn("test-editor-menu").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){const v=useLocal('v','');return h('input',{bind:v,autoFocus:true})}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let (identity, token) = {
        let snapshot = host.snapshot().unwrap();
        let node = &snapshot.frame.nodes[0];
        (
            node.identity.clone(),
            node.input.as_ref().unwrap().edit_token.unwrap(),
        )
    };
    host.text_handle_drag(false);
    let shown = menu(&host).unwrap();
    assert_eq!(
        shown.target,
        MenuTarget::Editor {
            identity: identity.to_string()
        }
    );
    assert_eq!(
        shown.actions,
        [MenuAction::Paste],
        "an empty editor can only paste"
    );
    let typed = serde_json::json!({
        "op": "edit", "identity": identity, "token": token, "submit": false,
        "edits": [{"op": "replace", "text": "hi", "cursor": 1, "composing": false}],
    });
    host.input(typed.to_string(), |_| {});
    platform.settle(&[&host]);
    // The adapter renders when the edit reply asks for it.
    host.request_frame();
    platform.settle(&[&host]);
    assert!(menu(&host).is_none(), "typing dismisses the menu");
    host.text_handle_drag(false);
    let shown = menu(&host).unwrap();
    assert!(
        shown.actions.contains(&MenuAction::SelectAll) && shown.actions.contains(&MenuAction::Undo)
    );
    assert!(
        !host.text_menu_action(MenuAction::Paste),
        "the input connection pastes"
    );
    assert!(menu(&host).is_none());
}

fn sent_edits(host: &ViewHost<Fake>) -> Vec<Option<String>> {
    host.adapter()
        .log
        .lock()
        .unwrap()
        .edits
        .iter()
        .filter_map(|(effect, _)| match effect {
            Effect::EditText { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn edits_for_two_editors_reach_the_platform_one_at_a_time() {
    let worker = ViewWorker::spawn("test-edit-queue").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){const a=useLocal('a','');const b=useLocal('b','');return h('view',{},h('input',{'data-testid':'amount',bind:a,autoFocus:true}),h('input',{'data-testid':'code',bind:b}))}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    // `fill` on the focused editor and `type` into the other both go through the input method.
    let act = |kind: &str, id: &str, text: &str| {
        let request = serde_json::json!({"op": "act", "css": format!("[data-testid=\"{id}\"]"), "action": {"kind": kind, "text": text}});
        host.automation_request(&request)
    };
    // As on a cold start: the second editor is still connecting when its edit arrives.
    host.adapter()
        .edit_outcomes
        .lock()
        .unwrap()
        .extend([EditOutcome::Applied, EditOutcome::NotReady]);
    act("fill", "amount", "12").unwrap();
    act("type", "code", "ABC").unwrap();
    assert_eq!(
        sent_edits(&host),
        [Some("12".to_owned())],
        "the second waits"
    );
    // The platform applies the first through its input connection.
    let (identity, token) = {
        let snapshot = host.snapshot().unwrap();
        let node = &snapshot.frame.nodes[1];
        (
            node.identity.clone(),
            node.input.as_ref().unwrap().edit_token.unwrap(),
        )
    };
    let applied = serde_json::json!({
        "op": "edit", "identity": identity, "token": token, "submit": false,
        "edits": [{"op": "replace", "text": "12", "cursor": 1, "composing": false}],
    });
    host.input(applied.to_string(), |_| {});
    platform.settle(&[&host]);
    host.request_frame();
    platform.settle(&[&host]);
    assert_eq!(
        sent_edits(&host),
        [Some("12".to_owned()), Some("ABC".to_owned())],
        "the frame showing the first releases the second"
    );
    // Not ready: offered again shortly, without waiting out an applied edit's patience.
    platform.advance(Duration::from_millis(50));
    assert_eq!(
        sent_edits(&host),
        [
            Some("12".to_owned()),
            Some("ABC".to_owned()),
            Some("ABC".to_owned())
        ]
    );
    let outcomes: Vec<EditOutcome> = host
        .adapter()
        .log
        .lock()
        .unwrap()
        .edits
        .iter()
        .map(|(_, o)| *o)
        .collect();
    assert_eq!(
        outcomes,
        [
            EditOutcome::Applied,
            EditOutcome::NotReady,
            EditOutcome::Applied
        ]
    );
}

#[test]
fn an_edit_the_platform_cannot_apply_fails_its_caller() {
    use crate::interaction::view::AccessibilityAction;
    let worker = ViewWorker::spawn("test-edit-failed").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render(){const a=useLocal('a','');return h('input',{'data-testid':'amount',bind:a})}"),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    host.adapter()
        .edit_outcomes
        .lock()
        .unwrap()
        .push_back(EditOutcome::Failed);
    let id = host.semantics(|projection, _| projection.unwrap().ids.id(0).unwrap());
    assert!(!host.accessibility(id, AccessibilityAction::SetText("12".into())));
    host.adapter()
        .edit_outcomes
        .lock()
        .unwrap()
        .push_back(EditOutcome::Failed);
    let request = serde_json::json!({"op": "act", "css": "[data-testid=\"amount\"]", "action": {"kind": "type", "text": "7"}});
    let failed = host.automation_request(&request).unwrap_err();
    assert!(failed.contains("refused the edit"), "{failed}");
    assert!(
        host.accessibility(id, AccessibilityAction::SetText("9".into())),
        "a later edit goes through"
    );
}

/// `fill` takes no focus: an editor without the input method gets its text from the core, its
/// binding hears it, and the platform's input method is never asked (so no keyboard rises).
#[test]
fn fill_sets_an_unfocused_field_in_the_core_without_the_input_method() {
    let worker = ViewWorker::spawn("test-fill-core").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render(){const a=useLocal('a','old');return h('view',{},h('input',{'data-testid':'amount',bind:a}),h('text',{'data-testid':'log'},'['+a.value+']'))}"),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let call = host
        .automation_request(&serde_json::json!({"op": "act", "css": "[data-testid=\"amount\"]", "action": {"kind": "fill", "text": "42"}}))
        .unwrap();
    platform.settle(&[&host]);
    host.request_frame();
    platform.settle(&[&host]);
    assert_eq!(
        host.automation_request(&serde_json::json!({"op": "result", "call": call["call"]}))
            .unwrap()["done"],
        true,
        "the call settles once a frame shows the text"
    );
    let query = |id: &str| {
        host.automation_request(
            &serde_json::json!({"op": "query", "css": format!("[data-testid=\"{id}\"]")}),
        )
        .unwrap()
    };
    assert_eq!(query("amount")["value"], "42");
    assert_eq!(
        query("log")["text"],
        "[42]",
        "Logic heard it through the binding"
    );
    assert_eq!(query("amount")["focused"], false, "fill took no focus");
    let log = host.adapter().log.lock().unwrap();
    assert!(
        log.edits.is_empty() && log.editors.is_empty(),
        "the input method was never asked"
    );
}

fn content_changes(host: &ViewHost<Fake>) -> usize {
    host.adapter()
        .log
        .lock()
        .unwrap()
        .events
        .iter()
        .filter(|event| {
            matches!(
                event,
                AccessibilityEffect::Change(crate::semantics::projection::Event::ContentChanged)
            )
        })
        .count()
}

#[test]
fn a_held_press_keeps_its_scale_transition_and_accessibility_hears_moves_at_a_quarter_rate() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let page = source(
        "function render(){return h('view',{},h('button',{'data-testid':'b',onClick(){},className:'active:x',style:{width:100,height:40,padding:0,transition:{properties:['transform'],durationMs:150,delayMs:0,easing:'linear'}}},'Press'))}",
    )
    .replace(
        "__lingxiaPublish(render);",
        "configureClasses([{name:'active:x',when:{state:'active'},style:{scale:[0.95,0.95]}}]);__lingxiaPublish(render);",
    );
    let host = platform.host(&worker, &page, false);
    host.set_visible(true);
    platform.settle(&[&host]);
    let bounds = |host: &ViewHost<Fake>| {
        let snapshot = host.snapshot().unwrap();
        snapshot
            .frame
            .nodes
            .iter()
            .find(|n| n.test_id == "b")
            .unwrap()
            .bounds
    };
    assert_eq!(bounds(&host)[2], 100.0);
    host.pointer(Pointer {
        phase: PointerPhase::Down,
        x: 100.0,
        y: 40.0,
        slop: 8.0,
        velocity: [0.0; 2],
        min_fling: 50.0,
    });
    platform.settle(&[&host]);
    let clears = || {
        host.adapter()
            .log
            .lock()
            .unwrap()
            .effects
            .iter()
            .filter(|e| **e == Effect::ClearPress)
            .count()
    };
    let (before, cleared) = (content_changes(&host), clears());
    run_frames(&platform, &host, 48);
    // Still held after 800 ms: fully pressed, never released by its own animation frames.
    assert!(
        (bounds(&host)[2] - 95.0).abs() < 0.01,
        "{:?}",
        bounds(&host)
    );
    assert_eq!(clears(), cleared);
    // Nine moving frames report at most every fourth, plus one when they settle.
    let moves = content_changes(&host) - before;
    assert!((1..=4).contains(&moves), "{moves}");
}

#[test]
fn a_spinning_decoration_sends_accessibility_no_content_changes() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(&format!(
            "function render(){{return h('view',{{}},h('text',{{}},'Loading'),h('view',{{style:{{width:20,height:20,backgroundColor:'#ff0000',animation:{SPIN}}}}}))}}"
        )),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let before = content_changes(&host);
    run_frames(&platform, &host, 24);
    assert_eq!(content_changes(&host), before);
}

#[test]
fn a_frame_drawn_off_the_worker_reports_its_presenter_counters() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render(){return h('view',{},h('text',{},'drawn'))}"),
        false,
    );
    host.adapter()
        .defer
        .store(true, std::sync::atomic::Ordering::Release);
    host.set_visible(true);
    platform.settle(&[&host]);
    let generation = *host
        .adapter()
        .log
        .lock()
        .unwrap()
        .generations
        .last()
        .unwrap();
    host.deferred_drawn(
        generation,
        true,
        crate::session::host::DrawStats {
            gpu_us: [0, 900, 0, 100],
            presenter: Some(serde_json::json!({
                "composite": {"draws": 3},
                "backdropCache": {"filtered": 1},
                "passes": [[500, 50]],
            })),
        },
    );
    platform.settle(&[&host]);
    let metrics = host
        .automation_request(&serde_json::json!({"op": "frame"}))
        .unwrap();
    assert_eq!(metrics["composite"]["draws"], 3);
    assert_eq!(metrics["backdropCache"]["filtered"], 1);
    assert_eq!(metrics["passes"], serde_json::json!([[500, 50]]));
}

#[test]
fn a_deferred_present_holds_the_next_frame_until_it_reaches_the_screen() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(&format!(
            "function render(){{return h('view',{{style:{{width:20,height:20,backgroundColor:'#ff0000',animation:{SPIN}}}}})}}"
        )),
        false,
    );
    host.adapter()
        .defer
        .store(true, std::sync::atomic::Ordering::Release);
    host.set_visible(true);
    platform.settle(&[&host]);
    run_frames(&platform, &host, 10);
    let queued = presents(&host);
    assert_eq!(queued, 1, "nothing prepares behind a frame still rastering");
    assert!(
        host.snapshot().is_none(),
        "not presented until it is on screen"
    );
    let generation = *host
        .adapter()
        .log
        .lock()
        .unwrap()
        .generations
        .last()
        .unwrap();
    host.deferred_presented(generation, true);
    platform.settle(&[&host]);
    assert!(host.snapshot().is_some());
    run_frames(&platform, &host, 1);
    assert_eq!(
        presents(&host),
        2,
        "the next frame follows the acknowledgement"
    );
    // A dropped deferred frame is prepared again.
    let generation = *host.adapter().log.lock().unwrap().handed.last().unwrap();
    host.deferred_presented(generation, false);
    run_frames(&platform, &host, 2);
    assert_eq!(presents(&host), 3);
    let log = host.adapter().log.lock().unwrap();
    assert!(
        log.handed.last().unwrap().get() > generation.get() + 1,
        "the frame prepared on the dropped one is dropped too: {:?}",
        log.handed
    );
}

fn dark_notice() -> u32 {
    let dark = crate::theme::Palette::builtin(true);
    crate::interaction::chrome::FeedbackStyle::for_palette(&dark, true).notice_text
}

#[test]
fn a_tap_during_a_running_animation_dispatches_and_rebuilds_within_a_frame() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(&format!(
            "function render(){{const n=useLocal('n',0);return h('view',{{}},h('button',{{onClick:()=>n.set(n.value+1),style:{{height:60}}}},`Count ${{n.value}}`),h('view',{{style:{{width:20,height:20,backgroundColor:'#ff0000',animation:{SPIN}}}}}))}}"
        )),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    run_frames(&platform, &host, 10);
    assert_eq!(last(&host), ["Count 0"]);
    for round in 1..=3 {
        tap(&host, 20.0, 20.0);
        run_frames(&platform, &host, 2);
        assert_eq!(last(&host), [format!("Count {round}")], "tap {round}");
        run_frames(&platform, &host, 5);
    }
}

#[derive(Default)]
struct Costs {
    worker: Duration,
    ui: Duration,
    worker_free: Duration,
    main_free: Duration,
    /// When each prepared frame can be taken by a vsync, in presentation order.
    ready: std::collections::VecDeque<Duration>,
    /// Refuse this many presentations.
    decline: u32,
    /// Keep only the newest prepared frame, as a presenter with one target does.
    single_target: bool,
    /// Decline a frame while this many prepared frames still hold a target.
    targets: Option<usize>,
    held: Arc<Mutex<Vec<CommitTicket>>>,
    declined: u32,
    on_screen: Option<Encoded>,
    /// Decline the first frame showing each new View, as a presenter short of targets may.
    decline_new_views: bool,
    declined_view: Option<Vec<String>>,
}

/// One display frame per 16.67 ms; a vsync presents only a frame that is ready by then.
fn run_timed(platform: &Platform, host: &ViewHost<Fake>, frames: u32) {
    let frame = Duration::from_micros(16_667);
    for _ in 0..frames {
        let now = {
            let mut clock = host.adapter().clock.lock().unwrap();
            *clock += frame;
            *clock
        };
        platform.advance(frame);
        let mut vsynced = false;
        loop {
            for worker in &platform.workers {
                let (done, wait) = channel();
                worker.run_background(move |_| {
                    let _ = done.send(());
                });
                wait.recv_timeout(Duration::from_secs(30)).unwrap();
            }
            let mut ran = false;
            while let Ok(task) = platform.tasks.try_recv() {
                task();
                ran = true;
            }
            let ready = host
                .adapter()
                .costs
                .lock()
                .unwrap()
                .as_ref()
                .is_none_or(|costs| costs.ready.front().is_some_and(|at| *at <= now));
            if !vsynced && ready && std::mem::take(&mut host.adapter().log.lock().unwrap().vsync) {
                host.vsync();
                vsynced = true;
                ran = true;
            }
            if !ran {
                break;
            }
        }
    }
}

#[test]
fn a_frame_asked_for_while_a_flight_lands_starts_after_it() {
    // A landing flight's Logic update asks for a frame before the landing frame is stored;
    // a flight started then would queue a third frame behind the two already waiting.
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = costly_spinner(&platform, &worker, true);
    assert_eq!(host.schedule_while_landing(), (false, true));
    run_timed(&platform, &host, 20);
    assert_eq!(host.ahead_refusals(), 0);
}

fn costly_spinner(platform: &Platform, worker: &Arc<ViewWorker>, ahead: bool) -> ViewHost<Fake> {
    // The button darkens while pressed, as the lab's buttons do: pressing relays the page out.
    let page = source(&format!(
        "function render(){{const n=useLocal('n',0);return h('view',{{}},h('button',{{onClick:()=>n.set(n.value+1),className:'active:x',style:{{height:60}}}},`Count ${{n.value}}`),h('view',{{style:{{width:20,height:20,backgroundColor:'#ff0000',animation:{SPIN}}}}}))}}"
    ))
    .replace(
        "__lingxiaPublish(render);",
        "configureClasses([{name:'active:x',when:{state:'active'},style:{opacity:0.8}}]);__lingxiaPublish(render);",
    );
    let host = platform.host(worker, &page, false);
    if !ahead {
        host.disable_frames_ahead();
    }
    host.set_visible(true);
    platform.settle(&[&host]);
    *host.adapter().costs.lock().unwrap() = Some(Costs {
        worker: Duration::from_millis(11),
        ui: Duration::from_millis(7),
        worker_free: *host.adapter().clock.lock().unwrap(),
        ..Default::default()
    });
    host.model_build_time(Duration::from_millis(11));
    host
}

#[test]
fn frames_prepared_ahead_keep_display_rate_when_one_frame_costs_more_than_an_interval() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    // 11 ms on the worker after 7 ms of UI-thread presenting misses every other vsync.
    let serial = costly_spinner(&platform, &worker, false);
    run_timed(&platform, &serial, 30);
    let start = presents(&serial);
    run_timed(&platform, &serial, 60);
    let rate = presents(&serial) - start;
    assert!((28..=32).contains(&rate), "one frame at a time: {rate}/s");
    serial.set_visible(false);
    platform.settle(&[&serial]);

    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let ahead = costly_spinner(&platform, &worker, true);
    run_timed(&platform, &ahead, 30);
    let start = presents(&ahead);
    run_timed(&platform, &ahead, 60);
    let rate = presents(&ahead) - start;
    assert!(rate >= 58, "prepared ahead: {rate}/s");
    // Taps still dispatch and rebuild within a frame of the frame already on its way.
    for round in 1..=3 {
        tap(&ahead, 20.0, 20.0);
        run_timed(&platform, &ahead, 2);
        assert_eq!(last(&ahead), [format!("Count {round}")], "tap {round}");
        run_timed(&platform, &ahead, 5);
    }
    // Hidden: nothing prepares ahead or presents.
    ahead.set_visible(false);
    platform.settle(&[&ahead]);
    let hidden = presents(&ahead);
    run_timed(&platform, &ahead, 30);
    assert_eq!(presents(&ahead), hidden);
}

#[test]
fn a_refused_frame_drops_the_frame_prepared_on_top_of_it() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = costly_spinner(&platform, &worker, true);
    run_timed(&platform, &host, 20);
    let presented = presents(&host);
    let prepared = *host
        .adapter()
        .log
        .lock()
        .unwrap()
        .generations
        .last()
        .unwrap();
    host.adapter()
        .costs
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .decline = 1;
    let refused = host.snapshot().unwrap().generation;
    // The first frame shown after the refusal was prepared after it, not on top of it.
    let mut frames = 0;
    while host.snapshot().unwrap().generation == refused {
        run_timed(&platform, &host, 1);
        frames += 1;
        assert!(frames < 10, "presentation did not recover");
    }
    let shown = host.snapshot().unwrap().generation;
    assert!(shown.get() > prepared.get(), "{prepared:?} {shown:?}");
    run_timed(&platform, &host, 30);
    assert!(presents(&host) >= presented + 30, "the pipeline resumes");
    let generations: Vec<u64> = host
        .adapter()
        .log
        .lock()
        .unwrap()
        .generations
        .iter()
        .map(|g| g.get())
        .collect();
    assert!(generations.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn a_single_target_presenter_is_named_in_the_log_and_falls_back_to_one_frame_at_a_time() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = costly_spinner(&platform, &worker, true);
    host.adapter()
        .costs
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .single_target = true;
    run_timed(&platform, &host, 30);
    let start = presents(&host);
    run_timed(&platform, &host, 60);
    let rate = presents(&host) - start;
    let warnings: Vec<String> = host
        .adapter()
        .log
        .lock()
        .unwrap()
        .warnings
        .iter()
        .filter(|w| w.starts_with("UI_FRAME: presenterSingleTarget"))
        .cloned()
        .collect();
    assert_eq!(warnings.len(), 1, "warned once: {warnings:?}");
    assert!(warnings[0].contains("Presenter::prepare must keep each prepared frame presentable"));
    let json: serde_json::Value = serde_json::from_str(host.snapshot().unwrap().json()).unwrap();
    assert_eq!(json["metrics"]["presenterSingleTarget"], true);
    assert!(
        rate >= 26,
        "one frame at a time instead of a collapse: {rate}/s"
    );
}

#[test]
fn every_deferred_frame_the_adapter_shows_is_reported_presented_once() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(&format!(
            "function render(){{return h('view',{{style:{{width:20,height:20,backgroundColor:'#ff0000',animation:{SPIN}}}}})}}"
        )),
        false,
    );
    host.adapter()
        .defer
        .store(true, std::sync::atomic::Ordering::Release);
    host.set_visible(true);
    platform.settle(&[&host]);
    let (mut acked, mut earlier) = (0, 0);
    for _ in 0..40 {
        // Rastering takes longer than a display frame: a frame handed over shows one
        // display frame later, after the next vsync.
        let handed = host.adapter().log.lock().unwrap().handed.clone();
        for generation in &handed[acked..earlier] {
            host.deferred_presented(*generation, true);
        }
        acked = earlier;
        earlier = handed.len();
        run_frames(&platform, &host, 1);
    }
    let log = host.adapter().log.lock().unwrap();
    assert!(acked >= 15, "{:?}", log.handed);
    assert_eq!(
        log.shown[..acked],
        log.handed[..acked],
        "each frame the raster thread showed is presented once, by its own generation"
    );
}

#[test]
fn every_message_adapters_receive_matches_the_frame_schema() {
    use crate::protocol::tests::frame_contract::conform;
    let worker = ViewWorker::spawn("test-frame-contract").unwrap();
    let platform = Platform::new(&[&worker]);
    let page = "function render(){
      const name=useLocal('name','Ada'), on=useLocal('on',true), level=useLocal('level',3), plan=useLocal('plan','b');
      return h('scroll',{'data-testid':'page',style:{height:400}},
        h('text',{'data-testid':'title',role:'heading',headingLevel:1,style:{userSelect:'text'}},'Frame contract'),
        h('input',{'data-testid':'name',bind:name,'aria-label':'Name',autoFocus:true,type:'email',enterKeyHint:'next',maxLength:40,placeholder:'Name'}),
        h('input',{'data-testid':'day','aria-label':'Day',type:'date',min:'2024-01-01'}),
        h('textarea',{'data-testid':'note','aria-label':'Note',rows:2,maxRows:4,value:'line'}),
        h('select',{'data-testid':'plan',bind:plan,'aria-label':'Plan',options:[{value:'a',label:'A'},{value:'b',label:'B',disabled:true}]}),
        h('slider',{'data-testid':'level',bind:level,'aria-label':'Level',min:0,max:10,step:1}),
        h('checkbox',{'data-testid':'on',bind:on},'On'),
        h('plot',{'data-testid':'chart',kind:'pie',values:[1,2,3],'aria-label':'Share',onSelect(){},style:{height:80}}),
        h('vector',{'data-testid':'icon','aria-hidden':true,paths:[{d:'M0 0H10V10Z'}],style:{width:10,height:10}}),
        h('image',{'data-testid':'photo',src:'a.png',alt:'Photo',style:{width:20,height:20,borderRadius:4}}),
        h('text',{},'Read the ',h('link',{onClick(){}},'terms')),
        h('button',{'data-testid':'go',onClick(){}},h('view',{style:{flexDirection:'row'}},h('text',{},'Go'),h('checkbox',{'data-testid':'remember',bind:on,'aria-label':'Remember'}))),
        h('scroll',{'data-testid':'inner',style:{height:20,overflowX:'auto'}},h('view',{style:{flexDirection:'row',width:900,height:20}})))
    }";
    let host = platform.host(&worker, &source(page), false);
    host.set_visible(true);
    platform.settle(&[&host]);
    let snapshot: serde_json::Value =
        serde_json::from_str(host.snapshot().unwrap().json()).unwrap();
    assert_eq!(conform("snapshot", &snapshot), Vec::<String>::new());
    let nodes: Vec<serde_json::Value> = host.semantics(|projection, _| {
        let projection = projection.unwrap();
        (0..projection.frame.nodes.len())
            .filter_map(|index| projection.ids.id(index))
            .filter_map(|id| projection.node(id))
            .map(|node| serde_json::to_value(node).unwrap())
            .collect()
    });
    assert!(nodes.len() > 8);
    for node in &nodes {
        assert_eq!(
            conform("semanticNode", node),
            Vec::<String>::new(),
            "{node}"
        );
    }
    let requests = [
        serde_json::json!({"op":"query","css":"[data-testid=\"name\"]","maxText":64}),
        serde_json::json!({"op":"query","css":"[data-testid=\"nothing\"]"}),
        serde_json::json!({"op":"query","css":"input","all":true}),
        serde_json::json!({"op":"tree"}),
        serde_json::json!({"op":"frame"}),
        serde_json::json!({"op":"frame","windowMs":1000}),
        serde_json::json!({"op":"act","css":"[data-testid=\"name\"]","action":{"kind":"fill","text":"Bo"}}),
        serde_json::json!({"op":"hud","on":false}),
    ];
    let replies = [
        "automationElement",
        "automationElement",
        "automationQueryAll",
        "automationTree",
        "automationFrame",
        "automationFrame",
    ];
    let mut exchanges = Vec::new();
    for (index, request) in requests.iter().enumerate() {
        assert_eq!(
            conform("automationRequest", request),
            Vec::<String>::new(),
            "{request}"
        );
        let reply = host.automation_request(request).unwrap();
        if let Some(message) = replies.get(index) {
            assert_eq!(
                conform(message, &reply),
                Vec::<String>::new(),
                "{request} → {reply}"
            );
            exchanges
                .push(serde_json::json!({"request": request, "reply": message, "value": reply}));
        } else {
            exchanges.push(serde_json::json!({ "request": request }));
        }
    }
    // A data read answers through `result` once the worker has it.
    let read = serde_json::json!({"op":"data"});
    let call = host.automation_request(&read).unwrap();
    platform.settle(&[&host]);
    let poll = serde_json::json!({"op":"result","call":call["call"]});
    let done = host.automation_request(&poll).unwrap();
    for (request, message, value) in [
        (read, "automationCall", call),
        (poll, "automationResult", done),
    ] {
        assert_eq!(conform("automationRequest", &request), Vec::<String>::new());
        assert_eq!(conform(message, &value), Vec::<String>::new(), "{value}");
        exchanges.push(serde_json::json!({"request": request, "reply": message, "value": value}));
    }
    assert!(
        !conform(
            "automationRequest",
            &serde_json::json!({"op":"act","css":"x","action":{"kind":"hover"}})
        )
        .is_empty()
    );
    // Input-protocol replies, the copy reply, semantic roots and the text menu, as adapters get them.
    platform.settle(&[&host]);
    let current: serde_json::Value = serde_json::from_str(host.snapshot().unwrap().json()).unwrap();
    let node = |test_id: &str| {
        current["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["test_id"] == test_id)
            .unwrap()
            .clone()
    };
    let (name, note) = (node("name"), node("note"));
    let (identity, generation) = (name["identity"].clone(), current["generation"].clone());
    let ask = |request: serde_json::Value| -> serde_json::Value {
        let (tx, rx) = channel();
        host.input(request.to_string(), move |reply| {
            let _ = tx.send(reply);
        });
        platform.settle(&[&host]);
        serde_json::from_str(&rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap()).unwrap()
    };
    let focused =
        ask(serde_json::json!({"op": "focus", "identity": identity, "generation": generation}));
    let token = focused["token"].clone();
    // Hit tests answer for an editor that is not the focused one.
    let bounds = &note["bounds"];
    let center = [
        bounds[0].as_f64().unwrap() + bounds[2].as_f64().unwrap() / 2.0,
        bounds[1].as_f64().unwrap() + bounds[3].as_f64().unwrap() / 2.0,
    ];
    let mut input = vec![focused];
    input.push(ask(
        serde_json::json!({"op": "query", "identity": identity, "token": token}),
    ));
    input.push(ask(
        serde_json::json!({"op": "edit", "identity": identity, "token": token, "submit": false,
        "edits": [{"op": "replace", "text": "Ada", "cursor": 1, "composing": false}]}),
    ));
    // The adapter renders when the edit reply asks for it.
    host.request_frame();
    platform.settle(&[&host]);
    let generation = serde_json::from_str::<serde_json::Value>(host.snapshot().unwrap().json())
        .unwrap()["generation"]
        .clone();
    input.push(ask(
        serde_json::json!({"op": "hit_test", "identity": note["identity"], "generation": generation,
        "point": center, "word": true}),
    ));
    for reply in &input {
        assert_eq!(
            conform("inputReply", reply),
            Vec::<String>::new(),
            "{reply}"
        );
    }
    let (tx, rx) = channel();
    host.copy_text_selection(move |selected| {
        let _ = tx.send(serde_json::json!({ "text": selected }));
    });
    platform.settle(&[&host]);
    let copied = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let roots = host.semantics(|projection, focus| {
        let projection = projection.unwrap();
        serde_json::json!({"roots": projection.roots(), "inputFocus": projection.input_focus(), "accessibilityFocus": focus})
    });
    let menu = serde_json::to_value(crate::session::host::TextMenu {
        target: crate::session::host::MenuTarget::Editor {
            identity: identity.as_str().unwrap().into(),
        },
        anchor: [10.0, 20.0, 80.0, 24.0],
        actions: vec![
            crate::session::host::MenuAction::SelectAll,
            crate::session::host::MenuAction::Paste,
        ],
    })
    .unwrap();
    for (message, value) in [
        ("copySelection", &copied),
        ("semanticRoots", &roots),
        ("textMenu", &menu),
    ] {
        assert_eq!(conform(message, value), Vec::<String>::new(), "{message}");
    }
    // Every input request form and edit op, as the platforms' generated builders must produce them.
    let requests = vec![
        serde_json::json!({"op": "focus", "identity": identity, "generation": generation}),
        serde_json::json!({"op": "query", "identity": identity, "token": token}),
        serde_json::json!({"op": "hit_test", "identity": note["identity"], "generation": generation, "point": [12.5, 20.0], "word": true}),
        serde_json::json!({"op": "drag_hit_test", "identity": identity, "generation": generation, "token": token, "point": [3.0, 4.5], "edge": [-1, 1]}),
        serde_json::json!({"op": "blur", "identity": identity, "token": token}),
        serde_json::json!({"op": "edit", "identity": identity, "token": token, "submit": true, "edits": [
            {"op": "replace", "text": "Ada", "cursor": -1, "composing": true},
            {"op": "selection", "anchor": 0, "focus": 3},
            {"op": "composition", "start": 1, "end": 2},
            {"op": "finish_composition"},
            {"op": "delete_surrounding", "before": 1, "after": 0, "code_points": true},
            {"op": "undo"},
            {"op": "redo"},
        ]}),
    ];
    for request in &requests {
        assert_eq!(
            conform("inputRequest", request),
            Vec::<String>::new(),
            "{request}"
        );
        assert!(
            crate::session::input_protocol::parses(&request.to_string()),
            "dispatch parses {request}"
        );
    }
    // Golden vectors the platform decoders replay; re-record with LINGXIA_UPDATE_VECTORS=1.
    let recorded = serde_json::json!({
        "inputRequests": requests,
        "snapshot": scrub(snapshot),
        "semanticNodes": nodes,
        "automation": exchanges.into_iter().map(scrub).collect::<Vec<_>>(),
        "inputReplies": input,
        "copySelection": copied,
        "semanticRoots": roots,
        "textMenu": menu,
    });
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/view/vectors/frame.json"
    );
    if std::env::var_os("LINGXIA_UPDATE_VECTORS").is_some() {
        std::fs::create_dir_all(std::path::Path::new(path).parent().unwrap()).unwrap();
        std::fs::write(
            path,
            serde_json::to_string_pretty(&recorded).unwrap() + "\n",
        )
        .unwrap();
    }
    let vectors: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(path)
            .expect("golden vectors; record with LINGXIA_UPDATE_VECTORS=1"),
    )
    .unwrap();
    assert_eq!(
        conform("snapshot", &vectors["snapshot"]),
        Vec::<String>::new()
    );
    for node in vectors["semanticNodes"].as_array().unwrap() {
        assert_eq!(conform("semanticNode", node), Vec::<String>::new());
    }
    for exchange in vectors["automation"].as_array().unwrap() {
        assert_eq!(
            conform("automationRequest", &exchange["request"]),
            Vec::<String>::new()
        );
        let Some(reply) = exchange["reply"].as_str() else {
            continue;
        };
        assert_eq!(
            conform(reply, &exchange["value"]),
            Vec::<String>::new(),
            "{reply}"
        );
    }
    for reply in vectors["inputReplies"].as_array().unwrap() {
        assert_eq!(conform("inputReply", reply), Vec::<String>::new());
    }
    for request in vectors["inputRequests"].as_array().unwrap() {
        assert_eq!(conform("inputRequest", request), Vec::<String>::new());
        assert!(crate::session::input_protocol::parses(&request.to_string()));
    }
    for message in ["copySelection", "semanticRoots", "textMenu"] {
        assert_eq!(
            conform(message, &vectors[message]),
            Vec::<String>::new(),
            "{message}"
        );
    }
}

/// Zeroes timings so recorded vectors are stable across runs.
fn scrub(mut value: serde_json::Value) -> serde_json::Value {
    fn walk(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, item) in map.iter_mut() {
                    if !(key.ends_with("Us") || key.ends_with("Ms")) {
                        walk(item);
                    } else if item.is_number() {
                        *item = serde_json::json!(0);
                    } else if let Some(stats) = item.as_object_mut() {
                        stats.values_mut().for_each(|v| *v = serde_json::json!(0));
                    }
                }
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(walk),
            _ => {}
        }
    }
    walk(&mut value);
    value
}

#[test]
fn an_lxapp_pack_paints_the_first_presented_frame() {
    let key = "test-app-pack";
    let pack = serde_json::from_value(serde_json::json!(
        {"name": "brand", "version": 1, "accent": {"seed": "#0f9d58", "derive": true}}))
    .unwrap();
    lingxia_theme::set_app_pack(key, Some(pack));
    let worker = ViewWorker::shared(key).unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render(){ return h('text',{},'Hi') }"),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let argb = |[r, g, b, a]: [u8; 4]| u32::from_be_bytes([a, r, g, b]);
    let builtin = argb(crate::theme::Palette::builtin(false).role(lingxia_theme::Role::Accent));
    let first = host
        .adapter()
        .log
        .lock()
        .unwrap()
        .feedback
        .first()
        .cloned()
        .expect("a presented frame");
    let handle = first["selectionHandle"].as_u64().unwrap() as u32;
    assert_ne!(
        handle, builtin,
        "the first frame already paints the app's accent"
    );
    drop(host);
    drop(platform);
    drop(worker);
    lingxia_theme::set_app_pack(key, None);
}

#[test]
fn a_page_on_a_fresh_worker_reads_the_pack_its_outgoing_worker_left() {
    let key = "test-app-pack-relaunch";
    let pack = serde_json::from_value(serde_json::json!(
        {"name": "brand", "version": 1, "accent": {"seed": "#0f9d58", "derive": true}}))
    .unwrap();
    lingxia_theme::set_app_pack(key, Some(pack));
    let argb = |[r, g, b, a]: [u8; 4]| u32::from_be_bytes([a, r, g, b]);
    let builtin = argb(crate::theme::Palette::builtin(false).role(lingxia_theme::Role::Accent));
    // A relaunch: the old stack and its worker go before the new page's host starts a worker.
    drop(ViewWorker::shared(key).unwrap());
    let worker = ViewWorker::shared(key).unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render(){ return h('text',{},'B') }"),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let first = host
        .adapter()
        .log
        .lock()
        .unwrap()
        .feedback
        .first()
        .cloned()
        .expect("a presented frame");
    assert_ne!(
        first["selectionHandle"].as_u64().unwrap() as u32,
        builtin,
        "the page paints the app's accent"
    );
    drop(host);
    drop(platform);
    drop(worker);
    lingxia_theme::set_app_pack(key, None);
}

#[test]
fn a_surface_push_rebuilds_only_pages_whose_view_read_the_host_context() {
    let worker = ViewWorker::spawn("test-host-context").unwrap();
    let platform = Platform::new(&[&worker]);
    let reader = platform.host(
        &worker,
        &source("function render(view){ return h('text',{},String(view.env.sizeClass)) }"),
        false,
    );
    let other = platform.host(
        &worker,
        &source("function render(){ return h('text',{},'Static') }"),
        false,
    );
    for host in [&reader, &other] {
        host.set_visible(true);
    }
    platform.settle(&[&reader, &other]);
    let presented = |host: &ViewHost<Fake>| host.adapter().log.lock().unwrap().presented.len();
    let before = (presented(&reader), presented(&other));
    let surface = serde_json::json!({"kind": "surface", "revision": 1,
        "context": {"sizeClass": "regular", "canDock": true, "width": 900.0, "height": 700.0}});
    for host in [&reader, &other] {
        host.update_environment(&surface.to_string()).unwrap();
    }
    platform.settle(&[&reader, &other]);
    assert!(
        presented(&reader) > before.0,
        "the page that read env.sizeClass rebuilds"
    );
    assert_eq!(
        presented(&other),
        before.1,
        "the page that never read it stays as it was"
    );
    assert!(
        other.frame_is_current(),
        "its kept frame still counts as current"
    );
    assert_eq!(
        reader
            .adapter()
            .log
            .lock()
            .unwrap()
            .presented
            .last()
            .unwrap(),
        &vec!["regular".to_string()]
    );
}

/// Like `run_timed`, but presses after `phase` main-thread steps of the next frame and
/// releases `hold` frames later, as a finger does.
fn run_timed_tapping(platform: &Platform, host: &ViewHost<Fake>, phase: usize, hold: u32) {
    let frame = Duration::from_micros(16_667);
    let now = {
        let mut clock = host.adapter().clock.lock().unwrap();
        *clock += frame;
        *clock
    };
    platform.advance(frame);
    let (mut vsynced, mut steps, mut tapped) = (false, 0, false);
    loop {
        if !tapped && steps >= phase {
            pointer(host, PointerPhase::Down);
            if hold == 0 {
                pointer(host, PointerPhase::Up);
            }
            tapped = true;
        }
        for worker in &platform.workers {
            let (done, wait) = channel();
            worker.run_background(move |_| {
                let _ = done.send(());
            });
            wait.recv_timeout(Duration::from_secs(30)).unwrap();
        }
        let mut ran = false;
        if let Ok(task) = platform.tasks.try_recv() {
            task();
            ran = true;
            steps += 1;
        }
        let ready = host
            .adapter()
            .costs
            .lock()
            .unwrap()
            .as_ref()
            .is_none_or(|costs| costs.ready.front().is_some_and(|at| *at <= now));
        if !vsynced && ready && std::mem::take(&mut host.adapter().log.lock().unwrap().vsync) {
            host.vsync();
            vsynced = true;
            ran = true;
            steps += 1;
        }
        if !ran {
            break;
        }
    }
    if !tapped {
        pointer(host, PointerPhase::Down);
        if hold == 0 {
            pointer(host, PointerPhase::Up);
        }
    }
    if hold > 0 {
        run_timed(platform, host, hold);
        pointer(host, PointerPhase::Up);
    }
}

fn pointer(host: &ViewHost<Fake>, phase: PointerPhase) {
    host.pointer(Pointer {
        phase,
        x: 20.0,
        y: 20.0,
        slop: 8.0,
        velocity: [0.0; 2],
        min_fling: 50.0,
    });
}

#[test]
fn taps_at_any_phase_of_a_pipelined_animation_dispatch_within_two_frames() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = costly_spinner(&platform, &worker, true);
    run_timed(&platform, &host, 20);
    let count = |host: &ViewHost<Fake>| -> u32 {
        last(host)[0].trim_start_matches("Count ").parse().unwrap()
    };
    let mut failed = Vec::new();
    for round in 1..=60usize {
        let before = count(&host);
        let (phase, hold) = (round % 7, (round % 4) as u32);
        run_timed_tapping(&platform, &host, phase, hold);
        run_timed(&platform, &host, 2);
        let after = count(&host);
        run_timed(&platform, &host, 3 + (round % 5) as u32);
        if after != before + 1 {
            failed.push((phase, hold, after - before, count(&host) - before));
        }
    }
    assert!(
        failed.is_empty(),
        "taps not counted within two frames of release (phase, hold, counted, eventually): {failed:?}"
    );
}

fn dropped_warnings(host: &ViewHost<Fake>) -> Vec<String> {
    host.adapter()
        .log
        .lock()
        .unwrap()
        .warnings
        .iter()
        .filter(|w| w.starts_with("UI_INPUT: dropped activation"))
        .cloned()
        .collect()
}

#[test]
fn an_activation_whose_control_went_away_before_dispatch_is_reported_not_silent() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){const n=useLocal('n',0);return h('view',{},n.value===0?h('button',{'data-testid':'b',onClick:()=>n.set(1),style:{height:60}},'Remove me'):h('text',{},'gone'))}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    tap(&host, 20.0, 20.0);
    // Its rebuild is in flight; the second tap is made on the button that rebuild removes.
    let worker_done = {
        let (done, wait) = channel();
        worker.run_background(move |_| {
            let _ = done.send(());
        });
        wait
    };
    worker_done.recv_timeout(Duration::from_secs(30)).unwrap();
    // The click's result reaches the main thread and schedules its rebuild.
    while let Ok(task) = platform.tasks.try_recv() {
        task();
    }
    tap(&host, 20.0, 20.0);
    platform.settle(&[&host]);
    let warnings = dropped_warnings(&host);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(
        warnings[0].contains("reason=the control is gone") && warnings[0].contains("node=\""),
        "{}",
        warnings[0]
    );
    host.request_frame();
    platform.settle(&[&host]);
    let json: serde_json::Value = serde_json::from_str(host.snapshot().unwrap().json()).unwrap();
    assert_eq!(json["metrics"]["droppedActivations"], 1);
}

#[test]
fn quick_taps_on_a_changing_toggle_during_an_animation_are_never_dropped() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = costly_spinner(&platform, &worker, true);
    run_timed(&platform, &host, 20);
    // A tap every 50 ms, while the previous tap's rebuild is still on its way (as on a
    // phone, where a tap's label flips 58–90 ms later).
    for round in 0..100usize {
        run_timed_tapping(&platform, &host, round % 5, 0);
        if round % 3 != 2 {
            run_timed(&platform, &host, 1);
        } else {
            run_timed(&platform, &host, 2);
        }
    }
    run_timed(&platform, &host, 10);
    assert!(
        dropped_warnings(&host).is_empty(),
        "{:?}",
        dropped_warnings(&host)
    );
    assert_eq!(last(&host), ["Count 100"]);
    let json: serde_json::Value = serde_json::from_str(host.snapshot().unwrap().json()).unwrap();
    assert_eq!(json["metrics"]["droppedActivations"], 0);
}

#[test]
fn every_tap_changes_state_exactly_once_while_the_presenter_declines_frames() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = costly_spinner(&platform, &worker, true);
    // Two offscreen targets, one held by the frame on screen; and the first frame of each
    // new View (each tap's rebuild) is declined once.
    {
        let mut costs = host.adapter().costs.lock().unwrap();
        let costs = costs.as_mut().unwrap();
        costs.targets = Some(2);
        costs.decline_new_views = true;
    }
    run_timed(&platform, &host, 20);
    for round in 0..100usize {
        run_timed_tapping(&platform, &host, round % 7, (round % 3) as u32);
        run_timed(&platform, &host, 1 + (round % 3) as u32);
    }
    run_timed(&platform, &host, 10);
    let declined = host
        .adapter()
        .costs
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .declined;
    assert!(declined >= 10, "the presenter declined frames: {declined}");
    assert_eq!(last(&host), ["Count 100"], "one state change per tap");
    assert!(
        dropped_warnings(&host).is_empty(),
        "{:?}",
        dropped_warnings(&host)
    );
    let json: serde_json::Value = serde_json::from_str(host.snapshot().unwrap().json()).unwrap();
    assert_eq!(json["metrics"]["activations"], 100);
    assert_eq!(json["metrics"]["droppedActivations"], 0);
    let clicks = host
        .adapter()
        .log
        .lock()
        .unwrap()
        .debug
        .iter()
        .filter(|line| line.starts_with("UI_INPUT: click page="))
        .count();
    assert_eq!(clicks, 100);
}

#[cfg(feature = "hot-swap")]
#[test]
fn a_hosted_showcase_page_keeps_a_nested_local_across_a_sibling_edit() {
    use crate::session::tests::hot_swap::{
        SIBLING_EDIT, showcase_page_module, showcase_page_module_edited,
    };
    let worker = ViewWorker::spawn("test-lab-swap").unwrap();
    let platform = Platform::new(&[&worker]);
    let module = showcase_page_module("view-controls");
    let host = platform.host(&worker, &module, false);
    // A phone-sized page, so the plan picker is on screen.
    if let Some(environment) = host.adapter().environment.lock().unwrap().as_mut() {
        (environment.width, environment.height, environment.scale) = (393 * 2, 852 * 2, 2.0);
    }
    host.set_visible(true);
    platform.settle(&[&host]);
    let act = |css: &str, action: serde_json::Value| {
        host.automation_request(&serde_json::json!({"op": "act", "css": css, "action": action}))
            .unwrap();
        platform.settle(&[&host]);
    };
    let values = || {
        host.automation_request(&serde_json::json!({"op": "query", "css": "[data-testid=\"lab-select-values\"]", "maxText": 400}))
            .unwrap()["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    };
    act(
        "[data-testid=\"lab-input-plan\"]",
        serde_json::json!({"kind": "select", "value": "yearly", "force": true}),
    );
    assert!(values().contains("Plan: yearly"), "{}", values());
    let edited = showcase_page_module_edited("view-controls", Some(SIBLING_EDIT));
    host.swap_module(edited);
    platform.settle(&[&host]);
    assert!(
        values().contains("Plan: yearly"),
        "after swap: {}",
        values()
    );
}

#[test]
fn an_editor_selection_offers_copy_cut_and_paste_to_assistive_tech() {
    use crate::interaction::view::AccessibilityAction;
    use crate::semantics::projection::Action;
    let worker = ViewWorker::spawn("test-editor-clipboard").unwrap();
    let platform = Platform::new(&[&worker]);
    let page = "function render(){
      const name=useLocal('name','Hello world'), pin=useLocal('pin','1234');
      return h('view',{},
        h('input',{'data-testid':'name',bind:name,'aria-label':'Name'}),
        h('input',{'data-testid':'pin',bind:pin,type:'password','aria-label':'PIN'}))
    }";
    let host = platform.host(&worker, &source(page), false);
    host.set_visible(true);
    platform.settle(&[&host]);
    let ask = |request: serde_json::Value| -> serde_json::Value {
        let (tx, rx) = channel();
        host.input(request.to_string(), move |reply| {
            let _ = tx.send(reply);
        });
        platform.settle(&[&host]);
        serde_json::from_str(&rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap()).unwrap()
    };
    let select = |test_id: &str, anchor: u32, focus: u32| -> u32 {
        let snapshot: serde_json::Value =
            serde_json::from_str(host.snapshot().unwrap().json()).unwrap();
        let node = snapshot["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["test_id"] == test_id)
            .unwrap()
            .clone();
        let focused = ask(
            serde_json::json!({"op": "focus", "identity": node["identity"], "generation": snapshot["generation"]}),
        );
        ask(
            serde_json::json!({"op": "edit", "identity": node["identity"], "token": focused["token"], "submit": false,
            "edits": [{"op": "selection", "anchor": anchor, "focus": focus}]}),
        );
        host.request_frame();
        platform.settle(&[&host]);
        host.semantics(|projection, _| {
            let projection = projection.unwrap();
            (0..projection.frame.nodes.len())
                .filter_map(|index| projection.ids.id(index))
                .find(|id| projection.node(*id).is_some_and(|n| n.test_id == test_id))
                .unwrap()
        })
    };
    let actions =
        |id: u32| host.semantics(|projection, _| projection.unwrap().node(id).unwrap().actions);
    let name = select("name", 0, 5);
    assert!(
        [Action::Copy, Action::Cut, Action::Paste]
            .iter()
            .all(|a| actions(name).contains(a)),
        "{:?}",
        actions(name)
    );
    assert!(host.accessibility(name, AccessibilityAction::Copy));
    assert_eq!(host.adapter().log.lock().unwrap().copied, ["Hello"]);
    assert!(host.accessibility(name, AccessibilityAction::Paste("Hi".into())));
    platform.settle(&[&host]);
    let replaced = |log: &Log| {
        log.edits.iter().rev().find_map(|(effect, _)| match effect {
            Effect::EditText {
                text: Some(text), ..
            } => Some(text.clone()),
            _ => None,
        })
    };
    assert_eq!(
        replaced(&host.adapter().log.lock().unwrap()).as_deref(),
        Some("Hi world")
    );
    // A secure editor never offers its text, not even a selected part of it.
    let pin = select("pin", 0, 2);
    let offered = actions(pin);
    assert!(
        !offered.contains(&Action::Copy)
            && !offered.contains(&Action::Cut)
            && !offered.contains(&Action::Paste),
        "{offered:?}"
    );
    assert!(!host.accessibility(pin, AccessibilityAction::Copy));
    assert_eq!(host.adapter().log.lock().unwrap().copied, ["Hello"]);
}

#[test]
fn a_sibling_after_an_appearing_conditional_keeps_its_local_and_semantic_id() {
    let worker = ViewWorker::spawn("test-conditional-slots").unwrap();
    let platform = Platform::new(&[&worker]);
    let page = "function Counter(){const n=useLocal('n',0);return h('button',{'data-testid':'count',onClick(){n.set(n.value+1)}},'n '+n.value)}
      function render(){const on=useLocal('on',false);
        return h('view',{}, on.value && h('text',{'data-testid':'banner'},'Hi'), h(Counter,{}),
          h('button',{'data-testid':'toggle',onClick(){on.set(!on.value)}},'Toggle'))}";
    let host = platform.host(&worker, &source(page), false);
    host.set_visible(true);
    platform.settle(&[&host]);
    let click = |test_id: &str| {
        host.automation_request(&serde_json::json!({"op": "act", "css": format!("[data-testid=\"{test_id}\"]"), "action": {"kind": "click"}}))
            .unwrap();
        platform.settle(&[&host]);
    };
    let count = || {
        host.semantics(|projection, _| {
            let projection = projection.unwrap();
            (0..projection.frame.nodes.len())
                .filter_map(|index| projection.ids.id(index))
                .filter_map(|id| projection.node(id))
                .find(|node| node.test_id == "count")
                .map(|node| (node.id, node.name))
                .unwrap()
        })
    };
    click("count");
    let before = count();
    assert_eq!(before.1, "n 1");
    click("toggle");
    let banner = host
        .automation_request(&serde_json::json!({"op": "query", "css": "[data-testid=\"banner\"]"}))
        .unwrap();
    assert_eq!(banner["exists"], true, "the conditional child appeared");
    assert_eq!(
        count(),
        before,
        "same semantic id and the same local after the banner appears"
    );
}

#[test]
fn a_pressable_row_holds_its_own_controls() {
    let worker = ViewWorker::spawn("test-nested-controls").unwrap();
    let platform = Platform::new(&[&worker]);
    let page = "function render(){
      const rows=useLocal('rows',0), on=useLocal('on',false), deletes=useLocal('deletes',0);
      return h('view',{},
        h('button',{'data-testid':'row',onClick(){rows.set(rows.value+1)},style:{height:60,width:190}},
          h('view',{style:{flexDirection:'row'}},h('text',{'data-testid':'label'},'Wi'),
            h('switch',{'data-testid':'wifi',bind:on,'aria-label':'Wi-Fi'}),
            h('button',{'data-testid':'delete',onClick(){deletes.set(deletes.value+1)}},'Del'))),
        h('text',{'data-testid':'state'},`rows=${rows.value} on=${on.value} deletes=${deletes.value}`))
    }";
    let host = platform.host(&worker, &source(page), false);
    if let Some(environment) = host.adapter().environment.lock().unwrap().as_mut() {
        (environment.width, environment.height) = (800, 800);
    }
    host.set_visible(true);
    platform.settle(&[&host]);
    let state = || {
        host.automation_request(
            &serde_json::json!({"op": "query", "css": "[data-testid=\"state\"]"}),
        )
        .unwrap()["text"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    // A real press at the center of a node, in device pixels.
    let press = |id: &str| {
        let snapshot: serde_json::Value =
            serde_json::from_str(host.snapshot().unwrap().json()).unwrap();
        let node = snapshot["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["test_id"] == id)
            .unwrap()
            .clone();
        let b = |i: usize| node["bounds"][i].as_f64().unwrap() as f32;
        tap(&host, (b(0) + b(2) / 2.0) * 2.0, (b(1) + b(3) / 2.0) * 2.0);
        // The switch eases between states; a frozen clock would never let it settle.
        run_frames(&platform, &host, 20);
    };
    // Only the innermost control shows the press.
    let snapshot: serde_json::Value =
        serde_json::from_str(host.snapshot().unwrap().json()).unwrap();
    let nodes = snapshot["nodes"].as_array().unwrap().clone();
    let index = |id: &str| nodes.iter().position(|n| n["test_id"] == id).unwrap();
    let b = |i: usize| nodes[index("wifi")]["bounds"][i].as_f64().unwrap() as f32;
    host.pointer(Pointer {
        phase: PointerPhase::Down,
        x: (b(0) + b(2) / 2.0) * 2.0,
        y: (b(1) + b(3) / 2.0) * 2.0,
        slop: 8.0,
        velocity: [0.0; 2],
        min_fling: 50.0,
    });
    platform.settle(&[&host]);
    let pressed: Vec<usize> = host
        .adapter()
        .log
        .lock()
        .unwrap()
        .effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::Press { node, .. } => Some(*node),
            _ => None,
        })
        .collect();
    // The switch paints its own press, so neither it nor the row gets an overlay.
    assert_eq!(
        pressed, [0usize; 0],
        "no overlay over the row or the switch box"
    );
    assert_eq!(nodes[index("wifi")]["pressed_style"], true);
    host.pointer(Pointer {
        phase: PointerPhase::Cancel,
        x: 0.0,
        y: 0.0,
        slop: 8.0,
        velocity: [0.0; 2],
        min_fling: 50.0,
    });
    run_frames(&platform, &host, 20);
    press("wifi");
    assert_eq!(
        state(),
        "rows=0 on=true deletes=0",
        "the switch takes its own press"
    );
    press("delete");
    assert_eq!(
        state(),
        "rows=0 on=true deletes=1",
        "the inner button takes its own press"
    );
    press("label");
    assert_eq!(
        state(),
        "rows=1 on=true deletes=1",
        "content outside the controls presses the row"
    );
    let nodes = host.semantics(|projection, _| {
        let projection = projection.unwrap();
        (0..projection.frame.nodes.len())
            .filter_map(|index| projection.ids.id(index))
            .filter_map(|id| projection.node(id))
            .map(|node| (node.test_id.clone(), node.role, node.name.clone()))
            .collect::<Vec<_>>()
    });
    let named = |id: &str| nodes.iter().find(|(test_id, ..)| test_id == id).cloned();
    assert_eq!(
        named("row").map(|n| (n.1, n.2)),
        Some((Role::Button, "Wi".to_string())),
        "the row keeps its own name"
    );
    assert_eq!(
        named("wifi").map(|n| n.1),
        Some(Role::Switch),
        "the switch is its own node"
    );
    assert_eq!(
        named("delete").map(|n| (n.1, n.2)),
        Some((Role::Button, "Del".to_string()))
    );
}

#[test]
fn host_chrome_takes_the_page_palette_before_its_first_frame_and_follows_the_look() {
    let worker = ViewWorker::spawn("test-chrome").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render(){return h('text',{},'page')}"),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let pushes = || {
        host.adapter()
            .log
            .lock()
            .unwrap()
            .chrome
            .iter()
            .map(|(_, c)| *c)
            .collect::<Vec<_>>()
    };
    let light = crate::theme::Palette::builtin_for(false, false);
    assert_eq!(pushes().len(), 1, "one push, not one per frame");
    assert_eq!(pushes()[0].canvas, light.role(lingxia_theme::Role::Canvas));
    assert_eq!(
        host.adapter().log.lock().unwrap().chrome[0].0,
        0,
        "before the first frame"
    );
    assert!(!host.adapter().log.lock().unwrap().presented.is_empty());
    host.redraw();
    platform.settle(&[&host]);
    assert_eq!(pushes().len(), 1, "an unchanged look pushes nothing");
    if let Some(environment) = host.adapter().environment.lock().unwrap().as_mut() {
        environment.value["theme"] = "dark".into();
    }
    host.redraw();
    platform.settle(&[&host]);
    let dark = crate::theme::Palette::builtin_for(true, false);
    assert_eq!(pushes().len(), 2);
    assert_eq!(pushes()[1].canvas, dark.role(lingxia_theme::Role::Canvas));
    assert!(pushes()[1].dark);
}

#[test]
fn declared_font_faces_load_by_lxapp_path_and_reshape_their_text() {
    let worker = ViewWorker::spawn("test-view-fonts").unwrap();
    let platform = Platform::new(&[&worker]);
    let page = source(
        "function render(){return h('view',{style:{alignItems:'flex-start'}},h('text',{'data-testid':'t',style:{fontFamily:'Wide, serif',fontSize:10}},'abc'))}",
    )
    .replace(
        "__lingxiaPublish(render);",
        "configureFonts([{family:'Wide',src:'public/fonts/wide.ttf'},{family:'Wide',src:'public/fonts/broken.ttf'}]);__lingxiaPublish(render);",
    );
    let host = platform.host(&worker, &page, false);
    host.set_visible(true);
    platform.settle(&[&host]);
    let width = |host: &ViewHost<Fake>| {
        host.snapshot()
            .unwrap()
            .frame
            .nodes
            .iter()
            .find(|n| n.test_id == "t")
            .unwrap()
            .bounds[2]
    };
    let fallback = width(&host);
    assert_ne!(fallback, 30.0);
    let fetched = host.adapter().log.lock().unwrap().fetched.clone();
    let sources = fetched.iter().map(|(_, s)| s.as_str()).collect::<Vec<_>>();
    assert_eq!(
        sources,
        ["public/fonts/wide.ttf", "public/fonts/broken.ttf"]
    );
    host.image_loaded(fetched[1].0, Ok(b"not a font".to_vec()));
    host.image_loaded(fetched[0].0, Ok(include_bytes!("wide.ttf").to_vec()));
    platform.settle(&[&host]);
    // Every glyph of the face advances one em.
    assert_eq!(width(&host), 30.0, "fallback was {fallback}");
    let warnings = host.adapter().log.lock().unwrap().warnings.clone();
    assert!(
        warnings
            .iter()
            .any(|w| w.starts_with("UI_FONT: public/fonts/broken.ttf")),
        "{warnings:?}"
    );
}

#[test]
fn an_off_screen_list_row_is_found_revealed_and_clicked_by_test_id() {
    let worker = ViewWorker::spawn("test-virtual-rows").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){const query=useLocal('query',''),picked=useLocal('picked','');const rows=Array.from({length:40},(_,i)=>({id:`row-${i}`,label:`Row ${i}`})).filter(r=>r.label.includes(query.value));return h('view',{},h('input',{type:'search','data-testid':'filter',bind:query,'aria-label':'Filter'}),h(List,{'data-testid':'list',style:{height:120},items:rows,getKey:r=>r.id,renderItem:r=>h('button',{'data-testid':r.id,onClick:()=>picked.set(r.label),style:{height:40}},r.label)}),h('text',{'data-testid':'picked'},`picked=${picked.value}`))}",
        ),
        false,
    );
    // The fake clock stands still between settles; reduced motion lands motion at once.
    host.adapter()
        .environment
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .value["reducedMotion"] = serde_json::json!(true);
    host.set_visible(true);
    platform.settle(&[&host]);
    let act = |css: &str, action: serde_json::Value| {
        let result =
            host.automation_request(&serde_json::json!({"op":"act","css":css,"action":action}));
        platform.settle(&[&host]);
        result
    };
    let query = |css: &str| {
        host.automation_request(&serde_json::json!({"op":"query","css":css}))
            .unwrap()
    };
    act(
        "[data-testid=\"filter\"]",
        serde_json::json!({"kind":"fill","text":"Row"}),
    )
    .unwrap();
    host.request_frame();
    platform.settle(&[&host]);
    let row = "[data-testid=\"row-31\"]";
    // The first miss probes the lists; the next query names the row without drawing it.
    assert_eq!(query(row)["exists"], false);
    platform.settle(&[&host]);
    let virtual_row = query(row);
    assert_eq!(
        (
            virtual_row["exists"].clone(),
            virtual_row["virtual"].clone()
        ),
        (serde_json::json!(true), serde_json::json!(true))
    );
    assert_eq!(query("[data-testid=\"row-99\"]")["exists"], false);
    // A locator's click: scroll into view, then act once the row draws.
    let mut clicked = false;
    for _ in 0..6 {
        let found = query(row);
        if found["inViewport"] == true && found["virtual"].is_null() {
            act(row, serde_json::json!({"kind":"click"})).unwrap();
            clicked = true;
            break;
        }
        let _ = act(row, serde_json::json!({"kind":"scroll"}));
        host.request_frame();
        platform.settle(&[&host]);
    }
    assert!(clicked, "row 31 never drew: {}", query(row));
    host.request_frame();
    platform.settle(&[&host]);
    assert_eq!(query("[data-testid=\"picked\"]")["text"], "picked=Row 31");
}

/// The act request/reply vectors the automation clients are written against.
#[test]
fn act_replies_match_the_automation_vectors() {
    let vectors: serde_json::Value =
        serde_json::from_str(include_str!("vectors/automation_act.json")).unwrap();
    let worker = ViewWorker::spawn("test-act-vectors").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render(){const n=useLocal('n',0);return h('button',{'data-testid':'add',onClick:()=>n.set(n.value+1)},`Count ${n.value}`)}"),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let act = &vectors["act"];
    assert_eq!(
        host.automation_request(&act["request"]).unwrap(),
        act["reply"]
    );
    platform.settle(&[&host]);
    let applied = &vectors["applied"];
    assert_eq!(
        host.automation_request(&applied["request"]).unwrap(),
        applied["reply"]
    );
    let refused = &vectors["refused"];
    assert_eq!(
        host.automation_request(&refused["request"]).unwrap_err(),
        refused["error"]
    );
    assert!(
        vectors["dropped"]["error"]
            .as_str()
            .unwrap()
            .starts_with("E_ACT_DROPPED: ")
    );
}

/// A click whose handler navigates away (the page parks) or brings a dialog over it (the page
/// hides) settles once its handler ran, though no further frame of the page commits.
#[test]
fn an_act_settles_when_its_page_leaves_or_hides_before_the_next_frame() {
    for leave in ["park", "hide"] {
        let worker = ViewWorker::spawn("test-act-leaves").unwrap();
        let platform = Platform::new(&[&worker]);
        let host = platform.host(
            &worker,
            &source(
                "function render(){const n=useLocal('n',0);return h('button',{'data-testid':'go',onClick:()=>n.set(n.value+1)},`Go ${n.value}`)}",
            ),
            false,
        );
        host.set_visible(true);
        platform.settle(&[&host]);
        let call = host
            .automation_request(&serde_json::json!({"op":"act","css":"[data-testid=\"go\"]","action":{"kind":"click"}}))
            .unwrap()["call"]
            .clone();
        // The handler runs on the worker; the navigation or dialog lands before its frame does.
        let (done, wait) = channel();
        worker.run_background(move |_| {
            let _ = done.send(());
        });
        wait.recv_timeout(Duration::from_secs(30)).unwrap();
        while let Ok(task) = platform.tasks.try_recv() {
            task();
        }
        if leave == "park" {
            host.park();
        } else {
            host.set_visible(false);
        }
        platform.settle(&[&host]);
        let result = host
            .automation_request(&serde_json::json!({"op":"result","call":call}))
            .unwrap();
        assert_eq!(result["done"], true, "{leave}: {result}");
    }
}

/// Turning on increased contrast re-resolves an lxapp pack's palette: the page presents again in
/// the pack's high-contrast look and the chrome follows it.
#[test]
fn a_pushed_pack_follows_increased_contrast_when_it_changes() {
    let worker = ViewWorker::spawn("test-contrast").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){return h('page',{},h('text',{style:{color:'text-muted'}},'Muted'))}",
        ),
        false,
    );
    host.update_environment(
        &serde_json::json!({"kind": "theme", "revision": 1,
            "pack": {"name": "brand", "version": 1, "accent": {"seed": "#0f9d58", "derive": true}}})
        .to_string(),
    )
    .unwrap();
    host.set_visible(true);
    platform.settle(&[&host]);
    let presented = |host: &ViewHost<Fake>| host.adapter().log.lock().unwrap().presented.len();
    let chrome =
        |host: &ViewHost<Fake>| host.adapter().log.lock().unwrap().chrome.last().unwrap().1;
    let (before, muted) = (presented(&host), chrome(&host).content_muted);
    host.adapter()
        .environment
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .value["highContrast"] = serde_json::json!(true);
    // What the Apple adapter does for a changed fact that is not a resize.
    host.request_frame();
    platform.settle(&[&host]);
    assert!(presented(&host) > before, "the page presents again");
    assert_ne!(
        chrome(&host).content_muted,
        muted,
        "in the high-contrast look"
    );
}

#[test]
fn a_list_row_below_the_fold_is_revealed_through_the_page_scroll_and_clicked() {
    let worker = ViewWorker::spawn("test-fold-rows").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){const query=useLocal('query',''),picked=useLocal('picked','');const rows=Array.from({length:40},(_,i)=>({id:`row-${i}`,label:`Row ${i}`})).filter(r=>r.label.includes(query.value));return h('scroll',{},h('input',{type:'search','data-testid':'filter',bind:query,'aria-label':'Filter'}),h('view',{style:{height:3000}}),h(List,{'data-testid':'list',style:{height:192},items:rows,getKey:r=>r.id,renderItem:r=>h('button',{'data-testid':r.id,onClick:()=>picked.set(r.label),style:{height:37}},r.label)}),h('text',{'data-testid':'picked'},`picked=${picked.value}`))}",
        ),
        false,
    );
    host.adapter()
        .environment
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .value["reducedMotion"] = serde_json::json!(true);
    host.set_visible(true);
    platform.settle(&[&host]);
    let act = |css: &str, action: serde_json::Value| {
        let result =
            host.automation_request(&serde_json::json!({"op":"act","css":css,"action":action}));
        platform.settle(&[&host]);
        result
    };
    let query = |css: &str| {
        host.automation_request(&serde_json::json!({"op":"query","css":css}))
            .unwrap()
    };
    act(
        "[data-testid=\"filter\"]",
        serde_json::json!({"kind":"fill","text":"Row 3"}),
    )
    .unwrap();
    host.request_frame();
    platform.settle(&[&host]);
    let row = "[data-testid=\"row-31\"]";
    // A locator's click as the test client drives it: reveal until the row draws on screen.
    let mut trace = Vec::new();
    // Locators resolve with `all`, then reveal a match outside the viewport.
    let resolve = |css: &str| {
        let all = host
            .automation_request(&serde_json::json!({"op":"query","css":css,"all":true}))
            .unwrap();
        all["items"].get(0).cloned().unwrap_or(all)
    };
    let mut clicked = false;
    for _ in 0..8 {
        let found = resolve(row);
        trace.push(found.to_string());
        if found["visible"] == true && found["inViewport"] == true && found["virtual"].is_null() {
            act(row, serde_json::json!({"kind":"click"})).unwrap();
            clicked = true;
            break;
        }
        // Only a visible match is revealed; anything else waits for the next poll.
        if found["visible"] == true {
            let scrolled = act(row, serde_json::json!({"kind":"scroll"}));
            trace.push(format!("scroll: {scrolled:?}"));
        }
        platform.settle(&[&host]);
    }
    assert!(clicked, "row 31 never drew: {trace:#?}");
    host.request_frame();
    platform.settle(&[&host]);
    assert_eq!(query("[data-testid=\"picked\"]")["text"], "picked=Row 31");
}

#[test]
fn a_date_editor_opens_the_views_picker_and_commits_through_the_editor() {
    use crate::interaction::view::Key;
    let worker = ViewWorker::spawn("test-view-picker").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){const day=useLocal('2026-10-29'),submits=useLocal(0);return h('view',{},h('input',{'data-testid':'day',type:'date',bind:day,'aria-label':'Day',min:'2026-10-01',onSubmit:()=>submits.set(submits.value+1)}),h('text',{'data-testid':'out'},`${day.value} ${submits.value}`))}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let query = |css: &str| {
        host.automation_request(&serde_json::json!({"op":"query","css":css}))
            .unwrap()
    };
    host.automation_request(
        &serde_json::json!({"op":"act","css":"[data-testid=\"day\"]","action":{"kind":"click"}}),
    )
    .unwrap();
    platform.settle(&[&host]);
    assert_eq!(
        query("[data-testid=\"lx-picker\"]")["exists"],
        true,
        "the picker opened"
    );
    assert_eq!(
        query("[data-testid=\"lx-picker-title\"]")["text"],
        "October 2026"
    );
    // A week down, then Return: the editor takes the date and submits it.
    assert!(host.key(Key::Down, false, false));
    platform.settle(&[&host]);
    assert!(host.key(Key::Activate, false, false));
    platform.settle(&[&host]);
    assert_eq!(
        query("[data-testid=\"lx-picker\"]")["exists"],
        false,
        "the picker closed"
    );
    assert_eq!(query("[data-testid=\"out\"]")["text"], "2026-11-05 1");
    // Escape cancels and leaves the value.
    host.automation_request(
        &serde_json::json!({"op":"act","css":"[data-testid=\"day\"]","action":{"kind":"click"}}),
    )
    .unwrap();
    platform.settle(&[&host]);
    assert!(host.key(Key::Escape, false, false));
    platform.settle(&[&host]);
    assert_eq!(query("[data-testid=\"lx-picker\"]")["exists"], false);
    assert_eq!(query("[data-testid=\"out\"]")["text"], "2026-11-05 1");
}

#[test]
fn the_views_pickers_announce_grid_cells_and_options_and_take_a_tapped_time() {
    let worker = ViewWorker::spawn("test-view-picker-roles").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){const day=useLocal('2026-10-21'),at=useLocal('18:30');return h('view',{},h('input',{'data-testid':'day',type:'date',bind:day,'aria-label':'Day'}),h('input',{'data-testid':'at',type:'time',bind:at,step:900,'aria-label':'At'}),h('text',{'data-testid':'out'},`${day.value} ${at.value}`))}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let request = |request: serde_json::Value| {
        let reply = host.automation_request(&request);
        platform.settle(&[&host]);
        reply
    };
    let query = |css: &str| request(serde_json::json!({"op":"query","css":css})).unwrap();
    request(
        serde_json::json!({"op":"act","css":"[data-testid=\"day\"]","action":{"kind":"click"}}),
    )
    .unwrap();
    let cell = query("[data-testid=\"lx-picker-day-2026-10-21\"]");
    assert_eq!(cell["role"], "gridcell", "{cell}");
    assert_eq!(cell["aria_label"], "Wednesday, October 21, 2026");
    assert_eq!(query("[role=\"grid\"]")["exists"], true);
    assert!(host.key(crate::interaction::view::Key::Escape, false, false));
    platform.settle(&[&host]);
    assert_eq!(
        query("[data-testid=\"lx-picker\"]")["exists"],
        false,
        "Escape cancels"
    );
    request(serde_json::json!({"op":"act","css":"[data-testid=\"at\"]","action":{"kind":"click"}}))
        .unwrap();
    assert_eq!(
        query("[data-testid=\"lx-picker-minute-45\"]")["role"],
        "option"
    );
    request(serde_json::json!({"op":"act","css":"[data-testid=\"lx-picker-minute-45\"]","action":{"kind":"click"}})).unwrap();
    assert!(host.key(crate::interaction::view::Key::Activate, false, false));
    platform.settle(&[&host]);
    assert_eq!(query("[data-testid=\"out\"]")["text"], "2026-10-21 18:45");
}

#[test]
fn a_snapping_scroll_settles_on_the_nearest_point_once_input_stops() {
    use crate::interaction::view::{Wheel, WheelPhase};
    let worker = ViewWorker::spawn("test-scroll-snap").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){return h('scroll',{'data-testid':'s',style:{height:100,scrollSnapType:'y'}},...Array.from({length:10},(_,i)=>h('view',{key:String(i),'data-testid':'r'+i,style:{height:40,scrollSnapAlign:'start'}})))}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let top = |id: &str| {
        host.automation_request(
            &serde_json::json!({"op":"query","css":format!("[data-testid=\"{id}\"]")}),
        )
        .unwrap()["rect"]["top"]
            .as_f64()
            .unwrap()
    };
    let wheel = |dy: f32| {
        host.wheel(Wheel {
            x: 50.0,
            y: 50.0,
            dx: 0.0,
            dy,
            precise: true,
            phase: WheelPhase::Changed,
        });
        platform.settle(&[&host]);
    };
    // 52 logical px down (device pixels at scale 2): closer to row 1 (40) than row 2 (80).
    wheel(-104.0);
    assert_eq!(top("r1"), -12.0, "the wheel moves freely");
    // More movement inside the quiet time restarts it.
    platform.advance(std::time::Duration::from_millis(60));
    wheel(-8.0);
    platform.advance(std::time::Duration::from_millis(100));
    platform.settle(&[&host]);
    assert_eq!(top("r1"), -16.0, "no settle while input continues");
    let mut tops = Vec::new();
    for _ in 0..60 {
        platform.advance(std::time::Duration::from_millis(16));
        platform.settle(&[&host]);
        tops.push(top("r1"));
    }
    assert_eq!(top("r1"), 0.0, "settled on row 1, the nearest point");
    // A critically damped spring: it never passes the point.
    assert!(tops.iter().all(|t| (-16.0..=0.0).contains(t)), "{tops:?}");
}

#[test]
fn a_close_request_closes_the_topmost_dialog_or_popover_and_is_refused_without_one() {
    let worker = ViewWorker::spawn("test-close-request").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){const open=useLocal('open',true);return h('page',{},h('text',{'data-testid':'state'},open.value?'open':'closed'),h('view',{id:'toast',popover:'manual',open:true,'data-testid':'toast'}),h('dialog',{open:open.value,onClose(){open.set(false)},'aria-label':'Confirm','data-testid':'d'}))}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let text = || {
        host.automation_request(&serde_json::json!({"op":"query","css":"[data-testid=\"state\"]"}))
            .unwrap()["text"]
            .clone()
    };
    assert!(host.close_request(), "the open dialog takes it");
    platform.settle(&[&host]);
    assert_eq!(text(), "closed");
    assert!(!host.close_request(), "a manual popover takes none");
}

#[test]
fn a_close_request_closes_an_open_picker_sheet_or_popover() {
    for (reduced, blur) in [(true, false), (false, false), (false, true)] {
        close_request_case(reduced, blur);
    }
}

fn close_request_case(reduced: bool, blur: bool) {
    let worker = ViewWorker::spawn("test-close-request-pickers").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){const c=useLocal('c',''),day=useLocal('day','2026-10-21');return h('view',{},h('select',{'data-testid':'country',bind:c,'aria-label':'Country',options:[{value:'de',label:'Germany'},{value:'fr',label:'France'}]}),h('input',{'data-testid':'day',type:'date',bind:day,'aria-label':'Day'}),h('button',{popoverTarget:'tip','data-testid':'trigger'},'Info'),h('view',{id:'tip',popover:'auto',role:'tooltip','data-testid':'tip'},h('text',{},'Saved')))}",
        ),
        false,
    );
    host.adapter()
        .environment
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .value["reducedMotion"] = serde_json::json!(reduced);
    host.set_visible(true);
    platform.settle(&[&host]);
    let count = |css: &str| {
        host.automation_request(&serde_json::json!({"op":"query","css":css,"all":true}))
            .unwrap()["count"]
            .clone()
    };
    let click = |id: &str| {
        host.automation_request(&serde_json::json!({"op":"act","css":format!("[data-testid=\"{id}\"]"),"action":{"kind":"click"}}))
            .unwrap();
        platform.settle(&[&host]);
    };
    assert!(
        !host.close_request(),
        "nothing open: the page's back goes on"
    );
    for (trigger, open) in [
        ("country", "[role=\"dialog\"]"),
        ("day", "[data-testid=\"lx-picker\"]"),
        ("trigger", "[data-testid=\"tip\"]"),
    ] {
        click(trigger);
        assert_eq!(count(open), 1, "{trigger} opened");
        assert!(host.close_request(), "{trigger}: the open layer takes it");
        if blur {
            host.window_focus(false);
            host.window_focus(true);
        }
        for _ in 0..60 {
            platform.advance(std::time::Duration::from_millis(16));
            platform.settle(&[&host]);
        }
        assert_eq!(
            count(open),
            0,
            "{trigger}: closed by the request (reduced={reduced} blur={blur})"
        );
        assert!(!host.close_request(), "{trigger}: nothing left open");
    }
}

#[path = "appearance.rs"]
mod appearance;
#[path = "automation_blur.rs"]
mod automation_blur;
#[path = "block_link.rs"]
mod block_link;
#[path = "box_reading.rs"]
mod box_reading;
#[path = "clipped_focus.rs"]
mod clipped_focus;
#[path = "close_request.rs"]
mod close_request;
#[path = "combobox_semantics.rs"]
mod combobox_semantics;
#[path = "context_menu.rs"]
mod context_menu;
#[path = "editor_keys.rs"]
mod editor_keys;
#[path = "failed_first_frame.rs"]
mod failed_first_frame;
#[path = "failed_page.rs"]
mod failed_page;
#[path = "fixed_in_popover.rs"]
mod fixed_in_popover;
#[path = "handler_failure.rs"]
mod handler_failure;
#[path = "hover_pin.rs"]
mod hover_pin;
#[path = "list_name.rs"]
mod list_name;
#[path = "orphan_overlays.rs"]
mod orphan_overlays;
#[path = "page_left.rs"]
mod page_left;
#[path = "pointer_events.rs"]
mod pointer_events;
#[path = "popover_anchor.rs"]
mod popover_anchor;
#[path = "range_slider.rs"]
mod range_slider;
#[path = "reveal_element.rs"]
mod reveal_element;
#[path = "space_key.rs"]
mod space_key;
#[path = "submenu.rs"]
mod submenu;
#[path = "tree_keys.rs"]
mod tree_keys;

#[test]
fn escape_pressed_on_any_match_closes_an_open_light_dismiss_popover() {
    let worker = ViewWorker::spawn("test-escape-popover").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){return h('page',{},h('button',{popoverTarget:'tip','data-testid':'trigger'},'Info'),h('view',{id:'tip',popover:'auto',role:'tooltip','data-testid':'tip'},h('text',{'data-testid':'note'},'Saved locally')))}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let act = |css: &str, action: serde_json::Value| {
        host.automation_request(&serde_json::json!({"op":"act","css":css,"action":action}))
    };
    let open = || {
        host.automation_request(&serde_json::json!({"op":"query","css":"[data-testid=\"tip\"]"}))
            .unwrap()["exists"]
            .as_bool()
            .unwrap()
    };
    for target in ["[data-testid=\"trigger\"]", "[data-testid=\"note\"]"] {
        act(
            "[data-testid=\"trigger\"]",
            serde_json::json!({"kind":"click"}),
        )
        .unwrap();
        platform.settle(&[&host]);
        assert!(open(), "the invoker opens the popover");
        act(target, serde_json::json!({"kind":"press","key":"Escape"}))
            .unwrap_or_else(|err| panic!("Escape on {target}: {err}"));
        platform.settle(&[&host]);
        assert!(!open(), "Escape on {target} closes the popover");
    }
    assert!(
        act(
            "[data-testid=\"trigger\"]",
            serde_json::json!({"kind":"press","key":"Escape"})
        )
        .is_err(),
        "with nothing open, Escape is the platform's"
    );
}

#[test]
fn a_select_sheet_is_drivable_and_a_choice_made_elsewhere_closes_it() {
    let worker = ViewWorker::spawn("test-select-sheet").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){const c=useLocal('c','');return h('view',{},h('select',{'data-testid':'country',bind:c,'aria-label':'Country',placeholder:'Choose a country',options:[{value:'de',label:'Germany'},{value:'fr',label:'France'}]}),h('text',{'data-testid':'out'},'c='+c.value))}",
        ),
        false,
    );
    host.adapter()
        .environment
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .value["reducedMotion"] = serde_json::json!(true);
    host.set_visible(true);
    platform.settle(&[&host]);
    let count = |css: &str| {
        host.automation_request(&serde_json::json!({"op":"query","css":css,"all":true}))
            .unwrap()["count"]
            .clone()
    };
    let act = |css: &str, action: serde_json::Value| {
        let acted =
            host.automation_request(&serde_json::json!({"op":"act","css":css,"action":action}));
        platform.settle(&[&host]);
        acted
    };
    fn flatten<'a>(node: &'a serde_json::Value, out: &mut Vec<&'a serde_json::Value>) {
        out.push(node);
        for child in node["children"].as_array().into_iter().flatten() {
            flatten(child, out);
        }
    }
    act(
        "[data-testid=\"country\"]",
        serde_json::json!({"kind":"click"}),
    )
    .unwrap();
    // The touch picker: a dialog named by the select, Cancel, Done and the options.
    let sheet = host
        .automation_request(&serde_json::json!({"op":"tree"}))
        .unwrap();
    let mut nodes = Vec::new();
    for node in sheet["nodes"].as_array().into_iter().flatten() {
        flatten(node, &mut nodes);
    }
    let has = |want: serde_json::Value| {
        nodes.iter().any(|node| {
            want.as_object()
                .unwrap()
                .iter()
                .all(|(key, value)| &node[key] == value)
        })
    };
    for part in [
        serde_json::json!({"name":"Country","role":"dialog"}),
        serde_json::json!({"testId":"lx-picker-cancel"}),
        serde_json::json!({"testId":"lx-picker-done"}),
        serde_json::json!({"name":"France","role":"option","selected":false}),
    ] {
        assert!(has(part.clone()), "{part} in {sheet}");
    }
    assert!(
        !has(serde_json::json!({"selected":true})),
        "no value, no tick: {sheet}"
    );
    // The select is behind a modal: out of role queries, still reached by its test id.
    assert_eq!(count("select, [role=\"combobox\"]"), 0);
    act(
        "[data-testid=\"country\"]",
        serde_json::json!({"kind":"select","value":"fr"}),
    )
    .unwrap();
    assert_eq!(
        count("[role=\"dialog\"]"),
        0,
        "the choice closes its picker"
    );
    let out = host
        .automation_request(&serde_json::json!({"op":"query","css":"[data-testid=\"out\"]"}))
        .unwrap();
    assert_eq!(out["text"], "c=fr");
}

#[test]
fn a_list_anchor_with_a_duration_glides_there_and_input_stops_it() {
    use crate::interaction::view::{Wheel, WheelPhase};
    let glide = |stop_at: Option<usize>| {
        let worker = ViewWorker::spawn("test-list-glide").unwrap();
        let platform = Platform::new(&[&worker]);
        let host = platform.host(
            &worker,
            &source(
                "function render(){return h(listTag,{count:200,anchor:{index:50,align:'start',duration:400},style:{height:100},row:i=>h('text',{'data-testid':'r'+i,style:{height:40}},String(i))})}",
            ),
            false,
        );
        host.set_visible(true);
        platform.settle(&[&host]);
        let offset = || {
            host.snapshot().unwrap().frame.nodes[0]
                .scroll
                .as_ref()
                .unwrap()
                .offset
        };
        let mut offsets = vec![offset()];
        for step in 0..40 {
            if stop_at == Some(step) {
                host.wheel(Wheel {
                    x: 50.0,
                    y: 50.0,
                    dx: 0.0,
                    dy: -2.0,
                    precise: true,
                    phase: WheelPhase::Changed,
                });
            }
            *host.adapter().clock.lock().unwrap() += Duration::from_millis(16);
            platform.advance(Duration::from_millis(16));
            platform.settle(&[&host]);
            offsets.push(offset());
        }
        // Every presented frame counts toward the stage medians automation reads.
        let frame = host
            .automation_request(&serde_json::json!({"op":"frame"}))
            .unwrap();
        let stages = &frame["stagesMs"];
        assert!(stages["frames"].as_u64().unwrap() > 10, "{stages}");
        assert!(stages["layoutPasses"].as_u64().is_some() && stages["build"].is_number());
        assert!(
            stages["worker"].is_number() && stages["main"].is_number(),
            "{stages}"
        );
        offsets
    };
    let offsets = glide(None);
    assert_eq!(offsets[0], 0.0, "a glide does not jump: {offsets:?}");
    assert_eq!(
        *offsets.last().unwrap(),
        2000.0,
        "row 50 at the top: {offsets:?}"
    );
    assert!(offsets.windows(2).all(|w| w[0] <= w[1]), "{offsets:?}");
    assert!(
        offsets.iter().filter(|o| **o > 0.0 && **o < 2000.0).count() > 10,
        "{offsets:?}"
    );
    let stopped = glide(Some(10));
    let at = stopped[11];
    assert!(at > 0.0 && at < 2000.0, "{stopped:?}");
    assert!(
        stopped[12..].iter().all(|o| *o == at),
        "input took it over: {stopped:?}"
    );
}

#[test]
fn a_held_slider_draws_its_active_parts_on_the_presented_frame() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let page = source(
        "function render(){const v=useLocal('v',50);return h('view',{},h('slider',{'data-testid':'s',bind:v,'aria-label':'Seek',className:'thin active:track:thick',style:{width:160}}))}",
    )
    .replace(
        "__lingxiaPublish(render);",
        "configureClasses([{name:'thin',when:{part:'track'},style:{height:3}},{name:'active:track:thick',when:{state:'active',part:'track'},style:{height:6}}]);__lingxiaPublish(render);",
    );
    let host = platform.host(&worker, &page, false);
    host.set_visible(true);
    platform.settle(&[&host]);
    let line = crate::theme::Palette::builtin(false).role(lingxia_theme::Role::Line);
    let track = |host: &ViewHost<Fake>| {
        host.adapter()
            .log
            .lock()
            .unwrap()
            .rects
            .iter()
            .find(|(color, _)| *color == line)
            .map(|(_, bounds)| bounds[3])
    };
    assert_eq!(track(&host), Some(3.0));
    host.pointer(Pointer {
        phase: PointerPhase::Down,
        x: 160.0,
        y: 44.0,
        slop: 8.0,
        velocity: [0.0; 2],
        min_fling: 50.0,
    });
    platform.settle(&[&host]);
    run_frames(&platform, &host, 20);
    assert_eq!(
        track(&host),
        Some(6.0),
        "held, the track takes its active height"
    );
}

#[test]
fn a_dragged_slider_keeps_its_active_parts_until_release() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let page = source(
        "function render(){const v=useLocal('v',30);return h('view',{},h('slider',{'data-testid':'s',bind:v,max:180,'aria-label':'Seek',className:'px-0 thin active:track:thick thumb:size-0 active:thumb:size-4',style:{width:160}}))}",
    )
    .replace(
        "__lingxiaPublish(render);",
        "configureClasses([{name:'px-0',style:{paddingLeft:0,paddingRight:0}},{name:'thin',when:{part:'track'},style:{height:3}},{name:'active:track:thick',when:{state:'active',part:'track'},style:{height:6}},{name:'thumb:size-0',when:{part:'thumb'},style:{width:0,height:0}},{name:'active:thumb:size-4',when:{state:'active',part:'thumb'},style:{width:16,height:16}}]);__lingxiaPublish(render);",
    );
    let host = platform.host(&worker, &page, false);
    host.set_visible(true);
    platform.settle(&[&host]);
    let line = crate::theme::Palette::builtin(false).role(lingxia_theme::Role::Line);
    let track = |host: &ViewHost<Fake>| {
        host.adapter()
            .log
            .lock()
            .unwrap()
            .rects
            .iter()
            .find(|(color, _)| *color == line)
            .map(|(_, bounds)| bounds[3])
    };
    let at = |phase, x: f32| Pointer {
        phase,
        x,
        y: 44.0,
        slop: 8.0,
        velocity: [0.0; 2],
        min_fling: 50.0,
    };
    assert_eq!(track(&host), Some(3.0));
    host.pointer(at(PointerPhase::Down, 100.0));
    run_frames(&platform, &host, 20);
    let mut seen = vec![("held", track(&host))];
    for step in 1..=6 {
        host.pointer(at(PointerPhase::Move, 100.0 + 12.0 * step as f32));
        run_frames(&platform, &host, 4);
        seen.push(("moved", track(&host)));
    }
    run_frames(&platform, &host, 20);
    seen.push(("settled", track(&host)));
    assert!(
        seen.iter().all(|(_, height)| *height == Some(6.0)),
        "{seen:?}"
    );
    host.pointer(at(PointerPhase::Up, 172.0));
    run_frames(&platform, &host, 20);
    assert_eq!(track(&host), Some(3.0), "released, the track rests");
}

#[test]
fn the_frame_reply_reports_the_bar_snapshot_the_presented_frame_draws() {
    let worker = ViewWorker::spawn("test-chrome-snapshot").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){return h('page',{},h('text',{},'behind'),h('dialog',{open:true,'aria-label':'Confirm'},h('text',{},'sure?')))}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let reported = || {
        host.automation_request(&serde_json::json!({"op": "frame"}))
            .unwrap()["chromeSnapshot"]
            .as_u64()
    };
    assert_eq!(reported(), Some(0), "no bars captured yet");
    let generation = host.set_chrome_snapshot(
        crate::scene::display::ChromeEdge::Top,
        vec![255; 4 * 4 * 2],
        4,
        2,
        [0.0, 0.0, 40.0, 20.0],
    );
    run_frames(&platform, &host, 4);
    assert_eq!(reported(), Some(generation));
}

#[test]
fn hover_styles_follow_the_pointer_and_end_when_the_window_loses_focus_or_the_page_hides() {
    let worker = ViewWorker::spawn("test-hover-leave").unwrap();
    let platform = Platform::new(&[&worker]);
    let page = source(
        "function render(){return h('view',{},h('view',{'data-testid':'b',className:'cold hover:hot',style:{width:100,height:40}}))}",
    )
    .replace(
        "__lingxiaPublish(render);",
        "configureClasses([{name:'cold',style:{backgroundColor:'#0000ff'}},{name:'hover:hot',when:{state:'hover'},style:{backgroundColor:'#ff0000'}}]);__lingxiaPublish(render);",
    );
    // A coarse primary pointer: a plugged-in mouse still hovers.
    let host = platform.host(&worker, &page, false);
    host.set_visible(true);
    platform.settle(&[&host]);
    let hot = |host: &ViewHost<Fake>| {
        let rects = host.adapter().log.lock().unwrap().rects.clone();
        rects.iter().any(|(color, _)| color[..3] == [255, 0, 0])
    };
    assert!(!hot(&host));
    host.hover(Some([20.0, 20.0]));
    run_frames(&platform, &host, 4);
    assert!(hot(&host), "the pointer is over the box");
    host.window_focus(false);
    run_frames(&platform, &host, 4);
    assert!(!hot(&host), "the window lost focus");
    host.hover(Some([20.0, 20.0]));
    run_frames(&platform, &host, 4);
    assert!(hot(&host));
    host.set_visible(false);
    host.set_visible(true);
    run_frames(&platform, &host, 4);
    assert!(!hot(&host), "the page hid");
}

#[test]
fn an_interest_trigger_shows_its_hint_after_a_rest_and_hides_it_after_the_grace() {
    let worker = ViewWorker::spawn("test-interest").unwrap();
    let platform = Platform::new(&[&worker]);
    let page = source(
        "function render(){return h('page',{},h('button',{interestFor:'tip','aria-label':'About','data-testid':'about',style:{width:100,height:40}}),h('view',{id:'tip',popover:'hint',style:{width:60,height:30,backgroundColor:'#ff0000'}}))}",
    );
    let host = platform.host(&worker, &page, false);
    host.set_visible(true);
    platform.settle(&[&host]);
    let shown = |host: &ViewHost<Fake>| {
        let rects = host.adapter().log.lock().unwrap().rects.clone();
        rects.iter().any(|(color, _)| color[..3] == [255, 0, 0])
    };
    assert!(!shown(&host));
    let about = host
        .automation_request(&serde_json::json!({"op": "query", "css": "[data-testid=\"about\"]"}))
        .unwrap();
    assert_eq!(
        about["enabled"], true,
        "a click-less trigger is not disabled: {about}"
    );
    host.hover(Some([20.0, 20.0]));
    platform.advance(Duration::from_millis(400));
    run_frames(&platform, &host, 2);
    assert!(!shown(&host), "the pointer has not rested long enough");
    platform.advance(Duration::from_millis(100));
    platform.settle(&[&host]);
    run_frames(&platform, &host, 4);
    assert!(shown(&host), "the hint shows after the rest");
    host.hover(None);
    platform.advance(Duration::from_millis(100));
    host.hover(Some([20.0, 20.0]));
    platform.advance(Duration::from_millis(500));
    platform.settle(&[&host]);
    run_frames(&platform, &host, 4);
    assert!(shown(&host), "returning within the grace keeps the hint");
    host.window_focus(false);
    platform.advance(Duration::from_millis(250));
    platform.settle(&[&host]);
    run_frames(&platform, &host, 4);
    assert!(!shown(&host), "the window lost focus");
    host.window_focus(true);
    assert!(
        host.key(crate::interaction::view::Key::Tab, false, false),
        "a click-less trigger takes focus"
    );
    platform.advance(Duration::from_millis(500));
    platform.settle(&[&host]);
    run_frames(&platform, &host, 4);
    assert!(shown(&host), "keyboard focus shows the hint");
    platform.advance(Duration::from_millis(300));
    platform.settle(&[&host]);
    run_frames(&platform, &host, 8);
    assert!(shown(&host), "the hint stays while its trigger keeps focus");
    assert!(
        host.key(crate::interaction::view::Key::Escape, false, false),
        "Escape is consumed while a hint shows"
    );
    platform.settle(&[&host]);
    run_frames(&platform, &host, 4);
    assert!(!shown(&host), "Escape closes the hint");
}

#[test]
fn a_fine_pointer_crossing_a_box_calls_its_enter_and_leave_without_bubbling() {
    let worker = ViewWorker::spawn("test-pointer-crossing").unwrap();
    let platform = Platform::new(&[&worker]);
    let page = source(
        "function render(){const log=useLocal('log','');const add=s=>()=>log.set(log.value+s);return h('page',{},h('view',{onPointerEnter:add('O+'),onPointerLeave:add('O-'),style:{width:200,height:100}},h('button',{onPointerEnter:add('I+'),onPointerLeave:add('I-'),onClick(){},style:{width:80,height:40}},'In')),h('text',{'data-testid':'log'},log.value||'-'))}",
    );
    let host = platform.host(&worker, &page, false);
    host.set_visible(true);
    platform.settle(&[&host]);
    let log = |host: &ViewHost<Fake>| {
        host.automation_request(&serde_json::json!({"op": "query", "css": "[data-testid=\"log\"]"}))
            .unwrap()["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    };
    let step = |point: Option<[f32; 2]>| {
        host.hover(point);
        platform.settle(&[&host]);
        run_frames(&platform, &host, 2);
    };
    step(Some([180.0, 120.0]));
    assert_eq!(log(&host), "O+", "the outer box only");
    step(Some([40.0, 40.0]));
    assert_eq!(
        log(&host),
        "O+I+",
        "the inner button enters; the outer hears nothing more"
    );
    step(Some([44.0, 42.0]));
    assert_eq!(log(&host), "O+I+", "moving inside fires nothing");
    step(Some([180.0, 120.0]));
    assert_eq!(
        log(&host),
        "O+I+I-",
        "leaving the button does not leave its parent"
    );
    step(Some([40.0, 40.0]));
    host.window_focus(false);
    platform.settle(&[&host]);
    run_frames(&platform, &host, 2);
    assert_eq!(
        log(&host),
        "O+I+I-I+I-O-",
        "a window losing focus leaves innermost first"
    );
}

#[test]
fn a_tap_elsewhere_closes_a_hint_whatever_opened_it_and_still_reaches_what_it_hit() {
    let worker = ViewWorker::spawn("test-hint-dismiss").unwrap();
    let platform = Platform::new(&[&worker]);
    let page = source(
        "function render(){const n=useLocal('n',0);return h('page',{},h('button',{popoverTarget:'tip','data-testid':'open',style:{width:100,height:40}},'Open'),h('button',{'data-testid':'other',onClick(){n.set(n.value+1)},style:{width:100,height:40,marginTop:100}},String(n.value)),h('view',{id:'tip',popover:'hint','data-testid':'tip',style:{width:60,height:30,backgroundColor:'#ff0000'}}))}",
    );
    let host = platform.host(&worker, &page, false);
    host.set_visible(true);
    platform.settle(&[&host]);
    let query = |host: &ViewHost<Fake>, id: &str| {
        host.automation_request(
            &serde_json::json!({"op": "query", "css": format!("[data-testid=\"{id}\"]")}),
        )
        .unwrap_or(serde_json::Value::Null)
    };
    let shown = |host: &ViewHost<Fake>| query(host, "tip")["rect"].is_object();
    let press = |host: &ViewHost<Fake>, id: &str| {
        let node = query(host, id);
        let r = |key: &str| node["rect"][key].as_f64().unwrap() as f32;
        let [x, y] = [r("left") + r("width") / 2.0, r("top") + r("height") / 2.0];
        tap(host, x * 2.0, y * 2.0);
        platform.settle(&[host]);
        run_frames(&platform, host, 4);
    };
    press(&host, "open");
    assert!(shown(&host), "the invoker's tap opens the hint");
    press(&host, "tip");
    assert!(shown(&host), "a tap on the hint keeps it");
    press(&host, "other");
    assert!(!shown(&host), "a tap elsewhere closes it");
    assert_eq!(
        query(&host, "other")["text"],
        "1",
        "and the tap still reaches the button it landed on"
    );
    press(&host, "open");
    assert!(shown(&host));
    press(&host, "open");
    assert!(
        !shown(&host),
        "a tap on its own invoker toggles it shut, not shut and open again"
    );
}

#[path = "autoclose.rs"]
mod autoclose;
#[path = "panel_press.rs"]
mod panel_press;
#[path = "picker_fill.rs"]
mod picker_fill;
#[path = "touch_target.rs"]
mod touch_target;

/// A page whose slider logs `onPressChange` as T/F; `extra` is spliced into its render.
fn press_page(platform: &Platform, worker: &Arc<ViewWorker>, extra: &str) -> ViewHost<Fake> {
    let page = source(&format!(
        "function render(){{const v=useLocal('v',5);const log=useLocal('log','');const gone=useLocal('gone',false);const off=useLocal('off',false);const press=p=>{{log.set(log.value+(p?'T':'F'));{extra}}};return h('page',{{}},gone.value?h('view',{{style:{{height:40}}}}):h('slider',{{'data-testid':'s',bind:v,max:10,disabled:off.value,'aria-label':'Level',onPressChange:press,style:{{width:80,height:40}}}}),h('text',{{'data-testid':'log'}},log.value||'-'))}}"
    ));
    let host = platform.host(worker, &page, false);
    host.set_visible(true);
    platform.settle(&[&host]);
    host
}

fn press_log(host: &ViewHost<Fake>) -> String {
    host.automation_request(&serde_json::json!({"op": "query", "css": "[data-testid=\"log\"]"}))
        .unwrap()["text"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// Down on the slider's middle, frames, then `after_down` frames later the finger lifts there.
fn hold_slider(platform: &Platform, host: &ViewHost<Fake>) -> (String, String) {
    let node = host
        .automation_request(&serde_json::json!({"op": "query", "css": "[data-testid=\"s\"]"}))
        .unwrap();
    let r = |key: &str| node["rect"][key].as_f64().unwrap() as f32;
    let at = |phase| Pointer {
        phase,
        x: (r("left") + r("width") / 2.0) * 2.0,
        y: (r("top") + r("height") / 2.0) * 2.0,
        slop: 8.0,
        velocity: [0.0; 2],
        min_fling: 50.0,
    };
    // Frames, not settle: a disable starts the parts' colour transition, which needs a clock.
    host.pointer(at(PointerPhase::Down));
    run_frames(platform, host, 20);
    let held = press_log(host);
    host.pointer(at(PointerPhase::Up));
    run_frames(platform, host, 20);
    (held, press_log(host))
}

#[test]
fn a_held_slider_hears_one_press_and_one_release_though_its_callback_is_renumbered() {
    let worker = ViewWorker::spawn("test-press-renumber").unwrap();
    let platform = Platform::new(&[&worker]);
    // The press re-renders the page, and every render issues new callback handles.
    let host = press_page(&platform, &worker, "");
    let (held, released) = hold_slider(&platform, &host);
    assert_eq!(held, "T", "a still finger is pressed, once");
    assert_eq!(
        released, "TF",
        "the release reaches the renumbered callback, once"
    );
}

#[test]
fn a_still_finger_stays_pressed_while_the_slider_re_renders_under_it() {
    let worker = ViewWorker::spawn("test-press-still").unwrap();
    let platform = Platform::new(&[&worker]);
    // The press moves the value, as a landed seek does, while the finger rests.
    let host = press_page(&platform, &worker, "if(p)v.set(9);");
    let (held, released) = hold_slider(&platform, &host);
    assert_eq!(held, "T");
    assert_eq!(released, "TF");
}

#[test]
fn a_slider_removed_or_disabled_mid_press_still_hears_exactly_one_release() {
    for (name, worker, extra) in [
        ("removed", "test-press-removed", "if(p)gone.set(true);"),
        ("disabled", "test-press-disabled", "if(p)off.set(true);"),
    ] {
        let worker = ViewWorker::spawn(worker).unwrap();
        let platform = Platform::new(&[&worker]);
        let host = press_page(&platform, &worker, extra);
        let (held, released) = hold_slider(&platform, &host);
        assert_eq!(
            held, "TF",
            "{name}: the release comes without the finger lifting"
        );
        assert_eq!(released, "TF", "{name}: and never twice");
    }
}

#[test]
fn webview_host_coalesces_messages_caps_fifo_queue_and_reconciles_fullscreen_without_remounting() {
    use crate::scene::webview::WebViewInbound;
    let worker = ViewWorker::spawn("test-webview-host").unwrap();
    let platform = Platform::new(&[&worker]);
    let page = source(
        "function render(){\
           const web=useWebView('embed');\
           const log=useLocal('log','');\
           const sheet=useLocal('sheet',false);\
           return h('page',{},\
             h('webview',{\
               web,\
               'aria-label':'Embed',\
               'data-testid':'wv',\
               src:'https://example.com',\
               onLoad:e=>log.set(log.value+'L:'+e.title+';'),\
               onMessage:e=>log.set(log.value+'M:'+e.data+';'),\
               onBlocked:e=>log.set(log.value+'B:'+e.url+';'),\
               onError:e=>log.set(log.value+'E:'+e.code+';'),\
               onFullscreenChange:e=>log.set(log.value+'F:'+e.fullscreen+';'),\
               style:{width:120,height:80}\
             }),\
             h('button',{'data-testid':'open-sheet',onClick:()=>sheet.set(true)},'Sheet'),\
             h('dialog',{open:sheet.value,closedby:'closerequest',onClose:()=>sheet.set(false),'aria-label':'Sheet',style:{width:'100%',height:'70%',marginTop:'auto',marginBottom:0,marginLeft:0,marginRight:0}},\
               h('webview',{web,'aria-label':'Embed',style:{width:'100%',height:'100%'}})\
             ),\
             h('text',{'data-testid':'log'},log.value||'-')\
           );\
         }",
    );
    let host = platform.host(&worker, &page, false);
    host.set_visible(true);
    platform.settle(&[&host]);

    let mounts = host
        .adapter()
        .log
        .lock()
        .unwrap()
        .webviews
        .last()
        .cloned()
        .unwrap();
    assert_eq!(mounts.len(), 1);
    assert_eq!(mounts[0].web_id, "wv-1");
    assert_eq!(mounts[0].bounds, [0.0, 0.0, 120.0, 80.0]);

    // Queued messages keep FIFO order up to 64 and report WEBVIEW_MESSAGE_DROPPED on overflow;
    // coalesced ones sent after them within one frame keep only the latest ("c"), delivered last.
    for i in 0..65 {
        host.webview_event(
            "wv-1".into(),
            WebViewInbound::Message {
                json: i.to_string(),
                origin: "https://example.com".into(),
                queued: true,
            },
        );
    }
    host.webview_event(
        "wv-1".into(),
        WebViewInbound::Message {
            json: "\"a\"".into(),
            origin: "https://example.com".into(),
            queued: false,
        },
    );
    host.webview_event(
        "wv-1".into(),
        WebViewInbound::Message {
            json: "\"c\"".into(),
            origin: "https://example.com".into(),
            queued: false,
        },
    );
    platform.settle(&[&host]);
    let text = press_log(&host);
    // 0 was dropped, 1..=64 delivered in order, then coalesced "c", then WEBVIEW_MESSAGE_DROPPED!
    assert!(!text.contains("M:0;"), "oldest queued dropped: {text}");
    assert!(text.starts_with("M:1;M:2;"), "FIFO preserved: {text}");
    assert!(
        text.contains("M:64;M:c;E:WEBVIEW_MESSAGE_DROPPED;"),
        "{text}"
    );

    // Web-initiated HTML5 fullscreen opens the bare full-window dialog fallback, and close_request
    // (Back / Escape) exits it and tells native WebView to exit fullscreen!
    host.webview_event("wv-1".into(), WebViewInbound::Fullscreen(true));
    platform.settle(&[&host]);
    let fs_mounts = host
        .adapter()
        .log
        .lock()
        .unwrap()
        .webviews
        .last()
        .cloned()
        .unwrap();
    assert_eq!(fs_mounts.len(), 1);
    assert_eq!(fs_mounts[0].web_id, "wv-1");
    assert!(fs_mounts[0].in_top_layer);
    assert_eq!(fs_mounts[0].bounds, [0.0, 0.0, 100.0, 200.0]);

    assert!(host.close_request());
    platform.settle(&[&host]);
    assert_eq!(
        host.adapter().log.lock().unwrap().exited_webview_fs,
        vec!["wv-1".to_owned()]
    );

    // Author-owned 70%-height `<dialog>` sheet moves `wv-1` to `[0, 60, 100, 140]` without remounting!
    host.automate(
        "[data-testid=\"open-sheet\"]",
        None,
        crate::interaction::view::AutomationAct::Click,
        false,
    )
    .unwrap();
    platform.settle(&[&host]);
    let sheet_mounts = host
        .adapter()
        .log
        .lock()
        .unwrap()
        .webviews
        .last()
        .cloned()
        .unwrap();
    assert_eq!(sheet_mounts.len(), 1);
    assert_eq!(sheet_mounts[0].web_id, "wv-1");
    assert!(sheet_mounts[0].in_top_layer);
    assert_eq!(sheet_mounts[0].bounds, [0.0, 60.0, 100.0, 140.0]);

    assert!(host.close_request());
    platform.settle(&[&host]);
    let back_mounts = host
        .adapter()
        .log
        .lock()
        .unwrap()
        .webviews
        .last()
        .cloned()
        .unwrap();
    assert_eq!(back_mounts[0].web_id, "wv-1");
    assert!(!back_mounts[0].in_top_layer);
    assert_eq!(back_mounts[0].bounds, [0.0, 0.0, 120.0, 80.0]);

    // Parking clears webview mounts.
    let cleared_before = host.adapter().log.lock().unwrap().cleared_webviews;
    host.park();
    platform.settle(&[&host]);
    assert_eq!(
        host.adapter().log.lock().unwrap().cleared_webviews,
        cleared_before + 1
    );
}

#[test]
fn a_surface_bound_after_a_stop_whose_start_never_came_shows_the_page_again() {
    let worker = ViewWorker::shared("test:stop-without-start").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render(){return h('text',{},'S')}"),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let presented = || host.adapter().log.lock().unwrap().presented.len();
    // Backgrounded: the window stops and its surface goes; the start is never delivered.
    host.window_visible(false);
    host.surface_lost();
    host.surface_changed();
    host.redraw();
    platform.settle(&[&host]);
    let stopped = presented();
    host.redraw();
    platform.settle(&[&host]);
    assert_eq!(presented(), stopped, "a resize alone does not end the stop");
    // Foregrounded: a new surface binds, which platforms do only for a showing window.
    host.surface_attached();
    host.redraw();
    platform.settle(&[&host]);
    assert!(presented() > stopped, "the page presents again");
}

/// Drags a built page 60 frames each way and reports the video opens it asked for. Needs the
/// bundles built: `LXV_DRAG_BUNDLES` lists their paths, `:`-separated.
#[cfg(feature = "gpu")]
#[test]
#[ignore]
fn dragging_a_built_page_opens_only_players_it_brings_into_view() {
    use crate::renderer::gpu::video::{open_nothing_real, opens_asked};
    open_nothing_real();
    let bundles = std::env::var("LXV_DRAG_BUNDLES").expect("LXV_DRAG_BUNDLES");
    for path in bundles.split(':') {
        let module = std::fs::read_to_string(path).unwrap();
        let worker = ViewWorker::spawn("test-drag-opens").unwrap();
        let platform = Platform::new(&[&worker]);
        let host = platform.host(&worker, &module, false);
        host.set_visible(true);
        platform.settle(&[&host]);
        let (before, keys) = opens_asked();
        let frames = host.adapter().log.lock().unwrap().presented.len();
        eprintln!(
            "{path}: {frames} frames at load; scrolls down: {} at the drag point: {}",
            host.can_scroll_vertically(1),
            host.can_scroll_vertically_at([100.0, 300.0], 1)
        );
        let pointer = |phase, y: f32| {
            host.pointer(Pointer {
                phase,
                x: 100.0,
                y,
                slop: 8.0,
                velocity: [0.0; 2],
                min_fling: 50.0,
            });
            run_frames(&platform, &host, 1);
        };
        // The fake display is 200 x 400 device pixels.
        for (from, step) in [(360.0, -5.0), (60.0, 5.0)] {
            pointer(PointerPhase::Down, from);
            for n in 1..=60 {
                pointer(PointerPhase::Move, from + step * n as f32);
            }
            pointer(PointerPhase::Up, from + step * 60.0);
            run_frames(&platform, &host, 30);
        }
        let (after, keys_after) = opens_asked();
        let moved = host.adapter().log.lock().unwrap().presented.len() - frames;
        eprintln!(
            "{path}: opens asked {before} after load, {} more over a 120-step drag ({moved} frames presented); players {keys:?} -> {keys_after:?}",
            after - before
        );
        assert!(moved >= 60, "{path}: the drag moved the page");
        // Only a player scrolled into view for the first time opens.
        let new = keys_after.iter().filter(|key| !keys.contains(key)).count() as u64;
        assert_eq!(after - before, new, "{path}: a drag asked for video opens");
    }
}

/// The device order: a test pin through `view.environment`, not a renderer field.
#[test]
fn a_pinned_text_scale_grows_a_select_chevron_and_its_room() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){const b=useLocal('b','q');return h('view',{style:{alignItems:'flex-start'}},h('select',{'data-testid':'s',bind:b,'aria-label':'Billing',style:{width:160,paddingLeft:12,paddingRight:12,paddingTop:8,paddingBottom:8,fontSize:14,borderWidth:1},options:[{value:'q',label:'Quarterly billing with the annual discount applied'},{value:'m',label:'Monthly'}]}))}",
        ),
        false,
    );
    // Wide enough that the select's end is on screen.
    if let Some(environment) = host.adapter().environment.lock().unwrap().as_mut() {
        (environment.width, environment.height) = (400 * 2, 400 * 2);
    }
    host.set_visible(true);
    platform.settle(&[&host]);
    let chevron = |host: &ViewHost<Fake>| {
        let log = host.adapter().log.lock().unwrap();
        let (points, width) = log.strokes.last().cloned().expect("a chevron");
        (points[2][0] - points[0][0], width)
    };
    assert_eq!(chevron(&host), (10.0, 1.75));
    let pinned = host.automation_request(&serde_json::json!({"op": "environment", "textScale": 2}));
    assert!(pinned.is_ok(), "{pinned:?}");
    platform.settle(&[&host]);
    run_frames(&platform, &host, 4);
    assert_eq!(chevron(&host), (15.0, 2.625), "1.5× at a 2.0 pin");
}

/// The field last in a page's content, on an iPhone 11 (414 x 896, bottom inset 34, keyboard
/// 318): whether the keyboard's height reaches the core before the focus or with it, the
/// field ends above the keyboard.
#[test]
fn a_last_field_clears_a_keyboard_that_arrives_before_or_with_the_focus() {
    for order in ["keyboard first", "same frame"] {
        let worker = ViewWorker::spawn("test-view").unwrap();
        let platform = Platform::new(&[&worker]);
        let host = platform.host(
            &worker,
            &source(
                "function render(){const v=useLocal('v','');return h('page',{},h('scroll',{style:{flexGrow:1,flexBasis:0}},h('view',{style:{padding:16,paddingBottom:40,gap:16}},h('view',{style:{height:700}}),h('input',{'data-testid':'f',bind:v,'aria-label':'Amount',inputMode:'decimal',style:{height:48}}))))}",
            ),
            false,
        );
        if let Some(environment) = host.adapter().environment.lock().unwrap().as_mut() {
            (environment.width, environment.height) = (414 * 2, 896 * 2);
            environment.value["insets"]["bottom"] = 34.into();
        }
        host.set_visible(true);
        platform.settle(&[&host]);
        let field = |host: &ViewHost<Fake>| {
            let rect = host
                .automation_request(
                    &serde_json::json!({"op": "query", "css": "[data-testid=\"f\"]"}),
                )
                .unwrap()["rect"]
                .clone();
            let at = |key: &str| rect[key].as_f64().unwrap() as f32;
            [at("left"), at("top"), at("width"), at("height")]
        };
        let [x, y, w, h] = field(&host);
        assert!(
            y + h < 896.0 - 34.0 && y + h > 896.0 - 318.0,
            "{order}: starts low: {y}"
        );
        let keyboard = |host: &ViewHost<Fake>| {
            if let Some(environment) = host.adapter().environment.lock().unwrap().as_mut() {
                environment.value["keyboard"]["height"] = 318.into();
            }
            host.environment_changed();
        };
        let focus = |host: &ViewHost<Fake>| {
            let current: serde_json::Value =
                serde_json::from_str(host.snapshot().unwrap().json()).unwrap();
            let identity = current["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|node| node["test_id"] == "f")
                .unwrap()["identity"]
                .clone();
            host.input(
                serde_json::json!({"op": "focus", "identity": identity, "generation": current["generation"]})
                    .to_string(),
                |_| {},
            );
        };
        if order == "keyboard first" {
            keyboard(&host);
            platform.settle(&[&host]);
            run_frames(&platform, &host, 2);
            tap(&host, (x + w / 2.0) * 2.0, (y + h / 2.0) * 2.0);
            platform.settle(&[&host]);
            focus(&host);
        } else {
            tap(&host, (x + w / 2.0) * 2.0, (y + h / 2.0) * 2.0);
            focus(&host);
            keyboard(&host);
        }
        // No redraw or input after the focus: the reveal must reach the screen on its own.
        platform.settle(&[&host]);
        run_frames(&platform, &host, 4);
        let [_, y, _, h] = field(&host);
        // The field clears the keyboard with room to spare, as native fields do.
        assert!(
            y + h <= 896.0 - 318.0 - 12.0 + 0.5,
            "{order}: the field clears the keyboard: {y}..{}",
            y + h
        );
        // Nothing clamps the revealed offset back.
        host.redraw();
        run_frames(&platform, &host, 2);
        assert_eq!(field(&host)[1], y, "{order}: the reveal stays");
    }
}

/// Gboard's two steps (the keyboard, then its suggestion strip) around a tap's focus: each
/// presented frame shows the field clear of the keyboard with no redraw or input, and a second
/// reveal starts from the first, never from the offset before the keyboard.
#[test]
fn a_field_stays_clear_of_a_keyboard_that_grows_in_two_steps() {
    {
        let worker = ViewWorker::spawn("test-view").unwrap();
        let platform = Platform::new(&[&worker]);
        let host = platform.host(
            &worker,
            &source(
                "function render(){const v=useLocal('v','');return h('page',{},h('scroll',{style:{flexGrow:1,flexBasis:0}},h('view',{style:{padding:16,paddingBottom:40,gap:16}},h('view',{style:{height:700}}),h('input',{'data-testid':'f',bind:v,'aria-label':'Amount',inputMode:'decimal',style:{height:48}}))))}",
            ),
            false,
        );
        if let Some(environment) = host.adapter().environment.lock().unwrap().as_mut() {
            (environment.width, environment.height) = (414 * 2, 896 * 2);
            environment.value["insets"]["bottom"] = 34.into();
        }
        host.set_visible(true);
        platform.settle(&[&host]);
        let field = |host: &ViewHost<Fake>| {
            let rect = host
                .automation_request(
                    &serde_json::json!({"op": "query", "css": "[data-testid=\"f\"]"}),
                )
                .unwrap()["rect"]
                .clone();
            rect["top"].as_f64().unwrap() as f32 + rect["height"].as_f64().unwrap() as f32
        };
        let keyboard = |host: &ViewHost<Fake>, height: f64| {
            if let Some(environment) = host.adapter().environment.lock().unwrap().as_mut() {
                environment.value["keyboard"]["height"] = height.into();
            }
            host.environment_changed();
        };
        keyboard(&host, 293.8);
        platform.settle(&[&host]);
        run_frames(&platform, &host, 2);
        let rect = host
            .automation_request(&serde_json::json!({"op": "query", "css": "[data-testid=\"f\"]"}))
            .unwrap()["rect"]
            .clone();
        let at = |key: &str| rect[key].as_f64().unwrap() as f32;
        tap(
            &host,
            (at("left") + at("width") / 2.0) * 2.0,
            (at("top") + at("height") / 2.0) * 2.0,
        );
        platform.settle(&[&host]);
        // The platform focuses the field it raised the keyboard for, as adapters do.
        let current: serde_json::Value =
            serde_json::from_str(host.snapshot().unwrap().json()).unwrap();
        let identity = current["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["test_id"] == "f")
            .unwrap()["identity"]
            .clone();
        host.input(
            serde_json::json!({"op": "focus", "identity": identity, "generation": current["generation"]})
                .to_string(),
            |_| {},
        );
        platform.settle(&[&host]);
        run_frames(&platform, &host, 4);
        let first = field(&host);
        assert!(
            first <= 896.0 - 293.8 - 12.0 + 0.5,
            "clear of the keyboard: {first}"
        );
        keyboard(&host, 337.8);
        platform.settle(&[&host]);
        run_frames(&platform, &host, 4);
        let second = field(&host);
        assert!(
            second <= 896.0 - 337.8 - 12.0 + 0.5 && second < first,
            "clear of the taller keyboard, moved from the first reveal: {first} -> {second}"
        );
        run_frames(&platform, &host, 4);
        assert_eq!(field(&host), second, "the reveal stays");
    }
}

/// Gboard's order on the Pixel: the keyboard reports a tall height, then its final one. The
/// field ends 12 above the final keyboard, not 12 above the first.
#[test]
fn a_revealed_field_settles_back_when_the_keyboard_shrinks() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){const v=useLocal('v','');return h('page',{},h('scroll',{style:{flexGrow:1,flexBasis:0}},h('view',{style:{padding:16,paddingBottom:40,gap:16}},h('view',{style:{height:700}}),h('input',{'data-testid':'f',bind:v,'aria-label':'Number',inputMode:'decimal',style:{height:48}}))))}",
        ),
        false,
    );
    if let Some(environment) = host.adapter().environment.lock().unwrap().as_mut() {
        (environment.width, environment.height) = (414 * 2, 896 * 2);
        environment.value["insets"]["bottom"] = 34.into();
    }
    host.set_visible(true);
    platform.settle(&[&host]);
    let field = |host: &ViewHost<Fake>| {
        let rect = host
            .automation_request(&serde_json::json!({"op": "query", "css": "[data-testid=\"f\"]"}))
            .unwrap()["rect"]
            .clone();
        rect["top"].as_f64().unwrap() as f32 + rect["height"].as_f64().unwrap() as f32
    };
    let keyboard = |host: &ViewHost<Fake>, height: f64| {
        if let Some(environment) = host.adapter().environment.lock().unwrap().as_mut() {
            environment.value["keyboard"]["height"] = height.into();
        }
        host.environment_changed();
    };
    let resting = field(&host);
    let current: serde_json::Value = serde_json::from_str(host.snapshot().unwrap().json()).unwrap();
    let identity = current["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["test_id"] == "f")
        .unwrap()["identity"]
        .clone();
    host.input(
        serde_json::json!({"op": "focus", "identity": identity, "generation": current["generation"]})
            .to_string(),
        |_| {},
    );
    platform.settle(&[&host]);
    keyboard(&host, 337.8);
    platform.settle(&[&host]);
    run_frames(&platform, &host, 4);
    let tall = field(&host);
    assert!(
        (tall - (896.0 - 337.8 - 12.0)).abs() < 1.0,
        "12 above the tall keyboard: {tall}"
    );
    keyboard(&host, 293.8);
    platform.settle(&[&host]);
    run_frames(&platform, &host, 4);
    let settled = field(&host);
    assert!(
        (settled - (896.0 - 293.8 - 12.0)).abs() < 1.0,
        "12 above the final keyboard, not {:.1}: {settled}",
        896.0 - 293.8 - settled
    );
    // A smaller keyboard still never scrolls the page back past where it rested.
    keyboard(&host, 40.0);
    platform.settle(&[&host]);
    run_frames(&platform, &host, 4);
    assert!(
        (field(&host) - resting).abs() < 1.0,
        "back where it rested, no further: {} vs {resting}",
        field(&host)
    );
}

/// The device order: a tap focuses the field, then the keyboard's final frame arrives.
/// Worker jobs and posted tasks only: no vsync, as when a platform request lands between frames.
fn drain_without_frames(platform: &Platform) {
    loop {
        for worker in &platform.workers {
            let (done, wait) = channel();
            worker.run_background(move |_| {
                let _ = done.send(());
            });
            wait.recv_timeout(Duration::from_secs(30)).unwrap();
        }
        let mut ran = false;
        while let Ok(task) = platform.tasks.try_recv() {
            task();
            ran = true;
        }
        if !ran {
            break;
        }
    }
}

#[test]
fn a_touch_on_a_filled_field_focuses_it() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    // Opaque field fills, as `bg-surface` draws them: a field's press must not cost its tap.
    let host = platform.host(
        &worker,
        &source(
            "function render(){const v=useLocal('v','');const a=useLocal('a','');const field={height:48,backgroundColor:'#ffffff'};return h('page',null,h('scroll',{style:{flexGrow:1}},h('view',{style:{padding:16,gap:16}},h('input',{'data-testid':'f',bind:v,'aria-label':'Code',inputMode:'numeric',style:field}),h('input',{'data-testid':'a',bind:a,type:'number','aria-label':'Amount',style:field}),h('input',{'data-testid':'n','aria-label':'Name',style:field}),h('view',{style:{height:40}}))))}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let request = |op: serde_json::Value| {
        let (tx, rx) = channel();
        host.input(op.to_string(), move |reply| {
            let _ = tx.send(reply);
        });
        drain_without_frames(&platform);
        rx.recv_timeout(Duration::from_secs(5)).unwrap()
    };
    for id in ["f", "a", "n"] {
        let css = format!("[data-testid=\"{id}\"]");
        let rect = host
            .automation_request(&serde_json::json!({"op": "query", "css": css}))
            .unwrap()["rect"]
            .clone();
        let at = |key: &str| rect[key].as_f64().unwrap() as f32;
        // Rects are in points; touches arrive in device pixels.
        let scale = host.snapshot().unwrap().scale;
        let [x, y] = [
            (at("left") + at("width") / 2.0) * scale,
            (at("top") + at("height") / 2.0) * scale,
        ];
        let pointer = |phase| Pointer {
            phase,
            x,
            y,
            slop: 8.0,
            velocity: [0.0; 2],
            min_fling: 50.0,
        };
        // The platform asks for the caret and the focus before any further frame.
        host.pointer(pointer(PointerPhase::Down));
        platform.settle(&[&host]);
        let editors = host.adapter().log.lock().unwrap().editors.len();
        host.pointer(pointer(PointerPhase::Up));
        drain_without_frames(&platform);
        let editor = host.adapter().log.lock().unwrap().editors[editors..]
            .last()
            .cloned();
        let Some(Dispatch::Editor {
            node,
            commit,
            point: Some(point),
            word,
            ..
        }) = editor
        else {
            panic!("the tap on {id} asks the input method to focus it: {editor:?}");
        };
        let snapshot: serde_json::Value =
            serde_json::from_str(host.snapshot().unwrap().json()).unwrap();
        let identity = snapshot["nodes"][node]["identity"].clone();
        let hit = request(
            serde_json::json!({"op": "hit_test", "identity": identity, "generation": commit, "point": point, "word": word}),
        );
        let hit: serde_json::Value =
            serde_json::from_str(&hit.unwrap_or_default()).unwrap_or_default();
        assert!(
            hit["selection"].is_array(),
            "the hit test on {id} answers: {hit}"
        );
        let focus =
            request(serde_json::json!({"op": "focus", "identity": identity, "generation": commit}));
        assert!(
            focus.is_some_and(|reply| reply != "null"),
            "the focus of {id} is granted"
        );
        let reconciled = host.adapter().log.lock().unwrap().reconciled.len();
        platform.settle(&[&host]);
        run_frames(&platform, &host, 4);
        let log = host.adapter().log.lock().unwrap();
        assert!(
            log.reconciled[reconciled..]
                .iter()
                .all(|focused| focused.as_deref() == identity.as_str()),
            "every frame after the focus of {id} keeps it: {:?}",
            &log.reconciled[reconciled..]
        );
        drop(log);
        let element = host
            .automation_request(&serde_json::json!({"op": "query", "css": css}))
            .unwrap();
        assert!(element["focused"] == true, "the touch focused {id}");
    }
}

#[test]
fn a_frame_built_before_a_tap_focus_lands_without_dropping_it() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render({env}){const v=useLocal('v','');return h('page',{'data-keyboard':String(env.keyboard.height)},h('view',{style:{padding:16}},h('input',{'data-testid':'f',bind:v,'aria-label':'Code',style:{height:48}})))}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let rect = host
        .automation_request(&serde_json::json!({"op": "query", "css": "[data-testid=\"f\"]"}))
        .unwrap()["rect"]
        .clone();
    let at = |key: &str| rect[key].as_f64().unwrap() as f32;
    // Rects are in points; touches arrive in device pixels.
    let scale = host.snapshot().unwrap().scale;
    let [x, y] = [
        (at("left") + at("width") / 2.0) * scale,
        (at("top") + at("height") / 2.0) * scale,
    ];
    let pointer = |phase| Pointer {
        phase,
        x,
        y,
        slop: 8.0,
        velocity: [0.0; 2],
        min_fling: 50.0,
    };
    host.pointer(pointer(PointerPhase::Down));
    platform.settle(&[&host]);
    host.pointer(pointer(PointerPhase::Up));
    drain_without_frames(&platform);
    let Some(Dispatch::Editor {
        node,
        commit,
        point: Some(point),
        ..
    }) = host.adapter().log.lock().unwrap().editors.last().cloned()
    else {
        panic!("the tap asks the input method to focus the field");
    };
    let snapshot: serde_json::Value =
        serde_json::from_str(host.snapshot().unwrap().json()).unwrap();
    let identity = snapshot["nodes"][node]["identity"].clone();
    let request = |op: serde_json::Value| {
        let (tx, rx) = channel();
        host.input(op.to_string(), move |reply| {
            let _ = tx.send(reply);
        });
        drain_without_frames(&platform);
        rx.recv_timeout(Duration::from_secs(5)).unwrap()
    };
    let hit = request(
        serde_json::json!({"op": "hit_test", "identity": identity, "generation": commit, "point": point, "word": false}),
    );
    assert!(
        hit.is_some_and(|hit| hit.contains("selection")),
        "the hit test answers"
    );
    // The keyboard starts to rise before the focus reply: that frame is built and handed to
    // the screen, which shows it only later.
    host.adapter()
        .defer
        .store(true, std::sync::atomic::Ordering::Release);
    if let Some(environment) = host.adapter().environment.lock().unwrap().as_mut() {
        environment.value["keyboard"]["height"] = 120.into();
    }
    host.environment_changed();
    drain_without_frames(&platform);
    host.vsync();
    drain_without_frames(&platform);
    let handed = *host
        .adapter()
        .log
        .lock()
        .unwrap()
        .handed
        .last()
        .expect("the keyboard's frame is handed");
    let focus =
        request(serde_json::json!({"op": "focus", "identity": identity, "generation": commit}));
    assert!(
        focus.is_some_and(|reply| reply != "null"),
        "the focus is granted"
    );
    // Only now does the frame built before the focus reach the screen.
    let reconciled = host.adapter().log.lock().unwrap().reconciled.len();
    host.deferred_presented(handed, true);
    host.adapter()
        .defer
        .store(false, std::sync::atomic::Ordering::Release);
    platform.settle(&[&host]);
    run_frames(&platform, &host, 4);
    let log = host.adapter().log.lock().unwrap();
    let after = &log.reconciled[reconciled..];
    assert!(
        !after.is_empty()
            && after
                .iter()
                .all(|focused| focused.as_deref() == identity.as_str()),
        "the input method never sees a frame without the field it just focused: {after:?}"
    );
}

#[test]
fn a_tapped_field_clears_a_keyboard_that_rises_after_the_focus() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    // As the showcase lays a page out: a growing scroll in the page, which reads the keyboard.
    let host = platform.host(
        &worker,
        &source(
            "function render({env}){const v=useLocal('v','');return h('page',{'data-keyboard':String(env.keyboard.height)},h('scroll',{style:{flexGrow:1}},h('view',{style:{padding:16,gap:16}},h('view',{style:{height:680}}),h('input',{'data-testid':'f',bind:v,'aria-label':'Amount',inputMode:'decimal',style:{height:48}}),h('view',{style:{height:40}}))))}",
        ),
        false,
    );
    if let Some(environment) = host.adapter().environment.lock().unwrap().as_mut() {
        (environment.width, environment.height) = (414 * 2, 896 * 2);
    }
    host.set_visible(true);
    platform.settle(&[&host]);
    let field = |host: &ViewHost<Fake>| {
        let rect = host
            .automation_request(&serde_json::json!({"op": "query", "css": "[data-testid=\"f\"]"}))
            .unwrap()["rect"]
            .clone();
        let at = |key: &str| rect[key].as_f64().unwrap() as f32;
        [at("left"), at("top"), at("width"), at("height")]
    };
    let [x, y, w, h] = field(&host);
    assert!(y + h < 896.0 && y > 896.0 - 318.0, "starts low: {y}");
    tap(&host, (x + w / 2.0) * 2.0, (y + h / 2.0) * 2.0);
    platform.settle(&[&host]);
    assert!(
        !host.adapter().log.lock().unwrap().editors.is_empty(),
        "the tap asked the input method to focus the field"
    );
    // The input method focuses it, as a platform answers that dispatch.
    let current: serde_json::Value = serde_json::from_str(host.snapshot().unwrap().json()).unwrap();
    let identity = current["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["test_id"] == "f")
        .unwrap()["identity"]
        .clone();
    let (tx, rx) = channel();
    host.input(
        serde_json::json!({"op": "focus", "identity": identity, "generation": current["generation"]})
            .to_string(),
        move |reply| {
            let _ = tx.send(reply);
        },
    );
    platform.settle(&[&host]);
    let reply = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(reply.is_some());
    host.redraw();
    platform.settle(&[&host]);
    run_frames(&platform, &host, 4);
    let element = host
        .automation_request(&serde_json::json!({"op": "query", "css": "[data-testid=\"f\"]"}))
        .unwrap();
    assert!(element["focused"] == true, "the tap focused the field");
    assert_eq!(
        element["selection"],
        serde_json::json!([0, 0]),
        "a caret in the empty field"
    );
    if let Some(environment) = host.adapter().environment.lock().unwrap().as_mut() {
        environment.value["keyboard"]["height"] = 318.into();
    }
    let reconciled = host.adapter().log.lock().unwrap().reconciled.len();
    host.environment_changed();
    platform.settle(&[&host]);
    run_frames(&platform, &host, 4);
    let [_, y, _, h] = field(&host);
    assert!(
        y + h <= 896.0 - 318.0,
        "the field clears the keyboard: {y}..{}",
        y + h
    );
    // The input method drops its editor, and the keyboard, on any frame without one.
    let log = host.adapter().log.lock().unwrap();
    assert!(
        log.reconciled[reconciled..]
            .iter()
            .all(|focused| focused.as_deref() == identity.as_str()),
        "every frame of the reveal keeps the field focused: {:?}",
        &log.reconciled[reconciled..]
    );
    drop(log);
    let element = host
        .automation_request(&serde_json::json!({"op": "query", "css": "[data-testid=\"f\"]"}))
        .unwrap();
    assert!(element["focused"] == true, "the reveal keeps the focus");
}

/// The showcase's own select, as a device draws it: a pinned text scale grows its chevron.
#[cfg(feature = "hot-swap")]
#[test]
fn a_showcase_select_chevron_grows_under_a_pinned_text_scale() {
    use crate::session::tests::hot_swap::showcase_page_module;
    let worker = ViewWorker::spawn("test-lab-chevron").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(&worker, &showcase_page_module("view-controls"), false);
    if let Some(environment) = host.adapter().environment.lock().unwrap().as_mut() {
        (environment.width, environment.height, environment.scale) = (393 * 2, 852 * 2, 2.0);
    }
    host.set_visible(true);
    platform.settle(&[&host]);
    host.automation_request(&serde_json::json!({"op": "act", "css": "[data-testid=\"lab-controls-billing\"]", "action": {"kind": "scroll"}}))
        .unwrap();
    platform.settle(&[&host]);
    let chevron = |host: &ViewHost<Fake>| {
        let rect = host
            .automation_request(&serde_json::json!({"op": "query", "css": "[data-testid=\"lab-controls-billing\"]"}))
            .unwrap()["rect"]
            .clone();
        let at = |key: &str| rect[key].as_f64().unwrap() as f32;
        let (left, top, right, bottom) = (at("left"), at("top"), at("right"), at("bottom"));
        let log = host.adapter().log.lock().unwrap();
        log.strokes
            .iter()
            .filter(|(points, _)| points.len() == 3)
            .find(|(points, _)| {
                points
                    .iter()
                    .all(|[x, y]| (left..=right).contains(x) && (top..=bottom).contains(y))
            })
            .map(|(points, width)| (points[2][0] - points[0][0], *width))
    };
    assert_eq!(chevron(&host), Some((10.0, 1.75)));
    host.automation_request(
        &serde_json::json!({"op": "environment", "textScale": 2, "still": true}),
    )
    .unwrap();
    platform.settle(&[&host]);
    // Larger text pushes the select down the page; bring it back on screen.
    host.automation_request(&serde_json::json!({"op": "act", "css": "[data-testid=\"lab-controls-billing\"]", "action": {"kind": "scroll"}}))
        .unwrap();
    platform.settle(&[&host]);
    run_frames(&platform, &host, 4);
    assert_eq!(chevron(&host), Some((15.0, 2.625)), "1.5× at a 2.0 pin");
}

/// The showcase's own amount field: focused, then the keyboard's final frame arrives.
#[cfg(feature = "hot-swap")]
#[test]
fn a_showcase_field_clears_a_keyboard_that_rises_after_the_focus() {
    use crate::session::tests::hot_swap::showcase_page_module;
    let worker = ViewWorker::spawn("test-lab-keyboard").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(&worker, &showcase_page_module("view-input"), false);
    if let Some(environment) = host.adapter().environment.lock().unwrap().as_mut() {
        (environment.width, environment.height, environment.scale) = (414 * 2, 896 * 2, 2.0);
    }
    host.set_visible(true);
    platform.settle(&[&host]);
    let act = |css: &str, kind: &str| {
        host.automation_request(
            &serde_json::json!({"op": "act", "css": css, "action": {"kind": kind, "force": true}}),
        )
        .unwrap();
        platform.settle(&[&host]);
    };
    let field = "[data-testid=\"lab-input-amount\"]";
    let rect = |host: &ViewHost<Fake>| {
        let rect = host
            .automation_request(&serde_json::json!({"op": "query", "css": field}))
            .unwrap()["rect"]
            .clone();
        (
            rect["top"].as_f64().unwrap() as f32,
            rect["bottom"].as_f64().unwrap() as f32,
        )
    };
    // Bring the field low on screen, where the keyboard will cover it.
    act(field, "scroll");
    let (top, _) = rect(&host);
    if top < 896.0 - 318.0 {
        host.automation_request(
            &serde_json::json!({"op": "act", "css": field, "action": {"kind": "scroll"}}),
        )
        .unwrap();
    }
    let current: serde_json::Value = serde_json::from_str(host.snapshot().unwrap().json()).unwrap();
    let identity = current["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["test_id"] == "lab-input-amount")
        .unwrap()["identity"]
        .clone();
    let (tx, rx) = channel();
    host.input(
        serde_json::json!({"op": "focus", "identity": identity, "generation": current["generation"]})
            .to_string(),
        move |reply| {
            let _ = tx.send(reply);
        },
    );
    platform.settle(&[&host]);
    assert!(rx.recv_timeout(Duration::from_secs(5)).unwrap().is_some());
    host.redraw();
    platform.settle(&[&host]);
    run_frames(&platform, &host, 4);
    let before = rect(&host);
    if let Some(environment) = host.adapter().environment.lock().unwrap().as_mut() {
        environment.value["keyboard"]["height"] = 318.into();
    }
    host.environment_changed();
    platform.settle(&[&host]);
    run_frames(&platform, &host, 4);
    let (_, bottom) = rect(&host);
    assert!(
        bottom <= 896.0 - 318.0,
        "the field clears the keyboard: {before:?} -> bottom {bottom}"
    );
}

/// Desktop Tab onto a field starts editing it as a click does: the input method focuses it, a
/// single-line field selects its text and a multiline one puts the caret at the end, and typed
/// text arrives. Shift+Tab goes back the same way. (`focusWithin` follows the core's editor
/// focus: `editor_focus.rs`.)
#[test]
fn tab_onto_a_field_starts_editing_it_as_a_click_does() {
    use crate::interaction::view::Key;
    let worker = ViewWorker::spawn("test-tab-editor").unwrap();
    let platform = Platform::new(&[&worker]);
    let module = source(
        "function render(){const a=useLocal('a','first'),b=useLocal('b','Ada'),c=useLocal('c','one\\ntwo');return h('view',{style:{gap:8}},h('input',{'data-testid':'a',bind:a}),h('input',{'data-testid':'b',bind:b}),h('textarea',{'data-testid':'c',bind:c,style:{height:80}}))}",
    );
    let host = platform.host(&worker, &module, false);
    host.set_visible(true);
    platform.settle(&[&host]);
    run_frames(&platform, &host, 2);
    let query = |id: &str| {
        host.automation_request(
            &serde_json::json!({"op": "query", "css": format!("[data-testid=\"{id}\"]")}),
        )
        .unwrap()
    };
    let request = |body: serde_json::Value| {
        let (tx, rx) = channel();
        host.input(body.to_string(), move |reply| {
            let _ = tx.send(reply);
        });
        platform.settle(&[&host]);
        let reply = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        host.request_frame();
        platform.settle(&[&host]);
        run_frames(&platform, &host, 4);
        reply
    };
    // The platform answers the editor dispatch by focusing it, as an input method does.
    let focus_dispatched = |id: &str| -> (String, u64) {
        let editor = host.adapter().log.lock().unwrap().editors.pop();
        let Some(Dispatch::Editor {
            node,
            point: None,
            keyboard: true,
            ..
        }) = editor
        else {
            panic!("Tab onto {id} asks the input method to focus it, without a point: {editor:?}");
        };
        let snapshot = host.snapshot().unwrap();
        let identity = snapshot.frame.nodes[node].identity.to_string();
        assert_eq!(snapshot.frame.nodes[node].test_id, id);
        let reply = request(serde_json::json!({
            "op": "focus", "identity": identity, "generation": snapshot.generation.get(),
        }))
        .expect("the focus is granted");
        let reply: serde_json::Value = serde_json::from_str(&reply).unwrap();
        (identity, reply["token"].as_u64().unwrap())
    };
    let leave = |identity: &str, token: u64| {
        request(serde_json::json!({"op": "blur", "identity": identity, "token": token}));
    };

    assert!(host.key(Key::Tab, false, false));
    platform.settle(&[&host]);
    let (a, token) = focus_dispatched("a");
    let field = query("a");
    assert_eq!(field["focused"], true);
    assert_eq!(
        field["selection"],
        serde_json::json!([0, 5]),
        "Tab selects a field's text"
    );

    leave(&a, token);
    assert!(host.key(Key::Tab, false, false));
    platform.settle(&[&host]);
    let (b, token) = focus_dispatched("b");
    assert_eq!(query("b")["selection"], serde_json::json!([0, 3]));
    request(serde_json::json!({
        "op": "edit", "identity": b, "token": token, "submit": false,
        "edits": [{"op": "replace", "text": "Grace", "cursor": 1, "composing": false}],
    }));
    assert_eq!(
        query("b")["value"],
        "Grace",
        "typing replaces the selected text"
    );

    leave(&b, token);
    assert!(host.key(Key::Tab, false, false));
    platform.settle(&[&host]);
    let (c, token) = focus_dispatched("c");
    assert_eq!(
        query("c")["selection"],
        serde_json::json!([7, 7]),
        "a multiline field takes the caret at its end"
    );

    leave(&c, token);
    assert!(host.key(Key::Tab, true, false));
    platform.settle(&[&host]);
    focus_dispatched("b");
    assert_eq!(
        query("b")["selection"],
        serde_json::json!([0, 5]),
        "Shift+Tab returns to the field and selects it"
    );
}

#[test]
fn tab_off_a_field_ends_its_editing_and_a_click_takes_focus_back() {
    use crate::interaction::view::Key;
    let worker = ViewWorker::spawn("test-tab-one-owner").unwrap();
    let platform = Platform::new(&[&worker]);
    let module = source(
        "function render(){const a=useLocal('a','secret'),log=useLocal('log','');const say=s=>()=>log.set(log.value+s+' ');return h('view',{style:{gap:8}},h('input',{'data-testid':'a',bind:a,'aria-label':'A',onFocus:say('a+'),onBlur:say('a-')}),h('button',{'data-testid':'go',onClick(){}},'Go'),h('text',{'data-testid':'log'},log.value||'-'))}",
    );
    let host = platform.host(&worker, &module, false);
    host.set_visible(true);
    platform.settle(&[&host]);
    run_frames(&platform, &host, 2);
    let settle = || {
        platform.settle(&[&host]);
        host.request_frame();
        platform.settle(&[&host]);
        run_frames(&platform, &host, 4);
    };
    let query = |id: &str| {
        host.automation_request(
            &serde_json::json!({"op": "query", "css": format!("[data-testid=\"{id}\"]")}),
        )
        .unwrap()
    };
    // The platform answers the editor dispatch by focusing it, as an input method does.
    let grant = || {
        let Some(Dispatch::Editor { node, .. }) = host.adapter().log.lock().unwrap().editors.pop()
        else {
            panic!("the editor dispatch reaches the input method");
        };
        let snapshot = host.snapshot().unwrap();
        let identity = snapshot.frame.nodes[node].identity.to_string();
        let (tx, rx) = channel();
        host.input(
            serde_json::json!({"op": "focus", "identity": identity, "generation": snapshot.generation.get()})
                .to_string(),
            move |reply| {
                let _ = tx.send(reply);
            },
        );
        platform.settle(&[&host]);
        rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
        settle();
    };

    assert!(host.key(Key::Tab, false, false));
    settle();
    grant();
    assert_eq!(
        (
            query("a")["focused"].clone(),
            query("go")["focused"].clone()
        ),
        (true.into(), false.into())
    );

    // Tab on to the button: the field stops editing, so focus has one owner.
    assert!(host.key(Key::Tab, false, false));
    settle();
    assert_eq!(query("go")["focused"], true);
    assert_eq!(
        query("a")["focused"],
        false,
        "the field left behind no longer edits"
    );
    assert_eq!(query("log")["text"], "a+ a- ");

    // A click on the field takes focus back from the button.
    host.automation_request(&serde_json::json!({"op": "act", "css": "[data-testid=\"a\"]", "action": {"kind": "click"}}))
        .unwrap();
    settle();
    grant();
    assert_eq!(query("a")["focused"], true, "the click focused the field");
    assert_eq!(
        query("go")["focused"],
        false,
        "the button gave up focus to the click"
    );
    assert_eq!(query("log")["text"], "a+ a- a+ ");
}

#[test]
fn one_tab_leaves_a_first_field_a_tap_focused() {
    use crate::interaction::view::Key;
    let worker = ViewWorker::spawn("test-tab-after-tap").unwrap();
    let platform = Platform::new(&[&worker]);
    let module = source(
        "function render(){const a=useLocal('a','');return h('view',{style:{gap:8}},h('input',{'data-testid':'a',bind:a,'aria-label':'Email'}),h('button',{'data-testid':'go',onClick(){}},'Go'))}",
    );
    let host = platform.host(&worker, &module, false);
    host.set_visible(true);
    platform.settle(&[&host]);
    run_frames(&platform, &host, 2);
    let settle = || {
        platform.settle(&[&host]);
        host.request_frame();
        platform.settle(&[&host]);
        run_frames(&platform, &host, 4);
    };
    let query = |id: &str| {
        host.automation_request(
            &serde_json::json!({"op": "query", "css": format!("[data-testid=\"{id}\"]")}),
        )
        .unwrap()
    };
    let rect = query("a")["rect"].clone();
    let edge = |key: &str| rect[key].as_f64().unwrap() as f32;
    let scale = host.snapshot().unwrap().scale;
    tap(
        &host,
        (edge("left") + edge("width") / 2.0) * scale,
        (edge("top") + edge("height") / 2.0) * scale,
    );
    settle();
    let Some(Dispatch::Editor { node, .. }) = host.adapter().log.lock().unwrap().editors.pop()
    else {
        panic!("the tap asks the input method to focus the field");
    };
    let snapshot = host.snapshot().unwrap();
    let identity = snapshot.frame.nodes[node].identity.to_string();
    let (tx, rx) = channel();
    host.input(
        serde_json::json!({"op": "focus", "identity": identity, "generation": snapshot.generation.get()})
            .to_string(),
        move |reply| {
            let _ = tx.send(reply);
        },
    );
    platform.settle(&[&host]);
    rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
    settle();
    assert_eq!(query("a")["focused"], true);

    assert!(host.key(Key::Tab, false, false));
    settle();
    assert_eq!(query("go")["focused"], true, "one Tab moves on");
    assert_eq!(query("a")["focused"], false);
}

#[path = "uncontrolled.rs"]
mod uncontrolled;
