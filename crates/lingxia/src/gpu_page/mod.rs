//! Binds lxapp GPU pages to a platform's native page shells: the creation
//! handshake, the live registry, dispose-once leases, the connection-generation
//! gate, image loads and the View worker key. Platform modules
//! only marshal calls between these functions and their native shells.
// Off Android it is compiled only for its tests, which exercise part of it.
#![cfg_attr(not(target_os = "android"), allow(dead_code, unused_imports))]
use lxapp::{
    GpuPageView, GpuPageViewFactory, GpuViewCreation, GpuViewGeneration, LeaveReason, LxAppError,
    PageInstance, PageViewEnvironment,
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
mod automation;
mod images;
pub(crate) use images::load_image;

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
    fn connect(&self, generation: GpuViewGeneration, nonce: &str) -> Result<(), LxAppError>;
    fn receive(&self, generation: GpuViewGeneration, frame: &str) -> Result<(), LxAppError>;
    fn request_leave(&self, generation: GpuViewGeneration, reason: &str) -> Result<(), LxAppError>;
    fn update_environment(&self, update: &serde_json::Value) -> Result<(), LxAppError>;
    fn reload(&self, source: &str) -> Result<(), LxAppError>;
    /// Dev sessions: replace the module keeping page state (`ViewHost::swap_module`);
    /// a shell without it reloads.
    fn swap_module(&self, source: &str) -> Result<(), LxAppError> {
        self.reload(source)
    }
    fn park(&self) -> Result<(), LxAppError>;
    fn retain_surface(&self, retained: bool) -> Result<(), LxAppError>;
    /// `{op: query|act|screenshot, css, index, all, maxText, action: {kind, text, force}}`,
    /// answered after the presented frame.
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
    ) -> Result<(), LxAppError>;
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

/// Disposes its native shell exactly once, however it is released.
struct Lease {
    token: Token,
    page: String,
    disposed: AtomicBool,
    factory: Arc<dyn NativeFactory>,
}

impl Lease {
    fn dispose(&self) {
        if self.disposed.swap(true, Ordering::AcqRel) {
            return;
        }
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
    _automation: Arc<automation::GpuAutomation>,
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
    fn post_message(&self, _: &str) -> Result<(), LxAppError> {
        Err(LxAppError::Runtime(
            "GPU messages require a captured connection generation".into(),
        ))
    }

    fn post_message_for_generation(
        &self,
        generation: GpuViewGeneration,
        message: &str,
    ) -> Result<(), LxAppError> {
        if self.current(generation.get()).is_none() {
            return Ok(());
        }
        self.native.receive(generation, message)
    }

    fn connect(&self, generation: GpuViewGeneration, nonce: &str) -> Result<(), LxAppError> {
        {
            let mut current = self.generation();
            if current.is_some_and(|current| current.get() > generation.get()) {
                return Err(LxAppError::Bridge("GPU connection was superseded".into()));
            }
            *current = Some(generation);
        }
        self.native.connect(generation, nonce)
    }

    fn request_leave(&self, reason: LeaveReason) -> Result<(), LxAppError> {
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

    fn update_environment(&self, update: PageViewEnvironment) -> Result<(), LxAppError> {
        if self.lease.disposed() {
            return Ok(());
        }
        let update = match update {
            PageViewEnvironment::DisplayLanguage { language, revision } => {
                serde_json::json!({"kind":"language","revision":revision,"language":language})
            }
            PageViewEnvironment::SurfaceContext { context, revision } => {
                serde_json::json!({"kind":"surface","revision":revision,"context":context})
            }
        };
        self.native.update_environment(&update)
    }

    fn reload(&self) -> Result<(), LxAppError> {
        let page = lxapp::find_page_by_instance_id(&self.lease.page)
            .ok_or_else(|| LxAppError::Runtime("GPU page was removed".into()))?;
        let source = page.gpu_module_source()?;
        *self.generation() = None;
        self.native.reload(&source)
    }

    fn park(&self) -> Result<(), LxAppError> {
        *self.generation() = None;
        self.native.park()
    }

    fn retain_surface(&self, retained: bool) -> Result<(), LxAppError> {
        if self.lease.disposed() {
            return Ok(());
        }
        self.native.retain_surface(retained)
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
            let lease = Lease {
                token,
                page: page.instance_id_string(),
                disposed: AtomicBool::new(false),
                factory: factory.clone(),
            };
            let worker = worker_key(&page.appid(), page.app_session_id());
            // Hosts read the pack when they are created, so the first frame already carries it.
            if let Some(pack) = page.gpu_theme_pack().as_deref().and_then(bundled_pack) {
                lingxia_theme::set_app_pack(&worker, Some(pack));
            }
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
                .map_err(|_| LxAppError::Runtime("GPU creation cancelled".into()))?
                .map_err(LxAppError::Runtime)?;
            Ok(register(lease, native) as Arc<dyn GpuPageView>)
        })
    }
}

