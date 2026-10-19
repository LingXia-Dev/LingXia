use super::log_store::{DevLogSession, create_session};
use crate::config::AppEnv;
use anyhow::{Context, Result, anyhow};
use lingxia_control_protocol::{
    ControlRequest,
    dev_session::{
        DEV_SESSION_MAX_MESSAGE_BYTES, DEV_SESSION_PROTOCOL_VERSION, DevSessionEvent,
        DevSessionMessage, DevSessionRole,
    },
};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tungstenite::handshake::server::{Request, Response};
use tungstenite::protocol::Message;
use tungstenite::{Error as WsError, WebSocket, accept_hdr};

const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
const COMMAND_TIMEOUT_BUFFER: Duration = Duration::from_secs(5);
const RUNTIME_EVENT_QUEUE_CAPACITY: usize = 64;
const DEV_LXAPP_HTTP_PREFIX: &str = "/__lingxia/dev/lxapp/";
/// Mirrors the Runner's automation controller lease: a test run nobody has
/// polled for this long no longer holds back file-watch reloads.
const TEST_RUN_LEASE: Duration = Duration::from_secs(180);
/// Desktop/Runner: after the runtime has connected once, a disconnect that is
/// not replaced within this window ends the session. Closing the window is
/// the stop signal — the user should not need `lingxia dev stop`.
/// Mobile keeps waiting: swiping the app away is not the end of a device session.
#[cfg_attr(test, allow(dead_code))]
const RUNTIME_GONE_GRACE: Duration = Duration::from_secs(1);
/// The same window while a test run is active: a runtime that drops its
/// connection mid-run (a transient socket loss, a busy host) gets this long
/// to come back before the session — and the run with it — ends.
#[cfg_attr(test, allow(dead_code))]
const RUNTIME_GONE_GRACE_DURING_RUN: Duration = Duration::from_secs(30);
/// Error code a `session.test.poll` gets while its run's runtime is away
/// but may still reconnect.
pub(crate) const RUNTIME_RECONNECTING: &str = "runtime_reconnecting";

pub struct DevServerHandle {
    session: DevLogSession,
    ws_addr: SocketAddr,
    stop_flag: Arc<AtomicBool>,
    server_thread: Option<JoinHandle<()>>,
    companion: Option<super::companion::DevCompanion>,
    state: Arc<DevServerState>,
    watch: Option<super::lxapp_watch::LxAppWatch>,
}

impl DevServerHandle {
    /// Connectable ws URL for same-machine peers. A physical-device server
    /// may bind `0.0.0.0`, which is not a destination address, so render it as
    /// loopback for local `lxdev`.
    pub fn ws_url(&self) -> String {
        if self.ws_addr.ip().is_unspecified() {
            format!("ws://127.0.0.1:{}", self.ws_addr.port())
        } else {
            format!("ws://{}", self.ws_addr)
        }
    }

    pub fn port(&self) -> u16 {
        self.ws_addr.port()
    }

    pub fn session(&self) -> &DevLogSession {
        &self.session
    }

    pub fn runtime_connected(&self) -> bool {
        self.state.runtime_sender().is_some()
    }

    /// Build `app_id` once and publish it, before anything is watched.
    pub fn rebuild_lxapp(
        &self,
        app_id: &str,
        framework: Option<&str>,
        release: bool,
    ) -> Result<()> {
        self.state.rebuild_lxapp(app_id, framework, release)
    }

    /// Keep `app_id` open in the app; see [`DevServerState::present`].
    pub fn present(&self, app_id: &str) -> Result<()> {
        self.state.present(app_id)
    }

    /// Watch local lxapp sources and rebuild + reload them on save.
    /// The watcher thread starts even when nothing is watchable yet, so a
    /// standalone lxapp can attach later. A watcher failure is reported and
    /// ignored so the session still runs.
    pub fn watch_lxapps(&mut self, framework: Option<&str>, release: bool) {
        if self.watch.is_some() {
            return;
        }
        match super::lxapp_watch::LxAppWatch::spawn(
            self.state.clone(),
            super::lxapp_watch::LxAppWatchOptions {
                framework: framework.map(ToOwned::to_owned),
                release,
            },
        ) {
            Ok(watch) => self.watch = watch,
            Err(err) => eprintln!("⚠ lxapp auto-reload not started ({err:#})"),
        }
    }

    pub fn stop(mut self) -> Result<()> {
        self.stop_flag.store(true, Ordering::Release);
        if let Some(watch) = self.watch.take() {
            watch.join();
        }
        let result = if let Some(thread) = self.server_thread.take() {
            thread
                .join()
                .map_err(|_| anyhow!("dev server thread panicked"))
        } else {
            Ok(())
        };
        super::companion::finish(result, self.companion.as_ref())
    }
}

pub(super) struct SessionLogWriter {
    log: Mutex<super::log_store::RotatingLog>,
}

impl SessionLogWriter {
    pub(super) fn new(session: &DevLogSession) -> Result<Self> {
        Ok(Self {
            log: Mutex::new(super::log_store::RotatingLog::open(
                &session.log_file,
                lingxia_control_protocol::dev_session::log_files::MAX_LOG_BYTES,
            )?),
        })
    }

    pub(super) fn append_events(&self, events: &[DevSessionEvent]) -> Result<()> {
        let mut lines = Vec::new();
        for event in events {
            serde_json::to_writer(&mut lines, event).context("Failed to encode event line")?;
            lines.push(b'\n');
        }
        self.log
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .append(&lines)
    }
}

#[path = "companion_relay.rs"]
mod companion_relay;

pub(crate) struct DevServerState {
    pub(crate) project_root: PathBuf,
    pub(crate) stop_flag: Arc<AtomicBool>,
    runtime_sender: Mutex<Option<(u64, Sender<DevSessionMessage>)>>,
    next_runtime_id: AtomicU64,
    pending_results: Mutex<std::collections::HashMap<String, Sender<DevSessionMessage>>>,
    command_lock: Mutex<()>,
    /// Held while an lxapp is rebuilt, and by a relayed `session.test` start
    /// until it is answered. A start may move the app onto an isolated
    /// profile, which closes and reopens it from its build output; a rebuild
    /// empties that output while it runs, so the two must not overlap.
    build_lock: Mutex<()>,
    /// When set (non-loopback bind), every peer must present this token in
    /// its `Hello` and HTTP requests must carry it as `?token=`.
    auth_token: Option<String>,
    /// A runtime peer failed token auth since the last successful runtime
    /// connection — almost always a physical-device host binary that predates
    /// session tokens. Folded into the "runtime is not connected" client error so
    /// the operator sees the cause on their side of the wire.
    runtime_rejected: AtomicBool,
    /// True for Runner / macOS / Windows: the launched UI *is* the session.
    end_on_runtime_gone: bool,
    /// Bumped on connect and on disconnect so an in-flight gone-timer is cancelled
    /// when a replacement runtime attaches during the grace window.
    runtime_epoch: AtomicU64,
    /// The `session.test` run relayed through this server that has not yet
    /// been seen to finish, with the time it was last polled.
    active_test_run: Mutex<Option<(String, Instant)>>,
    /// The active run whose runtime disconnected, with the disconnect time;
    /// restored as active if the runtime reconnects within the grace.
    interrupted_test_run: Mutex<Option<(String, Instant)>>,
    /// `session.watch.pause` leases holding the source watcher.
    watch_leases: Mutex<std::collections::HashMap<String, WatchLease>>,
    /// The build the connected runtime reported in its hello.
    runtime_build: Mutex<Option<lingxia_control_protocol::dev_session::PeerBuild>>,
    /// The session's companion, when one runs.
    companion: std::sync::OnceLock<Arc<super::companion::CompanionLink>>,
    /// Scenario owners with `function` rules installed in the companion.
    companion_owners: Mutex<std::collections::HashSet<String>>,
    /// Orders run preflight, explicit scenario changes and terminal cleanup.
    companion_scenario_lock: Mutex<()>,
    /// Mock selection owners the companion holds (`dev`, `test:<run>`).
    companion_mock_owners: Mutex<std::collections::HashSet<String>>,
    /// Orders mock reset/selection changes and terminal cleanup.
    companion_mock_lock: Mutex<()>,
    /// Owners whose cleanup was rejected or timed out. A later test must not
    /// run against companion state whose precedence is unknown.
    companion_cleanup_failed: Mutex<std::collections::HashSet<String>>,
    /// Each watched lxapp's last valid `mocks/`.
    mocks: Mutex<Vec<super::mocks::AppMocks>>,
    /// `--mock all` startup is held on a failed push until a reload succeeds.
    mocks_gated: AtomicBool,
    /// Lxapps a standalone `lingxia dev listen` attached to this session.
    attached: Mutex<Vec<AttachedLxapp>>,
    /// Commands into the watcher thread. `None` until it is running.
    watch_commands: Mutex<Option<mpsc::Sender<super::lxapp_watch::WatchCmd>>>,
    /// `--framework` this session was started with. An attach that omits one
    /// builds with it.
    watch_framework: Mutex<Option<String>>,
    /// `--release` this session was started with; an attach inherits it.
    watch_release: AtomicBool,
    /// Secret an attach must present. It is in a file only this user can
    /// read: a loopback peer may be any process on a USB-reversed device.
    attach_token: Mutex<Option<String>>,
    /// Lxapps this session keeps open: the one `listen --port` develops, and
    /// each attached one.
    presented: Mutex<Vec<Presented>>,
    /// Serializes opening presented lxapps, from an attach and a reconnect.
    present_lock: Mutex<()>,
    /// App ids rebuilt while the runtime could not restart them. A reconnect
    /// restarts them.
    stale: Mutex<std::collections::BTreeSet<String>>,
}

#[derive(Clone)]
struct Presented {
    app_id: String,
    /// Opened since it was presented. The first open restarts it, so an
    /// instance left from earlier shows the new build.
    shown: bool,
}

/// An lxapp this session builds and serves from outside `lingxia.yaml`.
#[derive(Clone)]
struct AttachedLxapp {
    app_id: String,
    source: PathBuf,
    framework: Option<String>,
    release: bool,
}

/// Removes the attach when the listen connection ends, including on panic
/// after the lxapp was registered.
struct DetachOnDrop<'a> {
    state: &'a DevServerState,
    app_id: String,
}

impl Drop for DetachOnDrop<'_> {
    fn drop(&mut self) {
        self.state.detach_attached(&self.app_id);
    }
}

/// One `session.watch.pause` lease.
#[derive(Debug, Clone)]
struct WatchLease {
    expires: Instant,
    ttl: Duration,
    /// A run whose terminal state releases the lease.
    run_id: Option<String>,
}

/// Longest lease a client may ask for; it renews, it does not park.
const MAX_WATCH_LEASE: Duration = Duration::from_secs(600);

impl DevServerState {
    fn new(
        project_root: PathBuf,
        stop_flag: Arc<AtomicBool>,
        auth_token: Option<String>,
        end_on_runtime_gone: bool,
    ) -> Self {
        Self {
            project_root,
            stop_flag,
            runtime_sender: Mutex::new(None),
            next_runtime_id: AtomicU64::new(1),
            pending_results: Mutex::new(std::collections::HashMap::new()),
            command_lock: Mutex::new(()),
            build_lock: Mutex::new(()),
            auth_token,
            runtime_rejected: AtomicBool::new(false),
            end_on_runtime_gone,
            runtime_epoch: AtomicU64::new(0),
            active_test_run: Mutex::new(None),
            interrupted_test_run: Mutex::new(None),
            watch_leases: Mutex::new(std::collections::HashMap::new()),
            runtime_build: Mutex::new(None),
            companion: std::sync::OnceLock::new(),
            companion_owners: Mutex::new(std::collections::HashSet::new()),
            companion_scenario_lock: Mutex::new(()),
            companion_mock_owners: Mutex::new(std::collections::HashSet::new()),
            companion_mock_lock: Mutex::new(()),
            companion_cleanup_failed: Mutex::new(std::collections::HashSet::new()),
            mocks: Mutex::new(Vec::new()),
            mocks_gated: AtomicBool::new(false),
            attached: Mutex::new(Vec::new()),
            watch_commands: Mutex::new(None),
            watch_framework: Mutex::new(None),
            watch_release: AtomicBool::new(false),
            attach_token: Mutex::new(None),
            presented: Mutex::new(Vec::new()),
            present_lock: Mutex::new(()),
            stale: Mutex::new(std::collections::BTreeSet::new()),
        }
    }

