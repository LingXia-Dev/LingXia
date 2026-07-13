use super::*;
use lingxia_platform::Platform;
use lingxia_platform::traits::ui::{
    SurfaceContent, SurfaceKind, SurfacePosition, SurfacePresenter,
    SurfaceRequest as PlatformSurfaceRequest, SurfaceRole,
};
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::OnceLock;
use std::time::Duration;

const SURFACE_DISPOSE_TTL_MS: u64 = 30_000;
static SURFACE_CLOSE_OBSERVER: OnceLock<fn(&str, &str) -> bool> = OnceLock::new();
/// Observer fired when a window's adaptive context (sizeClass/bottomOwner)
/// changes, so the logic layer can push `lx.onSurfaceContext` to subscribers.
/// Receives the window id whose context flipped.
static SURFACE_CONTEXT_OBSERVER: OnceLock<fn(&str)> = OnceLock::new();

/// The surface graph is per-WINDOW, not per-lxapp. The graph and its single
/// commit point live on a controller keyed by `window_id`; macOS/mobile are
/// single-window today (the `PRIMARY_WINDOW` entry), multi-window just adds more
/// entries to the registry.
pub(crate) struct WindowSurfaceController {
    window_id: String,
    manager: std::sync::Mutex<lingxia_surface::SurfaceManager>,
    runtime: std::sync::Arc<Platform>,
}

static WINDOW_CONTROLLERS: OnceLock<
    std::sync::Mutex<HashMap<String, std::sync::Arc<WindowSurfaceController>>>,
> = OnceLock::new();
pub(crate) const PRIMARY_WINDOW: &str = "primary";

/// Get-or-create the controller for a window. On first use of a window id we
/// clone the runtime handle and seed a fresh `SurfaceManager` for that window's
/// graph.
pub(crate) fn window_controller(
    window_id: &str,
    runtime: &std::sync::Arc<Platform>,
) -> std::sync::Arc<WindowSurfaceController> {
    let registry = WINDOW_CONTROLLERS.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    let mut map = registry.lock().unwrap();
    map.entry(window_id.to_string())
        .or_insert_with(|| {
            std::sync::Arc::new(WindowSurfaceController {
                window_id: window_id.to_string(),
                manager: std::sync::Mutex::new(lingxia_surface::SurfaceManager::new(700.0)),
                runtime: runtime.clone(),
            })
        })
        .clone()
}

impl WindowSurfaceController {
    /// THE single commit point for this window's graph mutations: re-derive the
    /// `DerivedLayout` and hand it to the platform skin to reconcile. Platforms
    /// without `present_layout` return `NotSupported`, ignored here. The manager
    /// lock is scoped to the `derive` call and dropped before `present_layout`,
    /// so the lock is never held across the outbound call.
    fn commit(&self) {
        let plan = self.manager.lock().unwrap().presentation_plan();
        let _ = self.runtime.present_layout(&self.window_id, &plan);
    }

    /// Mirror an opened surface into the core graph and read back the arbitrated
    /// presentation params + the set of surfaces the core evicted to make room.
    /// Does NOT commit: `open_surface` must render the new content between this
    /// mutation and the commit. Returns
    /// `(present_kind, present_position, present_role, evicted)`.
    fn open_node(
        &self,
        node: lingxia_surface::Surface,
        requested_position: SurfacePosition,
    ) -> (SurfaceKind, SurfacePosition, SurfaceRole, Vec<String>) {
        let id = node.id.clone();
        let mut present_kind = match node.role {
            lingxia_surface::Role::Main => SurfaceKind::Window,
            _ => SurfaceKind::Overlay,
        };
        let mut present_position = requested_position;
        let mut present_role = SurfaceRole::default();
        let mut manager = self.manager.lock().unwrap();
        let before: HashSet<String> = manager
            .graph()
            .surfaces()
            .iter()
            .map(|s| s.id.clone())
            .collect();
        let _decision = manager.open(node);
        if let Some(role) = manager.graph().role_of(&id) {
            let edge = manager.graph().get(&id).and_then(|s| s.placement.edge);
            (present_kind, present_position, present_role) =
                present_params_for_role(role, edge, requested_position);
        }
        let after: HashSet<String> = manager
            .graph()
            .surfaces()
            .iter()
            .map(|s| s.id.clone())
            .collect();
        let evicted = before
            .into_iter()
            .filter(|prev| prev != &id && !after.contains(prev))
            .collect();
        (present_kind, present_position, present_role, evicted)
    }

    fn close(&self, id: &str) -> bool {
        let removed = {
            let mut manager = self.manager.lock().unwrap();
            !manager.close(id).is_empty()
        };
        self.commit();
        removed
    }

