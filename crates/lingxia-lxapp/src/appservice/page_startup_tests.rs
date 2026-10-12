//! Drive production page creation and the production Logic message handler
//! against a native UI substitute with an adversarial, repeatable schedule.

use super::*;
use crate::appservice::LxAppWorkers;
use crate::lxapp::{AppSessionClass, config::LxAppLogicEntry};
use crate::page::startup_test_host::{Registration, TestHost};
use crate::page::{PageInstance, PageWebViewAwait};
use rong::{JSEngine, RongJS};
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::thread::JoinHandle;

const PATH: &str = "pages/home/index.html";
const PROBE: &str = "__startup_test_observation__";

struct Fixture {
    app: Arc<LxApp>,
    host: Arc<TestHost>,
    observations: mpsc::Receiver<HashMap<String, Value>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    creates: Arc<AtomicUsize>,
    _registration: Registration,
    _root: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let platform = Arc::new(
            lingxia_platform::Platform::new(
                root.path().join("data").display().to_string(),
                root.path().join("cache").display().to_string(),
                "en-US".to_string(),
            )
            .unwrap(),
        );
        let (workers, messages) = LxAppWorkers::manual_for_test(Duration::from_secs(3));
        let appid = format!("app.lingxia.page-startup.{}", uuid::Uuid::new_v4());
        crate::lxapp::register_synthetic_lxapp(&appid);
        let app = Arc::new(
            LxApp::new_with_session_class_for_test(
                appid.clone(),
                platform,
                workers,
                AppSessionClass::StandardApp,
            )
            .unwrap(),
        );
        app.bind_arc();
        app.config().logic = Some(LxAppLogicEntry::Enabled(true));
        let (host, registration) = TestHost::install(&appid);
        let (observation_tx, observations) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(0);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let worker_app = app.clone();
        let creates = Arc::new(AtomicUsize::new(0));
        let worker_creates = creates.clone();
        let worker = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let local = tokio::task::LocalSet::new();
            local.block_on(&rt, async move {
                let runtime = RongJS::runtime();
                let ctx = runtime.context();
                register_app_ctx(&ctx, &worker_app);
                app::init(&ctx).unwrap();
                page::init(&ctx).unwrap();
                rong_modules::init(&ctx, RONG_MODULES).unwrap();
                ctx.eval::<()>(Source::from_bytes(format!(
                    "__registerPage({PATH:?}, {{ data: {{ startupMarker: 'logic-state-ready' }} }});"
                ))).unwrap();
                let mut current = Some(ctx.clone());
                ready_tx.send(()).unwrap();
                while !worker_stop.load(Ordering::SeqCst) {
                    let message = match messages.recv_timeout(Duration::from_millis(10)) {
                        Ok(message) => message,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    };
                    let observe = matches!(&message, ServiceMessage::TerminatePage {
                        page_instance_id: Some(id), ..
                    } if id == PROBE);
                    if matches!(&message, ServiceMessage::CreatePage { .. }) {
                        worker_creates.fetch_add(1, Ordering::SeqCst);
                    }
                    lxapp_service_handler(0, runtime.clone(), message, &mut current).await;
                    if observe {
                        let pages = with_page_svc_map(&ctx, |map| Ok(map.borrow().clone())).unwrap();
                        let mut snapshots = HashMap::new();
                        for (id, svc) in pages {
                            let snapshot = svc.get_state_snapshot(Some("page")).await.unwrap();
                            snapshots.insert(id, serde_json::from_str(&snapshot).unwrap());
                        }
                        observation_tx.send(snapshots).unwrap();
                    }
                    tokio::task::yield_now().await;
                }
                shutdown_app_context(&ctx).await;
            });
        });
        ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        Self {
            app,
            host,
            observations,
            stop,
            worker: Some(worker),
            creates,
            _registration: registration,
            _root: root,
        }
    }

    fn snapshots(&self) -> HashMap<String, Value> {
        // A no-op termination is a FIFO barrier through the real worker queue.
        self.app
            .executor
            .terminate_page_svc(self.app.clone(), PATH.to_string(), Some(PROBE.to_string()))
            .unwrap();
        self.observations
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
    }

    fn pending_page(&self, isolated: bool) -> PageInstance {
        let page = PageInstance::new_with_isolation(
            self.app.appid.clone(),
            PATH.to_string(),
            &self.app,
            isolated,
        );
        self.app
            .state
            .lock()
            .unwrap()
            .pages_by_id
            .lock()
            .unwrap()
            .insert(page.instance_id_string(), page.clone());
        page
    }
}