    fn lock_attached(&self) -> std::sync::MutexGuard<'_, Vec<AttachedLxapp>> {
        self.attached
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(super) fn set_watch_commands(&self, sender: mpsc::Sender<super::lxapp_watch::WatchCmd>) {
        *self
            .watch_commands
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(sender);
    }

    pub(super) fn clear_watch_commands(&self) {
        *self
            .watch_commands
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    }

    fn watch_sender(&self) -> Option<mpsc::Sender<super::lxapp_watch::WatchCmd>> {
        self.watch_commands
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub(super) fn set_watch_build(&self, framework: Option<String>, release: bool) {
        *self
            .watch_framework
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = framework;
        self.watch_release.store(release, Ordering::Release);
    }

    fn attach_authorized(&self, presented: Option<&str>) -> bool {
        let expected = self
            .attach_token
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        expected.is_some() && expected.as_deref() == presented
    }

    fn watch_framework(&self) -> Option<String> {
        self.watch_framework
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn attached_spec(&self, app_id: &str) -> Option<AttachedLxapp> {
        self.lock_attached()
            .iter()
            .find(|item| item.app_id == app_id)
            .cloned()
    }

    fn attached_dist(&self, app_id: &str) -> Option<PathBuf> {
        self.attached_spec(app_id)
            .map(|item| item.source.join("dist"))
    }

    fn insert_attached(&self, spec: AttachedLxapp) -> Result<()> {
        let mut attached = self.lock_attached();
        if attached.iter().any(|item| item.app_id == spec.app_id) {
            return Err(anyhow!(
                "{} is already attached. Stop the other `lingxia dev listen` first.",
                spec.app_id
            ));
        }
        attached.push(spec);
        Ok(())
    }

    fn detach_attached(&self, app_id: &str) {
        self.lock_attached().retain(|item| item.app_id != app_id);
        self.lock_presented().retain(|item| item.app_id != app_id);
        let _ = super::lxapp_manifest::remove_manifest(&self.project_root, app_id);
        if let Some(sender) = self.watch_sender() {
            let _ = sender.send(super::lxapp_watch::WatchCmd::Remove(app_id.to_string()));
        }
    }

    fn lock_mocks(&self) -> std::sync::MutexGuard<'_, Vec<super::mocks::AppMocks>> {
        self.mocks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Bundle and validate every watched lxapp's `mocks/` and print the
    /// `Mock:` line. An invalid `mocks/` fails the session start.
    pub(crate) fn prepare_mocks(&self) -> Result<()> {
        let baseline = super::mocks::baseline();
        let roots = super::lxapp_watch::discover_watch_roots(&self.project_root)?;
        let mut loaded = Vec::new();
        let mut lines = Vec::new();
        for root in &roots {
            match super::mocks::load_app(&root.app_id, &root.path)
                .with_context(|| format!("lxapp {} ({})", root.app_id, root.path.display()))?
            {
                Some(app) => {
                    lines.push((root.app_id.clone(), app.summary(baseline)));
                    loaded.push(app);
                }
                None => {
                    lines.push((root.app_id.clone(), super::mocks::no_mocks_line(baseline)));
                }
            }
        }
        let functions = self.companion_mock_summary();
        for (app_id, line) in &lines {
            let prefix = if lines.len() > 1 {
                format!("{app_id}: ")
            } else {
                String::new()
            };
            let functions = functions
                .as_deref()
                .map(|text| format!(" · {text}"))
                .unwrap_or_default();
            println!("  Mock:     {prefix}{line}{functions}");
        }
        *self.lock_mocks() = loaded;
        Ok(())
    }

    /// `functions 7/12 mocked` when the companion switches mocks.
    fn companion_mock_summary(&self) -> Option<String> {
        let link = self.companion.get()?;
        if !link.supports(lingxia_control_protocol::dev_session::capabilities::MOCK) {
            return None;
        }
        let status = link
            .request(
                lingxia_control_protocol::mock::companion::STATUS,
                Some(serde_json::json!({})),
                Duration::from_secs(15),
            )
            .ok()??;
        let status: lingxia_control_protocol::mock::companion::StatusResult =
            serde_json::from_value(status).ok()?;
        Some(format!(
            "functions {}/{} mocked",
            status.mocked, status.total
        ))
    }

    /// Send one request to the connected runtime and wait for its answer.
    fn runtime_request(
        &self,
        method: &str,
        params: serde_json::Value,
        timeout: Duration,
    ) -> Result<serde_json::Value> {
        let sender = self
            .runtime_sender()
            .ok_or_else(|| anyhow!("the runtime is not connected"))?;
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = mpsc::channel();
        self.register_pending_result(id.clone(), tx);
        let request = DevSessionMessage::Request(ControlRequest {
            id: id.clone(),
            method: method.to_string(),
            params: Some(params),
        });
        if sender.send(request).is_err() {
            let _ = self.take_pending_result(&id);
            return Err(anyhow!("the runtime disconnected"));
        }
        match rx.recv_timeout(timeout) {
            Ok(DevSessionMessage::Response(response)) => match response.error {
                Some(error) => Err(anyhow!("{}", error.message)),
                None => Ok(response.result.unwrap_or_default()),
            },
            Ok(_) => Err(anyhow!("unexpected response to {method}")),
            Err(_) => {
                let _ = self.take_pending_result(&id);
                Err(anyhow!("{method} got no answer"))
            }
        }
    }

    /// Push one lxapp's mocks to the runtime. Kept well under the runtime's
    /// 30 s startup idle window, which each load re-arms.
    fn push_mocks(&self, app: &super::mocks::AppMocks) -> Result<serde_json::Value> {
        self.runtime_request(
            lingxia_control_protocol::methods::session::network::MOCK_LOAD,
            app.load_params(super::mocks::baseline()),
            Duration::from_secs(10),
        )
    }

    /// Release Logic startup once every app's mocks have settled. Under
    /// `--mock all`, an incomplete load must keep startup gated.
    fn push_all_mocks(&self) {
        let apps = self.lock_mocks().clone();
        let mut failed = false;
        for app in apps {
            if let Err(error) = self.push_mocks(&app) {
                failed = true;
                eprintln!(
                    "[lingxia dev] mocks of {} not loaded into the runtime: {error:#}",
                    app.app_id
                );
            }
        }
        if failed && super::mocks::baseline() == Some(lingxia_control_protocol::mock::MockMode::All)
        {
            eprintln!(
                "[lingxia dev] mock initialization failed; Logic stays gated under --mock all until mocks/ reloads cleanly"
            );
            self.mocks_gated.store(true, Ordering::SeqCst);
            return;
        }
        self.mocks_gated.store(false, Ordering::SeqCst);
        if let Err(error) = self.runtime_request(
            lingxia_control_protocol::methods::session::network::MOCK_READY,
            serde_json::json!({ "baseline": super::mocks::baseline() }),
            Duration::from_secs(10),
        ) {
            eprintln!("[lingxia dev] mock initialization was not acknowledged: {error:#}");
        }
    }

    /// A save under an lxapp's `mocks/`: bundle it again and push it. An
    /// invalid save keeps the last valid mocks answering. Never rebuilds
    /// or restarts the app.
    pub(crate) fn reload_mocks(&self, app_id: &str, root: &Path) {
        let previous = self
            .lock_mocks()
            .iter()
            .find(|app| app.app_id == app_id)
            .map(|app| app.bundle.keys.len());
        let loaded = match super::mocks::load_app(app_id, root) {
            Ok(Some(app)) => app,
            Ok(None) => {
                self.unload_mocks(app_id);
                return;
            }
            Err(error) => {
                let kept = match previous {
                    Some(count) if count > 0 => format!(
                        "the last valid mocks ({count} handler{}) keep answering",
                        if count == 1 { "" } else { "s" }
                    ),
                    _ => "no mock handlers are loaded".to_string(),
                };
                eprintln!("  ✗ {error:#}; {kept}");
                return;
            }
        };
        let handlers = loaded.bundle.keys.len();
        let result = if self.runtime_sender().is_some() {
            Some(self.push_mocks(&loaded))
        } else {
            None
        };
        match result {
            Some(Err(error)) => {
                eprintln!("  ✗ mocks of {app_id} rejected by the runtime — {error:#}");
                return;
            }
            Some(Ok(_)) => println!(
                "  ✓ mocks reloaded ({handlers} handler{}), state reset",
                if handlers == 1 { "" } else { "s" }
            ),
            None => println!(
                "  ✓ mocks bundled ({handlers} handler{}; runtime not connected yet)",
                if handlers == 1 { "" } else { "s" }
            ),
        }
        {
            let mut mocks = self.lock_mocks();
            mocks.retain(|app| app.app_id != app_id);
            mocks.push(loaded);
        }
        if result.is_some() && self.mocks_gated.load(Ordering::SeqCst) {
            self.push_all_mocks();
        }
    }

    /// `mocks/` (or its `index.ts`) is gone: the app has no mocks from now
    /// on, in the runtime and in what the session reports.
    fn unload_mocks(&self, app_id: &str) {
        let pushed = self.runtime_sender().is_some().then(|| {
            self.runtime_request(
                lingxia_control_protocol::methods::session::network::MOCK_UNLOAD,
                serde_json::json!({ "appid": app_id }),
                Duration::from_secs(30),
            )
        });
        if !matches!(&pushed, Some(Err(_))) {
            self.lock_mocks().retain(|app| app.app_id != app_id);
        }
        match pushed {
            Some(Err(error)) => eprintln!(
                "  ✗ mocks of {app_id} removed, but the runtime did not drop them — {error:#}"
            ),
            _ => println!(
                "  ✓ {app_id}: mocks/ removed; mock: {}",
                super::mocks::no_mocks_line(super::mocks::baseline())
            ),
        }
    }

    fn lock_watch_leases(
        &self,
    ) -> std::sync::MutexGuard<'_, std::collections::HashMap<String, WatchLease>> {
        self.watch_leases
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn watch_status(
        leases: &mut std::collections::HashMap<String, WatchLease>,
    ) -> lingxia_control_protocol::dev_session::WatchStatus {
        let now = Instant::now();
        leases.retain(|_, lease| lease.expires > now);
        lingxia_control_protocol::dev_session::WatchStatus {
            paused: !leases.is_empty(),
            leases: leases.len(),
        }
    }

    /// Take or renew a `session.watch.pause` lease.
    pub(crate) fn pause_watch(
        &self,
        args: lingxia_control_protocol::dev_session::WatchPauseArgs,
    ) -> lingxia_control_protocol::dev_session::WatchStatus {
        let ttl = Duration::from_millis(args.ttl_ms.max(1)).min(MAX_WATCH_LEASE);
        let mut leases = self.lock_watch_leases();
        let run_id = args.run_id.or_else(|| {
            leases
                .get(&args.lease)
                .and_then(|lease| lease.run_id.clone())
        });
        leases.insert(
            args.lease,
            WatchLease {
                expires: Instant::now() + ttl,
                ttl,
                run_id,
            },
        );
        Self::watch_status(&mut leases)
    }

    /// Release a `session.watch.pause` lease.
    pub(crate) fn resume_watch(
        &self,
        lease: &str,
    ) -> lingxia_control_protocol::dev_session::WatchStatus {
        let mut leases = self.lock_watch_leases();
        leases.remove(lease);
        Self::watch_status(&mut leases)
    }

    /// A relayed test response for `run_id`: a running run renews its
    /// leases, a finished one releases them.
    fn settle_watch_leases(&self, run_id: &str, running: bool) {
        let mut leases = self.lock_watch_leases();
        let now = Instant::now();
        leases.retain(|_, lease| {
            if lease.run_id.as_deref() != Some(run_id) {
                return true;
            }
            if running {
                lease.expires = now + lease.ttl;
            }
            running
        });
    }

    /// Whether saves must wait: a pause lease holds the watcher.
    pub(crate) fn watch_paused(&self) -> bool {
        Self::watch_status(&mut self.lock_watch_leases()).paused
    }

    fn set_runtime_build(&self, build: Option<lingxia_control_protocol::dev_session::PeerBuild>) {
        *self
            .runtime_build
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = build;
    }

    fn runtime_build(&self) -> Option<lingxia_control_protocol::dev_session::PeerBuild> {
        self.runtime_build
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn authorizes(&self, presented: Option<&str>) -> bool {
        match &self.auth_token {
            None => true,
            Some(expected) => presented == Some(expected.as_str()),
        }
    }

    fn claim_runtime_sender(&self, sender: Sender<DevSessionMessage>) -> (u64, bool) {
        let runtime_id = self.next_runtime_id.fetch_add(1, Ordering::AcqRel);
        let mut guard = self
            .runtime_sender
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let replaced = guard.is_some();
        *guard = Some((runtime_id, sender));
        // Cancel any in-flight "runtime gone" teardown from a previous disconnect.
        self.runtime_epoch.fetch_add(1, Ordering::AcqRel);
        // A run interrupted by the disconnect may have survived it.
        if let Some((run_id, since)) = self.lock_interrupted_test_run().take() {
            eprintln!(
                "[lingxia dev] runtime reconnected {:.1}s into test run {run_id}; resuming it.",
                since.elapsed().as_secs_f64()
            );
            *self.lock_active_test_run() = Some((run_id, Instant::now()));
        }
        if replaced {
            self.clear_pending_results();
        }
        (runtime_id, replaced)
    }

    fn clear_runtime_sender(&self, runtime_id: u64) -> Option<u64> {
        let mut guard = self
            .runtime_sender
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if guard.as_ref().is_some_and(|(id, _)| *id == runtime_id) {
            *guard = None;
            // The run lived in that runtime: it is interrupted, not over,
            // until the grace decides.
            let active = self
                .lock_active_test_run()
                .take()
                .filter(|(_, last_seen)| last_seen.elapsed() < TEST_RUN_LEASE);
            *self.lock_interrupted_test_run() = active.map(|(run_id, _)| (run_id, Instant::now()));
            // Serialize cleanup and the disconnect epoch with replacement claims.
            self.clear_pending_results();
            return Some(self.runtime_epoch.fetch_add(1, Ordering::AcqRel) + 1);
        }
        None
    }

    fn runtime_sender(&self) -> Option<Sender<DevSessionMessage>> {
        self.runtime_sender
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .map(|(_, sender)| sender.clone())
    }

    fn register_pending_result(&self, command_id: String, tx: Sender<DevSessionMessage>) {
        self.pending_results
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(command_id, tx);
    }

    fn take_pending_result(&self, command_id: &str) -> Option<Sender<DevSessionMessage>> {
        self.pending_results
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(command_id)
    }

    fn clear_pending_results(&self) {
        self.pending_results
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
    }

    fn lock_command_forwarding(&self) -> std::sync::MutexGuard<'_, ()> {
        self.command_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_build(&self) -> std::sync::MutexGuard<'_, ()> {
        self.build_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The locks relaying `method` holds until its response: every request
    /// takes the command lock; a test start first waits out a rebuild in
    /// progress (see `build_lock`). Always build lock, then command lock.
    fn lock_for_relay(
        &self,
        method: &str,
    ) -> (
        Option<std::sync::MutexGuard<'_, ()>>,
        std::sync::MutexGuard<'_, ()>,
    ) {
        let build = (method == lingxia_control_protocol::methods::session::test::START)
            .then(|| self.lock_build());
        (build, self.lock_command_forwarding())
    }

    fn request_shutdown(&self) {
        self.stop_flag.store(true, Ordering::Release);
    }

    /// Disconnect expiry and replacement claims share the runtime lock, so a
    /// timer cannot end a session after a replacement has already attached.
    fn finish_runtime_disconnect(&self, epoch: u64) {
        let guard = self
            .runtime_sender
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if self.end_on_runtime_gone
            && self.runtime_epoch.load(Ordering::Acquire) == epoch
            && guard.is_none()
        {
            let interrupted = self.lock_interrupted_test_run().take();
            match interrupted {
                Some((run_id, _)) => eprintln!(
                    "[lingxia dev] runtime did not reconnect during test run {run_id}; ending session."
                ),
                None => eprintln!("[lingxia dev] runtime disconnected; ending session."),
            }
            self.request_shutdown();
        }
    }

    fn drop_runtime(state: &Arc<Self>, runtime_id: u64) {
        let Some(epoch) = state.clear_runtime_sender(runtime_id) else {
            return;
        };
        // A dev scenario and the live mock selection fail closed with the
        // connection, on both sides.
        let companion = state.clone();
        std::thread::spawn(move || {
            companion
                .clear_companion_owner(lingxia_control_protocol::scenario::DEV_OWNER.to_string());
            companion
                .drop_companion_mock_owner(lingxia_control_protocol::mock::DEV_OWNER.to_string());
        });
        if !state.end_on_runtime_gone {
            println!("  • app disconnected; waiting for it to reconnect");
            return;
        }
        let grace = match state.interrupted_test_run() {
            Some(run_id) => {
                let grace = runtime_gone_grace_during_run();
                eprintln!(
                    "[lingxia dev] runtime disconnected during test run {run_id}; \
                     waiting {}s for it to reconnect.",
                    grace.as_secs()
                );
                grace
            }
            None => runtime_gone_grace(),
        };
        let state = Arc::clone(state);
        thread::spawn(move || {
            thread::sleep(grace);
            state.finish_runtime_disconnect(epoch);
        });
    }

    fn lock_interrupted_test_run(&self) -> std::sync::MutexGuard<'_, Option<(String, Instant)>> {
        self.interrupted_test_run
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The run whose runtime is away but still within its reconnect grace.
    fn interrupted_test_run(&self) -> Option<String> {
        self.lock_interrupted_test_run()
            .as_ref()
            .filter(|(_, since)| since.elapsed() < runtime_gone_grace_during_run())
            .map(|(run_id, _)| run_id.clone())
    }

    /// The error a poll for `run_id` gets while its runtime may reconnect.
    fn reconnecting_poll_error(&self, id: String, run_id: &str) -> Option<DevSessionMessage> {
        let interrupted = self.interrupted_test_run()?;
        (interrupted == run_id).then(|| {
            DevSessionMessage::error(
                id,
                RUNTIME_RECONNECTING,
                format!(
                    "runtime disconnected during test run {run_id}; waiting up to {}s for it \
                     to reconnect",
                    runtime_gone_grace_during_run().as_secs()
                ),
            )
        })
    }

    fn lock_active_test_run(&self) -> std::sync::MutexGuard<'_, Option<(String, Instant)>> {
        self.active_test_run
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Track `session.test` runs from the responses this server relays, so
    /// file-watch reloads can wait for the run instead of replacing the app
    /// under it.
    /// Track the active run; returns the companion owner of a run this
    /// response ended, once per run.
    fn observe_test_response(&self, method: &str, response: &DevSessionMessage) -> Option<String> {
        use lingxia_control_protocol::methods::session::test;
        if !matches!(method, test::START | test::POLL | test::CANCEL) {
            return None;
        }
        let DevSessionMessage::Response(response) = response else {
            return None;
        };
        let result = response
            .result
            .as_ref()
            .filter(|_| response.error.is_none())?;
        let run_id = result.get("run_id").and_then(serde_json::Value::as_str)?;
        let running = result.get("state").and_then(serde_json::Value::as_str) == Some("running");
        self.settle_watch_leases(run_id, running);
        let mut active = self.lock_active_test_run();
        let ended = if method == test::START {
            if running {
                *active = Some((run_id.to_string(), Instant::now()));
            }
            !running
        } else if active.as_ref().is_some_and(|(id, _)| id == run_id) {
            *active = running.then(|| (run_id.to_string(), Instant::now()));
            !running
        } else {
            false
        };
        ended.then(|| lingxia_control_protocol::scenario::test_owner(run_id))
    }

    #[cfg(test)]
    pub(crate) fn test_run_active(&self) -> bool {
        self.lock_active_test_run()
            .as_ref()
            .is_some_and(|(_, last_seen)| last_seen.elapsed() < TEST_RUN_LEASE)
    }

    pub(crate) fn rebuild_lxapp(
        &self,
        appid: &str,
        framework: Option<&str>,
        release: bool,
    ) -> Result<()> {
        if self.attached_spec(appid).is_some() {
            return self.build_attached(appid);
        }
        let mut args = serde_json::json!({
            "release": release,
            "appid": appid,
        });
        if let Some(framework) = framework {
            args["framework"] = serde_json::json!(framework);
        }
        let _build = self.lock_build();
        run_lxapp_build(&self.project_root, Some(&args))
    }

    /// Build an attached lxapp with the framework and release it was attached
    /// with, then publish its dist on this host's dev HTTP server.
    fn build_attached(&self, app_id: &str) -> Result<()> {
        if self.attached_spec(app_id).is_none() {
            return Err(anyhow!("{app_id} is not attached"));
        }
        let _build = self.lock_build();
        let Some(spec) = self.attached_spec(app_id) else {
            return Ok(());
        };
        build_lxapp_dir(&spec.source, spec.framework.as_deref(), spec.release)?;
        if self.attached_spec(app_id).is_none() {
            return Ok(());
        }
        let manifest =
            super::lxapp_manifest::publish_dist(&self.project_root, &spec.source.join("dist"))?;
        if manifest.app_id != app_id {
            let _ = super::lxapp_manifest::remove_manifest(&self.project_root, &manifest.app_id);
            return Err(anyhow!(
                "built appId '{}' does not match {app_id}",
                manifest.app_id
            ));
        }
        Ok(())
    }

    /// Restart a running lxapp unless a test run is active.
    ///
    /// The watcher checks for a run before it rebuilds, but a rebuild takes
    /// seconds and `lxdev test` can start one meanwhile; restarting then
    /// replaces the app under a running spec. The check is repeated here
    /// under the command lock, which every relayed `session.test` start holds
    /// until its response is observed, so no run can start between the check
    /// and the restart.
    pub(crate) fn restart_lxapp(&self, appid: &str) -> Result<RestartOutcome> {
        let outcome = self.forward_restart(appid);
        // What the runtime did not restart, a reconnect restarts.
        let mut stale = self.lock_stale();
        match &outcome {
            Ok(RestartOutcome::Restarted | RestartOutcome::NotOpen | RestartOutcome::Failed(_)) => {
                stale.remove(appid);
            }
            Ok(RestartOutcome::NotConnected) | Err(_) => {
                stale.insert(appid.to_string());
            }
            Ok(RestartOutcome::Deferred) => {}
        }
        outcome
    }

    fn forward_restart(&self, appid: &str) -> Result<RestartOutcome> {
        let Some(sender) = self.runtime_sender() else {
            return Ok(RestartOutcome::NotConnected);
        };
        let id = uuid::Uuid::new_v4().to_string();
        let _guard = self.lock_command_forwarding();
        if self.watch_paused() {
            return Ok(RestartOutcome::Deferred);
        }
        let (tx, rx) = mpsc::channel();
        self.register_pending_result(id.clone(), tx);
        let request = DevSessionMessage::Request(ControlRequest {
            id: id.clone(),
            method: lingxia_control_protocol::methods::lxapp::RESTART.to_string(),
            params: Some(serde_json::json!({ "appid": appid })),
        });
        if sender.send(request).is_err() {
            let _ = self.take_pending_result(&id);
            return Err(anyhow!("Failed to forward restart to runtime"));
        }
        match rx.recv_timeout(DEFAULT_COMMAND_TIMEOUT) {
            Ok(DevSessionMessage::Response(response)) => match response.error {
                Some(error) if error.code == "not_open" => Ok(RestartOutcome::NotOpen),
                Some(error) => Ok(RestartOutcome::Failed(error.message)),
                None => Ok(RestartOutcome::Restarted),
            },
            Ok(_) => Err(anyhow!("unexpected restart response")),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let _ = self.take_pending_result(&id);
                Err(anyhow!("lxapp restart timed out"))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = self.take_pending_result(&id);
                Err(anyhow!("runtime disconnected during restart"))
            }
        }
    }

    fn lock_stale(&self) -> std::sync::MutexGuard<'_, std::collections::BTreeSet<String>> {
        self.stale
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_presented(&self) -> std::sync::MutexGuard<'_, Vec<Presented>> {
        self.presented
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Keep `app_id` open in the app: now if it is connected, and again on
    /// every reconnect that finds it closed.
    pub(crate) fn present(&self, app_id: &str) -> Result<()> {
        {
            let mut presented = self.lock_presented();
            if !presented.iter().any(|item| item.app_id == app_id) {
                presented.push(Presented {
                    app_id: app_id.to_string(),
                    shown: false,
                });
            }
        }
        if self.runtime_sender().is_none() {
            return Ok(());
        }
        self.show(app_id)
    }

    /// Bring the connected app up to date: restart what was rebuilt while it
    /// was away, then open each presented lxapp it has not seen. Runs on
    /// every connect, and when a test run's pause ends.
    pub(crate) fn catch_up_current(&self) {
        if let Some(runtime_id) = self.current_runtime_id() {
            self.catch_up(runtime_id);
        }
    }

    /// `runtime_id` is the connection this pass is for. Once a newer one
    /// replaces it, that connection's own pass takes over; this one stops
    /// rather than fail against a dead sender and mark builds stale.
    fn catch_up(&self, runtime_id: u64) {
        let stale: Vec<String> = self.lock_stale().iter().cloned().collect();
        for app_id in stale {
            if self.current_runtime_id() != Some(runtime_id) {
                return;
            }
            match self.forward_restart(&app_id) {
                Ok(RestartOutcome::Restarted) => {
                    self.lock_stale().remove(&app_id);
                    println!("  ✓ reloaded {app_id}");
                }
                Ok(RestartOutcome::NotOpen) => {
                    self.lock_stale().remove(&app_id);
                }
                Ok(RestartOutcome::Failed(message)) => {
                    self.lock_stale().remove(&app_id);
                    eprintln!("  ✗ reload {app_id} failed: {message}");
                }
                Ok(RestartOutcome::NotConnected | RestartOutcome::Deferred) | Err(_) => {}
            }
        }
        let presented: Vec<String> = self
            .lock_presented()
            .iter()
            .map(|item| item.app_id.clone())
            .collect();
        for app_id in presented {
            if self.current_runtime_id() != Some(runtime_id) {
                return;
            }
            if let Err(err) = self.show(&app_id) {
                eprintln!("  ✗ open {app_id} failed: {err:#}");
            }
        }
    }

    fn current_runtime_id(&self) -> Option<u64> {
        self.runtime_sender
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .map(|(id, _)| *id)
    }

    fn show(&self, app_id: &str) -> Result<()> {
        let _present = self
            .present_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(shown) = self
            .lock_presented()
            .iter()
            .find(|item| item.app_id == app_id)
            .map(|item| item.shown)
        else {
            return Ok(());
        };
        if shown {
            // Only a relaunched app has never heard of it. A foregrounded one
            // keeps its page, and one the developer closed stays closed.
            if self.runtime_knows(app_id)? {
                return Ok(());
            }
        } else {
            // The restart pulls the new build first, so an instance left
            // from earlier shows it.
            match self.forward_restart(app_id) {
                // A test run holds the app; this opens when its pause ends.
                Ok(RestartOutcome::Deferred) => return Ok(()),
                Ok(RestartOutcome::Failed(message)) => {
                    eprintln!("  ✗ restart {app_id} failed: {message}");
                }
                Err(err) => eprintln!("  ✗ restart {app_id} failed: {err:#}"),
                Ok(_) => {}
            }
        }
        self.open_lxapp(app_id)?;
        if let Some(item) = self
            .lock_presented()
            .iter_mut()
            .find(|item| item.app_id == app_id)
        {
            item.shown = true;
        }
        println!("  ✓ opened {app_id}");
        Ok(())
    }

    fn runtime_knows(&self, app_id: &str) -> Result<bool> {
        let apps = self.runtime_request(
            lingxia_control_protocol::methods::lxapp::LIST,
            serde_json::json!({ "all": true }),
            DEFAULT_COMMAND_TIMEOUT,
        )?;
        Ok(apps.as_array().is_some_and(|apps| {
            apps.iter()
                .any(|app| app.get("appid").and_then(serde_json::Value::as_str) == Some(app_id))
        }))
    }

    fn open_lxapp(&self, app_id: &str) -> Result<()> {
        let _guard = self.lock_command_forwarding();
        self.runtime_request(
            lingxia_control_protocol::methods::lxapp::OPEN,
            serde_json::json!({ "appid": app_id }),
            DEFAULT_COMMAND_TIMEOUT,
        )?;
        Ok(())
    }
}

fn handle_watch_request(
    state: &DevServerState,
    id: &str,
    method: &str,
    params: Option<serde_json::Value>,
) -> DevSessionMessage {
    use lingxia_control_protocol::dev_session::{WatchPauseArgs, WatchResumeArgs};
    let params = params.unwrap_or(serde_json::Value::Null);
    let status = if method == lingxia_control_protocol::methods::session::watch::PAUSE {
        serde_json::from_value::<WatchPauseArgs>(params).map(|args| state.pause_watch(args))
    } else {
        serde_json::from_value::<WatchResumeArgs>(params)
            .map(|args| state.resume_watch(&args.lease))
    };
    match status {
        Ok(status) => DevSessionMessage::success(
            id.to_string(),
            Some(serde_json::to_value(status).unwrap_or_default()),
        ),
        Err(err) => {
            DevSessionMessage::error(id.to_string(), "invalid_params", format!("{method}: {err}"))
        }
    }
}

/// What [`DevServerState::restart_lxapp`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RestartOutcome {
    Restarted,
    /// The runtime answered with an error; retrying would get the same one.
    Failed(String),
    /// The app does not have it open; the build loads on its next open.
    NotOpen,
    /// No runtime is attached yet.
    NotConnected,
    /// A test run is active; restart after it ends.
    Deferred,
}

fn runtime_gone_grace() -> Duration {
    #[cfg(test)]
    {
        Duration::from_millis(50)
    }
    #[cfg(not(test))]
    {
        RUNTIME_GONE_GRACE
    }
}

fn runtime_gone_grace_during_run() -> Duration {
    #[cfg(test)]
    {
        Duration::from_millis(300)
    }
    #[cfg(not(test))]
    {
        RUNTIME_GONE_GRACE_DURING_RUN
    }
}

fn session_ends_on_runtime_gone(platform: &str) -> bool {
    matches!(platform, "runner" | "windows" | "macos")
}

pub fn start_server_on_with_stop(
    project_root: &Path,
    bind_addr: &str,
    platform: &str,
    app_env: AppEnv,
    stop_flag: Arc<AtomicBool>,
    auth_token: Option<String>,
) -> Result<DevServerHandle> {
    start_server_on_with_roots(
        project_root,
        project_root,
        bind_addr,
        session_ends_on_runtime_gone(platform),
        app_env,
        stop_flag,
        auth_token,
    )
}

fn start_server_on_with_roots(
    session_root: &Path,
    content_root: &Path,
    bind_addr: &str,
    end_on_runtime_gone: bool,
    app_env: AppEnv,
    stop_flag: Arc<AtomicBool>,
    auth_token: Option<String>,
) -> Result<DevServerHandle> {
    let session = create_session(session_root)?;
    let attach_token = write_attach_token(&session.log_file)?;
    let listener = TcpListener::bind(bind_addr).context("Failed to bind dev websocket")?;
    listener
        .set_nonblocking(true)
        .context("Failed to set dev websocket listener nonblocking")?;
    let ws_addr = listener
        .local_addr()
        .context("Failed to resolve dev websocket address")?;
    let writer = Arc::new(SessionLogWriter::new(&session)?);
    // The one prepare a dev session runs: before the first build, then the
    // companion keeps generated inputs live and watcher rebuilds skip it.
    for root in super::lxapp_watch::discover_watch_roots(content_root)? {
        crate::build_prepare::prepare(&root.path)?;
    }
    // The companion learns the session's env and decides itself whether a
    // local backend belongs in it.
    let companion = super::companion::DevCompanion::start(
        session_root,
        app_env,
        stop_flag.clone(),
        writer.clone(),
    )?;
    let state = Arc::new(DevServerState::new(
        content_root.to_path_buf(),
        stop_flag.clone(),
        auth_token,
        end_on_runtime_gone,
    ));
    *state
        .attach_token
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(attach_token);
    if let Some(companion) = &companion {
        let _ = state.companion.set(companion.link());
    }
    state.prepare_mocks()?;
    let thread_state = state.clone();
    let thread_stop_flag = stop_flag.clone();
    let server_thread =
        thread::spawn(move || run_server(listener, writer, thread_state, thread_stop_flag));

    Ok(DevServerHandle {
        session,
        ws_addr,
        stop_flag,
        server_thread: Some(server_thread),
        companion,
        state,
        watch: None,
    })
}

/// The session of `lingxia dev listen --port`. Bind `host:port` exactly:
/// unlike [`start_server_fixed_with_stop`], a busy port is an error, since it
/// is the port the app already dials. It launched nothing, so the app leaving
/// does not end it on any platform.
pub fn start_server_at_with_roots(
    session_root: &Path,
    content_root: &Path,
    host: &str,
    port: u16,
    stop_flag: Arc<AtomicBool>,
    auth_token: Option<String>,
) -> Result<DevServerHandle> {
    let _preparation = crate::build_prepare::Scope::enter();
    start_server_on_with_roots(
        session_root,
        content_root,
        &format!("{host}:{port}"),
        false,
        // `listen` takes no `--env`: the session keeps the default env.
        AppEnv::Dev,
        stop_flag,
        auth_token,
    )
}

pub fn start_server_fixed_with_stop(
    project_root: &Path,
    host: &str,
    platform: &str,
    app_env: AppEnv,
    stop_flag: Arc<AtomicBool>,
    auth_token: Option<String>,
) -> Result<DevServerHandle> {
    // The port fallback retries the whole start; prepare only once.
    let _preparation = crate::build_prepare::Scope::enter();
    let port = dev_port(&project_root.to_string_lossy(), platform);
    match start_server_on_with_stop(
        project_root,
        &format!("{host}:{port}"),
        platform,
        app_env,
        stop_flag.clone(),
        auth_token.clone(),
    ) {
        Ok(handle) => Ok(handle),
        Err(err) => {
            eprintln!(
                "⚠ dev port {port} unavailable ({err:#}); using an OS-assigned port — \
                 reconnection after a client restart may need a re-forward."
            );
            start_server_on_with_stop(
                project_root,
                &format!("{host}:0"),
                platform,
                app_env,
                stop_flag,
                auth_token,
            )
        }
    }
}

pub fn start_server_fixed_with_roots(
    session_root: &Path,
    content_root: &Path,
    host: &str,
    platform: &str,
    app_env: AppEnv,
    stop_flag: Arc<AtomicBool>,
    auth_token: Option<String>,
) -> Result<DevServerHandle> {
    // The port fallback retries the whole start; prepare only once.
    let _preparation = crate::build_prepare::Scope::enter();
    let port = dev_port(&session_root.to_string_lossy(), platform);
    match start_server_on_with_roots(
        session_root,
        content_root,
        &format!("{host}:{port}"),
        session_ends_on_runtime_gone(platform),
        app_env,
        stop_flag.clone(),
        auth_token.clone(),
    ) {
        Ok(handle) => Ok(handle),
        Err(error) => {
            eprintln!(
                "⚠ dev port {port} unavailable ({error:#}); using an OS-assigned port — \
                 reconnection after a client restart may need a re-forward."
            );
            start_server_on_with_roots(
                session_root,
                content_root,
                &format!("{host}:0"),
                session_ends_on_runtime_gone(platform),
                app_env,
                stop_flag,
                auth_token,
            )
        }
    }
}

/// Where a session keeps its attach secret: beside its log, readable only by
/// this user. `lingxia dev listen` reads it through the broker's log path.
pub(super) fn attach_token_path(log_file: &Path) -> PathBuf {
    log_file.with_extension("attach")
}

fn write_attach_token(log_file: &Path) -> Result<String> {
    let token = uuid::Uuid::new_v4().simple().to_string();
    let path = attach_token_path(log_file);
    std::fs::write(&path, &token).with_context(|| format!("Failed to write {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("Failed to restrict {}", path.display()))?;
    }
    Ok(token)
}

/// Deterministic dev-server port derived from a stable per-app key + platform,
/// so a restarted client reconnects to the same port (and the same adb/hdc
/// forward) without the host re-publishing a fresh OS-assigned port. FNV-1a over
/// `"{app_key}:{platform}"` mapped into a stable private range (39000–39999).
/// `app_key` is any value stable across the host's dev restarts (the dev flow
/// passes the project root path).
pub fn dev_port(app_key: &str, platform: &str) -> u16 {
    const FNV_OFFSET: u32 = 0x811c_9dc5;
    const FNV_PRIME: u32 = 0x0100_0193;
    let mut hash = FNV_OFFSET;
    for byte in format!("{app_key}:{platform}").bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    39_000 + (hash % 1000) as u16
}

fn run_server(
    listener: TcpListener,
    writer: Arc<SessionLogWriter>,
    state: Arc<DevServerState>,
    stop_flag: Arc<AtomicBool>,
) {
    while !stop_flag.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let writer = writer.clone();
                let state = state.clone();
                let stop_flag = stop_flag.clone();
                thread::spawn(move || {
                    if let Err(err) = handle_connection(stream, &writer, &state)
                        && !stop_flag.load(Ordering::Acquire)
                        && !is_expected_websocket_shutdown_error(&err)
                    {
                        eprintln!("[lingxia dev] websocket connection failed: {err}");
                    }
                });
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(err) => {
                eprintln!("[lingxia dev] websocket accept failed: {err}");
                break;
            }
        }
    }
}

