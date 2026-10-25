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
    presented: Vec<Vec<String>>,
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
            },
            worker.clone(),
            logic,
        );
        host.load(page.into());
        host
    }

    /// Run posted work and vsyncs until the worker and every host are quiet.
    fn settle(&self, hosts: &[&ViewHost<Fake>]) {
        loop {
            // The worker is FIFO: once a marker runs, earlier jobs have posted their results.
            for worker in &self.workers {
                let (done, wait) = channel();
                worker.run(move |_| {
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
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(&[255, 0, 0, 255], 1, 1, image::ExtendedColorType::Rgba8)
        .unwrap();
    bytes
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
        &source("function render({data}){return h('text',{},`count ${data.count}`)}"),
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
