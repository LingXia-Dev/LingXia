//! Binds lxapp GPU pages to a platform's native page shells: the creation
//! handshake, the live registry, dispose-once leases, the connection-generation
//! gate, image loads and the View worker key. Platform modules
//! only marshal calls between these functions and their native shells.
// Off Android it is compiled only for its tests, which exercise part of it.
#![cfg_attr(not(target_os = "android"), allow(dead_code, unused_imports))]
use lxapp::{
    GpuChunkReader, GpuPageView, GpuPageViewFactory, GpuStateSignal, GpuStateWrite,
    GpuViewCreation, GpuViewGeneration, GpuViewMessage, LeaveReason, PageInstance,
    PageViewEnvironment,
};
use std::{
    collections::HashMap,
    sync::{
        Arc, LazyLock, Mutex, MutexGuard, Weak,
        atomic::{AtomicBool, AtomicI64, Ordering},
    },
};
use tokio::sync::oneshot;

#[cfg(feature = "automation")]
pub(crate) mod automation;
mod images;
pub(crate) use images::{load_image, video_url};
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "macos",
    target_env = "ohos",
    windows
))]
pub(crate) mod state;
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "macos",
    target_env = "ohos",
    windows
))]
pub(crate) mod webviews;

/// Identifies one native page shell from creation to disposal.
pub(crate) type Token = i64;

/// Answers one automation request; called once, on any thread.
pub(crate) type AutomationReply = Box<dyn FnOnce(Result<serde_json::Value, String>) + Send>;

/// A native shell's JSON reply: `{"value": ...}` or `{"error": "..."}`.
pub(crate) fn decode_reply(reply: &str) -> Result<serde_json::Value, String> {
    let reply: serde_json::Value =
        serde_json::from_str(reply).map_err(|e| format!("GPU automation reply: {e}"))?;
    match reply["error"].as_str() {
        Some(error) => Err(error.to_owned()),
        None => Ok(reply["value"].clone()),
    }
}

/// A platform's native page shell. Calls enqueue on its UI thread without blocking.
pub(crate) trait NativePage: Send + Sync + 'static {
    fn connect(&self, generation: GpuViewGeneration) -> Result<(), lingxia_error::Error>;
    /// Any thread: a typed Logic message, straight into the View host.
    fn deliver(
        &self,
        generation: GpuViewGeneration,
        message: GpuViewMessage,
    ) -> Result<(), lingxia_error::Error>;
    /// Any thread: typed Logic state, straight into the View host.
    fn write_state(
        &self,
        generation: GpuViewGeneration,
        write: GpuStateWrite,
    ) -> Result<(), lingxia_error::Error>;
    fn request_leave(
        &self,
        generation: GpuViewGeneration,
        reason: &str,
    ) -> Result<(), lingxia_error::Error>;
    fn update_environment(&self, update: &serde_json::Value) -> Result<(), lingxia_error::Error>;
    fn reload(&self, source: &str) -> Result<(), lingxia_error::Error>;
    /// Dev sessions: replace the module keeping page state (`ViewHost::swap_module`);
    /// a shell without it reloads.
    #[cfg_attr(not(feature = "devtool"), allow(dead_code))]
    fn swap_module(&self, source: &str) -> Result<(), lingxia_error::Error> {
        self.reload(source)
    }
    fn park(&self) -> Result<(), lingxia_error::Error>;
    fn page_left(&self) -> Result<(), lingxia_error::Error> {
        Ok(())
    }
    fn retain_surface(&self, retained: bool) -> Result<(), lingxia_error::Error>;
    fn trim_surface(&self) -> Result<(), lingxia_error::Error> {
        Ok(())
    }
    /// `{op: query|act|screenshot, css, index, all, maxText, action: {kind, text, force}}`,
    /// answered after the presented frame.
    #[cfg_attr(not(feature = "automation"), allow(dead_code))]
    fn automation(&self, request: serde_json::Value, reply: AutomationReply);
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Creates native shells; each completes through [`created`] with its token.
pub(crate) trait NativeFactory: Send + Sync + 'static {
    fn create(
        &self,
        token: Token,
        page: &PageInstance,
        request: Creation,
    ) -> Result<(), lingxia_error::Error>;
    /// Release the shell for `token`, created or still pending.
    fn dispose(&self, token: Token);
}