fn handle_connection(
    stream: TcpStream,
    writer: &SessionLogWriter,
    state: &Arc<DevServerState>,
) -> Result<()> {
    if !is_websocket_request(&stream)? {
        return handle_http_connection(stream, state.as_ref());
    }
    let (mut websocket, handshake_token) = accept_websocket(stream)?;
    let hello = read_wire_message(&mut websocket)?;
    let DevSessionMessage::Hello {
        version,
        role,
        capabilities: _,
        build: peer_build,
    } = hello
    else {
        return Err(anyhow!("First websocket message must be hello"));
    };
    if version != DEV_SESSION_PROTOCOL_VERSION {
        let _ = websocket.close(None);
        return Err(anyhow!(
            "Unsupported dev session protocol version {version}; expected {DEV_SESSION_PROTOCOL_VERSION}"
        ));
    }
    if !state.authorizes(handshake_token.as_deref()) {
        let _ = websocket.close(None);
        if role == DevSessionRole::Runtime {
            state.runtime_rejected.store(true, Ordering::Release);
            return Err(anyhow!(
                "Rejected the app: its session token does not match this session. It was \
                 built on another machine or against an older CLI; rebuild it here with \
                 `lingxia dev` (or reinstall the Runner)"
            ));
        }
        return Err(anyhow!(
            "Rejected dev websocket client with a missing or wrong session token"
        ));
    }
    match role {
        DevSessionRole::Runtime => {
            state.runtime_rejected.store(false, Ordering::Release);
            state.set_runtime_build(peer_build);
            handle_devtool_connection(websocket, writer, state)
        }
        DevSessionRole::Controller => handle_client_connection(websocket, state.as_ref()),
        DevSessionRole::Companion => Err(anyhow!(
            "Companion participants use the supervised session transport"
        )),
    }
}

