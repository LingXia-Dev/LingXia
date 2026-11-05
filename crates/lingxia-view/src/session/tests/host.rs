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
use crate::session::tests::{bounded_jobs, source};
use image::ImageEncoder;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::ThreadId;
use std::time::Duration;

#[derive(Default)]
struct Log {
    prepared: Vec<ThreadId>,
    /// Display sizes of frames the fake target accepted.
    accepted: Vec<(u16, u16)>,
    presented: Vec<Vec<String>>,
    presented_at: Vec<std::time::Instant>,
    fetched: Vec<(u64, String)>,
    cancelled: Vec<u64>,
    events: Vec<AccessibilityEffect>,
    effects: Vec<Effect>,
    editors: Vec<Dispatch>,
    messages: Vec<(u64, String)>,
    recovery: Vec<Option<Failure>>,
    overdue: Vec<bool>,
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
    /// Generations the host reported presented (`HostEvents::presented`).
    shown: Vec<CommitTicket>,
    /// The feedback style each presented frame carried.
    feedback: Vec<serde_json::Value>,
    /// Deferred frames handed to the fake raster thread, in order.
    handed: Vec<CommitTicket>,
    debug: Vec<String>,
    released_transient: usize,
    /// Whether each prepared frame carried the HUD overlay.
    hud: Vec<bool>,
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
    fn reconcile(&self, _: &FrameSnapshot) {
        drop(self.main());
    }
    fn focus(&self, editor: &Dispatch, _: &FrameSnapshot) {
        self.main().editors.push(editor.clone());
    }
    fn activation(&self) {}
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
        let json: serde_json::Value = serde_json::from_str(&frame.json).unwrap();
        self.main().feedback.push(json["feedback"].clone());
        self.main().shown.push(frame.generation);
    }
    fn message(&self, connection: ConnectionTicket, message: &str) {
        self.main()
            .messages
            .push((connection.get(), message.into()));
    }
    fn restart(&self, _: ConnectionTicket) -> bool {
        self.restart
    }
    fn recovery(&self, failure: Option<&Failure>) {
        self.main().recovery.push(failure.cloned());
    }
    fn bootstrap_overdue(&self, overdue: bool) {
        self.main().overdue.push(overdue);
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
    fn text_menu(&self, menu: Option<&crate::session::host::TextMenu>) {
        self.main().menus.push(menu.cloned());
    }
    // Warnings may come from the worker, so they skip the main-thread record.
    fn warn(&self, message: &str) {
        self.log.lock().unwrap().warnings.push(message.to_owned());
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
                environment: Mutex::new(Some(FrameEnvironment {
                    width: 200,
                    height: 400,
                    scale: 2.0,
                    value: serde_json::json!({
                        "theme": "light",
                        "insets": {"top": 0, "right": 0, "bottom": 0, "left": 0},
                        "keyboard": {"height": 0},
                        "os": "test",
                        "formFactor": "phone",
                        "displayLanguage": "en",
                        "canDock": false,
                    }),
                })),
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

#[test]
fn a_fake_platform_hosts_frames_input_semantics_and_images() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){const n=useLocal('n',0);return h('column',{},h('button',{'data-testid':'add',onClick:()=>n.set(n.value+1),style:{height:60}},`Count ${n.value}`),n.value<2?h('image',{src:'lx://pic.png',alt:'Picture',style:{width:20,height:20}}):h('text',{},'done'))}",
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
            "function render(){const show=useLocal('show',true);return h('column',{},h('button',{onClick:()=>show.set(false),style:{height:60}},'hide'),show.value?h('image',{src:'lx://a.png',style:{width:20,height:20}}):h('text',{},'gone'))}",
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
        &source("function render(){return h('column',{},h('text',{},'A'),h('image',{src:'lx://slow.png',alt:'',style:{width:20,height:20}}))}"),
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
fn a_stalled_image_decode_never_holds_up_another_page_on_the_worker() {
    let worker = ViewWorker::shared("test:decode").unwrap();
    let platform = Platform::new(&[&worker]);
    let photo = platform.host(
        &worker,
        &source("function render(){return h('column',{},h('text',{},'A'),h('image',{src:'lx://photo.png',alt:'',style:{width:20,height:20}}))}"),
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
        &source("function render(){const s=[0,1,2,3,4].map(i=>useLocal('s'+i,'-'));return h('column',{},h('text',{},s.map(x=>x.value).join('')),...s.map((x,i)=>h('image',{src:'lx://big'+i+'.png',alt:'',onLoad:()=>x.set('L'),onError:()=>x.set('E'),style:{position:'absolute',top:0,left:0,width:2048,height:2048}})))}"),
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
            "function render({env}){const n=useLocal('n',0),l=useLocal('l','-');return h('column',{},h('button',{'data-testid':'look',onClick:()=>n.set(n.value+1),style:{width:100,height:40}},`${env.theme} ${n.value} ${l.value}`),h('image',{src:'lx://look.png',alt:'',onLoad:()=>l.set('L'),style:{width:10,height:10}}))}",
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
fn connections_say_hello_and_report_an_overdue_bootstrap() {
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
    host.connect(connection, "nonce".into());
    platform.settle(&[&host]);
    let hello: serde_json::Value =
        serde_json::from_str(&host.adapter().log.lock().unwrap().messages[0].1).unwrap();
    assert_eq!(hello["kind"], "hello");
    assert_eq!(hello["nonce"], "nonce");
    assert!(
        host.adapter().log.lock().unwrap().presented.is_empty(),
        "a Logic page waits for ready and state"
    );
    platform.advance(Duration::from_secs(5));
    assert_eq!(
        host.adapter().log.lock().unwrap().overdue.last(),
        Some(&true)
    );
    assert!(
        host.adapter()
            .log
            .lock()
            .unwrap()
            .warnings
            .iter()
            .any(|w| w.starts_with("UI_BOOTSTRAP_TIMEOUT:"))
    );
    host.receive(
        connection,
        serde_json::json!({"v":2,"kind":"helloAck","nonce":"nonce","protocol":2,"sessionId":"s"})
            .to_string(),
    );
    host.receive(
        connection,
        serde_json::json!({"v":2,"kind":"state.snapshot","rev":0,"state":{"count":3}}).to_string(),
    );
    host.receive(
        connection,
        serde_json::json!({"v":2,"kind":"ready","sessionId":"s"}).to_string(),
    );
    platform.settle(&[&host]);
    if bounded_jobs() {
        assert_eq!(last(&host), ["count 3"]);
        assert_eq!(
            host.adapter().log.lock().unwrap().overdue.last(),
            Some(&false)
        );
        // A superseded connection's frames are ignored.
        host.connect(ConnectionTicket::new(2), "next".into());
        host.receive(
            connection,
            serde_json::json!({"v":2,"kind":"state.snapshot","rev":1,"state":{"count":4}})
                .to_string(),
        );
        platform.settle(&[&host]);
        assert_eq!(last(&host), ["count 3"]);
    } else {
        // Engines without bounded jobs refuse Logic actions; the failure is recoverable chrome.
        let failure = host.failure().unwrap();
        assert!(failure.error.contains("UI_ACTION"), "{}", failure.error);
    }
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
    let json: serde_json::Value = serde_json::from_str(&snapshot.json).unwrap();
    json["metrics"]["animationFrames"].as_u64().unwrap()
}

#[test]
fn a_spinner_presents_at_display_rate_only_while_visible_and_present() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(&format!(
            "function render(){{const on=useLocal('on',true);return h('column',{{}},h('button',{{onClick:()=>on.set(false),style:{{height:60}}}},'stop'),on.value?h('column',{{style:{{width:20,height:20,backgroundColor:'#ff0000',animation:{SPIN}}}}}):null)}}"
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
            "function render(){{return h('column',{{}},h('text',{{}},'spin'),h('column',{{style:{{width:20,height:20,backgroundColor:'#ff0000',animation:{SPIN}}}}}))}}"
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
            "function render(){{const n=useLocal('n','none');return h('column',{{}},h('button',{{onClick:()=>{{measureFrames(500).then(s=>n.set(s.windowMs+':'+(s.samples>=25)+':'+(s.frameTimeMs.p50<17)))}},style:{{height:60}}}},'measured '+n.value),h('column',{{style:{{width:20,height:20,backgroundColor:'#ff0000',animation:{SPIN}}}}}))}}"
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
            "function render(){{return h('column',{{style:{{width:20,height:20,backgroundColor:'#ff0000',animation:{fade}}}}})}}"
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
            "function render(){{return h('column',{{style:{{width:20,height:20,backgroundColor:'#ff0000',animation:{SPIN}}}}})}}"
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
fn a_frame_past_its_budget_is_dropped_without_recovery_and_counted() {
    let worker = ViewWorker::spawn("test-budget-overrun").unwrap();
    let platform = Platform::new(&[&worker]);
    let budget = crate::session::eval_budget().as_millis();
    // The page's second render runs two budgets once; every other render is quick.
    let host = platform.host(
        &worker,
        &source(&format!(
            "let n=0;function render(){{n++;if(n===2){{const t=Date.now();while(Date.now()-t<{}){{}}}}return h('text',{{}},'frame '+n)}}",
            budget * 2
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
    host.connect(connection, "nonce".into());
    for frame in [
        serde_json::json!({"v":2,"kind":"helloAck","nonce":"nonce","protocol":2,"sessionId":"s"}),
        serde_json::json!({"v":2,"kind":"state.snapshot","rev":0,"state":{"count":1}}),
        serde_json::json!({"v":2,"kind":"ready","sessionId":"s"}),
    ] {
        host.receive(connection, frame.to_string());
    }
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
    host.receive(
        connection,
        serde_json::json!({"v":2,"kind":"state.snapshot","rev":1,"state":{"count":3}}).to_string(),
    );
    host.receive(
        connection,
        serde_json::json!({"v":2,"kind":"res","id":request["id"],"ok":true,"result":{"count":3}})
            .to_string(),
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
    host.receive(
        connection,
        serde_json::json!({"v":2,"kind":"res","id":id,"ok":false,"error":{"code":"E_LIMIT","message":"too many"}}).to_string(),
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
        "function render(){const name=useLocal('name',''),plan=useLocal('plan','');return h('column',{},h('input',{'data-testid':'name',bind:name,'aria-label':'Name'}),h('select',{'data-testid':'plan',bind:plan,'aria-label':'Plan',options:[{value:'a',label:'A'},{value:'b',label:'B'}]}),h('text',{'data-testid':'out'},`name=${name.value} plan=${plan.value}`))}",
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
        host.connect(connection, "nonce".into());
        for frame in [
            serde_json::json!({"v":2,"kind":"helloAck","nonce":"nonce","protocol":2,"sessionId":"s"}),
            serde_json::json!({"v":2,"kind":"state.snapshot","rev":0,"state":{"n":0}}),
            serde_json::json!({"v":2,"kind":"ready","sessionId":"s"}),
        ] {
            host.receive(connection, frame.to_string());
        }
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
    let snapshot: serde_json::Value = serde_json::from_str(&host.snapshot().unwrap().json).unwrap();
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
        &host.snapshot().unwrap().json,
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
    host.connect(connection, "nonce".into());
    for frame in [
        serde_json::json!({"v":2,"kind":"helloAck","nonce":"nonce","protocol":2,"sessionId":"s"}),
        serde_json::json!({"v":2,"kind":"state.snapshot","rev":0,"state":{"n":0}}),
        serde_json::json!({"v":2,"kind":"ready","sessionId":"s"}),
    ] {
        host.receive(connection, frame.to_string());
    }
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
        host.receive(connection, serde_json::json!({"v":2,"kind":"req","id":id,"method":method,"params":params,"cap":"page"}).to_string());
        platform.settle(&[&host]);
    };
    let query = |css: &str| {
        host.automation_request(&serde_json::json!({"op":"query","css":css}))
            .unwrap()
    };
    // A method the GPU View does not serve is refused at once.
    call("lv_1", "ui.unknown", serde_json::json!({}));
    let refused = replies()
        .into_iter()
        .find(|r| r["id"] == "lv_1")
        .expect("answered");
    assert_eq!(
        (refused["ok"].as_bool(), refused["error"]["code"].as_str()),
        (Some(false), Some("E_UNSUPPORTED"))
    );
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
    let dialog = query("[role=\"dialog\"]");
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
    host.connect(connection, "nonce".into());
    for frame in [
        serde_json::json!({"v":2,"kind":"helloAck","nonce":"nonce","protocol":2,"sessionId":"s"}),
        serde_json::json!({"v":2,"kind":"state.snapshot","rev":0,"state":{"n":0}}),
        serde_json::json!({"v":2,"kind":"ready","sessionId":"s"}),
    ] {
        host.receive(connection, frame.to_string());
    }
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
fn a_parked_page_reads_blank_and_refuses_actions() {
    let worker = ViewWorker::spawn("test-automation-parked").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){const n=useLocal('n',0);return h('column',{'data-testid':'card','data-mode':'live'},h('text',{},'Total'),h('button',{'data-testid':'add',onClick:()=>n.set(n.value+1)},`Count ${n.value}`))}",
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
    let snapshot: serde_json::Value = serde_json::from_str(&host.snapshot().unwrap().json).unwrap();
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
    host.set_text_selection(identity.clone(), 0, 5);
    platform.settle(&[&host]);
    let shown = menu(&host).unwrap();
    assert_eq!(
        shown.target,
        MenuTarget::Text {
            identity: identity.clone()
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
            identity: identity.clone()
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
            "function render(){const a=useLocal('a','');const b=useLocal('b','');return h('column',{},h('input',{'data-testid':'amount',bind:a,autoFocus:true}),h('input',{'data-testid':'code',bind:b}))}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let fill = |id: &str, text: &str| {
        let request = serde_json::json!({"op": "act", "css": format!("[data-testid=\"{id}\"]"), "action": {"kind": "fill", "text": text}});
        host.automation_request(&request)
    };
    // As on a cold start: the second editor is still connecting when its edit arrives.
    host.adapter()
        .edit_outcomes
        .lock()
        .unwrap()
        .extend([EditOutcome::Applied, EditOutcome::NotReady]);
    fill("amount", "12").unwrap();
    fill("code", "ABC").unwrap();
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
    let request = serde_json::json!({"op": "act", "css": "[data-testid=\"amount\"]", "action": {"kind": "fill", "text": "7"}});
    let failed = host.automation_request(&request).unwrap_err();
    assert!(failed.contains("refused the edit"), "{failed}");
    assert!(
        host.accessibility(id, AccessibilityAction::SetText("9".into())),
        "a later edit goes through"
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
        "function render(){return h('column',{},h('button',{'data-testid':'b',onClick(){},className:'active:x',style:{width:100,height:40,padding:0,transition:{properties:['transform'],durationMs:150,delayMs:0,easing:'linear'}}},'Press'))}",
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
fn a_deferred_present_holds_the_next_frame_until_it_reaches_the_screen() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(&format!(
            "function render(){{return h('column',{{style:{{width:20,height:20,backgroundColor:'#ff0000',animation:{SPIN}}}}})}}"
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
            "function render(){{const n=useLocal('n',0);return h('column',{{}},h('button',{{onClick:()=>n.set(n.value+1),style:{{height:60}}}},`Count ${{n.value}}`),h('column',{{style:{{width:20,height:20,backgroundColor:'#ff0000',animation:{SPIN}}}}}))}}"
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

fn costly_spinner(platform: &Platform, worker: &Arc<ViewWorker>, ahead: bool) -> ViewHost<Fake> {
    // The button darkens while pressed, as the lab's buttons do: pressing relays the page out.
    let page = source(&format!(
        "function render(){{const n=useLocal('n',0);return h('column',{{}},h('button',{{onClick:()=>n.set(n.value+1),className:'active:x',style:{{height:60}}}},`Count ${{n.value}}`),h('column',{{style:{{width:20,height:20,backgroundColor:'#ff0000',animation:{SPIN}}}}}))}}"
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
    let json: serde_json::Value = serde_json::from_str(&host.snapshot().unwrap().json).unwrap();
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
            "function render(){{return h('column',{{style:{{width:20,height:20,backgroundColor:'#ff0000',animation:{SPIN}}}}})}}"
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
        h('button',{'data-testid':'go',onClick(){}},h('row',{},h('text',{},'Go'),h('checkbox',{'data-testid':'remember',bind:on,'aria-label':'Remember'}))),
        h('scroll',{'data-testid':'inner',style:{height:20,overflowX:'auto'}},h('row',{style:{width:900,height:20}})))
    }";
    let host = platform.host(&worker, &source(page), false);
    host.set_visible(true);
    platform.settle(&[&host]);
    let snapshot: serde_json::Value = serde_json::from_str(&host.snapshot().unwrap().json).unwrap();
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
    let current: serde_json::Value = serde_json::from_str(&host.snapshot().unwrap().json).unwrap();
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
    let generation = serde_json::from_str::<serde_json::Value>(&host.snapshot().unwrap().json)
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
    assert!(
        lingxia_theme::app_pack(key).is_none(),
        "the pack ends with its app session"
    );
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
            "function render(){const n=useLocal('n',0);return h('column',{},n.value===0?h('button',{'data-testid':'b',onClick:()=>n.set(1),style:{height:60}},'Remove me'):h('text',{},'gone'))}",
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
    let json: serde_json::Value = serde_json::from_str(&host.snapshot().unwrap().json).unwrap();
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
    let json: serde_json::Value = serde_json::from_str(&host.snapshot().unwrap().json).unwrap();
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
    let json: serde_json::Value = serde_json::from_str(&host.snapshot().unwrap().json).unwrap();
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
fn a_hosted_view_lab_keeps_a_nested_local_across_a_sibling_edit() {
    use crate::session::tests::hot_swap::view_lab_module_edited;
    let worker = ViewWorker::spawn("test-lab-swap").unwrap();
    let platform = Platform::new(&[&worker]);
    let module = view_lab_module_edited(None);
    let host = platform.host(&worker, &module, false);
    // A phone-sized page, so the tabs and the plan picker are on screen.
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
        host.automation_request(&serde_json::json!({"op": "query", "css": "[data-testid=\"lab-input-values\"]", "maxText": 400}))
            .unwrap()["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    };
    act(
        "[data-testid=\"lab-tab-input\"]",
        serde_json::json!({"kind": "click", "force": true}),
    );
    act(
        "[data-testid=\"lab-input-plan\"]",
        serde_json::json!({"kind": "select", "value": "yearly", "force": true}),
    );
    assert!(values().ends_with("plan=yearly"), "{}", values());
    let edited = view_lab_module_edited(Some((
        "Monthly bills</text>",
        "Monthly bills and more</text>\n      <text className=\"text-xs\">Edited</text>",
    )));
    host.swap_module(edited);
    platform.settle(&[&host]);
    assert!(
        values().ends_with("plan=yearly"),
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
      return h('column',{},
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
            serde_json::from_str(&host.snapshot().unwrap().json).unwrap();
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
        return h('column',{}, on.value && h('text',{'data-testid':'banner'},'Hi'), h(Counter,{}),
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
      return h('column',{},
        h('button',{'data-testid':'row',onClick(){rows.set(rows.value+1)},style:{height:60,width:190}},
          h('row',{},h('text',{'data-testid':'label'},'Wi'),
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
            serde_json::from_str(&host.snapshot().unwrap().json).unwrap();
        let node = snapshot["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["test_id"] == id)
            .unwrap()
            .clone();
        let b = |i: usize| node["bounds"][i].as_f64().unwrap() as f32;
        tap(&host, (b(0) + b(2) / 2.0) * 2.0, (b(1) + b(3) / 2.0) * 2.0);
        platform.settle(&[&host]);
    };
    // Only the innermost control shows the press.
    let snapshot: serde_json::Value = serde_json::from_str(&host.snapshot().unwrap().json).unwrap();
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
    assert_eq!(
        pressed,
        [index("wifi")],
        "the switch is pressed, not the row"
    );
    host.pointer(Pointer {
        phase: PointerPhase::Cancel,
        x: 0.0,
        y: 0.0,
        slop: 8.0,
        velocity: [0.0; 2],
        min_fling: 50.0,
    });
    platform.settle(&[&host]);
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
        "function render(){return h('column',{style:{alignItems:'flex-start'}},h('text',{'data-testid':'t',style:{fontFamily:'Wide, serif',fontSize:10}},'abc'))}",
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
            "function render(){const query=useLocal('query',''),picked=useLocal('picked','');const rows=Array.from({length:40},(_,i)=>({id:`row-${i}`,label:`Row ${i}`})).filter(r=>r.label.includes(query.value));return h('column',{},h('input',{type:'search','data-testid':'filter',bind:query,'aria-label':'Filter'}),h(List,{'data-testid':'list',style:{height:120},items:rows,getKey:r=>r.id,renderItem:r=>h('button',{'data-testid':r.id,onClick:()=>picked.set(r.label),style:{height:40}},r.label)}),h('text',{'data-testid':'picked'},`picked=${picked.value}`))}",
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
            "function render(){const day=useLocal('2026-10-29'),submits=useLocal(0);return h('column',{},h('input',{'data-testid':'day',type:'date',bind:day,'aria-label':'Day',min:'2026-10-01',onSubmit:()=>submits.set(submits.value+1)}),h('text',{'data-testid':'out'},`${day.value} ${submits.value}`))}",
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
            "function render(){const day=useLocal('2026-10-21'),at=useLocal('18:30');return h('column',{},h('input',{'data-testid':'day',type:'date',bind:day,'aria-label':'Day'}),h('input',{'data-testid':'at',type:'time',bind:at,step:900,'aria-label':'At'}),h('text',{'data-testid':'out'},`${day.value} ${at.value}`))}",
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
            "function render(){return h('scroll',{'data-testid':'s',style:{height:100,scrollSnapType:'y'}},...Array.from({length:10},(_,i)=>h('column',{key:String(i),'data-testid':'r'+i,style:{height:40,scrollSnapAlign:'start'}})))}",
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