/// What a platform needs to build a native shell.
pub(crate) struct Creation {
    pub source: String,
    /// Pages with the same key share one View worker and isolate.
    pub worker: String,
    pub logic: bool,
}

type Created = Result<Arc<dyn NativePage>, String>;

#[derive(Default)]
struct Registry {
    pending: HashMap<Token, oneshot::Sender<Created>>,
    live: HashMap<Token, Weak<Endpoint>>,
    /// Page instance id to its live token.
    pages: HashMap<String, Token>,
}

static REGISTRY: LazyLock<Mutex<Registry>> = LazyLock::new(Default::default);
static NEXT: AtomicI64 = AtomicI64::new(1);

fn registry() -> MutexGuard<'static, Registry> {
    REGISTRY.lock().unwrap_or_else(|p| p.into_inner())
}

/// The worker key: pages of one app session share a View isolate.
pub(crate) fn worker_key(appid: &str, session: u64) -> String {
    format!("{appid}:{session}")
}

/// Live pages per View worker key: an app session's theme pack lasts while any page holds it,
/// so a relaunch's new page keeps the pack the outgoing pages release.
static PACK_HOLDERS: LazyLock<Mutex<HashMap<String, usize>>> = LazyLock::new(Default::default);

/// Disposes its native shell exactly once, however it is released.
struct Lease {
    token: Token,
    page: String,
    worker: String,
    disposed: AtomicBool,
    factory: Arc<dyn NativeFactory>,
}

impl Lease {
    /// Holds the app session's pack, registering `pack` before the page's host reads it.
    fn new(
        token: Token,
        page: String,
        worker: String,
        pack: Option<lingxia_theme::Pack>,
        chunks: GpuChunkReader,
        factory: Arc<dyn NativeFactory>,
    ) -> Self {
        let mut holders = PACK_HOLDERS.lock().unwrap_or_else(|p| p.into_inner());
        *holders.entry(worker.clone()).or_default() += 1;
        install_app_chunks(&worker, chunks);
        if let Some(pack) = pack {
            lingxia_theme::set_app_pack(&worker, Some(pack));
        }
        Self {
            token,
            page,
            worker,
            disposed: AtomicBool::new(false),
            factory,
        }
    }

    fn dispose(&self) {
        if self.disposed.swap(true, Ordering::AcqRel) {
            return;
        }
        {
            let mut holders = PACK_HOLDERS.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(count) = holders.get_mut(&self.worker) {
                *count -= 1;
                if *count == 0 {
                    holders.remove(&self.worker);
                    lingxia_theme::set_app_pack(&self.worker, None);
                    clear_app_chunks(&self.worker);
                }
            }
        }
        #[cfg(any(
            target_os = "android",
            target_os = "ios",
            target_os = "macos",
            target_env = "ohos",
            windows
        ))]
        webviews::clear_webviews(self.token, None);
        {
            let mut registry = registry();
            registry.pending.remove(&self.token);
            registry.live.remove(&self.token);
            if registry.pages.get(&self.page) == Some(&self.token) {
                registry.pages.remove(&self.page);
            }
        }
        // The factory may call host code; never while holding the registry.
        self.factory.dispose(self.token);
    }

    fn disposed(&self) -> bool {
        self.disposed.load(Ordering::Acquire)
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.dispose();
    }
}

pub(crate) struct Endpoint {
    lease: Lease,
    native: Arc<dyn NativePage>,
    generation: Mutex<Option<GpuViewGeneration>>,
    /// Registered with lingxia-automation; unregisters when the endpoint drops.
    #[cfg(feature = "automation")]
    _automation: Arc<automation::GpuAutomation<dyn NativePage>>,
}

impl Endpoint {
    fn generation(&self) -> MutexGuard<'_, Option<GpuViewGeneration>> {
        self.generation.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The connection generation is current and the shell is not disposed.
    fn current(&self, generation: u64) -> Option<GpuViewGeneration> {
        let current = *self.generation();
        current.filter(|current| !self.lease.disposed() && current.get() == generation)
    }
}

impl GpuPageView for Endpoint {
    fn write_state(
        &self,
        generation: GpuViewGeneration,
        write: GpuStateWrite,
    ) -> Result<(), lingxia_error::Error> {
        if self.current(generation.get()).is_none() {
            return Ok(());
        }
        self.native.write_state(generation, write)
    }

