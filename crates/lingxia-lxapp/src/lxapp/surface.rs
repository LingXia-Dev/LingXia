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
/// Observer fired when one lxapp presentation's actual viewport changes.
/// Receives that lxapp's app id.
static SURFACE_CONTEXT_OBSERVER: OnceLock<fn(&str)> = OnceLock::new();
static SURFACE_VIEWPORTS: OnceLock<std::sync::Mutex<HashMap<String, SurfaceViewportContext>>> =
    OnceLock::new();

#[derive(Debug, Clone, Copy)]
struct SurfaceViewportContext {
    session_id: u64,
    width: f64,
    height: f64,
    size_class: lingxia_surface::SizeClass,
}

/// The surface graph is per-WINDOW, not per-lxapp. The graph and its single
/// commit point live on a controller keyed by `window_id`; macOS/mobile are
/// single-window today (the `PRIMARY_WINDOW` entry), multi-window just adds more
/// entries to the registry.
pub(crate) struct WindowSurfaceController {
    window_id: String,
    manager: std::sync::Mutex<lingxia_surface::SurfaceManager>,
    runtime: std::sync::Arc<Platform>,
}

struct OpenNodeResult {
    surface_id: String,
    kind: SurfaceKind,
    position: SurfacePosition,
    role: SurfaceRole,
    evicted: Vec<String>,
    reused: bool,
    overlay: bool,
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
    ) -> OpenNodeResult {
        let requested_id = node.id.clone();
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
        let outcome = manager.open(node);
        let resolved_id = outcome.resolved_surface_id;
        if let Some(role) = manager.graph().role_of(&resolved_id) {
            let edge = manager
                .graph()
                .get(&resolved_id)
                .and_then(|s| s.placement.edge);
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
            .filter(|prev| prev != &resolved_id && !after.contains(prev))
            .collect();
        OpenNodeResult {
            reused: resolved_id != requested_id,
            surface_id: resolved_id,
            kind: present_kind,
            position: present_position,
            role: present_role,
            evicted,
            overlay: outcome.overlay,
        }
    }

    fn close(&self, id: &str) -> Vec<String> {
        let removed = {
            let mut manager = self.manager.lock().unwrap();
            manager.close(id)
        };
        self.commit();
        removed
    }

    fn contains(&self, id: &str) -> bool {
        self.manager.lock().unwrap().graph().get(id).is_some()
    }

    fn show_surface(&self, app_id: &str, id: &str) -> Result<(), LxAppError> {
        {
            let manager = self.manager.lock().unwrap();
            if manager.graph().get(id).is_none() {
                return Err(LxAppError::InvalidParameter(format!(
                    "unknown surface: {id}"
                )));
            }
        }
        self.runtime.show_surface(app_id, id)?;
        if !self.manager.lock().unwrap().show(id) {
            let _ = self.runtime.hide_surface(app_id, id);
            return Err(LxAppError::InvalidParameter(format!(
                "unknown surface: {id}"
            )));
        }
        self.commit();
        Ok(())
    }

    fn hide_surface(&self, app_id: &str, id: &str) -> Result<(), LxAppError> {
        {
            let manager = self.manager.lock().unwrap();
            if manager.graph().role_of(id) == Some(lingxia_surface::Role::Main) {
                return Err(LxAppError::UnsupportedOperation(
                    "a main surface cannot be hidden".to_string(),
                ));
            }
            if manager.graph().get(id).is_none() {
                return Err(LxAppError::InvalidParameter(format!(
                    "unknown surface: {id}"
                )));
            }
        }
        self.runtime.hide_surface(app_id, id)?;
        if self.manager.lock().unwrap().hide(id) {
            self.commit();
        }
        Ok(())
    }

    fn set_managed_surface_visible(
        &self,
        id: &str,
        visible: bool,
        edge: Option<&str>,
    ) -> Result<(), LxAppError> {
        let parsed_edge = edge.map(parse_surface_edge).transpose()?;
        // The host handler owns first presentation and may register a declared
        // surface into this graph. Delegate before mirroring visibility so a
        // declared-but-never-opened surface is not rejected as unknown.
        self.runtime
            .set_managed_surface_visible(id, visible, edge)?;
        let changed = {
            let mut manager = self.manager.lock().unwrap();
            if visible {
                if let Some(edge) = parsed_edge
                    && let Some(mut surface) = manager.graph().get(id).cloned()
                {
                    surface.placement.edge = Some(edge);
                    manager.open(surface);
                }
                manager.show(id)
            } else {
                manager.hide(id)
            }
        };
        if changed {
            self.commit();
        }
        Ok(())
    }

    fn surface_presentation(&self, id: &str) -> Option<&'static str> {
        let plan = self.manager.lock().unwrap().presentation_plan();
        plan.aside_slots
            .iter()
            .find(|slot| slot.children.iter().any(|child| child == id))
            .map(|slot| if slot.overlay { "overlay" } else { "dock" })
    }

    /// Mirror a host-declared aside into the core graph, seeding the root `main`
    /// if absent so the aside has a primary to dock to, and commit.
    fn register_host_aside(
        &self,
        surface_id: &str,
        content_id: &str,
        edge: &str,
        root_main: lingxia_surface::Surface,
    ) {
        let node = host_aside_node(surface_id, content_id, edge);
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

    /// Report the container width so the core resolves size class and physical
    /// aside admission, seeding the root main if absent. Commits whenever the
    /// render plan changes; returns whether the adaptive size class flipped.
    fn set_width(&self, width: f64, root_main: lingxia_surface::Surface) -> bool {
        self.set_layout_metrics(width, None, root_main)
    }

    fn set_layout_metrics(
        &self,
        width: f64,
        sidebar_width: Option<f64>,
        root_main: lingxia_surface::Surface,
    ) -> bool {
        let (class_changed, plan_changed) = {
            let mut manager = self.manager.lock().unwrap();
            let before = manager.presentation_plan();
            let mut seeded = false;
            if manager.graph().mains().is_empty() {
                manager.open(root_main);
                seeded = true;
            }
            let class_changed = manager.set_width(width);
            if let Some(sidebar_width) = sidebar_width {
                manager.set_sidebar_width(sidebar_width);
            }
            let after = manager.presentation_plan();
            (class_changed, seeded || before != after)
        };
        if plan_changed {
            self.commit();
        }
        class_changed
    }

    fn set_sidebar_width(&self, width: f64) -> bool {
        let plan_changed = {
            let mut manager = self.manager.lock().unwrap();
            let before = manager.presentation_plan();
            manager.set_sidebar_width(width);
            before != manager.presentation_plan()
        };
        if plan_changed {
            self.commit();
        }
        plan_changed
    }

    fn presentation_plan(&self) -> lingxia_surface::LayoutPresentationPlan {
        self.manager.lock().unwrap().presentation_plan()
    }
}