fn ready_outcome(page: &PageInstance) -> mpsc::Receiver<PageWebViewAwait> {
    let (tx, rx) = mpsc::channel();
    page.on_webview_ready(move |outcome| tx.send(outcome).unwrap());
    rx
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.worker.take().unwrap().join().unwrap();
    }
}

#[test]
fn page_startup_eager_worker_delivers_initial_state() {
    let mut missing = 0;
    let mut skipped_creation = 0;
    let trials = 100;
    for _ in 0..trials {
        let f = Fixture::new();
        let page = f.app.get_or_create_page(PATH);
        let first = f.host.outcomes.lock().unwrap()[0].clone();
        if first
            .as_ref()
            .is_err_and(|e| e.contains("page instance disposed"))
        {
            skipped_creation += 1;
        }
        // Reload after startup must retain a PageSvc and its initial state.
        page.ensure_live_webview();
        let snapshots = f.snapshots();
        let initial = snapshots.get(&page.instance_id_string());
        assert_eq!(f.host.html_loads.load(Ordering::SeqCst), 1);
        if let Some(initial) = initial {
            assert_eq!(
                initial["state"],
                json!({"startupMarker": "logic-state-ready"})
            );
        } else {
            missing += 1;
        }
    }
    eprintln!(
        "page_startup: trials={trials}, skipped_creation={skipped_creation}, HTML_without_PageSvc={missing}"
    );
    assert_eq!(
        missing, 0,
        "HTML loaded but the actual Logic registry has no PageSvc or initial snapshot"
    );
}

#[test]
fn page_startup_navigation_entry_registers_before_worker_runs() {
    let f = Fixture::new();
    let page = f.app.create_page_for_entry(PATH);
    assert_eq!(&*f.host.outcomes.lock().unwrap(), &[Ok(())]);
    assert_eq!(
        f.snapshots()[&page.instance_id_string()]["state"],
        json!({"startupMarker": "logic-state-ready"})
    );
}

#[test]
fn page_startup_failed_logic_cannot_reload_html_without_a_service() {
    let f = Fixture::new();
    // Real JS registration failure, not a synthesized setup error.
    let page = f.app.get_or_create_page("pages/missing.html");
    assert!(f.host.outcomes.lock().unwrap()[0].is_err());
    assert!(f.app.get_page_by_instance_id(&page.instance_id()).is_none());
    assert!(
        !page.is_discarded(),
        "failed startup is not a reclaimed live page"
    );
    page.ensure_live_webview();
    assert_eq!(f.host.outcomes.lock().unwrap().len(), 1);
    assert_eq!(f.host.html_loads.load(Ordering::SeqCst), 0);
    assert!(f.snapshots().is_empty());
}

#[test]
fn page_startup_reclaimed_webview_preserves_its_logic_service() {
    let f = Fixture::new();
    let page = f.app.get_or_create_page(PATH);
    let original = f.snapshots();
    assert_eq!(original.len(), 1);
    f.app.mark_hidden();
    page.discard_webview().unwrap();
    assert!(page.is_discarded());
    page.ensure_live_webview();
    assert_eq!(f.snapshots(), original);
    assert_eq!(f.creates.load(Ordering::SeqCst), 1);
    assert_eq!(f.host.html_loads.load(Ordering::SeqCst), 2);
}

#[test]
fn page_startup_concurrent_route_candidates_start_only_the_winner() {
    let f = Fixture::new();
    *f.host.allocation_barrier.lock().unwrap() = Some(Arc::new(std::sync::Barrier::new(2)));
    let a = f.app.clone();
    let b = f.app.clone();
    let a = std::thread::spawn(move || a.get_or_create_page(PATH));
    let b = std::thread::spawn(move || b.get_or_create_page(PATH));
    let a = a.join().unwrap();
    let b = b.join().unwrap();
    assert_eq!(a.instance_id(), b.instance_id());
    assert_eq!(f.host.outcomes.lock().unwrap().len(), 1);
    assert_eq!(f.creates.load(Ordering::SeqCst), 1);
    assert_eq!(f.snapshots().len(), 1);
}