    fn deliver(
        &self,
        generation: GpuViewGeneration,
        message: GpuViewMessage,
    ) -> Result<(), lingxia_error::Error> {
        if self.current(generation.get()).is_none() {
            return Ok(());
        }
        self.native.deliver(generation, message)
    }

    fn connect(&self, generation: GpuViewGeneration) -> Result<(), lingxia_error::Error> {
        {
            let mut current = self.generation();
            if current.is_some_and(|current| current.get() > generation.get()) {
                return Err(lingxia_error::Error::internal(
                    "GPU connection was superseded",
                ));
            }
            *current = Some(generation);
        }
        self.native.connect(generation)
    }

    fn request_leave(&self, reason: LeaveReason) -> Result<(), lingxia_error::Error> {
        let Some(generation) = *self.generation() else {
            return Ok(());
        };
        if self.lease.disposed() {
            return Ok(());
        }
        let reason = match reason {
            LeaveReason::Back => "back",
            LeaveReason::Home => "home",
        };
        self.native.request_leave(generation, reason)
    }

    fn update_environment(&self, update: PageViewEnvironment) -> Result<(), lingxia_error::Error> {
        if self.lease.disposed() {
            return Ok(());
        }
        let update = match update {
            PageViewEnvironment::DisplayLanguage {
                language,
                content,
                revision,
            } => serde_json::json!({
                "kind": "language",
                "revision": revision,
                "language": language,
                "contentLanguage": content,
            }),
            PageViewEnvironment::SurfaceContext { context, revision } => {
                serde_json::json!({"kind":"surface","revision":revision,"context":context})
            }
            PageViewEnvironment::PageChrome { capsule, revision } => serde_json::json!({
                "kind": "chrome",
                "revision": revision,
                "capsule": capsule.map(|[x, y, width, height]| {
                    serde_json::json!({"x": x, "y": y, "width": width, "height": height})
                }),
            }),
            PageViewEnvironment::TextScaleLimit { max } => {
                serde_json::json!({"kind": "textScaleLimit", "max": max})
            }
            PageViewEnvironment::Contrast { more, revision } => {
                serde_json::json!({"kind": "contrast", "revision": revision, "more": more})
            }
        };
        self.native.update_environment(&update)
    }

    fn reload(&self) -> Result<(), lingxia_error::Error> {
        let page = lxapp::find_page_by_instance_id(&self.lease.page)
            .ok_or_else(|| lingxia_error::Error::internal("GPU page was removed"))?;
        let source = page.gpu_module_source()?;
        *self.generation() = None;
        self.native.reload(&source)
    }

    fn park(&self) -> Result<(), lingxia_error::Error> {
        *self.generation() = None;
        self.native.park()
    }

    fn page_left(&self) -> Result<(), lingxia_error::Error> {
        if self.lease.disposed() {
            return Ok(());
        }
        self.native.page_left()
    }

    fn retain_surface(&self, retained: bool) -> Result<(), lingxia_error::Error> {
        if self.lease.disposed() {
            return Ok(());
        }
        self.native.retain_surface(retained)
    }

    fn trim_surface(&self) -> Result<(), lingxia_error::Error> {
        if self.lease.disposed() {
            return Ok(());
        }
        self.native.trim_surface()
    }

    fn dispose(&self) {
        self.lease.dispose();
    }
}

/// The lxapp-facing factory over a platform's native shells.
pub(crate) struct Binding(pub Arc<dyn NativeFactory>);

impl GpuPageViewFactory for Binding {
    fn create(&self, page: PageInstance) -> GpuViewCreation {
        let factory = self.0.clone();
        Box::pin(async move {
            let source = page.gpu_module_source()?;
            let token = NEXT.fetch_add(1, Ordering::Relaxed);
            let worker = worker_key(&page.appid(), page.app_session_id());
            // Hosts read the pack when they are created, so the first frame already carries it.
            let lease = Lease::new(
                token,
                page.instance_id_string(),
                worker.clone(),
                page.gpu_theme_pack().as_deref().and_then(bundled_pack),
                page.gpu_chunk_reader(),
                factory.clone(),
            );
            let (tx, rx) = oneshot::channel();
            registry().pending.insert(token, tx);
            factory.create(
                token,
                &page,
                Creation {
                    source,
                    worker,
                    logic: page.logic_enabled(),
                },
            )?;
            let native = rx
                .await
                .map_err(|_| lingxia_error::Error::internal("GPU creation cancelled"))?
                .map_err(lingxia_error::Error::internal)?;
            Ok(register(lease, native) as Arc<dyn GpuPageView>)
        })
    }