/// Capture the session credential from the HTTP upgrade target. Protocol
/// messages carry identity and capabilities, never transport credentials.
#[allow(clippy::result_large_err)] // Tungstenite fixes this type in its callback contract.
fn accept_websocket(stream: TcpStream) -> Result<(WebSocket<TcpStream>, Option<String>)> {
    let mut handshake_token = None;
    let mut websocket = accept_hdr(stream, |request: &Request, response: Response| {
        handshake_token =
            lingxia_control_protocol::dev_session::token_from_ws_url(&request.uri().to_string());
        Ok(response)
    })
    .map_err(|err| anyhow!("Failed to accept websocket: {err}"))?;
    websocket.set_config(|config| {
        config.max_frame_size = Some(DEV_SESSION_MAX_MESSAGE_BYTES);
        config.max_message_size = Some(DEV_SESSION_MAX_MESSAGE_BYTES);
    });
    Ok((websocket, handshake_token))
}

fn is_websocket_request(stream: &TcpStream) -> Result<bool> {
    let mut buf = [0u8; 2048];
    let n = stream
        .peek(&mut buf)
        .context("Failed to inspect dev server request")?;
    let request = String::from_utf8_lossy(&buf[..n]).to_ascii_lowercase();
    Ok(request.contains("\r\nupgrade: websocket")
        || request.contains("\nupgrade: websocket")
        || request.contains("\r\nsec-websocket-key:")
        || request.contains("\nsec-websocket-key:"))
}