    /// Mirror a host-declared aside into the core graph, seeding the root `main`
    /// if absent so the aside has a primary to dock to, and commit.
    fn register_host_aside(
        &self,
        surface_id: &str,
        edge: &str,
        root_main: lingxia_surface::Surface,
    ) {
        use lingxia_surface::{
            Edge as LxEdge, Placement, Role as LxRole, Surface as LxSurface,
            SurfaceContent as LxContent, SurfaceOwner as LxOwner, SurfaceState as LxState,
        };
        let edge = match edge {
            "left" | "leading" => LxEdge::Left,
            "top" => LxEdge::Top,
            "bottom" => LxEdge::Bottom,
            _ => LxEdge::Right,
        };
        let node = LxSurface {
            id: surface_id.to_string(),
            role: LxRole::Aside,
            content: LxContent::Entry {
                id: surface_id.to_string(),
                path: None,
            },
            owner: LxOwner::Host,
            placement: Placement {
                edge: Some(edge),
                preferred_size: None,
            },
            state: LxState::Mounted,
            float: None,
        };
        {
            let mut manager = self.manager.lock().unwrap();
            if manager.graph().mains().is_empty() {
                manager.open(root_main);
            }
            let _ = manager.open(node);
        }
        self.commit();
    }

    /// Make `app_id`'s main the active (primary) main, seeding its root `main`
    /// into the graph first if it isn't a node yet, then commit. The commit
    /// rebuilds the plan with the new `activeMainId` and pushes `present_layout`,
    /// so the skin reconciler drives the actual switch. Idempotent: when the
    /// node already exists and is already active, `set_active_main` does not
    /// change state, but we still commit so a reconciler that missed the
    /// (already-correct) plan can re-converge — the reconciler is itself a no-op
    /// when the target main is already attached.
    fn set_active_main(&self, app_id: &str, root_main: lingxia_surface::Surface) {
        {
            let mut manager = self.manager.lock().unwrap();
            // A tab's appid may not be a graph node yet (the main is seeded lazily
            // by set_width / register_host_aside). Seed it before switching, else
            // set_active_main silently no-ops on an unknown id.
            if manager.graph().role_of(app_id).is_none() {
                manager.open(root_main);
            }
            manager.set_active_main(app_id);
        }
        self.commit();
    }

    fn unregister_host_aside(&self, surface_id: &str) {
        {
            let _ = self.manager.lock().unwrap().close(surface_id);
        }
        self.commit();
    }

    /// Focus a surface (any role) and commit. Drives aside-slot tab switches:
    /// the plan's `activeChild` follows the graph focus, so the skin reconciler
    /// swaps the slot's visible child. Returns `false` for an unknown id.
    fn focus_surface(&self, surface_id: &str) -> bool {
        let focused = self.manager.lock().unwrap().set_focus(surface_id);
        if focused {
            self.commit();
        }
        focused
    }

    /// Report the container width so the core resolves the right `sizeClass`
    /// (with hysteresis), seeding the root `main` if absent. Commits (and
    /// returns `true`) only when the `sizeClass` flipped.
    fn set_width(&self, width: f64, root_main: lingxia_surface::Surface) -> bool {
        let changed = {
            let mut manager = self.manager.lock().unwrap();
            if manager.graph().mains().is_empty() {
                manager.open(root_main);
            }
            manager.set_width(width)
        };
        if changed {
            self.commit();
            // The sizeClass flip changed this window's derived adaptive context;
            // notify so `lx.onSurfaceContext` subscribers see the new context.
            notify_surface_context_observer(&self.window_id);
        }
        changed
    }

    fn presentation_plan(&self) -> lingxia_surface::LayoutPresentationPlan {
        self.manager.lock().unwrap().presentation_plan()
    }
}

pub fn register_surface_close_observer(observer: fn(&str, &str) -> bool) {
    let _ = SURFACE_CLOSE_OBSERVER.set(observer);
}

fn notify_surface_close_observer(id: &str, reason: &str) {
    if let Some(observer) = SURFACE_CLOSE_OBSERVER.get() {
        let _ = observer(id, reason);
    }
}

pub fn register_surface_context_observer(observer: fn(&str)) {
    let _ = SURFACE_CONTEXT_OBSERVER.set(observer);
}

fn notify_surface_context_observer(window_id: &str) {
    if let Some(observer) = SURFACE_CONTEXT_OBSERVER.get() {
        observer(window_id);
    }
}

#[derive(Debug, Clone)]
pub struct PageSurfaceRequest {
    pub id: String,
    pub target: PageSurfaceTarget,
    pub query: Option<PageQueryInput>,
    pub kind: SurfaceKind,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub width_ratio: Option<f64>,
    pub height_ratio: Option<f64>,
    pub position: SurfacePosition,
    /// Authoritative core role for this surface. `Aside` is the one path that
    /// docks (splits the main); `Float` is a popup; `Main` is a window. `kind`
    /// still drives the dispose-TTL distinction.
    pub role: lingxia_surface::Role,
}

#[derive(Debug, Clone)]
pub enum PageSurfaceTarget {
    Page(PageTarget),
    Url(String),
}