    fn app_closed(&self, appid: &str, session: u64) {
        close_worker(&worker_key(appid, session));
    }
}

/// Registers the lxapp's chunk reader for `key` so QuickJS resolves `lx:view/*` from dist.
fn install_app_chunks(key: &str, reader: GpuChunkReader) {
    #[cfg(target_os = "android")]
    use lingxia_view_android::lingxia_view;
    #[cfg(any(target_os = "ios", target_os = "macos"))]
    use lingxia_view_apple::lingxia_view;
    #[cfg(target_env = "ohos")]
    use lingxia_view_harmony::lingxia_view;
    #[cfg(windows)]
    use lingxia_view_windows::lingxia_view;
    #[cfg(any(
        target_os = "android",
        target_os = "ios",
        target_os = "macos",
        target_env = "ohos",
        windows
    ))]
    {
        let loader: lingxia_view::ChunkLoader = Arc::new(move |path| match reader(path) {
            Ok(source) => Ok(source),
            Err(error) if error.is(lingxia_error::code::NOT_FOUND) => {
                Err(lingxia_view::ChunkLoadError::NotFound)
            }
            Err(_) => Err(lingxia_view::ChunkLoadError::IntegrityMismatch),
        });
        lingxia_view::set_app_chunks(key, Some(loader));
    }
    #[cfg(not(any(
        target_os = "android",
        target_os = "ios",
        target_os = "macos",
        target_env = "ohos",
        windows
    )))]
    let _ = (key, reader);
}

fn clear_app_chunks(key: &str) {
    #[cfg(target_os = "android")]
    lingxia_view_android::lingxia_view::set_app_chunks(key, None);
    #[cfg(any(target_os = "ios", target_os = "macos"))]
    lingxia_view_apple::lingxia_view::set_app_chunks(key, None);
    #[cfg(target_env = "ohos")]
    lingxia_view_harmony::lingxia_view::set_app_chunks(key, None);
    #[cfg(windows)]
    lingxia_view_windows::lingxia_view::set_app_chunks(key, None);
    #[cfg(not(any(
        target_os = "android",
        target_os = "ios",
        target_os = "macos",
        target_env = "ohos",
        windows
    )))]
    let _ = key;
}

/// Stops the session's View worker once its pages are gone instead of keeping it for a next page.
fn close_worker(key: &str) {
    #[cfg(target_os = "android")]
    lingxia_view_android::lingxia_view::session::host::ViewWorker::close(key);
    #[cfg(any(target_os = "ios", target_os = "macos"))]
    lingxia_view_apple::lingxia_view::session::host::ViewWorker::close(key);
    #[cfg(target_env = "ohos")]
    lingxia_view_harmony::lingxia_view::session::host::ViewWorker::close(key);
    #[cfg(windows)]
    lingxia_view_windows::lingxia_view::session::host::ViewWorker::close(key);
    #[cfg(not(any(
        target_os = "android",
        target_os = "ios",
        target_os = "macos",
        target_env = "ohos",
        windows
    )))]
    let _ = key;
}

/// The lxapp's bundled theme; the host checks and resolves it, and a bad one keeps the
/// default palette.
fn bundled_pack(pack: &str) -> Option<lingxia_theme::Pack> {
    serde_json::from_str(pack)
        .inspect_err(|error| log::warn!("theme.json is not a theme: {error}"))
        .ok()
}

fn register(lease: Lease, native: Arc<dyn NativePage>) -> Arc<Endpoint> {
    let (token, page) = (lease.token, lease.page.clone());
    #[cfg(feature = "automation")]
    let automation = {
        let automation = Arc::new(automation::GpuAutomation {
            native: Arc::downgrade(&native),
        });
        let backend: Arc<dyn lingxia_automation::gpu::GpuPageAutomation> = automation.clone();
        lingxia_automation::gpu::register(&page, Arc::downgrade(&backend));
        automation
    };
    let endpoint = Arc::new(Endpoint {
        lease,
        native,
        generation: Mutex::new(None),
        #[cfg(feature = "automation")]
        _automation: automation,
    });
    let mut registry = registry();
    registry.live.insert(token, Arc::downgrade(&endpoint));
    registry.pages.insert(page, token);
    endpoint
}