fn handle_http_connection(mut stream: TcpStream, state: &DevServerState) -> Result<()> {
    let request = read_http_request_head(&mut stream)?;
    let Some((method, target)) = parse_http_request_line(&request) else {
        return write_http_error(&mut stream, 400, "Bad Request");
    };
    if method != "GET" {
        return write_http_error(&mut stream, 405, "Method Not Allowed");
    }
    let presented = lingxia_control_protocol::dev_session::token_from_ws_url(target);
    if !state.authorizes(presented.as_deref()) {
        return write_http_error(&mut stream, 403, "Forbidden");
    }
    let target = target
        .split_once('?')
        .map(|(path, _)| path)
        .unwrap_or(target);
    // The runtime reads 404 as "not published here" and keeps its own
    // source; any other failure is an error it reports and backs off from.
    match handle_dev_lxapp_http_request(state, target) {
        Ok(Some((content_type, body))) => {
            write_http_response(&mut stream, 200, content_type, &body)
        }
        Ok(None) => write_http_error(&mut stream, 404, "Not Found"),
        Err(err) => write_http_error(&mut stream, 400, &err.to_string()),
    }
}

fn read_http_request_head(stream: &mut TcpStream) -> Result<String> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 512];
    loop {
        let n = stream
            .read(&mut chunk)
            .context("Failed to read HTTP request")?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.windows(4).any(|window| window == b"\r\n\r\n") || buf.len() > 16 * 1024 {
            break;
        }
    }
    String::from_utf8(buf).context("HTTP request was not valid UTF-8")
}

fn parse_http_request_line(request: &str) -> Option<(&str, &str)> {
    let line = request.lines().next()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    let target = parts.next()?;
    Some((method, target))
}

/// `None` is a 404: no such endpoint, an app id this session does not
/// publish, or a file its manifest does not list.
fn handle_dev_lxapp_http_request(
    state: &DevServerState,
    target: &str,
) -> Result<Option<(&'static str, Vec<u8>)>> {
    let Some(rest) = target.strip_prefix(DEV_LXAPP_HTTP_PREFIX) else {
        return Ok(None);
    };

    if let Some(app_id) = rest.strip_suffix("/manifest.json") {
        let app_id = decode_url_path_component(app_id)?;
        if !super::lxapp_manifest::is_published(&state.project_root, &app_id) {
            return Ok(None);
        }
        let manifest = super::lxapp_manifest::load_manifest(&state.project_root, &app_id)?;
        let body = serde_json::to_vec_pretty(&manifest)?;
        return Ok(Some(("application/json; charset=utf-8", body)));
    }

    let Some((app_id, relative_path)) = rest.split_once("/files/") else {
        return Ok(None);
    };
    let app_id = decode_url_path_component(app_id)?;
    let relative_path = decode_url_path_component(relative_path)?;
    if !super::lxapp_manifest::is_published(&state.project_root, &app_id)
        || !super::lxapp_manifest::load_manifest(&state.project_root, &app_id)?
            .files
            .iter()
            .any(|file| file.path == relative_path)
    {
        return Ok(None);
    }
    let file_path = if let Some(dist) = state.attached_dist(&app_id) {
        super::lxapp_manifest::resolve_published_file(
            &state.project_root,
            &app_id,
            &relative_path,
            &dist,
        )?
    } else {
        super::lxapp_manifest::resolve_dist_file(&state.project_root, &app_id, &relative_path)?
    };
    let body = std::fs::read(&file_path)
        .with_context(|| format!("Failed to read {}", file_path.display()))?;
    Ok(Some((content_type_for_path(&relative_path), body)))
}

fn decode_url_path_component(value: &str) -> Result<String> {
    urlencoding::decode(value)
        .map(|value| value.into_owned())
        .map_err(|err| anyhow!("invalid URL path encoding: {}", err))
}

fn content_type_for_path(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

fn write_http_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Error",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Cache-Control: no-store\r\n\
         Connection: close\r\n\r\n",
        body.len()
    )
    .context("Failed to write HTTP response headers")?;
    stream
        .write_all(body)
        .context("Failed to write HTTP response body")?;
    Ok(())
}

fn write_http_error(stream: &mut TcpStream, status: u16, message: &str) -> Result<()> {
    write_http_response(
        stream,
        status,
        "text/plain; charset=utf-8",
        message.as_bytes(),
    )
}

fn handle_devtool_connection(
    mut websocket: WebSocket<TcpStream>,
    writer: &SessionLogWriter,
    state: &Arc<DevServerState>,
) -> Result<()> {
    websocket
        .get_mut()
        .set_read_timeout(Some(Duration::from_millis(100)))
        .context("Failed to set devtool websocket read timeout")?;

    let (outgoing_tx, outgoing_rx) = mpsc::channel::<DevSessionMessage>();
    let (runtime_id, replaced_runtime) = state.claim_runtime_sender(outgoing_tx);
    if replaced_runtime {
        eprintln!("[lingxia dev] devtool runtime reconnected; replacing stale runtime connection");
    } else {
        println!("  ✓ app connected");
    }
    // The runtime's mocks end with its connection: load them again, off
    // this thread, which routes the answers.
    let pushing = Arc::clone(state);
    thread::spawn(move || {
        pushing.push_all_mocks();
        pushing.catch_up(runtime_id);
    });

    let result = thread::scope(|scope| {
        let (event_tx, event_rx) = mpsc::sync_channel(RUNTIME_EVENT_QUEUE_CAPACITY);
        let persistence = scope
            .spawn(move || persist_runtime_events(event_rx, |events| writer.append_events(events)));
        let mut dropped_event_batch_warning = false;
        let connection_result = loop {
            if let Err(err) = drain_outgoing_messages(&mut websocket, &outgoing_rx) {
                if is_expected_websocket_shutdown_error(&err) {
                    break Ok(());
                }
                break Err(err);
            }

            match websocket.read() {
                Ok(message) => match parse_text_message(message)? {
                    ParsedWireMessage::Wire(message) => {
                        route_runtime_message(
                            *message,
                            state,
                            &event_tx,
                            &mut dropped_event_batch_warning,
                        )?;
                    }
                    ParsedWireMessage::Ignored => {}
                    ParsedWireMessage::Closed => break Ok(()),
                },
                Err(WsError::Io(err))
                    if err.kind() == std::io::ErrorKind::WouldBlock
                        || err.kind() == std::io::ErrorKind::TimedOut => {}
                Err(err) if is_expected_tungstenite_shutdown_error(&err) => break Ok(()),
                Err(WsError::ConnectionClosed) | Err(WsError::AlreadyClosed) => break Ok(()),
                Err(err) => break Err(err.into()),
            }
        };
        drop(event_tx);
        let persistence_result = persistence
            .join()
            .map_err(|_| anyhow!("session log writer thread panicked"))?;
        connection_result.and(persistence_result)
    });

    DevServerState::drop_runtime(state, runtime_id);
    result
}

fn route_runtime_message(
    message: DevSessionMessage,
    state: &Arc<DevServerState>,
    event_tx: &mpsc::SyncSender<Vec<DevSessionEvent>>,
    dropped_event_batch_warning: &mut bool,
) -> Result<()> {
    match message {
        DevSessionMessage::EventBatch { events } => match event_tx.try_send(events) {
            Ok(()) => {}
            Err(mpsc::TrySendError::Full(events)) => {
                if !*dropped_event_batch_warning {
                    eprintln!(
                        "[lingxia dev] session log writer fell behind; dropping a batch of {} events",
                        events.len()
                    );
                    *dropped_event_batch_warning = true;
                }
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                return Err(anyhow!("session log writer stopped"));
            }
        },
        DevSessionMessage::Response(response) => {
            let id = response.id.clone();
            let payload = DevSessionMessage::Response(response);
            if let Some(tx) = state.take_pending_result(&id) {
                let _ = tx.send(payload);
            }
        }
        // A host run's `t.scenario.use()` reaches the companion this way;
        // the relay can wait on it, so it must not hold up this connection.
        DevSessionMessage::Request(request)
            if request
                .method
                .starts_with(lingxia_control_protocol::methods::session::companion::PREFIX) =>
        {
            let state = Arc::clone(state);
            thread::spawn(move || {
                let reply = state.companion_reply(request.id, &request.method, request.params);
                if let Some(sender) = state.runtime_sender() {
                    let _ = sender.send(reply);
                }
            });
        }
        DevSessionMessage::Hello { .. } | DevSessionMessage::Request(_) => {}
    }
    Ok(())
}

fn persist_runtime_events(
    event_rx: mpsc::Receiver<Vec<DevSessionEvent>>,
    mut persist: impl FnMut(&[DevSessionEvent]) -> Result<()>,
) -> Result<()> {
    while let Ok(events) = event_rx.recv() {
        persist(&events)?;
    }
    Ok(())
}

fn is_expected_websocket_shutdown_error(err: &anyhow::Error) -> bool {
    let message = err.to_string();
    message.contains("Connection reset without closing handshake")
        || message.contains("Connection reset by peer")
        || message.contains("Broken pipe")
}

fn is_expected_tungstenite_shutdown_error(err: &WsError) -> bool {
    match err {
        WsError::ConnectionClosed | WsError::AlreadyClosed => true,
        WsError::Io(io) => matches!(
            io.kind(),
            std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::BrokenPipe
                | std::io::ErrorKind::UnexpectedEof
        ),
        _ => err
            .to_string()
            .contains("Connection reset without closing handshake"),
    }
}

