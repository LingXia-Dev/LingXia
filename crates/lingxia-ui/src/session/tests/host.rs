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
    loaded: usize,
    clicked: usize,
    vsync: bool,
    carets: Vec<bool>,
    /// `hold_surface` of each reveal.
    revealed: Vec<bool>,
    copied: Vec<String>,
    /// Edits offered to the input method and what it answered.
    edits: Vec<(Effect, EditOutcome)>,
    menus: Vec<Option<crate::session::host::TextMenu>>,
    /// Threads that ran main-thread callbacks.
    main: Vec<ThreadId>,
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
}

impl Presenter for Fake {
    type Frame = Encoded;
    fn prepare(
        &self,
        display: DisplayList,
        _: SurfaceTicket,
        _: CommitTicket,
    ) -> Result<Option<Encoded>, String> {
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
        }))
    }
    fn present(&self, frame: Encoded) -> Result<bool, String> {
        let mut log = self.main();
        log.carets.push(frame.caret);
        log.presented.push(frame.texts);
        log.presented_at.push(std::time::Instant::now());
        Ok(true)
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
    fn presented(&self, _: &FrameSnapshot, _: &FrameTiming) {}
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
    fn text_menu(&self, menu: Option<&crate::session::host::TextMenu>) {
        self.main().menus.push(menu.cloned());
    }
    fn warn(&self, _: &str) {}
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
                        "runner": false,
                    }),
                })),
                restart: false,
                pending: Default::default(),
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
            "function render(){const n=useLocal('n',0);return h('column',{},h('button',{testId:'add',onClick:()=>n.set(n.value+1),style:{height:60}},`Count ${n.value}`),n.value<2?h('image',{src:'lx://pic.png',alt:'Picture',style:{width:20,height:20}}):h('text',{},'done'))}",
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
fn hiding_a_page_drops_its_text_selection_from_the_kept_frame() {
    let worker = ViewWorker::shared("test:hide-selection").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){return h('text',{testId:'p',style:{userSelect:'text',fontSize:16}},'hello brave world')}",
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

/// Advance the fake clock by `frames` display frames, firing due wakeups each frame.
fn run_frames(platform: &Platform, host: &ViewHost<Fake>, frames: u32) {
    let frame = Duration::from_micros(16_667);
    for _ in 0..frames {
        *host.adapter().clock.lock().unwrap() += frame;
        platform.advance(frame);
        platform.settle(&[host]);
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
            "function render(){const n=useLocal('n',0);return h('button',{testId:'add',onClick:()=>n.set(n.value+1)},`Count ${n.value}`)}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let query = serde_json::json!({"op": "query", "css": "[data-testid=\"add\"]"});
    let found = host.automation_request(&query).unwrap();
    assert!(found.to_string().contains("Count 0"), "{found}");
    let click = serde_json::json!({"op": "act", "css": "[data-testid=\"add\"]", "action": {"kind": "click"}});
    assert_eq!(
        host.automation_request(&click).unwrap(),
        serde_json::Value::Null
    );
    platform.settle(&[&host]);
    assert_eq!(last(&host), ["Count 1"]);
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
fn accessibility_selects_and_copies_static_text_through_the_session_selection() {
    use crate::interaction::view::AccessibilityAction;
    let worker = ViewWorker::spawn("test-static-copy").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render(){return h('text',{testId:'p',style:{userSelect:'text'}},'hello brave world')}"),
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
    assert_eq!(
        snapshot["feedback"],
        serde_json::to_value(FeedbackStyle::for_theme(false)).unwrap()
    );
    assert_eq!(snapshot["feedback"]["pressOverlay"], 0x2200_0000u32);
    let dark = FeedbackStyle::for_theme(true);
    assert_ne!(dark.focus_ring, FeedbackStyle::for_theme(false).focus_ring);
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
        &source("function render(){return h('text',{testId:'p',style:{userSelect:'text'}},'hello brave world')}"),
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
            "function render(){const a=useLocal('a','');const b=useLocal('b','');return h('column',{},h('input',{testId:'amount',bind:a,autoFocus:true}),h('input',{testId:'code',bind:b}))}",
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
        &source("function render(){const a=useLocal('a','');return h('input',{testId:'amount',bind:a})}"),
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