/// Dev sessions: swap the module of every live GPU page `select` returns a
/// source for (by page instance id). Returns how many pages took it.
#[cfg_attr(not(feature = "devtool"), allow(dead_code))]
fn swap_pages(select: impl Fn(&str) -> Option<String>) -> Result<usize, lingxia_error::Error> {
    let pages: Vec<(String, Token)> = registry()
        .pages
        .iter()
        .map(|(id, token)| (id.clone(), *token))
        .collect();
    let mut swapped = 0;
    for (id, token) in pages {
        let (Some(source), Some(endpoint)) = (select(&id), live(token)) else {
            continue;
        };
        endpoint.native.swap_module(&source)?;
        swapped += 1;
    }
    Ok(swapped)
}

/// Dev sessions: the pages of `appid` at `path` take the module now in the bundle. A page
/// that could not swap in place reloads with it, and that is returned as the error.
#[cfg_attr(not(feature = "devtool"), allow(dead_code))]
pub(crate) fn hot_swap(appid: &str, path: &str) -> Result<usize, lingxia_error::Error> {
    #[cfg(all(
        feature = "devtool",
        any(
            target_os = "android",
            target_os = "ios",
            target_os = "macos",
            target_env = "ohos",
            windows
        )
    ))]
    state::install_swap_reporter();
    let swaps = swap_reports::Swaps::new();
    let swapped = swap_pages(|id| {
        let page = lxapp::find_page_by_instance_id(id)?;
        (page.appid() == appid && page_key(&page.path()) == page_key(path))
            .then(|| page.gpu_module_source().ok())
            .flatten()
            .inspect(|source| swaps.expect(source))
    })?;
    swaps.outcome(swapped, swap_reports::WAIT)?;
    Ok(swapped)
}

/// Dev sessions: swaps waiting to hear from the View how they ended, by the module they
/// swap in.
#[cfg_attr(not(feature = "devtool"), allow(dead_code))]
mod swap_reports {
    use std::hash::{Hash, Hasher};
    use std::sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    };
    use std::time::{Duration, Instant};

    /// Within the dev server's wait for the swap's reply.
    pub(super) const WAIT: Duration = Duration::from_secs(5);

    type Outcome = Result<(), String>;
    /// `(module key, swap call, its channel)`
    static WAITING: Mutex<Vec<(u64, u64, mpsc::Sender<Outcome>)>> = Mutex::new(Vec::new());
    static CALLS: AtomicU64 = AtomicU64::new(0);

    fn key(source: &str) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        source.hash(&mut hasher);
        hasher.finish()
    }

    fn waiting() -> std::sync::MutexGuard<'static, Vec<(u64, u64, mpsc::Sender<Outcome>)>> {
        WAITING
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The View swapped `source` into one page, or loaded it afresh.
    pub(crate) fn report(source: &str, outcome: Outcome) {
        let key = key(source);
        let mut waiting = waiting();
        if let Some(index) = waiting.iter().position(|(k, ..)| *k == key) {
            let (_, _, sender) = waiting.remove(index);
            let _ = sender.send(outcome);
        }
    }

    pub(super) struct Swaps {
        call: u64,
        sender: mpsc::Sender<Outcome>,
        receiver: mpsc::Receiver<Outcome>,
    }

    impl Swaps {
        pub(super) fn new() -> Self {
            let (sender, receiver) = mpsc::channel();
            Self {
                call: CALLS.fetch_add(1, Ordering::Relaxed),
                sender,
                receiver,
            }
        }

        /// Registered before the swap starts: a fast worker may report before it returns.
        pub(super) fn expect(&self, source: &str) {
            waiting().push((key(source), self.call, self.sender.clone()));
        }

        /// How `pages` swaps ended; one that has not reported by `wait` counts as landed.
        pub(super) fn outcome(
            self,
            pages: usize,
            wait: Duration,
        ) -> Result<(), lingxia_error::Error> {
            let deadline = Instant::now() + wait;
            let mut errors = Vec::new();
            for _ in 0..pages {
                let left = deadline.saturating_duration_since(Instant::now());
                match self.receiver.recv_timeout(left) {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => errors.push(error),
                    Err(_) => break,
                }
            }
            if errors.is_empty() {
                return Ok(());
            }
            errors.dedup();
            Err(lingxia_error::Error::internal(format!(
                "could not update in place, reloaded (local state lost): {}",
                errors.join("; ")
            )))
        }
    }

    impl Drop for Swaps {
        fn drop(&mut self) {
            let call = self.call;
            waiting().retain(|(_, c, _)| *c != call);
        }
    }
}