#[test]
fn page_startup_native_failure_reports_failed_after_removal() {
    let f = Fixture::new();
    let page = f.pending_page(false);
    let outcome = ready_outcome(&page);
    *f.host.native_create_error.lock().unwrap() = Some("native creation failed".to_string());
    page.start_webview(|_| async { panic!("native failure must skip setup") });
    assert!(f.app.get_page_by_instance_id(&page.instance_id()).is_none());
    let late_outcome = ready_outcome(&page);
    drop(page);
    for outcome in [outcome, late_outcome] {
        assert!(matches!(
            outcome.recv_timeout(Duration::from_secs(5)).unwrap(),
            PageWebViewAwait::Failed(error) if error == "native creation failed"
        ));
    }
    assert_eq!(f.creates.load(Ordering::SeqCst), 0);
}

#[test]
fn page_startup_disposed_before_native_failure_reports_gone() {
    let f = Fixture::new();
    let page = f.pending_page(false);
    let outcome = ready_outcome(&page);
    f.app
        .dispose_page_instance(&page.instance_id(), crate::CloseReason::Programmatic)
        .unwrap();
    *f.host.native_create_error.lock().unwrap() = Some("native creation cancelled".to_string());
    page.start_webview(|_| async { panic!("native failure must skip setup") });
    assert!(matches!(
        outcome.recv_timeout(Duration::from_secs(5)).unwrap(),
        PageWebViewAwait::Gone
    ));
}

#[test]
fn page_startup_isolated_failure_closes_its_hosting_surface() {
    use crate::lxapp::SurfaceRecord;
    use lingxia_platform::traits::ui::{SurfaceContentKind, SurfaceKind, WindowChrome};

    let f = Fixture::new();
    let page = f.pending_page(true);
    let surface = SurfaceRecord {
        chrome: WindowChrome::default(),
        owner_page_instance_id: Some("owner".to_string()),
        content_page_instance_id: Some(page.instance_id_string()),
        content: SurfaceContentKind::Page,
        target: PATH.to_string(),
        kind: SurfaceKind::Overlay,
        role: lingxia_surface::Role::Float,
        url_callback: false,
        ephemeral_web_data: false,
    };
    {
        let state = f.app.state.lock().unwrap();
        let mut surfaces = state.surfaces.lock().unwrap();
        surfaces.insert("hosting".to_string(), surface.clone());
        surfaces.insert(
            "unrelated".to_string(),
            SurfaceRecord {
                content_page_instance_id: Some("another-page".to_string()),
                ..surface
            },
        );
    }
    page.start_webview(|_| async { Err("initial setup failed".to_string()) });
    assert!(f.app.get_page_by_instance_id(&page.instance_id()).is_none());
    assert_eq!(
        *f.host.closed_surfaces.lock().unwrap(),
        [("hosting".to_string(), "programmatic".to_string())]
    );
}

#[test]
fn page_startup_headless_failed_reload_retains_service_and_pin() {
    let f = Fixture::new();
    let page = f.app.ensure_headless_page_service(PATH).unwrap();
    assert!(matches!(
        ready_outcome(&page)
            .recv_timeout(Duration::from_secs(5))
            .unwrap(),
        PageWebViewAwait::Ready(_)
    ));
    let original = f.snapshots();
    assert_eq!(original.len(), 1);
    f.app.mark_hidden();
    page.discard_webview().unwrap();
    *f.host.html_error.lock().unwrap() = Some("document reload failed".to_string());
    page.ensure_live_webview();
    assert!(f.host.outcomes.lock().unwrap()[0].is_err());
    assert!(f.app.get_page_by_instance_id(&page.instance_id()).is_some());
    assert!(page.is_discarded());
    assert_eq!(f.snapshots(), original);
    assert_eq!(
        f.app
            .state
            .lock()
            .unwrap()
            .path_pins
            .lock()
            .unwrap()
            .get(PATH),
        Some(&page.instance_id_string())
    );
    *f.host.html_error.lock().unwrap() = None;
    page.ensure_live_webview();
    assert_eq!(f.snapshots(), original);
    assert_eq!(f.creates.load(Ordering::SeqCst), 1);
}