#[derive(Debug, Clone)]
pub struct PageSurface {
    pub id: String,
    pub page_path: Option<String>,
    pub page_instance_id: Option<String>,
    pub kind: SurfaceKind,
}

/// Automation-facing metadata for a live lxapp-owned surface.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LxAppRuntimeSurfaceInfo {
    pub appid: String,
    pub id: String,
    pub content: &'static str,
    pub target: String,
    pub owner_page_instance_id: Option<String>,
    pub content_page_instance_id: Option<String>,
    pub kind: &'static str,
    pub role: &'static str,
    pub url_callback: bool,
    pub ephemeral_web_data: bool,
}

/// A presented URL surface paired with a URL-callback interception channel:
/// the web content loads in the surface, and the navigation to the callback
/// URL is cancelled and delivered here instead. Dropping the handle closes the
/// surface and stops the interception, so an abandoned wait (e.g. a cancelled
/// future) tears the surface down with it.
pub struct UrlCallbackSurface {
    appid: String,
    surface: PageSurface,
    channel: lingxia_webview::url_callback::UrlCallbackChannel,
}

impl UrlCallbackSurface {
    /// The presented surface.
    pub fn surface(&self) -> &PageSurface {
        &self.surface
    }

    /// Waits for the navigation to the callback URL and returns the full
    /// navigated URL, query and fragment included. Pends indefinitely until it
    /// happens — bound the wait externally (a timeout or an abort race).
    pub async fn recv(&mut self) -> String {
        self.channel.recv().await
    }

    /// Returns an already-intercepted URL without waiting.
    pub fn try_recv(&mut self) -> Option<String> {
        self.channel.try_recv()
    }

    /// Close the surface now (same as dropping the handle).
    pub fn close(self) {}
}