/// A page route without query, leading slash or module extension: the runtime knows
/// `pages/a/index.js`, the dev CLI names the source `pages/a/index`.
#[cfg_attr(not(feature = "devtool"), allow(dead_code))]
fn page_key(path: &str) -> &str {
    let path = path.split('?').next().unwrap_or("").trim_start_matches('/');
    match path.rsplit_once('.') {
        Some((base, ext)) if !ext.contains('/') => base,
        _ => path,
    }
}

fn live(token: Token) -> Option<Arc<Endpoint>> {
    registry()
        .live
        .get(&token)
        .and_then(Weak::upgrade)
        .filter(|endpoint| !endpoint.lease.disposed())
}

/// A native shell finished creation (or failed to).
pub(crate) fn created(token: Token, outcome: Created) {
    let pending = registry().pending.remove(&token);
    if let Some(pending) = pending {
        let _ = pending.send(outcome);
    }
}

/// The View worker: what the View sends Logic for its connection generation, in order.
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "macos",
    target_env = "ohos",
    windows
))]
pub(crate) fn view_events(token: Token, generation: u64, messages: Vec<state::ToLogic>) {
    let Some(endpoint) = live(token) else {
        return;
    };
    let Some(current) = endpoint.current(generation) else {
        return;
    };
    let Some(page) = lxapp::find_page_by_instance_id(&endpoint.lease.page) else {
        return;
    };
    let endpoint = endpoint as Arc<dyn GpuPageView>;
    for message in messages {
        if let Err(error) = page.gpu_view_event(&endpoint, current, state::event_from_view(message))
        {
            log::warn!("GPU bridge ingress rejected: {error}");
        }
    }
}

/// What the View made of Logic's typed state writes for its connection generation.
pub(crate) fn state_signal(token: Token, generation: u64, signal: GpuStateSignal) {
    let Some(endpoint) = live(token) else {
        return;
    };
    let Some(current) = endpoint.current(generation) else {
        return;
    };
    let Some(page) = lxapp::find_page_by_instance_id(&endpoint.lease.page) else {
        return;
    };
    if let Err(error) = page.gpu_state_signal(&(endpoint as Arc<dyn GpuPageView>), current, signal)
    {
        log::warn!("GPU state signal rejected: {error}");
    }
}

/// A frame for `generation` reached the screen; generation 0 is a standalone page. True once
/// the page took it as its first frame, or can never take one (it left); false while the frame
/// is not its current view's, so the platform keeps reporting the frames that follow.
pub(crate) fn presented(token: Token, generation: u64) -> bool {
    let Some(endpoint) = live(token) else {
        return true;
    };
    let current = *endpoint.generation();
    if !current.map_or(generation == 0, |current| current.get() == generation) {
        log::debug!(
            "GPU page {} presented generation {generation}, not its current {current:?}; waiting for its next frame",
            endpoint.lease.page
        );
        return false;
    }
    match lxapp::find_page_by_instance_id(&endpoint.lease.page) {
        Some(page) => page.gpu_frame_presented(&(endpoint as Arc<dyn GpuPageView>)),
        None => true,
    }
}

/// Restart the Logic connection after a session failure; false when it cannot.
pub(crate) fn restart(token: Token, generation: u64) -> bool {
    let Some(endpoint) = live(token) else {
        return false;
    };
    // Release the generation lock before reload calls host code.
    let current = *endpoint.generation();
    if !current.map_or(generation == 0, |current| current.get() == generation) {
        return false;
    }
    let Some(page) = lxapp::find_page_by_instance_id(&endpoint.lease.page) else {
        return false;
    };
    match page.reload_gpu_view(&(endpoint as Arc<dyn GpuPageView>)) {
        Ok(()) => true,
        Err(error) => {
            log::warn!("GPU page recovery failed: {error}");
            false
        }
    }
}