fn finish_attach_error(
    websocket: &mut WebSocket<TcpStream>,
    id: String,
    message: String,
) -> Result<()> {
    send_wire_message(
        websocket,
        &DevSessionMessage::error(id, "attach_failed", message),
    )?;
    let _ = websocket.close(None);
    Ok(())
}

fn handle_lxapp_attach(
    mut websocket: WebSocket<TcpStream>,
    state: &DevServerState,
    id: String,
    params: Option<serde_json::Value>,
) -> Result<()> {
    let spec = match validate_attach(state, params.as_ref()) {
        Ok(spec) => spec,
        Err(err) => return finish_attach_error(&mut websocket, id, format!("{err:#}")),
    };
    let app_id = spec.app_id.clone();
    if let Err(err) = state.insert_attached(spec.clone()) {
        return finish_attach_error(&mut websocket, id, format!("{err:#}"));
    }
    let _detach = DetachOnDrop {
        state,
        app_id: app_id.clone(),
    };
    // The session prepared only its own content at start. Prepare before
    // watching: generated inputs it writes are not a save.
    if let Err(err) = crate::build_prepare::prepare(&spec.source) {
        return finish_attach_error(&mut websocket, id, format!("{err:#}"));
    }
    if let Err(err) = state.watch_add(super::lxapp_watch::WatchRoot {
        app_id: app_id.clone(),
        path: spec.source.clone(),
    }) {
        return finish_attach_error(&mut websocket, id, format!("{err:#}"));
    }
    if let Err(err) = state.build_attached(&app_id) {
        return finish_attach_error(&mut websocket, id, format!("{err:#}"));
    }
    let connected = state.runtime_sender().is_some();
    if let Err(err) = state.present(&app_id) {
        return finish_attach_error(&mut websocket, id, format!("{err:#}"));
    }
    send_wire_message(
        &mut websocket,
        &DevSessionMessage::success(
            id,
            Some(serde_json::json!({ "appId": app_id, "opened": connected })),
        ),
    )?;
    hold_attach(&mut websocket);
    Ok(())
}