/// The lxapp's bundled pack; the host checks and resolves it, and a bad one keeps the
/// default palette.
fn bundled_pack(pack: &str) -> Option<lingxia_theme::Pack> {
    serde_json::from_str(pack)
        .inspect_err(|error| log::warn!("theme/pack.json is not a theme pack: {error}"))
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
fn swap_pages(select: impl Fn(&str) -> Option<String>) -> Result<usize, LxAppError> {
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

/// Dev sessions: the pages of `appid` at `path` take the module now in the bundle.
pub(crate) fn hot_swap(appid: &str, path: &str) -> Result<usize, LxAppError> {
    swap_pages(|id| {
        let page = lxapp::find_page_by_instance_id(id)?;
        (page.appid() == appid && page_key(&page.path()) == page_key(path))
            .then(|| page.gpu_module_source().ok())
            .flatten()
    })
}

/// A page route without query, leading slash or module extension: the runtime knows
/// `pages/a/index.js`, the dev CLI names the source `pages/a/index`.
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

/// An outbound View message for its connection generation.
pub(crate) fn message(token: Token, generation: u64, body: &str) {
    let Some(endpoint) = live(token) else {
        return;
    };
    let Some(current) = endpoint.current(generation) else {
        return;
    };
    let Some(page) = lxapp::find_page_by_instance_id(&endpoint.lease.page) else {
        return;
    };
    if let Err(error) =
        page.handle_incoming_gpu_message(&(endpoint as Arc<dyn GpuPageView>), current, body)
    {
        log::warn!("GPU bridge ingress rejected: {error}");
    }
}

/// The first frame for `generation` reached the screen; generation 0 is a standalone page.
pub(crate) fn presented(token: Token, generation: u64) {
    let Some(endpoint) = live(token) else {
        return;
    };
    let current = *endpoint.generation();
    if !current.map_or(generation == 0, |current| current.get() == generation) {
        log::debug!(
            "GPU page {} presented generation {generation}, not its current {current:?}; not ready from it",
            endpoint.lease.page
        );
        return;
    }
    match lxapp::find_page_by_instance_id(&endpoint.lease.page) {
        Some(page) => page.gpu_frame_presented(&(endpoint as Arc<dyn GpuPageView>)),
        None => log::debug!("GPU page {} presented after it left", endpoint.lease.page),
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

/// The live native shell of a page, for platform view lookup.
pub(crate) fn native_view(page: &PageInstance) -> Option<Arc<dyn NativePage>> {
    let token = *registry().pages.get(&page.instance_id_string())?;
    live(token).map(|endpoint| endpoint.native.clone())
}

/// The page a live token belongs to.
pub(crate) fn page_of(token: Token) -> Option<PageInstance> {
    live(token).and_then(|endpoint| lxapp::find_page_by_instance_id(&endpoint.lease.page))
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