/// Whether the recovery panel of page `token` offers Back.
pub(crate) fn can_go_back(token: Token) -> bool {
    page_of(token)
        .and_then(|page| lxapp::try_get(&page.appid()))
        .is_some_and(|app| app.can_go_back())
}

/// The recovery panel's Back; false when the page is not on top or has nowhere to go.
pub(crate) fn go_back(token: Token) -> bool {
    let Some(page) = page_of(token) else {
        return false;
    };
    lxapp::try_get(&page.appid()).is_some_and(|app| app.leave_failed_page(&page))
}

/// The live native shell of a page, for platform view lookup.
pub(crate) fn native_view(page: &PageInstance) -> Option<Arc<dyn NativePage>> {
    let token = *registry().pages.get(&page.instance_id_string())?;
    live(token).map(|endpoint| endpoint.native.clone())
}

/// The page a live token belongs to.
pub(crate) fn page_of(token: Token) -> Option<PageInstance> {
    live(token).and_then(|endpoint| lxapp::find_page_by_instance_id(&endpoint.lease.page))
}

/// Page `token`'s View failed and shows recovery; a running test hears it.
pub(crate) fn failed(token: Token, error: &str) {
    #[cfg(feature = "automation")]
    if lxapp::automation::page_events::armed()
        && let Some(page) = page_of(token)
    {
        page.note_page_event(
            lxapp::automation::page_events::PageEventKind::PageFailed,
            error,
        );
    }
    #[cfg(not(feature = "automation"))]
    let _ = (token, error);
}

/// A line page `token`'s View `console` wrote, tagged as that page's console like a WebView's.
pub(crate) fn console(token: Token, level: log::Level, message: &str) {
    let level = match level {
        log::Level::Error => lingxia_log::LogLevel::Error,
        log::Level::Warn => lingxia_log::LogLevel::Warn,
        log::Level::Info => lingxia_log::LogLevel::Info,
        log::Level::Debug | log::Level::Trace => lingxia_log::LogLevel::Debug,
    };
    let builder = lingxia_log::LogBuilder::new(lingxia_log::LogTag::WebViewConsole, message)
        .with_level(level);
    if let Some(page) = page_of(token) {
        builder.with_path(page.path()).with_appid(page.appid());
    }
}

/// A full-screen video on page `token` locks the window to its shape, or (`None`) gives the
/// window back to the page's own orientation. The lock lives on the page, so the host's
/// own reapplies (page switches, resume) keep it while that page shows.
pub(crate) fn video_orientation(token: Token, orientation: Option<lxapp::PageOrientation>) {
    use lingxia_platform::traits::ui::UIUpdate;
    let Some(page) = page_of(token) else {
        return;
    };
    if !page.set_presentation_orientation(orientation) {
        return;
    }
    let Some(app) = lxapp::try_get(&page.appid()) else {
        return;
    };
    if let Err(error) = app.runtime.update_orientation_ui(app.appid.clone()) {
        log::warn!("full-screen orientation: {error}");
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

/// Builds the GPU renderer while the home page loads, so its first frame skips the pipeline
/// compile.
pub(crate) fn warm() {
    #[cfg(target_os = "android")]
    lingxia_view_android::warm();
}

/// Compiled View modules persist under `dir`, so a later open skips the QuickJS compile.
pub(crate) fn set_bytecode_dir(dir: std::path::PathBuf) {
    #[cfg(target_os = "android")]
    lingxia_view_android::lingxia_view::engine::set_bytecode_dir(dir);
    #[cfg(any(target_os = "ios", target_os = "macos"))]
    lingxia_view_apple::lingxia_view::engine::set_bytecode_dir(dir);
    #[cfg(target_env = "ohos")]
    lingxia_view_harmony::lingxia_view::engine::set_bytecode_dir(dir);
    #[cfg(windows)]
    lingxia_view_windows::lingxia_view::engine::set_bytecode_dir(dir);
    #[cfg(not(any(
        target_os = "android",
        target_os = "ios",
        target_os = "macos",
        target_env = "ohos",
        windows
    )))]
    let _ = dir;
}