impl Drop for UrlCallbackSurface {
    fn drop(&mut self) {
        // A vanished lxapp already took its surfaces with it.
        if let Some(app) = crate::lxapp::try_get(&self.appid) {
            let _ = app.close_surface(&self.surface.id, "programmatic");
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SurfaceRecord {
    pub owner_page_instance_id: Option<String>,
    /// The page instance hosted inside this surface (when content is a page).
    /// Used to close the surface when its inner page is disposed (e.g. SDK
    /// reclaim after long hide) so the owner's `Surface` handle reliably
    /// receives an onClose event.
    pub content_page_instance_id: Option<String>,
    pub content: SurfaceContent,
    pub target: String,
    pub kind: SurfaceKind,
    pub role: SurfaceRole,
    pub url_callback: bool,
    pub ephemeral_web_data: bool,
}

impl LxApp {
    pub fn open_surface(&self, request: PageSurfaceRequest) -> Result<PageSurface, LxAppError> {
        self.open_surface_with_web_data(request, false, false)
    }

    fn open_surface_with_web_data(
        &self,
        request: PageSurfaceRequest,
        ephemeral_web_data: bool,
        url_callback: bool,
    ) -> Result<PageSurface, LxAppError> {
        if !self.is_opened() {
            return Err(LxAppError::UnsupportedOperation(
                "lxapp is closed; surface suppressed".to_string(),
            ));
        }

        let id = request.id.trim().to_string();
        if id.is_empty() {
            return Err(LxAppError::InvalidParameter(
                "surface id must not be empty".to_string(),
            ));
        }

        // A window-kind surface is a bare standalone window (no sidebar / shell
        // chrome). It is NOT part of the main window's adaptive layout, so it
        // must bypass the per-window surface graph / reconciler entirely.
        if request.kind == SurfaceKind::Window {
            return self.open_window_surface(id, request);
        }

        let owner_page_instance_id = self.current_page().ok().map(|page| page.instance_id());
        let owner = owner_page_instance_id
            .clone()
            .map(PageOwner::Page)
            .unwrap_or_else(|| PageOwner::Scene(SceneId("system".to_string())));
        let presentation_kind = match request.kind {
            SurfaceKind::Window => PresentationKind::Window,
            SurfaceKind::Overlay => PresentationKind::Overlay,
        };
        let (path, page_instance_id, content, page_path) = match request.target {
            PageSurfaceTarget::Page(target) => {
                let dispose_ttl = match request.kind {
                    // A standalone window surface is a persistent window that
                    // lives until explicitly closed; only hideable overlays are
                    // reclaimed by the dispose timer after a long hide.
                    SurfaceKind::Window => None,
                    SurfaceKind::Overlay => Some(Duration::from_millis(SURFACE_DISPOSE_TTL_MS)),
                };
                let created = self.create_page_instance(
                    owner,
                    target,
                    request.query,
                    presentation_kind,
                    dispose_ttl,
                )?;
                (
                    created.resolved_path.clone(),
                    created.page_instance_id.to_string(),
                    SurfaceContent::Page,
                    Some(created.resolved_path),
                )
            }
            PageSurfaceTarget::Url(url) => (url, String::new(), SurfaceContent::Url, None),
        };

        let content_page_instance_id = if page_instance_id.is_empty() {
            None
        } else {
            Some(page_instance_id.clone())
        };
        let owner_pid = owner_page_instance_id.map(|id| id.to_string());
        let controller = window_controller(PRIMARY_WINDOW, &self.runtime);
        // Default to the requested kind/position; the core may arbitrate a
        // different role (e.g. an aside downgraded to a main on a compact
        // window), in which case the native presentation must follow the
        // arbitrated outcome — the core graph is the single source of truth.
        let mut present_kind = request.kind;
        let mut present_position = request.position;
        let mut present_role = SurfaceRole::default();
        // Surfaces the core evicted to make room for this one (arbitration
        // replacement). Closed natively after the new surface is presented so
        // the platform never leaks the victim's window/pane.
        let mut evicted: Vec<String> = Vec::new();
        if let Ok(state) = self.state.lock() {
            // Mirror into the Adaptive Surface Layout core (authoritative model).
            let node = self.build_surface_node(
                &id,
                content,
                request.position,
                &path,
                &page_path,
                owner_pid.as_deref(),
                request.role,
            );
            (present_kind, present_position, present_role, evicted) =
                controller.open_node(node, request.position);
            state.surfaces.lock().unwrap().insert(
                id.clone(),
                SurfaceRecord {
                    owner_page_instance_id: owner_pid.clone(),
                    content_page_instance_id,
                    content,
                    target: path.clone(),
                    kind: present_kind,
                    role: present_role,
                    url_callback,
                    ephemeral_web_data,
                },
            );
        }

        let present_result = self.runtime.present_surface(PlatformSurfaceRequest {
            id: id.clone(),
            app_id: self.appid.clone(),
            path,
            session_id: self.session_id(),
            page_instance_id: page_instance_id.clone(),
            content,
            kind: present_kind,
            width: finite_or_nan(request.width),
            height: finite_or_nan(request.height),
            width_ratio: finite_or_nan(request.width_ratio),
            height_ratio: finite_or_nan(request.height_ratio),
            position: present_position,
            role: present_role,
            ephemeral_web_data,
        });
        if let Err(err) = present_result {
            self.forget_surface(&id);
            if !page_instance_id.is_empty() {
                let _ = dispose_page_instance_by_id(&page_instance_id, CloseReason::Programmatic);
            }
            return Err(err.into());
        }

        // Now that the replacement is up, close the surfaces the core evicted.
        // The graph is window-global, so a victim may belong to the host or
        // another lxapp; `close_surface` no-ops for those. For a non-local
        // victim fire the global close observer (routes onClose to the owner by
        // id) and clear any host-managed visibility; the native aside undock is
        // handled by the commit below (the reconciler drops surfaces no longer
        // in the tree).
        for victim in &evicted {
            let owned = self
                .state
                .lock()
                .ok()
                .map(|state| state.surfaces.lock().unwrap().contains_key(victim.as_str()))
                .unwrap_or(false);
            if owned {
                let _ = self.close_surface(victim, "programmatic");
            } else {
                notify_surface_close_observer(victim, "programmatic");
                let _ = self
                    .runtime
                    .set_managed_surface_visible(victim, false, None);
            }
        }

        // Reconcile aside docking from the (now-mutated) core graph.
        controller.commit();

        Ok(PageSurface {
            id,
            page_path,
            page_instance_id: (!page_instance_id.is_empty()).then_some(page_instance_id),
            kind: present_kind,
        })
    }

    /// Present a bare standalone window surface. Unlike
    /// `open_surface`, this does NOT mirror into the per-window surface graph or
    /// run the layout reconciler — a standalone window lives outside the main
    /// window's adaptive layout. It still reuses the page-instance creation and
    /// the `SurfaceRecord` bookkeeping so close()/dispose work, and presents
    /// directly with `kind: Window` / `role: Main` so macOS routes it to the
    /// bare-window (kindWindow) path in `LxAppSurface`.
    fn open_window_surface(
        &self,
        id: String,
        request: PageSurfaceRequest,
    ) -> Result<PageSurface, LxAppError> {
        let owner_page_instance_id = self.current_page().ok().map(|page| page.instance_id());
        let owner = owner_page_instance_id
            .clone()
            .map(PageOwner::Page)
            .unwrap_or_else(|| PageOwner::Scene(SceneId("system".to_string())));
        let (path, page_instance_id, content, page_path) = match request.target {
            PageSurfaceTarget::Page(target) => {
                // A standalone window is persistent (lives until explicitly
                // closed): no dispose TTL, like the window branch in open_surface.
                let created = self.create_page_instance(
                    owner,
                    target,
                    request.query,
                    PresentationKind::Window,
                    None,
                )?;
                (
                    created.resolved_path.clone(),
                    created.page_instance_id.to_string(),
                    SurfaceContent::Page,
                    Some(created.resolved_path),
                )
            }
            PageSurfaceTarget::Url(_) => {
                return Err(LxAppError::InvalidParameter(
                    "a window hosts this lxapp's own page, not external web".to_string(),
                ));
            }
        };

        let content_page_instance_id = if page_instance_id.is_empty() {
            None
        } else {
            Some(page_instance_id.clone())
        };
        let owner_pid = owner_page_instance_id.map(|id| id.to_string());
        if let Ok(state) = self.state.lock() {
            state.surfaces.lock().unwrap().insert(
                id.clone(),
                SurfaceRecord {
                    owner_page_instance_id: owner_pid,
                    content_page_instance_id,
                    content,
                    target: path.clone(),
                    kind: SurfaceKind::Window,
                    role: SurfaceRole::Main,
                    url_callback: false,
                    ephemeral_web_data: false,
                },
            );
        }

        // Present directly with the authoritative window mapping; do NOT consult
        // the graph (no open_node / present_params_for_role / commit).
        let present_result = self.runtime.present_surface(PlatformSurfaceRequest {
            id: id.clone(),
            app_id: self.appid.clone(),
            path,
            session_id: self.session_id(),
            page_instance_id: page_instance_id.clone(),
            content,
            kind: SurfaceKind::Window,
            width: finite_or_nan(request.width),
            height: finite_or_nan(request.height),
            width_ratio: finite_or_nan(request.width_ratio),
            height_ratio: finite_or_nan(request.height_ratio),
            position: SurfacePosition::Center,
            role: SurfaceRole::Main,
            // Window surfaces host this lxapp's own pages, never external web.
            ephemeral_web_data: false,
        });
        if let Err(err) = present_result {
            // Remove only our bookkeeping; there is no graph node to close.
            if let Ok(state) = self.state.lock() {
                state.surfaces.lock().unwrap().remove(&id);
            }
            if !page_instance_id.is_empty() {
                let _ = dispose_page_instance_by_id(&page_instance_id, CloseReason::Programmatic);
            }
            return Err(err.into());
        }

        Ok(PageSurface {
            id,
            page_path,
            page_instance_id: (!page_instance_id.is_empty()).then_some(page_instance_id),
            kind: SurfaceKind::Window,
        })
    }

    pub fn close_surface(&self, id: &str, reason: &str) -> Result<(), LxAppError> {
        let id = id.trim();
        if id.is_empty() {
            return Err(LxAppError::InvalidParameter(
                "surface id must not be empty".to_string(),
            ));
        }

        let is_known = self
            .state
            .lock()
            .ok()
            .map(|state| state.surfaces.lock().unwrap().contains_key(id))
            .unwrap_or(false);
        if !is_known {
            return Ok(());
        }

        match self.runtime.close_surface(&self.appid, id, reason) {
            Ok(()) => Ok(()),
            Err(err) => {
                self.forget_surface(id);
                notify_surface_close_observer(id, "failed");
                Err(err.into())
            }
        }
    }

    /// Present a URL surface and intercept the navigation to `callback_url`
    /// (see [`lingxia_webview::url_callback`] for the matching rules): await
    /// the URL with [`UrlCallbackSurface::recv`], drop the handle to close the
    /// surface. `request.target` must be [`PageSurfaceTarget::Url`]. The
    /// interception channel opens before the surface presents, so the sentinel
    /// can never load unobserved.
    pub fn open_url_callback_surface(
        &self,
        callback_url: impl Into<String>,
        request: PageSurfaceRequest,
    ) -> Result<UrlCallbackSurface, LxAppError> {
        if !matches!(request.target, PageSurfaceTarget::Url(_)) {
            return Err(LxAppError::InvalidParameter(
                "a URL callback surface requires PageSurfaceTarget::Url".to_string(),
            ));
        }
        let channel = lingxia_webview::url_callback::open_channel(callback_url)
            .map_err(|err| LxAppError::InvalidParameter(err.to_string()))?;
        // Handoff flows persist through their callback payload (tokens), never
        // through WebView cookies, so every handoff surface gets an ephemeral
        // web session: logout is real, and a new login can pick a different
        // account instead of silently reusing a prior SSO cookie.
        let surface = self.open_surface_with_web_data(request, true, true)?;
        Ok(UrlCallbackSurface {
            appid: self.appid.clone(),
            surface,
            channel,
        })
    }

    /// Whether a surface with this id is currently open on this lxapp. Flips
    /// false once the surface closes for any reason, including the user
    /// dismissing it — poll it to bound a wait on a surface-driven flow.
    pub fn has_surface(&self, id: &str) -> bool {
        self.state
            .lock()
            .ok()
            .map(|state| state.surfaces.lock().unwrap().contains_key(id))
            .unwrap_or(false)
    }

    /// Snapshot all live dynamic surfaces owned by this lxapp.
    pub fn runtime_surface_info(&self) -> Vec<LxAppRuntimeSurfaceInfo> {
        let mut surfaces = self
            .state
            .lock()
            .ok()
            .map(|state| {
                state
                    .surfaces
                    .lock()
                    .map(|surfaces| {
                        surfaces
                            .iter()
                            .map(|(id, record)| runtime_surface_info(&self.appid, id, record))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        surfaces.sort_by(|left, right| left.id.cmp(&right.id));
        surfaces
    }

    pub fn show_surface(&self, id: &str) -> Result<(), LxAppError> {
        let id = id.trim();
        if id.is_empty() {
            return Err(LxAppError::InvalidParameter(
                "surface id must not be empty".to_string(),
            ));
        }
        if !self.has_surface(id) {
            return Err(LxAppError::InvalidParameter(format!(
                "unknown surface: {id}"
            )));
        }
        self.runtime
            .show_surface(&self.appid, id)
            .map_err(Into::into)
    }

    pub fn hide_surface(&self, id: &str) -> Result<(), LxAppError> {
        let id = id.trim();
        if id.is_empty() {
            return Err(LxAppError::InvalidParameter(
                "surface id must not be empty".to_string(),
            ));
        }
        if !self.has_surface(id) {
            return Err(LxAppError::InvalidParameter(format!(
                "unknown surface: {id}"
            )));
        }
        self.runtime
            .hide_surface(&self.appid, id)
            .map_err(Into::into)
    }

    /// Show or hide a host-declared top-level surface (e.g. the AI-chat panel
    /// or terminal) by its `ui` id. `edge` overrides the declared edge for
    /// this show; `None` keeps the current placement. Delegates to the
    /// platform host shell; platforms without one return an error.
    pub fn set_shell_surface_visible(
        &self,
        id: &str,
        visible: bool,
        edge: Option<&str>,
    ) -> Result<(), LxAppError> {
        let id = id.trim();
        if id.is_empty() {
            return Err(LxAppError::InvalidParameter(
                "shell surface id must not be empty".to_string(),
            ));
        }
        self.runtime
            .set_managed_surface_visible(id, visible, edge)
            .map_err(Into::into)
    }

    /// Mirror a host-declared aside (e.g. the assistant/terminal attach-panel)
    /// into the window's surface graph so the core's DerivedLayout reflects it
    /// and the derived layout includes host surfaces. Owner is `Host`
    /// (window-scoped, not page/lxapp).
    pub fn register_host_aside(&self, surface_id: &str, edge: &str) {
        let surface_id = surface_id.trim();
        if surface_id.is_empty() {
            return;
        }
        window_controller(PRIMARY_WINDOW, &self.runtime).register_host_aside(
            surface_id,
            edge,
            self.root_main_node(),
        );
    }

    /// Make this lxapp's main the active (primary) main in the window graph,
    /// seeding its root `main` node if absent, then commit. The commit pushes a
    /// `present_layout` carrying the new `activeMainId`, which the skin reconciler
    /// uses to attach this lxapp's content to the primary area. The skin must NOT
    /// drive the switch imperatively — it routes the switch through here so the
    /// graph stays the single source of truth.
    pub fn set_active_main(&self) {
        window_controller(PRIMARY_WINDOW, &self.runtime)
            .set_active_main(&self.appid, self.root_main_node());
    }

    /// Remove a host-declared aside from the surface graph.
    pub fn unregister_host_aside(&self, surface_id: &str) {
        let surface_id = surface_id.trim();
        if surface_id.is_empty() {
            return;
        }
        window_controller(PRIMARY_WINDOW, &self.runtime).unregister_host_aside(surface_id);
    }

    /// Focus a surface in the window graph (aside-slot tab switch). The commit
    /// pushes a plan whose slot `activeChild` follows the focus, and the skin
    /// reconciler swaps the visible child. Returns `false` for an unknown id.
    pub fn focus_shell_surface(&self, surface_id: &str) -> bool {
        let surface_id = surface_id.trim();
        if surface_id.is_empty() {
            return false;
        }
        window_controller(PRIMARY_WINDOW, &self.runtime).focus_surface(surface_id)
    }

    /// Flip a host-declared top-level surface's visibility by its `ui` id.
    pub fn toggle_shell_surface(&self, id: &str) -> Result<(), LxAppError> {
        let id = id.trim();
        if id.is_empty() {
            return Err(LxAppError::InvalidParameter(
                "shell surface id must not be empty".to_string(),
            ));
        }
        self.runtime.toggle_managed_surface(id).map_err(Into::into)
    }

    pub fn forget_surface(&self, id: &str) -> bool {
        let id = id.trim();
        if id.is_empty() {
            return false;
        }
        let removed = self
            .state
            .lock()
            .ok()
            .and_then(|state| state.surfaces.lock().unwrap().remove(id))
            .is_some();
        // Keep the Adaptive Surface Layout core in sync with removals; the
        // controller re-derives and reconciles aside docking.
        window_controller(PRIMARY_WINDOW, &self.runtime).close(id);
        removed
    }

    /// Report the container width so the core resolves the right `sizeClass`
    /// (with hysteresis). Returns `true` when the `sizeClass` flipped.
    ///
    /// Also seeds the app's root `main` surface into the graph if absent — the
    /// app's own primary content must be the `main`, otherwise asides have no
    /// primary to dock to and arbitration promotes them.
    pub fn set_surface_width(&self, width: f64) -> bool {
        // A sizeClass flip changes the DerivedLayout (e.g. compact folds asides
        // into mainFallback), so on resize the native layout must be reconciled
        // — not just the core state. The controller commits internally only when
        // the sizeClass flips.
        window_controller(PRIMARY_WINDOW, &self.runtime).set_width(width, self.root_main_node())
    }

    /// The app's root primary, represented as a `main` surface (id = appid).
    fn root_main_node(&self) -> lingxia_surface::Surface {
        use lingxia_surface::{
            Role, Surface as LxSurface, SurfaceContent, SurfaceOwner, SurfaceState,
        };
        LxSurface {
            id: self.appid.clone(),
            role: Role::Main,
            content: SurfaceContent::Entry {
                id: self.appid.clone(),
                path: None,
            },
            owner: SurfaceOwner::Host,
            placement: Default::default(),
            state: SurfaceState::Mounted,
            float: None,
        }
    }

    /// Snapshot the core's `LayoutPresentationPlan` for this app's window — the
    /// stable, renderable contract `lx.surface.derivedLayout()` returns (the
    /// same plan the skin reconciler binds via `present_layout`).
    pub fn surface_derived_layout(&self) -> Option<lingxia_surface::LayoutPresentationPlan> {
        Some(window_controller(PRIMARY_WINDOW, &self.runtime).presentation_plan())
    }

    /// Build an Adaptive Surface Layout node from the request's authoritative
    /// `role` (the core relationship) and `kind` (content/owner shaping).
    #[allow(clippy::too_many_arguments)]
    fn build_surface_node(
        &self,
        id: &str,
        content: SurfaceContent,
        position: SurfacePosition,
        path_or_url: &str,
        page_path: &Option<String>,
        owner_page_instance_id: Option<&str>,
        role: lingxia_surface::Role,
    ) -> lingxia_surface::Surface {
        use lingxia_surface::{
            Edge as LxEdge, FloatSpec, Placement, Role as LxRole, Surface as LxSurface,
            SurfaceContent as LxContent, SurfaceOwner as LxOwner, SurfaceState as LxState,
        };
        // Edge only matters for a docked aside; a float popup is unanchored.
        let edge = if role == LxRole::Aside {
            match position {
                SurfacePosition::Left => Some(LxEdge::Left),
                SurfacePosition::Right => Some(LxEdge::Right),
                SurfacePosition::Top => Some(LxEdge::Top),
                SurfacePosition::Bottom => Some(LxEdge::Bottom),
                SurfacePosition::Center => None,
            }
        } else {
            None
        };
        let node_content = match content {
            SurfaceContent::Page => LxContent::Entry {
                id: self.appid.clone(),
                path: page_path.clone(),
            },
            SurfaceContent::Url => LxContent::Web {
                url: path_or_url.to_string(),
            },
        };
        // A surface opened dynamically by an lxapp is caller-scoped: owned by the
        // calling page when there is one (closes with the page), else by the
        // lxapp. Host-declared surfaces are created elsewhere, not here.
        let owner = match owner_page_instance_id {
            Some(pid) => LxOwner::Page {
                page_instance_id: pid.to_string(),
            },
            None => LxOwner::Lxapp {
                app_id: self.appid.clone(),
            },
        };
        LxSurface {
            id: id.to_string(),
            role,
            content: node_content,
            owner,
            placement: Placement {
                edge,
                preferred_size: None,
            },
            state: LxState::Mounted,
            float: (role == LxRole::Float).then(FloatSpec::default),
        }
    }

    pub(crate) fn close_surfaces_for_owner(
        &self,
        owner_page_instance_id: &PageInstanceId,
        reason: CloseReason,
    ) {
        let ids = self.surface_ids(|record| {
            record.owner_page_instance_id.as_deref() == Some(owner_page_instance_id.as_str())
        });
        self.close_surfaces(ids, reason);
    }

    /// Close any surfaces hosting the given page as their content.
    /// Used when a page-in-surface is disposed (e.g. SDK reclaim after a
    /// long hide) so the owner's `Surface` handle reliably receives an
    /// onClose event instead of being left holding a dead handle.
    pub(crate) fn close_surfaces_hosting(
        &self,
        content_page_instance_id: &PageInstanceId,
        reason: CloseReason,
    ) {
        let ids = self.surface_ids(|record| {
            record.content_page_instance_id.as_deref() == Some(content_page_instance_id.as_str())
        });
        self.close_surfaces(ids, reason);
    }

    pub(crate) fn close_all_surfaces(&self, reason: CloseReason) {
        let ids = self.surface_ids(|_| true);
        self.close_surfaces(ids, reason);
    }

    fn surface_ids(&self, filter: impl Fn(&SurfaceRecord) -> bool) -> Vec<String> {
        self.state
            .lock()
            .ok()
            .map(|state| {
                state
                    .surfaces
                    .lock()
                    .unwrap()
                    .iter()
                    .filter_map(|(id, record)| filter(record).then_some(id.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn close_surfaces(&self, ids: Vec<String>, reason: CloseReason) {
        let reason = close_reason_str(reason);
        for id in ids {
            if let Err(err) = self.close_surface(&id, reason) {
                warn!("Failed to close surface {}: {}", id, err).with_appid(self.appid.clone());
            }
        }
    }
}

pub(crate) type SurfaceRecords = HashMap<String, SurfaceRecord>;

/// Map a core-arbitrated role (+ resolved edge) back to the platform present
/// parameters, so native presentation follows the core's decision. A float keeps
/// its requested position (popup at that edge/center); an aside docks at its
/// edge; a main is a window.
fn present_params_for_role(
    role: lingxia_surface::Role,
    edge: Option<lingxia_surface::Edge>,
    requested_position: SurfacePosition,
) -> (SurfaceKind, SurfacePosition, SurfaceRole) {
    use lingxia_surface::{Edge as LxEdge, Role as LxRole};
    match role {
        LxRole::Main => (
            SurfaceKind::Window,
            SurfacePosition::Center,
            SurfaceRole::Main,
        ),
        LxRole::Float => (SurfaceKind::Overlay, requested_position, SurfaceRole::Float),
        LxRole::Aside => {
            let position = match edge {
                Some(LxEdge::Left) => SurfacePosition::Left,
                Some(LxEdge::Right) => SurfacePosition::Right,
                Some(LxEdge::Top) => SurfacePosition::Top,
                Some(LxEdge::Bottom) => SurfacePosition::Bottom,
                None => requested_position,
            };
            (SurfaceKind::Overlay, position, SurfaceRole::Aside)
        }
    }
}

fn finite_or_nan(value: Option<f64>) -> f64 {
    match value {
        Some(value) if value.is_finite() => value,
        _ => f64::NAN,
    }
}

fn surface_content_str(content: SurfaceContent) -> &'static str {
    match content {
        SurfaceContent::Page => "page",
        SurfaceContent::Url => "url",
    }
}

fn surface_kind_str(kind: SurfaceKind) -> &'static str {
    match kind {
        SurfaceKind::Window => "window",
        SurfaceKind::Overlay => "overlay",
    }
}

fn surface_role_str(role: SurfaceRole) -> &'static str {
    match role {
        SurfaceRole::Main => "main",
        SurfaceRole::Aside => "aside",
        SurfaceRole::Float => "float",
    }
}

fn runtime_surface_info(appid: &str, id: &str, record: &SurfaceRecord) -> LxAppRuntimeSurfaceInfo {
    LxAppRuntimeSurfaceInfo {
        appid: appid.to_string(),
        id: id.to_string(),
        content: surface_content_str(record.content),
        target: record.target.clone(),
        owner_page_instance_id: record.owner_page_instance_id.clone(),
        content_page_instance_id: record.content_page_instance_id.clone(),
        kind: surface_kind_str(record.kind),
        role: surface_role_str(record.role),
        url_callback: record.url_callback,
        ephemeral_web_data: record.ephemeral_web_data,
    }
}

fn close_reason_str(reason: CloseReason) -> &'static str {
    match reason {
        CloseReason::User => "user",
        CloseReason::Programmatic => "programmatic",
        CloseReason::OwnerClosed => "owner_closed",
        CloseReason::AppClosed => "app_closed",
        CloseReason::Reclaimed => "reclaimed",
        CloseReason::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url_record(url_callback: bool, ephemeral_web_data: bool) -> SurfaceRecord {
        SurfaceRecord {
            owner_page_instance_id: Some("owner".to_string()),
            content_page_instance_id: None,
            content: SurfaceContent::Url,
            target: "https://example.com/login".to_string(),
            kind: SurfaceKind::Overlay,
            role: SurfaceRole::Aside,
            url_callback,
            ephemeral_web_data,
        }
    }

    #[test]
    fn automation_surface_inventory_distinguishes_url_callbacks() {
        let regular = runtime_surface_info("demo", "web", &url_record(false, false));
        assert_eq!(regular.content, "url");
        assert!(!regular.url_callback);
        assert!(!regular.ephemeral_web_data);

        let callback = runtime_surface_info("demo", "login", &url_record(true, true));
        assert_eq!(callback.target, "https://example.com/login");
        assert!(callback.url_callback);
        assert!(callback.ephemeral_web_data);
    }
}