fn validate_attach(
    state: &DevServerState,
    params: Option<&serde_json::Value>,
) -> Result<AttachedLxapp> {
    let path = params
        .and_then(|value| value.get("path"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("session.lxapp.attach requires path"))?;
    let source = PathBuf::from(path);
    if !source.join("lxapp.json").is_file() {
        return Err(anyhow!("{} has no lxapp.json", source.display()));
    }
    // Same `\\?\` prefix Node cannot load; see `resolve_source`.
    let source = dunce::canonicalize(&source)
        .with_context(|| format!("Failed to resolve {}", source.display()))?;
    let app_id = super::lxapp_watch::read_app_id(&source)?;
    if let Some(message) =
        super::lxapp_manifest::declared_bundle_conflict(&state.project_root, &source, &app_id)?
    {
        return Err(anyhow!(message));
    }
    // Its own content (a `listen --port` lxapp) is already built, served, and
    // watched; attaching it again would share, then delete, that state.
    if super::lxapp_watch::discover_watch_roots(&state.project_root)?
        .iter()
        .any(|root| root.app_id == app_id || root.path == source)
    {
        return Err(anyhow!(
            "this session already develops {app_id}; save in its directory to reload it"
        ));
    }
    if state.watch_sender().is_none() {
        return Err(anyhow!("this session is not watching lxapps"));
    }
    let framework = params
        .and_then(|value| value.get("framework"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| state.watch_framework());
    let release = params
        .and_then(|value| value.get("release"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or_else(|| state.watch_release.load(Ordering::Acquire));
    Ok(AttachedLxapp {
        app_id,
        source,
        framework,
        release,
    })
}

impl DevServerState {
    fn watch_add(&self, root: super::lxapp_watch::WatchRoot) -> Result<()> {
        let sender = self
            .watch_sender()
            .ok_or_else(|| anyhow!("this session is not watching lxapps"))?;
        let (ack_tx, ack_rx) = mpsc::channel();
        sender
            .send(super::lxapp_watch::WatchCmd::Add { root, ack: ack_tx })
            .map_err(|_| anyhow!("the lxapp watcher stopped"))?;
        match ack_rx.recv() {
            Ok(Ok(())) => Ok(()),
            Ok(Err(err)) => Err(anyhow!(err)),
            Err(_) => Err(anyhow!("the lxapp watcher stopped")),
        }
    }
}

fn hold_attach(websocket: &mut WebSocket<TcpStream>) {
    let _ = websocket
        .get_mut()
        .set_read_timeout(Some(Duration::from_secs(1)));
    loop {
        match websocket.read() {
            Ok(Message::Close(_)) => return,
            Ok(_) => {}
            Err(WsError::Io(err))
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) => {}
            Err(_) => return,
        }
    }
}

fn handle_client_connection(
    mut websocket: WebSocket<TcpStream>,
    state: &DevServerState,
) -> Result<()> {
    let message = read_wire_message(&mut websocket)?;
    let DevSessionMessage::Request(ControlRequest { id, method, params }) = message else {
        return Err(anyhow!("Client websocket must send exactly one command"));
    };

    // Attach keeps this socket open. Disconnect is what unregisters the extra
    // watch, the same way a session's broker connection is its liveness.
    if method.as_str() == lingxia_control_protocol::methods::session::LXAPP_ATTACH {
        // An attach builds the directory it names. Only this machine may ask:
        // a LAN peer holding an iOS session's token must not run builds here.
        let local = websocket
            .get_ref()
            .peer_addr()
            .is_ok_and(|addr| addr.ip().to_canonical().is_loopback());
        let token = params
            .as_ref()
            .and_then(|value| value.get("attachToken"))
            .and_then(serde_json::Value::as_str);
        if !local || !state.attach_authorized(token) {
            return finish_attach_error(
                &mut websocket,
                id,
                "an lxapp can only be attached by `lingxia dev listen` as this user on this \
                 machine"
                    .to_string(),
            );
        }
        return handle_lxapp_attach(websocket, state, id, params);
    }

    // `lxapp.build` is an orchestrator-level rebuild operation: the dev server
    // owns the project + build pipeline, so it builds in-process rather than
    // forwarding to the runtime (which has no build toolchain). Works even with
    // no app attached.
    if method.as_str() == lingxia_control_protocol::methods::lxapp::BUILD {
        let attached = params
            .as_ref()
            .and_then(|value| value.get("appid"))
            .and_then(serde_json::Value::as_str)
            .filter(|appid| state.attached_spec(appid).is_some());
        let built = match attached {
            Some(appid) => state.build_attached(appid),
            None => {
                let _build = state.lock_build();
                run_lxapp_build(&state.project_root, params.as_ref())
            }
        };
        let payload = match built {
            Ok(()) => DevSessionMessage::success(id, None),
            Err(err) => DevSessionMessage::error(id, "build_failed", format!("{err:#}")),
        };
        send_wire_message(&mut websocket, &payload)?;
        let _ = websocket.close(None);
        return Ok(());
    }

    if method.as_str() == lingxia_control_protocol::methods::ECHO {
        let runtime_connected = state.runtime_sender().is_some();
        let mut result = serde_json::json!({
            "runtimeConnected": runtime_connected,
        });
        if runtime_connected && let Some(build) = state.runtime_build() {
            result["runtimeBuild"] = serde_json::to_value(build).unwrap_or_default();
        }
        send_wire_message(
            &mut websocket,
            &DevSessionMessage::success(id, Some(result)),
        )?;
        let _ = websocket.close(None);
        return Ok(());
    }

    if matches!(
        method.as_str(),
        lingxia_control_protocol::methods::session::watch::PAUSE
            | lingxia_control_protocol::methods::session::watch::RESUME
    ) {
        let reply = handle_watch_request(state, &id, &method, params);
        send_wire_message(&mut websocket, &reply)?;
        let _ = websocket.close(None);
        return Ok(());
    }

    if method.starts_with(lingxia_control_protocol::methods::session::companion::PREFIX) {
        let reply = state.companion_reply(id, &method, params);
        send_wire_message(&mut websocket, &reply)?;
        let _ = websocket.close(None);
        return Ok(());
    }

    if method.as_str() == lingxia_control_protocol::methods::session::SHUTDOWN {
        send_wire_message(&mut websocket, &DevSessionMessage::success(id, None))?;
        state.request_shutdown();
        let _ = websocket.close(None);
        return Ok(());
    }

    let Some(runtime_sender) = state.runtime_sender() else {
        if method.as_str() == lingxia_control_protocol::methods::session::test::POLL
            && let Some(run_id) = params
                .as_ref()
                .and_then(|params| params.get("run_id"))
                .and_then(serde_json::Value::as_str)
            && let Some(reply) = state.reconnecting_poll_error(id.clone(), run_id)
        {
            send_wire_message(&mut websocket, &reply)?;
            let _ = websocket.close(None);
            return Ok(());
        }
        let error = if state.runtime_rejected.load(Ordering::Acquire) {
            "devtool runtime is not connected: a runtime peer was rejected for a missing or \
             wrong session token — the device host likely predates session tokens; rebuild \
             it from this revision (or reinstall the Runner)"
                .to_string()
        } else {
            "devtool runtime is not connected".to_string()
        };
        send_wire_message(
            &mut websocket,
            &DevSessionMessage::error(id, "runtime_unavailable", error),
        )?;
        let _ = websocket.close(None);
        return Ok(());
    };

    let relay_guards = state.lock_for_relay(&method);
    if method == lingxia_control_protocol::methods::session::test::START
        && let Some(reason) = state.companion_cleanup_error()
    {
        send_wire_message(
            &mut websocket,
            &DevSessionMessage::error(id, "companion_cleanup_unconfirmed", reason),
        )?;
        let _ = websocket.close(None);
        return Ok(());
    }
    let command_timeout = command_timeout(params.as_ref());
    let relayed_method = method.clone();
    let (result_tx, result_rx) = mpsc::channel::<DevSessionMessage>();
    state.register_pending_result(id.clone(), result_tx);
    let bridged_command = DevSessionMessage::Request(ControlRequest {
        id: id.clone(),
        method,
        params,
    });

    runtime_sender.send(bridged_command).map_err(|_| {
        let _ = state.take_pending_result(&id);
        anyhow!("Failed to forward command to devtool")
    })?;

    match result_rx.recv_timeout(command_timeout) {
        Ok(result) => {
            let ended_owner = state.observe_test_response(&relayed_method, &result);
            let sent = send_wire_message(&mut websocket, &result);
            // Fallback for the host's own cleanup; not under the command
            // lock, and never in the way of the run's result.
            drop(relay_guards);
            if let Some(owner) = ended_owner {
                state.clear_companion_owner(owner.clone());
                state.drop_companion_mock_owner(owner);
            }
            sent?;
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            let _ = state.take_pending_result(&id);
            send_wire_message(
                &mut websocket,
                &DevSessionMessage::error(id, "request_timeout", "request timed out"),
            )?;
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            let _ = state.take_pending_result(&id);
            send_wire_message(
                &mut websocket,
                &DevSessionMessage::error(
                    id,
                    "runtime_disconnected",
                    "request channel disconnected",
                ),
            )?;
        }
    }
    let _ = websocket.close(None);
    Ok(())
}

fn command_timeout(args: Option<&serde_json::Value>) -> Duration {
    let Some(timeout_ms) = args
        .and_then(|value| value.get("timeout_ms"))
        .and_then(serde_json::Value::as_u64)
    else {
        return DEFAULT_COMMAND_TIMEOUT;
    };
    Duration::from_millis(timeout_ms).saturating_add(COMMAND_TIMEOUT_BUFFER)
}

/// Rebuild the lxapp front-end bundle in-process. The dev orchestrator owns the
/// project + build pipeline; this mirrors `lingxia build` run in a standalone
/// lxapp dir. Output streams to the `lingxia dev` terminal; the client receives
/// only ok/error.
fn build_lxapp_dir(dir: &Path, framework: Option<&str>, release: bool) -> Result<()> {
    let mut build_args = vec!["build".to_string()];
    if release {
        build_args.push("--release".to_string());
    }
    if let Some(framework) = framework {
        build_args.push("--framework".to_string());
        build_args.push(framework.to_string());
    }
    crate::lxapp::run_in_dir_for_dev(&build_args, dir)
}

fn run_lxapp_build(project_root: &Path, args: Option<&serde_json::Value>) -> Result<()> {
    let release = args
        .and_then(|value| value.get("release"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let framework = args
        .and_then(|value| value.get("framework"))
        .and_then(serde_json::Value::as_str);
    let appid = args
        .and_then(|value| value.get("appid"))
        .and_then(serde_json::Value::as_str);
    build_lxapp_dir(&resolve_lxapp_dir(project_root, appid)?, framework, release)?;

    // Host dev sessions serve bundles through the generated manifest. Without
    // refreshing it here, the runtime sees the previous dist hash and a
    // successful in-place reload silently restarts stale assets.
    refresh_lxapp_manifests(project_root)?;
    Ok(())
}

fn refresh_lxapp_manifests(project_root: &Path) -> Result<()> {
    if project_root.join("lingxia.yaml").is_file() {
        let config = crate::config::LingXiaConfig::load(project_root)?;
        super::lxapp_manifest::write_configured_manifests(project_root, &config)?;
    } else if project_root.join("lxapp.json").is_file() {
        // A standalone session (`listen --port`) serves its own lxapp.
        super::lxapp_manifest::publish_dist(project_root, &project_root.join("dist"))?;
    }
    Ok(())
}

/// The directory holding the lxapp to build: the project root itself for a
/// standalone project, otherwise the requested resource or the host's home
/// lxapp when no runtime is currently open.
fn resolve_lxapp_dir(project_root: &Path, requested_appid: Option<&str>) -> Result<PathBuf> {
    if project_root.join("lxapp.json").exists() || project_root.join("lxplugin.json").exists() {
        return Ok(project_root.to_path_buf());
    }
    let config = crate::config::LingXiaConfig::load(project_root)?;
    let appid = match requested_appid {
        Some(appid) => appid,
        None => config
            .app
            .as_ref()
            .ok_or_else(|| anyhow!("Missing app settings in lingxia.yaml"))?
            .home_app_id
            .as_deref()
            .ok_or_else(|| {
                anyhow!("this host has no local control lxapp; rebuilding requires app.homeAppId")
            })?,
    };
    let path = config
        .resources
        .as_ref()
        .and_then(|resources| {
            resources
                .bundles
                .iter()
                .find(|bundle| bundle.app_id == appid)
        })
        .and_then(|bundle| bundle.path.as_deref())
        .ok_or_else(|| anyhow!("lxapp '{appid}' has no local path in lingxia.yaml resources"))?;
    Ok(project_root.join(path))
}

fn drain_outgoing_messages(
    websocket: &mut WebSocket<TcpStream>,
    rx: &Receiver<DevSessionMessage>,
) -> Result<()> {
    while let Ok(message) = rx.try_recv() {
        send_wire_message(websocket, &message)?;
    }
    Ok(())
}

fn read_wire_message(websocket: &mut WebSocket<TcpStream>) -> Result<DevSessionMessage> {
    loop {
        let message = websocket.read()?;
        match parse_text_message(message)? {
            ParsedWireMessage::Wire(parsed) => return Ok(*parsed),
            ParsedWireMessage::Ignored => {}
            ParsedWireMessage::Closed => {
                return Err(anyhow!(
                    "websocket closed before receiving required message"
                ));
            }
        }
    }
}

enum ParsedWireMessage {
    Wire(Box<DevSessionMessage>),
    Ignored,
    Closed,
}

fn parse_text_message(message: Message) -> Result<ParsedWireMessage> {
    match message {
        Message::Text(text) => {
            let parsed = serde_json::from_str::<DevSessionMessage>(&text)
                .context("Failed to parse websocket JSON message")?;
            Ok(ParsedWireMessage::Wire(Box::new(parsed)))
        }
        Message::Binary(_) => Err(anyhow!("Binary websocket messages are not supported")),
        Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => Ok(ParsedWireMessage::Ignored),
        Message::Close(_) => Ok(ParsedWireMessage::Closed),
    }
}

pub fn send_wire_message<S>(websocket: &mut WebSocket<S>, message: &DevSessionMessage) -> Result<()>
where
    S: std::io::Read + std::io::Write,
{
    let text = serde_json::to_string(message).context("Failed to encode websocket JSON message")?;
    websocket
        .send(Message::Text(text.into()))
        .context("Failed to send websocket message")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        DevServerState, TEST_RUN_LEASE, accept_websocket, dev_port, handle_watch_request,
        persist_runtime_events, read_wire_message, refresh_lxapp_manifests, resolve_lxapp_dir,
        route_runtime_message, send_wire_message,
    };
    use lingxia_control_protocol::dev_session::{
        DEV_SESSION_PROTOCOL_VERSION, DevSessionMessage, DevSessionRole, WatchPauseArgs,
        capabilities,
    };
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, mpsc};
    use std::thread;
    use std::time::{Duration, Instant};

    fn authenticated_state() -> DevServerState {
        DevServerState::new(
            PathBuf::from("project"),
            Arc::new(AtomicBool::new(false)),
            Some("secret".to_string()),
            false,
        )
    }

    #[test]
    fn an_attach_needs_the_session_attach_token() {
        let state = authenticated_state();
        assert!(!state.attach_authorized(Some("anything")));
        *state.attach_token.lock().unwrap() = Some("attach".to_string());
        assert!(!state.attach_authorized(None));
        assert!(!state.attach_authorized(Some("secret")));
        assert!(state.attach_authorized(Some("attach")));
    }

    #[test]
    fn a_removed_mocks_dir_leaves_the_app_without_mocks() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        std::fs::write(
            root.join("lxapp.json"),
            r#"{ "appId": "demo.app", "appName": "Demo", "version": "1.0.0", "pages": [] }"#,
        )
        .unwrap();
        std::fs::create_dir_all(root.join("mocks")).unwrap();
        std::fs::write(
            root.join("mocks/index.ts"),
            "export default { 'GET **/a': { json: {} } };\n",
        )
        .unwrap();
        let state = authenticated_state();
        // Created after the session started: the reload loads it.
        state.reload_mocks("demo.app", root);
        assert_eq!(state.lock_mocks().len(), 1);
        std::fs::remove_dir_all(root.join("mocks")).unwrap();
        state.reload_mocks("demo.app", root);
        assert!(
            state.lock_mocks().is_empty(),
            "no mocks/ is reported as none, not the last loaded"
        );
    }

    fn desktop_state(stop: Arc<AtomicBool>) -> Arc<DevServerState> {
        Arc::new(DevServerState::new(
            PathBuf::from("project"),
            stop,
            None,
            true,
        ))
    }

    #[test]
    fn desktop_runtime_gone_ends_session_after_grace() {
        let stop = Arc::new(AtomicBool::new(false));
        let state = desktop_state(stop.clone());
        let (tx, _rx) = mpsc::channel();
        let (id, _) = state.claim_runtime_sender(tx);
        DevServerState::drop_runtime(&state, id);
        thread::sleep(Duration::from_millis(120));
        assert!(
            stop.load(std::sync::atomic::Ordering::Acquire),
            "closing the runtime should end a desktop/Runner session"
        );
    }

    #[test]
    fn desktop_runtime_reconnect_cancels_gone_teardown() {
        let stop = Arc::new(AtomicBool::new(false));
        let state = desktop_state(stop.clone());
        let (tx, _rx) = mpsc::channel();
        let (id, _) = state.claim_runtime_sender(tx);
        DevServerState::drop_runtime(&state, id);
        let (tx2, _rx2) = mpsc::channel();
        let _ = state.claim_runtime_sender(tx2);
        thread::sleep(Duration::from_millis(120));
        assert!(
            !stop.load(std::sync::atomic::Ordering::Acquire),
            "a replacement runtime during the grace window must keep the session"
        );
    }

    #[test]
    fn mobile_runtime_gone_does_not_end_session() {
        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::new(DevServerState::new(
            PathBuf::from("project"),
            stop.clone(),
            None,
            false,
        ));
        let (tx, _rx) = mpsc::channel();
        let (id, _) = state.claim_runtime_sender(tx);
        DevServerState::drop_runtime(&state, id);
        thread::sleep(Duration::from_millis(120));
        assert!(
            !stop.load(std::sync::atomic::Ordering::Acquire),
            "swiping a device app away must not kill lingxia dev"
        );
    }

    #[test]
    fn never_connected_runtime_does_not_end_session() {
        let stop = Arc::new(AtomicBool::new(false));
        let state = desktop_state(stop.clone());
        DevServerState::drop_runtime(&state, 1);
        thread::sleep(Duration::from_millis(80));
        assert!(!stop.load(std::sync::atomic::Ordering::Acquire));
    }

    #[test]
    fn stale_disconnect_preserves_replacement_pending_commands() {
        let state = desktop_state(Arc::new(AtomicBool::new(false)));
        let (tx, _rx) = mpsc::channel();
        let (old, _) = state.claim_runtime_sender(tx);
        let (tx, _rx) = mpsc::channel();
        state.claim_runtime_sender(tx);
        let (tx, _rx) = mpsc::channel();
        state.register_pending_result("replacement".into(), tx);
        DevServerState::drop_runtime(&state, old);
        assert!(state.take_pending_result("replacement").is_some());
        assert!(state.runtime_sender().is_some());
    }

    #[test]
    fn older_disconnect_cannot_expire_a_new_disconnect_grace() {
        let stop = Arc::new(AtomicBool::new(false));
        let state = desktop_state(stop.clone());
        let (tx, _rx) = mpsc::channel();
        let (old, _) = state.claim_runtime_sender(tx);
        let old_epoch = state.clear_runtime_sender(old).unwrap();
        let (tx, _rx) = mpsc::channel();
        let (new, _) = state.claim_runtime_sender(tx);
        state.finish_runtime_disconnect(old_epoch);
        assert!(!stop.load(std::sync::atomic::Ordering::Acquire));
        let new_epoch = state.clear_runtime_sender(new).unwrap();
        state.finish_runtime_disconnect(old_epoch);
        assert!(!stop.load(std::sync::atomic::Ordering::Acquire));
        state.finish_runtime_disconnect(new_epoch);
        assert!(stop.load(std::sync::atomic::Ordering::Acquire));
    }

    fn test_response(run_id: &str, state: &str) -> DevSessionMessage {
        DevSessionMessage::success(
            "cmd".to_string(),
            Some(serde_json::json!({ "run_id": run_id, "state": state })),
        )
    }

    #[test]
    fn relayed_test_run_is_active_until_a_terminal_poll() {
        use lingxia_control_protocol::methods::session::test;
        let state = authenticated_state();
        assert!(!state.test_run_active());

        state.observe_test_response(test::START, &test_response("run-1", "running"));
        assert!(state.test_run_active());
        state.observe_test_response(test::POLL, &test_response("run-1", "running"));
        assert!(state.test_run_active());
        // A stale run id cannot end the active run.
        state.observe_test_response(test::POLL, &test_response("run-0", "passed"));
        assert!(state.test_run_active());
        state.observe_test_response(test::POLL, &test_response("run-1", "failed"));
        assert!(!state.test_run_active());
    }

    #[test]
    fn a_restart_waits_for_a_run_that_started_during_the_rebuild() {
        use lingxia_control_protocol::methods::session::test;
        let state = authenticated_state();
        let (tx, rx) = mpsc::channel();
        state.claim_runtime_sender(tx);
        // The watcher saw no pause and started rebuilding; `lxdev test` took
        // its lease and started a run meanwhile.
        let _ = state.pause_watch(pause("lxdev-test-1", 300_000, None));
        state.observe_test_response(test::START, &test_response("run-1", "running"));
        assert_eq!(
            state.restart_lxapp("demo").unwrap(),
            super::RestartOutcome::Deferred
        );
        assert!(rx.try_recv().is_err(), "no restart reaches the runtime");
    }

    #[test]
    fn a_test_start_waits_for_a_rebuild_in_progress() {
        use lingxia_control_protocol::methods::session::test;
        let state = Arc::new(authenticated_state());
        let rebuilding = state.lock_build();
        let relay = |method: &'static str| {
            let state = Arc::clone(&state);
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let _guards = state.lock_for_relay(method);
                tx.send(()).unwrap();
            });
            rx
        };
        // Other requests are not held up by a rebuild.
        relay(test::POLL)
            .recv_timeout(Duration::from_secs(5))
            .expect("a poll is relayed during a rebuild");
        // A start would move the app onto an isolated profile, reopening it
        // from a half-written build output: it waits for the rebuild.
        let start = relay(test::START);
        assert!(start.recv_timeout(Duration::from_millis(200)).is_err());
        drop(rebuilding);
        start
            .recv_timeout(Duration::from_secs(5))
            .expect("the start is relayed once the rebuild ends");
    }

    fn pause(lease: &str, ttl_ms: u64, run_id: Option<&str>) -> WatchPauseArgs {
        WatchPauseArgs {
            lease: lease.to_string(),
            ttl_ms,
            run_id: run_id.map(str::to_string),
        }
    }

    #[test]
    fn a_watch_pause_holds_until_resumed_and_counts_leases() {
        let state = authenticated_state();
        assert!(!state.watch_paused());
        assert!(state.pause_watch(pause("a", 60_000, None)).paused);
        assert_eq!(state.pause_watch(pause("b", 60_000, None)).leases, 2);
        assert!(state.resume_watch("a").paused);
        assert!(state.watch_paused());
        let status = state.resume_watch("b");
        assert!(!status.paused && status.leases == 0);
        assert!(!state.watch_paused());
        // Resuming twice, or an unknown lease, is harmless.
        assert!(!state.resume_watch("b").paused);
    }

    #[test]
    fn a_watch_pause_lapses_without_renewal() {
        let state = authenticated_state();
        state.pause_watch(pause("a", 1, None));
        std::thread::sleep(Duration::from_millis(10));
        assert!(
            !state.watch_paused(),
            "a client that died cannot hold the watcher"
        );
        // A renewal extends it.
        state.pause_watch(pause("b", 60_000, None));
        state.pause_watch(pause("b", 60_000, None));
        assert!(state.watch_paused());
    }

    #[test]
    fn a_run_bound_pause_is_released_by_every_terminal_state() {
        use lingxia_control_protocol::methods::session::test;
        for terminal in ["passed", "failed", "timed_out", "cancelled", "error"] {
            let state = authenticated_state();
            state.pause_watch(pause("lease", 60_000, None));
            // Bound once the run exists; the binding survives renewals.
            state.pause_watch(pause("lease", 60_000, Some("run-1")));
            state.pause_watch(pause("lease", 60_000, None));
            state.observe_test_response(test::POLL, &test_response("run-1", "running"));
            assert!(state.watch_paused(), "{terminal}");
            // Another run's end does not release it.
            state.observe_test_response(test::POLL, &test_response("run-0", terminal));
            assert!(state.watch_paused(), "{terminal}");
            state.observe_test_response(test::POLL, &test_response("run-1", terminal));
            assert!(!state.watch_paused(), "{terminal} releases the pause");
        }
        // A cancel reply that already reports the end releases it too.
        let state = authenticated_state();
        state.pause_watch(pause("lease", 60_000, Some("run-2")));
        state.observe_test_response(test::CANCEL, &test_response("run-2", "cancelled"));
        assert!(!state.watch_paused());
    }

    #[test]
    fn a_restart_waits_while_the_watch_is_paused() {
        let state = authenticated_state();
        let (tx, rx) = mpsc::channel();
        state.claim_runtime_sender(tx);
        state.pause_watch(pause("lease", 60_000, None));
        assert_eq!(
            state.restart_lxapp("demo").unwrap(),
            super::RestartOutcome::Deferred
        );
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn watch_requests_are_answered_by_the_server() {
        use lingxia_control_protocol::methods::session::watch;
        let state = authenticated_state();
        let reply = handle_watch_request(
            &state,
            "1",
            watch::PAUSE,
            Some(serde_json::json!({ "lease": "l", "ttl_ms": 60000 })),
        );
        let DevSessionMessage::Response(response) = reply else {
            panic!("expected a response");
        };
        assert_eq!(response.result.unwrap()["paused"], true);
        let reply = handle_watch_request(&state, "2", watch::RESUME, Some(serde_json::json!({})));
        let DevSessionMessage::Response(response) = reply else {
            panic!("expected a response");
        };
        assert_eq!(response.error.unwrap().code, "invalid_params");
    }

    #[test]
    fn unpolled_test_run_stops_deferring_after_the_lease() {
        let state = authenticated_state();
        *state.lock_active_test_run() = Some((
            "run-1".to_string(),
            Instant::now() - TEST_RUN_LEASE - Duration::from_secs(1),
        ));
        assert!(!state.test_run_active());
    }

    #[test]
    fn a_disconnect_during_a_run_waits_longer_and_reports_it() {
        use lingxia_control_protocol::methods::session::test;
        let stop = Arc::new(AtomicBool::new(false));
        let state = desktop_state(stop.clone());
        let (tx, _rx) = mpsc::channel();
        let (id, _) = state.claim_runtime_sender(tx);
        state.observe_test_response(test::START, &test_response("run-1", "running"));
        DevServerState::drop_runtime(&state, id);
        // Past the plain grace, still inside the run grace.
        thread::sleep(Duration::from_millis(120));
        assert!(
            !stop.load(std::sync::atomic::Ordering::Acquire),
            "an active run extends the grace"
        );
        match state.reconnecting_poll_error("p1".into(), "run-1") {
            Some(DevSessionMessage::Response(response)) => {
                assert_eq!(response.error.unwrap().code, super::RUNTIME_RECONNECTING);
            }
            other => panic!("expected a reconnecting error, got {other:?}"),
        }
        assert!(
            state
                .reconnecting_poll_error("p2".into(), "run-0")
                .is_none()
        );

        // The runtime comes back: the run is active again, the session lives.
        let (tx2, _rx2) = mpsc::channel();
        let _ = state.claim_runtime_sender(tx2);
        assert!(state.test_run_active());
        thread::sleep(Duration::from_millis(350));
        assert!(!stop.load(std::sync::atomic::Ordering::Acquire));
        assert!(
            state
                .reconnecting_poll_error("p3".into(), "run-1")
                .is_none()
        );
    }

    #[test]
    fn a_runtime_that_stays_away_ends_the_run_and_session() {
        use lingxia_control_protocol::methods::session::test;
        let stop = Arc::new(AtomicBool::new(false));
        let state = desktop_state(stop.clone());
        let (tx, _rx) = mpsc::channel();
        let (id, _) = state.claim_runtime_sender(tx);
        state.observe_test_response(test::START, &test_response("run-1", "running"));
        DevServerState::drop_runtime(&state, id);
        thread::sleep(Duration::from_millis(450));
        assert!(stop.load(std::sync::atomic::Ordering::Acquire));
        assert!(state.reconnecting_poll_error("p".into(), "run-1").is_none());
    }

    #[test]
    fn runtime_disconnect_ends_the_tracked_test_run() {
        use lingxia_control_protocol::methods::session::test;
        let state = authenticated_state();
        let (tx, _rx) = mpsc::channel();
        let (runtime_id, _) = state.claim_runtime_sender(tx);
        state.observe_test_response(test::START, &test_response("run-1", "running"));
        assert!(state.test_run_active());
        state.clear_runtime_sender(runtime_id);
        assert!(!state.test_run_active());
    }

    #[test]
    fn reload_manifest_refresh_tracks_rebuilt_dist() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path();
        let dist = project.join("lxapp/dist");
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::write(
            project.join("lingxia.yaml"),
            "app:\n  projectName: demo-host\n  packageId: app.example.demohost\n  productName: Demo Host\n  productVersion: 1.0.0\n  platforms: [windows]\n  homeAppId: demo\nresources:\n  bundles:\n    - type: lxapp\n      appId: demo\n      path: lxapp\n",
        )
        .unwrap();
        std::fs::write(
            dist.join("lxapp.json"),
            r#"{"appId":"demo","version":"1.0.0","pages":{}}"#,
        )
        .unwrap();
        std::fs::write(dist.join("logic.js"), "Page({ value: 1 })").unwrap();

        refresh_lxapp_manifests(project).unwrap();
        let manifest_path = project.join(".lingxia/dev/lxapp/demo/manifest.json");
        let first: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();

        std::fs::write(dist.join("logic.js"), "Page({ value: 2 })").unwrap();
        refresh_lxapp_manifests(project).unwrap();
        let second: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();

        assert_ne!(first["distHash"], second["distHash"]);
    }

    #[test]
    fn reload_builds_the_selected_lxapp_resource() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path();
        std::fs::write(
            project.join("lingxia.yaml"),
            "app:\n  projectName: demo-host\n  packageId: app.example.demohost\n  productName: Demo Host\n  productVersion: 1.0.0\n  platforms: [windows]\n  homeAppId: demo\nresources:\n  bundles:\n    - type: lxapp\n      appId: demo\n      path: home\n    - type: lxapp\n      appId: settings\n      path: settings\n",
        )
        .unwrap();

        assert_eq!(
            resolve_lxapp_dir(project, Some("settings")).unwrap(),
            project.join("settings")
        );
        assert_eq!(
            resolve_lxapp_dir(project, None).unwrap(),
            project.join("home")
        );
    }

    #[test]
    fn websocket_auth_uses_handshake_token() {
        let state = authenticated_state();

        assert!(state.authorizes(Some("secret")));
        assert!(!state.authorizes(None));
        assert!(!state.authorizes(Some("wrong")));
    }

    #[test]
    fn websocket_upgrade_captures_token_independently_of_hello() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let (mut websocket, handshake_token) = accept_websocket(stream).unwrap();
            let hello = read_wire_message(&mut websocket).unwrap();
            let DevSessionMessage::Hello {
                version,
                role,
                capabilities: peer_capabilities,
                ..
            } = hello
            else {
                panic!("expected hello");
            };
            assert_eq!(version, DEV_SESSION_PROTOCOL_VERSION);
            assert_eq!(role, DevSessionRole::Runtime);
            assert_eq!(peer_capabilities, [capabilities::REQUESTS]);
            authenticated_state().authorizes(handshake_token.as_deref())
        });

        let url = format!("ws://{addr}/?token=secret");
        let (mut websocket, _) = tungstenite::connect(&url).unwrap();
        send_wire_message(
            &mut websocket,
            &DevSessionMessage::Hello {
                version: DEV_SESSION_PROTOCOL_VERSION,
                role: DevSessionRole::Runtime,
                capabilities: vec![capabilities::REQUESTS.to_string()],
                build: None,
            },
        )
        .unwrap();

        assert!(server.join().unwrap());
    }

    #[test]
    fn slow_session_log_persistence_does_not_block_runtime_responses() {
        let state = Arc::new(authenticated_state());
        let (event_tx, event_rx) = mpsc::sync_channel(1);
        let (persistence_started_tx, persistence_started_rx) = mpsc::channel();
        let (release_persistence_tx, release_persistence_rx) = mpsc::channel();
        let (response_tx, response_rx) = mpsc::channel();
        state.register_pending_result("poll".to_string(), response_tx);

        thread::scope(|scope| {
            let persistence = scope.spawn(move || {
                persist_runtime_events(event_rx, |_| {
                    persistence_started_tx.send(()).unwrap();
                    release_persistence_rx.recv().unwrap();
                    Ok(())
                })
            });
            let mut dropped_event_batch_warning = false;
            route_runtime_message(
                DevSessionMessage::EventBatch { events: Vec::new() },
                &state,
                &event_tx,
                &mut dropped_event_batch_warning,
            )
            .unwrap();
            persistence_started_rx
                .recv_timeout(Duration::from_secs(1))
                .unwrap();

            route_runtime_message(
                DevSessionMessage::success("poll".to_string(), None),
                &state,
                &event_tx,
                &mut dropped_event_batch_warning,
            )
            .unwrap();
            assert!(
                response_rx.recv_timeout(Duration::from_millis(100)).is_ok(),
                "runtime response waited for session log persistence"
            );

            release_persistence_tx.send(()).unwrap();
            drop(event_tx);
            persistence.join().unwrap().unwrap();
        });
    }

    #[test]
    fn dev_port_is_stable_and_in_range() {
        let port = dev_port("com.example.app", "android");
        assert_eq!(port, dev_port("com.example.app", "android"));
        assert!((39_000..40_000).contains(&port));
    }

    #[test]
    fn dev_port_distinguishes_platforms() {
        assert_ne!(
            dev_port("com.example.app", "android"),
            dev_port("com.example.app", "ios")
        );
    }
}

#[cfg(test)]
mod large_frame_tests {
    #[test]
    fn accepts_runtime_artifact_frames_above_tungstenite_default() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let bytes = 17 * 1024 * 1024;
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let (mut socket, _) = super::accept_websocket(stream).unwrap();
            assert_eq!(socket.read().unwrap().to_text().unwrap().len(), bytes);
        });
        let (mut socket, _) = tungstenite::connect(format!("ws://{address}")).unwrap();
        socket
            .send(tungstenite::Message::Text("a".repeat(bytes).into()))
            .unwrap();
        server.join().unwrap();
    }
}