fn host_aside_node(surface_id: &str, content_id: &str, edge: &str) -> lingxia_surface::Surface {
    use lingxia_surface::{
        Edge, Placement, Role, Surface, SurfaceContent, SurfaceOwner, SurfaceState,
    };
    let edge = match edge {
        "left" | "leading" => Edge::Left,
        "top" => Edge::Top,
        "bottom" => Edge::Bottom,
        _ => Edge::Right,
    };
    Surface {
        id: surface_id.to_string(),
        role: Role::Aside,
        content: SurfaceContent::Entry {
            id: content_id.to_string(),
            path: None,
        },
        owner: SurfaceOwner::Host,
        placement: Placement {
            edge: Some(edge),
            preferred_size: None,
        },
        state: SurfaceState::Mounted,
        float: None,
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
    /// Overrides the interaction preset selected by the opening API.
    pub interaction: Option<lingxia_surface::SurfaceInteraction>,
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
    pub role: SurfaceRole,
    pub position: SurfacePosition,
    pub presentation: String,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum UrlCallbackWaitError {
    #[error("URL callback surface was cancelled before a callback arrived")]
    Cancelled,
}

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

    /// Waits only for the callback URL. Prefer [`Self::wait`] when user
    /// dismissal should cancel the flow.
    pub async fn recv(&mut self) -> String {
        self.channel.recv().await
    }

    /// Waits for either the callback URL or dismissal of the presented surface.
    /// Consuming the handle guarantees that the ephemeral surface is torn down
    /// on every outcome.
    pub async fn wait(mut self) -> Result<String, UrlCallbackWaitError> {
        loop {
            tokio::select! {
                url = self.channel.recv() => return Ok(url),
                _ = tokio::time::sleep(Duration::from_millis(50)) => {
                    let open = crate::lxapp::try_get(&self.appid)
                        .is_some_and(|app| app.has_surface(&self.surface.id));
                    if !open {
                        return Err(UrlCallbackWaitError::Cancelled);
                    }
                }
            }
        }
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

        let interaction = request.interaction.unwrap_or_else(|| {
            if url_callback {
                lingxia_surface::SurfaceInteraction::url_callback()
            } else if request.kind == SurfaceKind::Window {
                lingxia_surface::SurfaceInteraction::window()
            } else {
                lingxia_surface::SurfaceInteraction::standard()
            }
        });
        validate_surface_interaction(request.kind, url_callback, interaction)?;

        // A window-kind surface is a bare standalone window (no sidebar / shell
        // chrome). It is NOT part of the main window's adaptive layout, so it
        // must bypass the per-window surface graph / reconciler entirely.
        if request.kind == SurfaceKind::Window {
            return self.open_window_surface(id, request, interaction);
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
        let mut resolved_id = id.clone();
        let mut reused = false;
        let mut overlay = false;
        // Surfaces the core evicted to make room for this one (arbitration
        // replacement). Closed natively after the new surface is presented so
        // the platform never leaks the victim's window/pane.
        let mut evicted: Vec<String> = Vec::new();
        if self.state.lock().is_ok() {
            // Mirror into the Adaptive Surface Layout core (authoritative model).
            let node = self.build_surface_node(
                &id,
                content,
                request.position,
                &path,
                &page_path,
                owner_pid.as_deref(),
                request.role,
                url_callback,
                interaction,
            );
            let opened = controller.open_node(node, request.position);
            present_kind = opened.kind;
            present_position = opened.position;
            present_role = opened.role;
            resolved_id = opened.surface_id;
            evicted = opened.evicted;
            reused = opened.reused;
            overlay = opened.overlay;
        }

        if reused {
            if !page_instance_id.is_empty() {
                let _ = dispose_page_instance_by_id(&page_instance_id, CloseReason::Programmatic);
            }
            controller.commit();
            return Ok(PageSurface {
                id: resolved_id,
                page_path: None,
                page_instance_id: None,
                kind: present_kind,
                role: present_role,
                position: present_position,
                presentation: surface_presentation(present_kind, present_role, overlay).to_string(),
            });
        }

        if let Ok(state) = self.state.lock() {
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
            interaction,
            ephemeral_web_data,
            url_callback,
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
            id: resolved_id,
            page_path,
            page_instance_id: (!page_instance_id.is_empty()).then_some(page_instance_id),
            kind: present_kind,
            role: present_role,
            position: present_position,
            presentation: surface_presentation(present_kind, present_role, overlay).to_string(),
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
        interaction: lingxia_surface::SurfaceInteraction,
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
            interaction,
            // Window surfaces host this lxapp's own pages, never external web.
            ephemeral_web_data: false,
            url_callback: false,
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
            role: SurfaceRole::Main,
            position: SurfacePosition::Center,
            presentation: "window".to_string(),
        })
    }

    pub fn close_surface(&self, id: &str, reason: &str) -> Result<(), LxAppError> {
        let id = id.trim();
        if id.is_empty() {
            return Err(LxAppError::InvalidParameter(
                "surface id must not be empty".to_string(),
            ));
        }

        let controller = window_controller(PRIMARY_WINDOW, &self.runtime);
        let is_known = self
            .state
            .lock()
            .ok()
            .map(|state| state.surfaces.lock().unwrap().contains_key(id))
            .unwrap_or(false)
            || controller.contains(id);
        if !is_known {
            return Ok(());
        }

        let platform_owner_appid = surface_owner_appid(id).unwrap_or_else(|| self.appid.clone());
        match self
            .runtime
            .close_surface(&platform_owner_appid, id, reason)
        {
            Ok(()) => {
                let mut removed = controller.close(id);
                if !removed.iter().any(|removed| removed == id) {
                    removed.push(id.to_string());
                }
                for removed_id in removed {
                    remove_surface_record_from_owner(&removed_id);
                }
                Ok(())
            }
            Err(err) => Err(err.into()),
        }
    }

    /// Present a URL surface and intercept the navigation to `callback_url`
    /// (see [`lingxia_webview::url_callback`] for the matching rules): await
    /// [`UrlCallbackSurface::wait`] to handle callback or dismissal.
    /// `request.target` must be [`PageSurfaceTarget::Url`]. The
    /// interception channel opens before the surface presents, so the sentinel
    /// can never load unobserved. Targets require HTTPS outside a dev session;
    /// loopback HTTP is always allowed and dev sessions may use other HTTP,
    /// while file URLs are always rejected.
    pub fn open_url_callback_surface(
        &self,
        callback_url: impl Into<String>,
        request: PageSurfaceRequest,
    ) -> Result<UrlCallbackSurface, LxAppError> {
        let PageSurfaceTarget::Url(target_url) = &request.target else {
            return Err(LxAppError::InvalidParameter(
                "a URL callback surface requires PageSurfaceTarget::Url".to_string(),
            ));
        };
        validate_url_callback_target(target_url, is_dev_session())?;
        let surface_id = request.id.trim();
        if !surface_id.is_empty()
            && (self.has_surface(surface_id)
                || window_controller(PRIMARY_WINDOW, &self.runtime).contains(surface_id))
        {
            return Err(LxAppError::InvalidParameter(format!(
                "a URL callback surface requires a unique surface id: {surface_id}"
            )));
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
        let platform_owner_appid = surface_owner_appid(id).unwrap_or_else(|| self.appid.clone());
        let controller = window_controller(PRIMARY_WINDOW, &self.runtime);
        if controller.contains(id) {
            controller.show_surface(&platform_owner_appid, id)
        } else if self.has_surface(id) {
            self.runtime
                .show_surface(&platform_owner_appid, id)
                .map_err(Into::into)
        } else {
            Err(LxAppError::InvalidParameter(format!(
                "unknown surface: {id}"
            )))
        }
    }

    pub fn hide_surface(&self, id: &str) -> Result<(), LxAppError> {
        let id = id.trim();
        if id.is_empty() {
            return Err(LxAppError::InvalidParameter(
                "surface id must not be empty".to_string(),
            ));
        }
        let platform_owner_appid = surface_owner_appid(id).unwrap_or_else(|| self.appid.clone());
        let controller = window_controller(PRIMARY_WINDOW, &self.runtime);
        if controller.contains(id) {
            controller.hide_surface(&platform_owner_appid, id)
        } else if self.has_surface(id) {
            self.runtime
                .hide_surface(&platform_owner_appid, id)
                .map_err(Into::into)
        } else {
            Err(LxAppError::InvalidParameter(format!(
                "unknown surface: {id}"
            )))
        }
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
        window_controller(PRIMARY_WINDOW, &self.runtime)
            .set_managed_surface_visible(id, visible, edge)
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
            surface_id,
            edge,
            self.root_main_node(),
        );
    }

    /// Mirror a host aside whose stable surface id differs from its content
    /// identity, such as the shell-owned terminal surface.
    pub fn register_host_aside_content(&self, surface_id: &str, content_id: &str, edge: &str) {
        let surface_id = surface_id.trim();
        let content_id = content_id.trim();
        if surface_id.is_empty() || content_id.is_empty() {
            return;
        }
        window_controller(PRIMARY_WINDOW, &self.runtime).register_host_aside(
            surface_id,
            content_id,
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

    pub fn shell_surface_presentation(&self, surface_id: &str) -> Option<&'static str> {
        window_controller(PRIMARY_WINDOW, &self.runtime).surface_presentation(surface_id)
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

    /// Atomically report window and sidebar widths so admission never
    /// publishes a plan calculated with one stale metric.
    pub fn set_surface_layout_metrics(&self, width: f64, sidebar_width: f64) -> bool {
        window_controller(PRIMARY_WINDOW, &self.runtime).set_layout_metrics(
            width,
            Some(sidebar_width),
            self.root_main_node(),
        )
    }

    /// Report the sidebar's current logical width for physical aside
    /// admission. Desktop shells update it on resize/collapse; other hosts
    /// leave the zero default.
    pub fn set_surface_sidebar_width(&self, width: f64) -> bool {
        window_controller(PRIMARY_WINDOW, &self.runtime).set_sidebar_width(width)
    }

    /// Report this lxapp presentation's actual viewport. Unlike shell width,
    /// this is measured after sidebar/navbar/aside layout and therefore drives
    /// the content-facing `lx.onSurfaceContext` size class.
    pub fn set_surface_viewport(&self, width: f64, height: f64) -> bool {
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return false;
        }
        let viewports = SURFACE_VIEWPORTS.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
        let changed = if let Ok(mut viewports) = viewports.lock() {
            let previous = viewports
                .get(&self.appid)
                .filter(|context| context.session_id == self.session_id());
            let size_class = lingxia_surface::SizeClass::resolve(
                previous.map(|context| context.size_class),
                width,
                lingxia_surface::DEFAULT_HYSTERESIS,
            );
            let next = SurfaceViewportContext {
                session_id: self.session_id(),
                width,
                height,
                size_class,
            };
            let changed = previous.is_none_or(|previous| {
                previous.width != width
                    || previous.height != height
                    || previous.size_class != size_class
            });
            viewports.insert(self.appid.clone(), next);
            changed
        } else {
            false
        };
        if changed {
            notify_surface_context_observer(&self.appid);
        }
        changed
    }

    pub fn surface_viewport(&self) -> Option<(f64, f64, lingxia_surface::SizeClass)> {
        SURFACE_VIEWPORTS
            .get()
            .and_then(|viewports| viewports.lock().ok())
            .and_then(|viewports| viewports.get(&self.appid).copied())
            .filter(|context| context.session_id == self.session_id())
            .map(|context| (context.width, context.height, context.size_class))
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
        url_callback: bool,
        interaction: lingxia_surface::SurfaceInteraction,
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
                reuse_by_url: !url_callback,
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
            float: (role == LxRole::Float).then(|| FloatSpec {
                dismiss: interaction.dismiss,
                modal: interaction.modal,
                close_button: interaction.close_button,
                ..FloatSpec::default()
            }),
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

fn validate_url_callback_target(target: &str, dev_mode: bool) -> Result<(), LxAppError> {
    let raw_scheme = target
        .split_once(':')
        .map(|(scheme, _)| scheme.to_ascii_lowercase());
    if raw_scheme.as_deref() == Some("file") {
        return Err(LxAppError::InvalidParameter(
            "a URL callback surface cannot load file URLs".to_string(),
        ));
    }
    let uri = target.parse::<http::Uri>().map_err(|_| {
        LxAppError::InvalidParameter(
            "a URL callback surface requires an absolute HTTPS URL".to_string(),
        )
    })?;
    if uri.authority().is_none() {
        return Err(LxAppError::InvalidParameter(
            "a URL callback surface requires an absolute HTTPS URL".to_string(),
        ));
    }
    let scheme = uri.scheme_str().map(str::to_ascii_lowercase);
    match scheme.as_deref() {
        Some("https") => Ok(()),
        Some("http") if dev_mode || uri.host().is_some_and(is_url_callback_loopback_host) => Ok(()),
        Some("http") => Err(LxAppError::InvalidParameter(
            "a URL callback surface requires HTTPS or a loopback HTTP URL outside dev mode"
                .to_string(),
        )),
        _ => Err(LxAppError::InvalidParameter(
            "a URL callback surface requires an absolute HTTPS URL".to_string(),
        )),
    }
}

fn is_url_callback_loopback_host(host: &str) -> bool {
    let host = host
        .trim_matches(|ch| ch == '[' || ch == ']')
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") {
        return true;
    }
    host.parse::<std::net::IpAddr>()
        .is_ok_and(|ip| ip.is_loopback())
}

fn validate_surface_interaction(
    kind: SurfaceKind,
    url_callback: bool,
    interaction: lingxia_surface::SurfaceInteraction,
) -> Result<(), LxAppError> {
    if kind == SurfaceKind::Window
        && interaction.dismiss == lingxia_surface::FloatDismiss::TapOutside
    {
        return Err(LxAppError::InvalidParameter(
            "tapOutside dismissal requires an overlay surface".to_string(),
        ));
    }
    if url_callback
        && interaction.dismiss == lingxia_surface::FloatDismiss::Manual
        && !interaction.close_button
    {
        return Err(LxAppError::InvalidParameter(
            "a manual URL callback surface requires closeButton".to_string(),
        ));
    }
    Ok(())
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

fn parse_surface_edge(edge: &str) -> Result<lingxia_surface::Edge, LxAppError> {
    match edge.trim() {
        "left" | "leading" => Ok(lingxia_surface::Edge::Left),
        "right" | "trailing" => Ok(lingxia_surface::Edge::Right),
        "top" => Ok(lingxia_surface::Edge::Top),
        "bottom" => Ok(lingxia_surface::Edge::Bottom),
        other => Err(LxAppError::InvalidParameter(format!(
            "unknown surface edge: {other}"
        ))),
    }
}

fn surface_presentation(kind: SurfaceKind, role: SurfaceRole, overlay: bool) -> &'static str {
    match (role, kind, overlay) {
        (SurfaceRole::Main, _, _) => "main",
        (SurfaceRole::Aside, _, true) => "overlay",
        (SurfaceRole::Aside, _, false) => "dock",
        (SurfaceRole::Float, _, _) => "popover",
    }
}

fn surface_owner_appid(id: &str) -> Option<String> {
    crate::lxapp::list_lxapps()
        .into_iter()
        .find(|info| crate::lxapp::try_get(&info.appid).is_some_and(|owner| owner.has_surface(id)))
        .map(|info| info.appid)
}

fn remove_surface_record_from_owner(id: &str) -> bool {
    let Some(appid) = surface_owner_appid(id) else {
        return false;
    };
    crate::lxapp::try_get(&appid)
        .and_then(|owner| {
            owner
                .state
                .lock()
                .ok()
                .and_then(|state| state.surfaces.lock().ok()?.remove(id))
        })
        .is_some()
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

    #[test]
    fn url_callback_target_requires_https_outside_dev_mode() {
        assert!(validate_url_callback_target("https://auth.example.com/authorize", false).is_ok());
        assert!(matches!(
            validate_url_callback_target("http://192.168.1.20:18080/authorize", false),
            Err(LxAppError::InvalidParameter(message))
                if message == "a URL callback surface requires HTTPS or a loopback HTTP URL outside dev mode"
        ));
    }

    #[test]
    fn url_callback_target_allows_loopback_http_in_standard_mode() {
        for target in [
            "http://127.0.0.1:18080/authorize",
            "http://127.23.4.5/authorize",
            "http://localhost:18080/authorize",
            "http://auth.localhost/authorize",
            "http://[::1]:18080/authorize",
        ] {
            assert!(
                validate_url_callback_target(target, false).is_ok(),
                "loopback target should be accepted: {target}"
            );
        }
    }

    #[test]
    fn url_callback_target_rejects_hosts_that_only_resemble_loopback() {
        for target in [
            "http://localhost.example.com/authorize",
            "http://127.0.0.1.example.com/authorize",
            "http://192.168.1.20/authorize",
        ] {
            assert!(validate_url_callback_target(target, false).is_err());
        }
    }

    #[test]
    fn url_callback_target_allows_http_only_in_dev_mode() {
        assert!(validate_url_callback_target("http://127.0.0.1:18080/authorize", true).is_ok());
        assert!(validate_url_callback_target("http://192.168.1.20:18080/authorize", true).is_ok());
    }

    #[test]
    fn url_callback_target_never_allows_file_urls() {
        for dev_mode in [false, true] {
            assert!(matches!(
                validate_url_callback_target("file:///tmp/authorize.html", dev_mode),
                Err(LxAppError::InvalidParameter(message))
                    if message == "a URL callback surface cannot load file URLs"
            ));
        }
    }

    #[test]
    fn url_callback_target_rejects_other_or_relative_urls() {
        for target in [
            "ftp://auth.example.com/authorize",
            "/authorize",
            "not a url",
            " https://auth.example.com/authorize",
            "https://auth.example.com/authorize ",
        ] {
            assert!(validate_url_callback_target(target, true).is_err());
        }
    }

    #[test]
    fn url_callback_manual_dismissal_requires_native_close_button() {
        let invalid = lingxia_surface::SurfaceInteraction {
            close_button: false,
            dismiss: lingxia_surface::FloatDismiss::Manual,
            modal: true,
        };
        assert!(validate_surface_interaction(SurfaceKind::Overlay, true, invalid).is_err());

        assert!(
            validate_surface_interaction(
                SurfaceKind::Overlay,
                true,
                lingxia_surface::SurfaceInteraction::url_callback(),
            )
            .is_ok()
        );
    }

    #[test]
    fn window_rejects_tap_outside_dismissal() {
        assert!(
            validate_surface_interaction(
                SurfaceKind::Window,
                false,
                lingxia_surface::SurfaceInteraction::standard(),
            )
            .is_err()
        );
        assert!(
            validate_surface_interaction(
                SurfaceKind::Window,
                false,
                lingxia_surface::SurfaceInteraction::window(),
            )
            .is_ok()
        );
    }

    #[test]
    fn host_aside_keeps_surface_identity_separate_from_native_content() {
        let surface = host_aside_node("shell:terminal", "terminal", "bottom");

        assert_eq!(surface.id, "shell:terminal");
        assert_eq!(
            surface.content.slot_kind(),
            lingxia_surface::SlotKind::Native
        );
        assert_eq!(surface.placement.edge, Some(lingxia_surface::Edge::Bottom));
    }
}
