use super::*;
use lingxia_platform::Platform;
use lingxia_platform::traits::ui::{
    ManagedSurfaceProvider, ManagedSurfaceProviderDestroyRequest, ManagedSurfaceProviderRequest,
    SurfaceContent, SurfaceKind, SurfacePosition, SurfacePresenter,
    SurfaceRequest as PlatformSurfaceRequest, SurfaceRole as PlatformSurfaceRole, WindowChrome,
};
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

const SURFACE_DISPOSE_TTL_MS: u64 = 30_000;
static SURFACE_CLOSE_OBSERVER: OnceLock<fn(&str, &str) -> bool> = OnceLock::new();
type SurfaceActiveMainObserver = fn(Option<&str>, Option<&str>) -> bool;
static SURFACE_ACTIVE_MAIN_OBSERVER: OnceLock<SurfaceActiveMainObserver> = OnceLock::new();
type SurfaceVisibilityObserver = fn(&str, bool) -> bool;
static SURFACE_VISIBILITY_OBSERVER: OnceLock<SurfaceVisibilityObserver> = OnceLock::new();
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
    native_declarations: std::sync::Mutex<HashMap<String, NativeSurfaceDeclaration>>,
    managed_surface_serializers: ManagedSurfaceSerializers,
    runtime: std::sync::Arc<Platform>,
    next_native_surface_id: AtomicU64,
    last_published_active_main: std::sync::Mutex<Option<String>>,
    active_main_publication_blocked: std::sync::Mutex<bool>,
}

#[derive(Clone)]
struct NativeSurfaceDeclaration {
    surface: lingxia_surface::Surface,
    presentation: lingxia_surface::SurfacePresentation,
}

type ManagedSurfaceLock = futures::lock::Mutex<()>;
type ManagedSurfaceKey = (String, Option<String>);

#[derive(Default)]
struct ManagedSurfaceSerializers {
    locks: std::sync::Mutex<HashMap<ManagedSurfaceKey, std::sync::Weak<ManagedSurfaceLock>>>,
}

impl ManagedSurfaceSerializers {
    fn lock_for(
        &self,
        declaration_id: &str,
        instance_key: Option<&str>,
    ) -> Arc<ManagedSurfaceLock> {
        let key = (
            declaration_id.to_string(),
            instance_key.map(ToString::to_string),
        );
        let mut locks = self.locks.lock().unwrap();
        locks.retain(|_, lock| lock.strong_count() > 0);
        if let Some(lock) = locks.get(&key).and_then(std::sync::Weak::upgrade) {
            return lock;
        }
        let lock = Arc::new(ManagedSurfaceLock::new(()));
        locks.insert(key, Arc::downgrade(&lock));
        lock
    }
}

struct OpenNodeResult {
    surface_id: String,
    kind: SurfaceKind,
    position: SurfacePosition,
    role: lingxia_surface::Role,
    evicted: Vec<String>,
    reused: bool,
    overlay: bool,
}

fn previous_main_for_visibility<'a>(
    plan: &lingxia_surface::LayoutPresentationPlan,
    previous: Option<&'a str>,
) -> Option<&'a str> {
    previous.filter(|id| plan.mains.iter().any(|main| main == id))
}

fn managed_provider_for_surface(surface: &lingxia_surface::Surface) -> ManagedSurfaceProvider {
    match surface.content.native_identity() {
        Some((capability, instance_key)) => ManagedSurfaceProvider::Native {
            capability: capability.to_string(),
            instance_key: instance_key.map(str::to_string),
        },
        None => ManagedSurfaceProvider::Declared,
    }
}

static WINDOW_CONTROLLERS: OnceLock<
    std::sync::Mutex<HashMap<String, std::sync::Arc<WindowSurfaceController>>>,
> = OnceLock::new();
pub(crate) const PRIMARY_WINDOW: &str = "primary";

/// Get-or-create the controller for a window. On first use of a window id we
/// clone the runtime handle and seed a fresh `SurfaceManager` for that window's
/// graph.
/// Slot kind named by a shell control ("lxapp" / "browser" / "native").
fn shell_slot_kind(kind: &str) -> Option<lingxia_surface::SlotKind> {
    match kind.trim() {
        "lxapp" => Some(lingxia_surface::SlotKind::Lxapp),
        "browser" => Some(lingxia_surface::SlotKind::Browser),
        "native" => Some(lingxia_surface::SlotKind::Native),
        _ => None,
    }
}

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
                native_declarations: std::sync::Mutex::new(HashMap::new()),
                managed_surface_serializers: ManagedSurfaceSerializers::default(),
                runtime: runtime.clone(),
                next_native_surface_id: AtomicU64::new(1),
                last_published_active_main: std::sync::Mutex::new(None),
                active_main_publication_blocked: std::sync::Mutex::new(false),
            })
        })
        .clone()
}

impl WindowSurfaceController {
    async fn open_managed_native_surface(
        &self,
        declaration_id: &str,
        instance_key: Option<&str>,
        requested_role: Option<SurfaceRole>,
        requested_edge: Option<&str>,
    ) -> Result<ManagedNativeSurface, LxAppError> {
        let declaration_id = declaration_id.trim();
        let instance_key = instance_key.map(str::trim).filter(|key| !key.is_empty());
        // Provider creation is asynchronous. Serialize the full
        // resolve/ensure/register sequence per public declaration identity so
        // concurrent opens cannot allocate duplicate keyed workspaces.
        let open_lock = self
            .managed_surface_serializers
            .lock_for(declaration_id, instance_key);
        let _open_guard = open_lock.lock().await;
        let requested_edge = requested_edge.map(parse_surface_edge).transpose()?;
        let declaration = self
            .native_declarations
            .lock()
            .unwrap()
            .get(declaration_id)
            .cloned()
            .ok_or_else(|| {
                LxAppError::ResourceNotFound(format!(
                    "unknown declared native surface: {declaration_id}"
                ))
            })?;
        let capability = declaration
            .surface
            .content
            .native_identity()
            .map(|(capability, _)| capability.to_string())
            .ok_or_else(|| {
                LxAppError::InvalidParameter(format!(
                    "surface '{declaration_id}' does not use a native provider"
                ))
            })?;
        let (surface, presentation) = {
            let manager = self.manager.lock().unwrap();
            let existing = if instance_key.is_none() {
                manager.graph().get(declaration_id)
            } else {
                manager.graph().surfaces().iter().find(|surface| {
                    surface.content.native_identity() == Some((capability.as_str(), instance_key))
                })
            };
            if let Some(existing) = existing {
                let mut surface = existing.clone();
                surface.state = lingxia_surface::SurfaceState::Mounted;
                if let Some(role) = requested_role {
                    if manager.graph().is_root_main(&surface.id)
                        && role != lingxia_surface::Role::Main
                    {
                        return Err(LxAppError::UnsupportedOperation(
                            "the stable root main surface cannot change role".to_string(),
                        ));
                    }
                    surface.role = role;
                    if surface.role != lingxia_surface::Role::Aside {
                        surface.placement.edge = None;
                    }
                }
                if surface.role == lingxia_surface::Role::Aside
                    && let Some(edge) = requested_edge
                {
                    surface.placement.edge = Some(edge);
                }
                (surface, None)
            } else {
                let sequence = instance_key
                    .map(|_| self.next_native_surface_id.fetch_add(1, Ordering::Relaxed));
                let (surface, presentation) = instantiate_native_declaration(
                    declaration,
                    &capability,
                    instance_key,
                    sequence,
                    requested_role,
                    requested_edge,
                );
                (surface, Some(presentation))
            }
        };
        if requested_edge.is_some() && surface.role != lingxia_surface::Role::Aside {
            return Err(LxAppError::InvalidParameter(
                "edge is only valid for an aside surface".to_string(),
            ));
        }
        let platform_role = match surface.role {
            lingxia_surface::Role::Main => PlatformSurfaceRole::Main,
            lingxia_surface::Role::Aside => PlatformSurfaceRole::Aside,
            lingxia_surface::Role::Float => {
                return Err(LxAppError::UnsupportedOperation(
                    "native float surfaces do not support instances".to_string(),
                ));
            }
        };
        let edge = surface.placement.edge.map(surface_edge_name);
        self.runtime
            .ensure_managed_surface_provider(ManagedSurfaceProviderRequest {
                surface_id: surface.id.clone(),
                provider: ManagedSurfaceProvider::Native {
                    capability: capability.clone(),
                    instance_key: instance_key.map(str::to_string),
                },
                role: Some(platform_role),
                edge: edge.map(str::to_string),
            })
            .await?;
        {
            let mut manager = self.manager.lock().unwrap();
            manager.open(surface.clone());
            if let Some(presentation) = presentation {
                manager.set_presentation(&surface.id, presentation);
            }
            if surface.role == lingxia_surface::Role::Main {
                manager.set_active_main(&surface.id);
            }
        }
        if surface.role == lingxia_surface::Role::Main {
            *self.active_main_publication_blocked.lock().unwrap() = false;
        }
        self.commit();
        Ok(ManagedNativeSurface {
            surface_id: surface.id,
            role: surface.role,
        })
    }

    /// THE single commit point for this window's graph mutations: re-derive the
    /// `DerivedLayout` and hand it to the platform skin to reconcile. Platforms
    /// without `present_layout` return `NotSupported`, treated as a successful
    /// no-op here. Other presentation failures leave active-main publication
    /// pending so a later successful commit can reconcile observers. The manager
    /// lock is scoped to the `derive` call and dropped before `present_layout`,
    /// so the lock is never held across the outbound call.
    fn commit(&self) -> bool {
        let plan = self.manager.lock().unwrap().presentation_plan();
        match self.runtime.present_layout(&self.window_id, &plan) {
            Ok(()) | Err(lingxia_platform::PlatformError::NotSupported(_)) => {}
            Err(_) => return false,
        }
        if *self.active_main_publication_blocked.lock().unwrap() {
            return true;
        }
        let current = plan.active_main_id.clone();
        let previous = {
            let mut published = self.last_published_active_main.lock().unwrap();
            if *published == current {
                return true;
            }
            std::mem::replace(&mut *published, current.clone())
        };
        if let Some(observer) = SURFACE_ACTIVE_MAIN_OBSERVER.get() {
            // A role migration can remove the previous active id from `mains`
            // while keeping that same surface mounted as an aside. Do not let
            // the old main-deactivation notification hide its reused handle.
            let previous = previous_main_for_visibility(&plan, previous.as_deref());
            let _ = observer(previous, current.as_deref());
        }
        true
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
        let mut present_role = lingxia_surface::Role::Main;
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
            (present_kind, present_position) =
                present_params_for_role(role, edge, requested_position);
            present_role = role;
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

    fn close(&self, id: &str, reason: &str) -> lingxia_surface::CloseOutcome {
        let before = self.manager.lock().unwrap().graph().active_main_id.clone();
        let outcome = self.close_deferred(id, reason);
        let active_changed = self.manager.lock().unwrap().graph().active_main_id != before;
        if active_changed {
            *self.active_main_publication_blocked.lock().unwrap() = false;
        }
        self.commit();
        outcome
    }

    fn close_deferred(&self, id: &str, reason: &str) -> lingxia_surface::CloseOutcome {
        let (outcome, active_changed) = {
            let mut manager = self.manager.lock().unwrap();
            let before = manager.graph().active_main_id.clone();
            let outcome = manager.close(id);
            let changed = manager.graph().active_main_id != before;
            (outcome, changed)
        };
        if active_changed {
            *self.active_main_publication_blocked.lock().unwrap() = true;
        }
        for removed in outcome.removed() {
            notify_surface_close_observer(removed, reason);
        }
        outcome
    }

    fn close_other_mains(&self, keeping: &str) -> Vec<String> {
        let removed = self.manager.lock().unwrap().close_other_mains(keeping);
        if !removed.is_empty() {
            for surface_id in &removed {
                notify_surface_close_observer(surface_id, "user");
            }
            self.commit();
        }
        removed
    }

    fn close_mains_after(&self, surface_id: &str) -> Vec<String> {
        let removed = self.manager.lock().unwrap().close_mains_after(surface_id);
        if !removed.is_empty() {
            for surface_id in &removed {
                notify_surface_close_observer(surface_id, "user");
            }
            self.commit();
        }
        removed
    }

    fn contains(&self, id: &str) -> bool {
        self.manager.lock().unwrap().graph().get(id).is_some()
    }

    fn is_root_main(&self, id: &str) -> bool {
        self.manager.lock().unwrap().graph().is_root_main(id)
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

    async fn set_managed_surface_visible(
        &self,
        id: &str,
        visible: bool,
        role: Option<SurfaceRole>,
        edge: Option<&str>,
    ) -> Result<(), LxAppError> {
        // Provider creation is asynchronous. Serialize it with close for this
        // concrete surface id so close cannot destroy the provider between the
        // ensure and the graph mutation and leave a mounted dead reference.
        let lifecycle_lock = self.managed_surface_serializers.lock_for(id, None);
        let _lifecycle_guard = lifecycle_lock.lock().await;
        let parsed_edge = edge.map(parse_surface_edge).transpose()?;
        let (current_role, is_root_main, provider) = {
            let manager = self.manager.lock().unwrap();
            let provider = manager
                .graph()
                .get(id)
                .map(managed_provider_for_surface)
                .unwrap_or(ManagedSurfaceProvider::Declared);
            (
                manager.graph().role_of(id),
                manager.graph().is_root_main(id),
                provider,
            )
        };
        if is_root_main && role.is_some_and(|role| role != lingxia_surface::Role::Main) {
            return Err(LxAppError::UnsupportedOperation(
                "the stable root main surface cannot change role".to_string(),
            ));
        }
        let effective_role = role.or(current_role);
        if parsed_edge.is_some()
            && effective_role.is_some_and(|role| role != lingxia_surface::Role::Aside)
        {
            return Err(LxAppError::InvalidParameter(
                "edge is only valid for an aside surface".to_string(),
            ));
        }
        if visible {
            self.runtime
                .ensure_managed_surface_provider(ManagedSurfaceProviderRequest {
                    surface_id: id.to_string(),
                    provider,
                    role: effective_role.map(Into::into),
                    edge: edge.map(str::to_string),
                })
                .await?;
        }
        let changed = {
            let mut manager = self.manager.lock().unwrap();
            if visible {
                if (role.is_some() || parsed_edge.is_some())
                    && let Some(mut surface) = manager.graph().get(id).cloned()
                {
                    if let Some(role) = role {
                        surface.role = if manager.graph().is_root_main(id) {
                            lingxia_surface::Role::Main
                        } else {
                            role
                        };
                        if surface.role != lingxia_surface::Role::Aside {
                            surface.placement.edge = None;
                        }
                    }
                    if surface.role == lingxia_surface::Role::Aside
                        && let Some(edge) = parsed_edge
                    {
                        surface.placement.edge = Some(edge);
                    }
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

    async fn close_managed_surface(
        &self,
        id: &str,
        role: Option<SurfaceRole>,
    ) -> Result<(), LxAppError> {
        let lifecycle_lock = self.managed_surface_serializers.lock_for(id, None);
        let _lifecycle_guard = lifecycle_lock.lock().await;
        let (provider, current_role) = {
            let manager = self.manager.lock().unwrap();
            let surface = manager.graph().get(id);
            (
                surface
                    .map(managed_provider_for_surface)
                    .unwrap_or(ManagedSurfaceProvider::Declared),
                surface.map(|surface| surface.role),
            )
        };
        let role = current_role.or(role);
        let outcome = self.close_deferred(id, "programmatic");
        match outcome {
            lingxia_surface::CloseOutcome::Closed { .. } => {
                self.commit();
            }
            lingxia_surface::CloseOutcome::RejectedRoot { .. } => {
                return Err(LxAppError::UnsupportedOperation(
                    "the stable root main surface cannot be closed".to_string(),
                ));
            }
            lingxia_surface::CloseOutcome::NotFound => {
                return Err(LxAppError::InvalidParameter(format!(
                    "unknown surface: {id}"
                )));
            }
        }
        let destroy_result = self
            .runtime
            .destroy_managed_surface_provider(ManagedSurfaceProviderDestroyRequest {
                surface_id: id.to_string(),
                provider,
                role: role.map(Into::into),
            })
            .await;
        // Platforms normally activate the successor during provider teardown
        // and call set_active_main_surface, which unblocks publication. Treat
        // provider completion as the fallback barrier so a missing callback or
        // teardown error cannot suppress the new active main indefinitely.
        let publication_was_blocked = {
            let mut blocked = self.active_main_publication_blocked.lock().unwrap();
            std::mem::replace(&mut *blocked, false)
        };
        if publication_was_blocked {
            self.commit();
        }
        destroy_result?;
        Ok(())
    }

    fn surface_presentation(&self, id: &str) -> Option<&'static str> {
        let plan = self.manager.lock().unwrap().presentation_plan();
        if plan.floats.iter().any(|surface| surface.id == id) {
            return Some("popover");
        }
        plan.aside_slots
            .iter()
            .find(|slot| slot.children.iter().any(|child| child == id))
            .map(|slot| if slot.overlay { "overlay" } else { "dock" })
    }

    fn managed_surface_role(&self, id: &str) -> Option<SurfaceRole> {
        self.manager.lock().unwrap().graph().role_of(id)
    }

    fn managed_surface_visible(&self, id: &str) -> Option<bool> {
        let manager = self.manager.lock().unwrap();
        let graph = manager.graph();
        let surface = graph.get(id)?;
        Some(match surface.role {
            lingxia_surface::Role::Main => graph.active_main_id.as_deref() == Some(id),
            lingxia_surface::Role::Aside | lingxia_surface::Role::Float => {
                surface.state == lingxia_surface::SurfaceState::Mounted
            }
        })
    }

    /// Mirror a host-declared aside into the core graph, seeding the root `main`
    /// if absent so the aside has a primary to dock to, and commit.
    fn register_host_aside(
        &self,
        surface_id: &str,
        content: lingxia_surface::SurfaceContent,
        edge: &str,
        root_main: lingxia_surface::Surface,
    ) {
        let node = host_aside_node(surface_id, content, edge);
        {
            let mut manager = self.manager.lock().unwrap();
            if manager.graph().mains().is_empty() {
                manager.open(root_main);
            }
            let _ = manager.open(node);
        }
        self.commit();
    }

    fn register_native_aside_declaration(&self, surface_id: &str, capability: &str, edge: &str) {
        let surface = host_aside_node(
            surface_id,
            lingxia_surface::SurfaceContent::Native {
                capability: capability.to_string(),
                instance_key: None,
            },
            edge,
        );
        self.native_declarations.lock().unwrap().insert(
            surface_id.to_string(),
            NativeSurfaceDeclaration {
                presentation: lingxia_surface::SurfacePresentation::for_content(&surface.content),
                surface,
            },
        );
    }

    /// Make `app_id`'s main the active (primary) main, seeding its root `main`
    /// into the graph first if it isn't a node yet, then commit. The commit
    /// rebuilds the plan with the new `activeMainId` and pushes `present_layout`,
    /// so the skin reconciler drives the actual switch. Idempotent: when the
    /// node already exists and is already active, `set_active_main` does not
    /// change state, but we still commit so a reconciler that missed the
    /// (already-correct) plan can re-converge — the reconciler is itself a no-op
    /// when the target main is already attached.
    fn set_active_main(&self, app_id: &str, title: &str, root_main: lingxia_surface::Surface) {
        {
            let mut manager = self.manager.lock().unwrap();
            // A tab's appid may not be a graph node yet (the main is seeded lazily
            // by set_width / register_host_aside). Seed it before switching, else
            // set_active_main silently no-ops on an unknown id.
            if manager.graph().role_of(app_id).is_none() {
                let mut presentation = lxapp_workspace_presentation(&root_main.content);
                presentation.automatic_title = Some(title.to_string());
                // The first main remains the stable, non-closable root by graph
                // identity. Later lxapps are ordinary workspaces and must expose
                // lifecycle controls in the platform switcher.
                let _ = manager.open_main(root_main, presentation);
            } else {
                manager.update_automatic_title(app_id, Some(title));
            }
            manager.set_active_main(app_id);
        }
        *self.active_main_publication_blocked.lock().unwrap() = false;
        self.commit();
    }

    fn replace_host_mains(
        &self,
        registrations: Vec<HostMainSurfaceRegistration>,
    ) -> Result<lingxia_surface::SurfaceSwitcherSnapshot, LxAppError> {
        let mains = registrations
            .into_iter()
            .map(HostMainSurfaceRegistration::into_surface)
            .collect::<Result<Vec<_>, LxAppError>>()?;
        {
            let mut declarations = self.native_declarations.lock().unwrap();
            for (surface, presentation) in &mains {
                if let Some((_, None)) = surface.content.native_identity() {
                    declarations.insert(
                        surface.id.clone(),
                        NativeSurfaceDeclaration {
                            surface: surface.clone(),
                            presentation: presentation.clone(),
                        },
                    );
                }
            }
        }
        let snapshot = self
            .manager
            .lock()
            .unwrap()
            .replace_mains(mains)
            .map_err(|error| LxAppError::InvalidParameter(error.to_string()))?;
        self.commit();
        Ok(snapshot)
    }

    fn open_host_main(
        &self,
        registration: HostMainSurfaceRegistration,
    ) -> Result<lingxia_surface::SurfaceSwitcherSnapshot, LxAppError> {
        let (surface, presentation) = registration.into_surface()?;
        if let Some((_, None)) = surface.content.native_identity() {
            self.native_declarations.lock().unwrap().insert(
                surface.id.clone(),
                NativeSurfaceDeclaration {
                    surface: surface.clone(),
                    presentation: presentation.clone(),
                },
            );
        }
        let snapshot = self
            .manager
            .lock()
            .unwrap()
            .open_main(surface, presentation)
            .map_err(|error| LxAppError::InvalidParameter(error.to_string()))?;
        self.commit();
        Ok(snapshot)
    }

    fn set_active_main_surface(&self, surface_id: &str) -> bool {
        let active = self.manager.lock().unwrap().set_active_main(surface_id);
        if active {
            *self.active_main_publication_blocked.lock().unwrap() = false;
            self.commit();
        }
        active
    }

    fn switcher_snapshot(&self) -> lingxia_surface::SurfaceSwitcherSnapshot {
        self.manager.lock().unwrap().switcher_snapshot()
    }

    fn main_surface_content(&self, surface_id: &str) -> Option<lingxia_surface::SurfaceContent> {
        self.manager
            .lock()
            .unwrap()
            .graph()
            .get(surface_id)
            .filter(|surface| surface.role == lingxia_surface::Role::Main)
            .map(|surface| surface.content.clone())
    }

    fn surface_menu(
        &self,
        surface_id: &str,
        content_groups: Vec<Vec<lingxia_shell::SurfaceMenuItem>>,
    ) -> Option<lingxia_shell::SurfaceMenuSnapshot> {
        let snapshot = self.manager.lock().unwrap().switcher_snapshot();
        let index = snapshot
            .items
            .iter()
            .position(|item| item.surface_id == surface_id)?;
        let item = &snapshot.items[index];
        Some(lingxia_shell::compose_surface_menu(
            lingxia_shell::SurfaceMenuContext {
                revision: snapshot.revision,
                surface_id: item.surface_id.clone(),
                closable: item.closable,
                renameable: item.renameable,
                title_overridden: item.title_overridden,
                has_other_closable: snapshot
                    .items
                    .iter()
                    .any(|candidate| candidate.surface_id != surface_id && candidate.closable),
                has_closable_before: snapshot
                    .items
                    .iter()
                    .take(index)
                    .any(|candidate| candidate.closable),
                has_closable_after: snapshot
                    .items
                    .iter()
                    .skip(index + 1)
                    .any(|candidate| candidate.closable),
            },
            content_groups,
        ))
    }

    fn perform_surface_menu_intent(
        &self,
        intent: lingxia_shell::SurfaceMenuIntent,
    ) -> HostSurfaceMenuExecution {
        let before = self.manager.lock().unwrap().graph().active_main_id.clone();
        let execution = self.perform_surface_menu_intent_deferred(intent);
        if execution.accepted {
            let active_changed = self.manager.lock().unwrap().graph().active_main_id != before;
            if active_changed {
                *self.active_main_publication_blocked.lock().unwrap() = false;
            }
            self.commit();
        }
        execution
    }

    fn perform_surface_menu_intent_deferred(
        &self,
        intent: lingxia_shell::SurfaceMenuIntent,
    ) -> HostSurfaceMenuExecution {
        let (execution, active_changed) = {
            let mut manager = self.manager.lock().unwrap();
            let before = manager.graph().active_main_id.clone();
            let execution = execute_surface_menu_intent(&mut manager, intent);
            let changed = manager.graph().active_main_id != before;
            (execution, changed)
        };
        if active_changed {
            *self.active_main_publication_blocked.lock().unwrap() = true;
        }
        if execution.accepted {
            for surface_id in &execution.removed_surface_ids {
                notify_surface_close_observer(surface_id, "user");
            }
        }
        execution
    }

    fn update_surface_automatic_title(&self, surface_id: &str, title: Option<&str>) -> bool {
        let updated = self
            .manager
            .lock()
            .unwrap()
            .update_automatic_title(surface_id, title);
        if updated {
            self.commit();
        }
        updated
    }

    fn rename_surface(&self, surface_id: &str, title: Option<&str>) -> bool {
        let updated = self.manager.lock().unwrap().rename(surface_id, title);
        if updated {
            self.commit();
        }
        updated
    }

    fn unregister_host_aside(&self, surface_id: &str) {
        let _ = self.close(surface_id, "programmatic");
    }

    /// Mirror a visibility change initiated by the platform shell without
    /// re-entering the platform presenter.
    fn mark_surface_hidden_from_shell(&self, surface_id: &str) -> bool {
        let hidden = self.manager.lock().unwrap().hide(surface_id);
        if hidden {
            self.commit();
            if let Some(observer) = SURFACE_VISIBILITY_OBSERVER.get() {
                let _ = observer(surface_id, false);
            }
        }
        hidden
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

    fn slot_collapsed(&self, kind: lingxia_surface::SlotKind) -> bool {
        self.manager.lock().unwrap().graph().slot_collapsed(kind)
    }

    fn set_slot_collapsed(&self, kind: lingxia_surface::SlotKind, collapsed: bool) -> bool {
        let changed = self
            .manager
            .lock()
            .unwrap()
            .set_slot_collapsed(kind, collapsed);
        if changed {
            self.commit();
        }
        changed
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

fn lxapp_workspace_presentation(
    content: &lingxia_surface::SurfaceContent,
) -> lingxia_surface::SurfacePresentation {
    let mut presentation = lingxia_surface::SurfacePresentation::for_content(content);
    presentation.capabilities.close = true;
    presentation
}

fn instantiate_native_declaration(
    declaration: NativeSurfaceDeclaration,
    capability: &str,
    instance_key: Option<&str>,
    sequence: Option<u64>,
    requested_role: Option<SurfaceRole>,
    requested_edge: Option<lingxia_surface::Edge>,
) -> (
    lingxia_surface::Surface,
    lingxia_surface::SurfacePresentation,
) {
    let mut surface = declaration.surface;
    let mut presentation = declaration.presentation;
    if let Some(instance_key) = instance_key {
        surface.id = format!(
            "native:{capability}:{}",
            sequence.expect("keyed native instances require a sequence")
        );
        surface.role = requested_role.unwrap_or(surface.role);
        surface.placement = if surface.role == lingxia_surface::Role::Aside {
            lingxia_surface::Placement {
                edge: requested_edge.or(surface.placement.edge),
                preferred_size: surface.placement.preferred_size,
            }
        } else {
            Default::default()
        };
        surface.float = None;
        presentation.capabilities.close = true;
        presentation.capabilities.rename = true;
        presentation.automatic_title = Some(match capability {
            "terminal" => "Terminal".to_string(),
            "browser" => "Browser".to_string(),
            _ => capability.to_string(),
        });
        surface.content = lingxia_surface::SurfaceContent::Native {
            capability: capability.to_string(),
            instance_key: Some(instance_key.to_string()),
        };
    } else {
        surface.content = lingxia_surface::SurfaceContent::Native {
            capability: capability.to_string(),
            instance_key: None,
        };
    }
    (surface, presentation)
}

fn host_aside_node(
    surface_id: &str,
    content: lingxia_surface::SurfaceContent,
    edge: &str,
) -> lingxia_surface::Surface {
    use lingxia_surface::{Edge, Placement, Role, Surface, SurfaceOwner, SurfaceState};
    let edge = match edge {
        "left" | "leading" => Edge::Left,
        "top" => Edge::Top,
        "bottom" => Edge::Bottom,
        _ => Edge::Right,
    };
    Surface {
        id: surface_id.to_string(),
        role: Role::Aside,
        content,
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

pub fn register_surface_active_main_observer(observer: fn(Option<&str>, Option<&str>) -> bool) {
    let _ = SURFACE_ACTIVE_MAIN_OBSERVER.set(observer);
}

pub fn register_surface_visibility_observer(observer: fn(&str, bool) -> bool) {
    let _ = SURFACE_VISIBILITY_OBSERVER.set(observer);
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

impl PageSurfaceRequest {
    /// Required identity and content. Optional fields start unset; a float
    /// overlay is the honest default for a sheet that did not pick a shape.
    /// There is no `Default` — an empty id is not a request.
    pub fn new(id: impl Into<String>, target: PageSurfaceTarget) -> Self {
        Self {
            id: id.into(),
            target,
            query: None,
            kind: SurfaceKind::Overlay,
            width: None,
            height: None,
            width_ratio: None,
            height_ratio: None,
            position: SurfacePosition::Center,
            role: lingxia_surface::Role::Float,
            interaction: None,
        }
    }

    pub fn query(mut self, query: impl Into<Option<PageQueryInput>>) -> Self {
        self.query = query.into();
        self
    }

    pub fn kind(mut self, kind: SurfaceKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn width(mut self, width: impl Into<Option<f64>>) -> Self {
        self.width = width.into();
        self
    }

    pub fn height(mut self, height: impl Into<Option<f64>>) -> Self {
        self.height = height.into();
        self
    }

    pub fn width_ratio(mut self, width_ratio: impl Into<Option<f64>>) -> Self {
        self.width_ratio = width_ratio.into();
        self
    }

    pub fn height_ratio(mut self, height_ratio: impl Into<Option<f64>>) -> Self {
        self.height_ratio = height_ratio.into();
        self
    }

    pub fn position(mut self, position: SurfacePosition) -> Self {
        self.position = position;
        self
    }

    pub fn role(mut self, role: lingxia_surface::Role) -> Self {
        self.role = role;
        self
    }

    pub fn interaction(
        mut self,
        interaction: impl Into<Option<lingxia_surface::SurfaceInteraction>>,
    ) -> Self {
        self.interaction = interaction.into();
        self
    }
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
    pub role: lingxia_surface::Role,
    pub position: SurfacePosition,
    pub presentation: String,
    /// Window decoration this surface was opened with. Drives the page-chrome
    /// top inset so a `chrome: 'full'` page can lay out under the drag strip.
    pub chrome: WindowChrome,
}

/// Resolved identity and declaration-owned role of a managed native surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedNativeSurface {
    pub surface_id: String,
    pub role: lingxia_surface::Role,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostMainSurfaceRegistration {
    pub id: String,
    pub content: lingxia_surface::SurfaceContent,
    pub presentation: lingxia_surface::SurfacePresentation,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostSurfaceMenuExecution {
    pub accepted: bool,
    pub removed_surface_ids: Vec<String>,
    pub snapshot: lingxia_surface::SurfaceSwitcherSnapshot,
}

impl HostSurfaceMenuExecution {
    fn rejected(snapshot: lingxia_surface::SurfaceSwitcherSnapshot) -> Self {
        Self {
            accepted: false,
            removed_surface_ids: Vec::new(),
            snapshot,
        }
    }
}

fn execute_surface_menu_intent(
    manager: &mut lingxia_surface::SurfaceManager,
    intent: lingxia_shell::SurfaceMenuIntent,
) -> HostSurfaceMenuExecution {
    let before = manager.switcher_snapshot();
    let Some(item) = before
        .items
        .iter()
        .find(|item| item.surface_id == intent.surface_id)
    else {
        return HostSurfaceMenuExecution::rejected(before);
    };
    if before.revision != intent.revision {
        return HostSurfaceMenuExecution::rejected(before);
    }

    let mut removed_surface_ids = Vec::new();
    let accepted = match intent.action {
        lingxia_shell::SurfaceMenuAction::Switcher { action } => match action {
            lingxia_shell::SurfaceMenuBuiltinAction::Rename => intent
                .value
                .as_deref()
                .map(str::trim)
                .filter(|title| !title.is_empty())
                .is_some_and(|title| manager.rename(&intent.surface_id, Some(title))),
            lingxia_shell::SurfaceMenuBuiltinAction::ResetTitle => {
                item.renameable && manager.rename(&intent.surface_id, None)
            }
            lingxia_shell::SurfaceMenuBuiltinAction::Close => {
                if !item.closable {
                    false
                } else {
                    match manager.close(&intent.surface_id) {
                        lingxia_surface::CloseOutcome::Closed { removed } => {
                            removed_surface_ids = removed;
                            true
                        }
                        lingxia_surface::CloseOutcome::RejectedRoot { .. }
                        | lingxia_surface::CloseOutcome::NotFound => false,
                    }
                }
            }
            lingxia_shell::SurfaceMenuBuiltinAction::CloseOthers => {
                removed_surface_ids = manager.close_other_mains(&intent.surface_id);
                !removed_surface_ids.is_empty()
            }
            lingxia_shell::SurfaceMenuBuiltinAction::CloseAfter => {
                removed_surface_ids = manager.close_mains_after(&intent.surface_id);
                !removed_surface_ids.is_empty()
            }
        },
        // Provider actions are dispatched by the provider integration, never
        // interpreted as shell lifecycle operations.
        lingxia_shell::SurfaceMenuAction::Information {}
        | lingxia_shell::SurfaceMenuAction::Lxapp { .. }
        | lingxia_shell::SurfaceMenuAction::External { .. } => false,
    };
    HostSurfaceMenuExecution {
        accepted,
        removed_surface_ids,
        snapshot: manager.switcher_snapshot(),
    }
}

impl HostMainSurfaceRegistration {
    fn into_surface(
        self,
    ) -> Result<
        (
            lingxia_surface::Surface,
            lingxia_surface::SurfacePresentation,
        ),
        LxAppError,
    > {
        let id = self.id.trim().to_string();
        if id.is_empty() {
            return Err(LxAppError::InvalidParameter(
                "host main surface id must not be empty".into(),
            ));
        }
        Ok((
            lingxia_surface::Surface {
                id,
                role: lingxia_surface::Role::Main,
                content: self.content,
                owner: lingxia_surface::SurfaceOwner::Host,
                placement: Default::default(),
                state: lingxia_surface::SurfaceState::Mounted,
                float: None,
            },
            self.presentation,
        ))
    }
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
    /// Window decoration, so the page-chrome layout can publish the drag
    /// strip's height to the page hosted inside.
    pub chrome: WindowChrome,
    pub owner_page_instance_id: Option<String>,
    /// The page instance hosted inside this surface (when content is a page).
    /// Used to close the surface when its inner page is disposed (e.g. SDK
    /// reclaim after long hide) so the owner's `Surface` handle reliably
    /// receives an onClose event.
    pub content_page_instance_id: Option<String>,
    pub content: SurfaceContent,
    pub target: String,
    pub kind: SurfaceKind,
    pub role: lingxia_surface::Role,
    pub url_callback: bool,
    pub ephemeral_web_data: bool,
}

impl LxApp {
    pub fn open_surface(&self, request: PageSurfaceRequest) -> Result<PageSurface, LxAppError> {
        self.open_surface_with_chrome(request, WindowChrome::default())
    }

    /// Opens a surface with explicit window decoration.
    ///
    /// Decoration stays outside [`PageSurfaceRequest`] so provider crates that
    /// construct that public request remain source-compatible.
    pub fn open_surface_with_chrome(
        &self,
        request: PageSurfaceRequest,
        chrome: WindowChrome,
    ) -> Result<PageSurface, LxAppError> {
        self.open_surface_with_web_data(request, chrome, false, false)
    }

    fn open_surface_with_web_data(
        &self,
        request: PageSurfaceRequest,
        chrome: WindowChrome,
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
            return self.open_window_surface(id, request, interaction, chrome);
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
        let mut present_role = lingxia_surface::Role::Main;
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
                chrome,
            });
        }

        if let Ok(state) = self.state.lock() {
            state.surfaces.lock().unwrap().insert(
                id.clone(),
                SurfaceRecord {
                    chrome,
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
            role: present_role.into(),
            interaction,
            chrome,
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
        // id). The commit below is the single visibility projection and drops
        // evicted providers from the rendered tree.
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
            chrome,
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
        chrome: WindowChrome,
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
                    chrome,
                    owner_page_instance_id: owner_pid,
                    content_page_instance_id,
                    content,
                    target: path.clone(),
                    kind: SurfaceKind::Window,
                    role: lingxia_surface::Role::Main,
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
            role: PlatformSurfaceRole::Main,
            interaction,
            chrome,
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
            role: lingxia_surface::Role::Main,
            position: SurfacePosition::Center,
            presentation: "window".to_string(),
            chrome,
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
        if controller.is_root_main(id) {
            return Err(LxAppError::UnsupportedOperation(format!(
                "root main surface '{id}' cannot be closed"
            )));
        }

        let platform_owner_appid = surface_owner_appid(id).unwrap_or_else(|| self.appid.clone());
        match self
            .runtime
            .close_surface(&platform_owner_appid, id, reason)
        {
            Ok(()) => {
                let mut removed = controller.close(id, reason).into_removed();
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
        let surface =
            self.open_surface_with_web_data(request, WindowChrome::default(), true, true)?;
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
    pub async fn set_shell_surface_visible(
        &self,
        id: &str,
        visible: bool,
        role: Option<SurfaceRole>,
        edge: Option<&str>,
    ) -> Result<(), LxAppError> {
        let id = id.trim();
        if id.is_empty() {
            return Err(LxAppError::InvalidParameter(
                "shell surface id must not be empty".to_string(),
            ));
        }
        window_controller(PRIMARY_WINDOW, &self.runtime)
            .set_managed_surface_visible(id, visible, role, edge)
            .await
    }

    /// Destroy a managed surface by runtime id. The stable root main rejects
    /// this operation; providers report unknown ids as errors.
    pub async fn close_shell_managed_surface(
        &self,
        surface_id: &str,
        role: Option<SurfaceRole>,
    ) -> Result<(), LxAppError> {
        let surface_id = surface_id.trim();
        if surface_id.is_empty() {
            return Err(LxAppError::InvalidParameter(
                "surface id must not be empty".to_string(),
            ));
        }
        window_controller(PRIMARY_WINDOW, &self.runtime)
            .close_managed_surface(surface_id, role)
            .await
    }

    /// Open or focus a declared native capability. An instance key is trimmed
    /// and must contain 1 to 128 UTF-8 bytes; equal keys are serialized and
    /// resolve to the same live surface.
    pub async fn open_shell_native_surface(
        &self,
        declaration_id: &str,
        instance_key: Option<&str>,
        role: Option<SurfaceRole>,
        edge: Option<&str>,
    ) -> Result<ManagedNativeSurface, LxAppError> {
        let declaration_id = declaration_id.trim();
        if declaration_id.is_empty() {
            return Err(LxAppError::InvalidParameter(
                "surface declaration id must not be empty".to_string(),
            ));
        }
        let instance_key = instance_key.map(str::trim);
        if instance_key.is_some_and(|key| key.is_empty() || key.len() > 128) {
            return Err(LxAppError::InvalidParameter(
                "instance key must contain 1 to 128 UTF-8 bytes".to_string(),
            ));
        }
        window_controller(PRIMARY_WINDOW, &self.runtime)
            .open_managed_native_surface(declaration_id, instance_key, role, edge)
            .await
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
            lingxia_surface::SurfaceContent::Lxapp {
                app_id: surface_id.to_string(),
                path: None,
            },
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
            lingxia_surface::SurfaceContent::Native {
                capability: content_id.to_string(),
                instance_key: None,
            },
            edge,
            self.root_main_node(),
        );
    }

    pub fn register_host_native_aside_declaration(
        &self,
        surface_id: &str,
        capability: &str,
        edge: &str,
    ) {
        let surface_id = surface_id.trim();
        let capability = capability.trim();
        if surface_id.is_empty() || capability.is_empty() {
            return;
        }
        window_controller(PRIMARY_WINDOW, &self.runtime)
            .register_native_aside_declaration(surface_id, capability, edge);
    }

    /// Make this lxapp's main the active (primary) main in the window graph,
    /// seeding its root `main` node if absent, then commit. The commit pushes a
    /// `present_layout` carrying the new `activeMainId`, which the skin reconciler
    /// uses to attach this lxapp's content to the primary area. The skin must NOT
    /// drive the switch imperatively — it routes the switch through here so the
    /// graph stays the single source of truth.
    pub fn set_active_main(&self) {
        let title = self.get_lxapp_info().app_name;
        window_controller(PRIMARY_WINDOW, &self.runtime).set_active_main(
            &self.appid,
            &title,
            self.root_main_node(),
        );
    }

    /// Explicitly bring this main provider to the front. Unlike the startup
    /// publication above, this one-shot intent may replace a browser that is
    /// physically covering an already-active lxapp graph node.
    pub fn activate_main(&self) {
        self.runtime.request_lxapp_main_activation(&self.appid);
        self.set_active_main();
    }

    pub fn replace_host_mains(
        &self,
        registrations: Vec<HostMainSurfaceRegistration>,
    ) -> Result<lingxia_surface::SurfaceSwitcherSnapshot, LxAppError> {
        window_controller(PRIMARY_WINDOW, &self.runtime).replace_host_mains(registrations)
    }

    pub fn open_host_main(
        &self,
        registration: HostMainSurfaceRegistration,
    ) -> Result<lingxia_surface::SurfaceSwitcherSnapshot, LxAppError> {
        window_controller(PRIMARY_WINDOW, &self.runtime).open_host_main(registration)
    }

    pub fn set_active_main_surface(&self, surface_id: &str) -> bool {
        let surface_id = surface_id.trim();
        !surface_id.is_empty()
            && window_controller(PRIMARY_WINDOW, &self.runtime).set_active_main_surface(surface_id)
    }

    pub fn surface_switcher_snapshot(&self) -> lingxia_surface::SurfaceSwitcherSnapshot {
        window_controller(PRIMARY_WINDOW, &self.runtime).switcher_snapshot()
    }

    pub fn main_surface_content(
        &self,
        surface_id: &str,
    ) -> Option<lingxia_surface::SurfaceContent> {
        let surface_id = surface_id.trim();
        (!surface_id.is_empty())
            .then(|| {
                window_controller(PRIMARY_WINDOW, &self.runtime).main_surface_content(surface_id)
            })
            .flatten()
    }

    pub fn shell_surface_menu(
        &self,
        surface_id: &str,
    ) -> Option<lingxia_shell::SurfaceMenuSnapshot> {
        let surface_id = surface_id.trim();
        if surface_id.is_empty() {
            return None;
        }
        let content_groups = match self.main_surface_content(surface_id) {
            Some(lingxia_surface::SurfaceContent::Lxapp { app_id, .. }) => crate::try_get(&app_id)
                .map(|app| {
                    let info = app.get_lxapp_info();
                    let mut groups = vec![
                        vec![lingxia_shell::SurfaceMenuItem::information(
                            lxapp_surface_menu_header(
                                &app_id,
                                &info.app_name,
                                &info.version,
                                &info.release_type,
                            ),
                        )],
                        vec![
                            lingxia_shell::SurfaceMenuItem::lxapp(
                                lingxia_shell::LxappSurfaceMenuAction::Restart,
                            ),
                            lingxia_shell::SurfaceMenuItem::lxapp(
                                lingxia_shell::LxappSurfaceMenuAction::CleanCacheRestart,
                            ),
                        ],
                    ];
                    let actions = app.more_actions();
                    let generation = actions.generation;
                    let more_actions = actions
                        .items
                        .into_iter()
                        .enumerate()
                        .map(|(index, item)| {
                            lingxia_shell::SurfaceMenuItem::external(
                                app_id.clone(),
                                generation,
                                index.to_string(),
                                item.label,
                                (!item.icon_path.trim().is_empty()).then_some(item.icon_path),
                            )
                        })
                        .collect::<Vec<_>>();
                    if !more_actions.is_empty() {
                        groups.push(more_actions);
                    }
                    groups
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        window_controller(PRIMARY_WINDOW, &self.runtime).surface_menu(surface_id, content_groups)
    }

    pub fn perform_shell_surface_menu_intent(
        &self,
        intent: lingxia_shell::SurfaceMenuIntent,
    ) -> HostSurfaceMenuExecution {
        if matches!(
            &intent.action,
            lingxia_shell::SurfaceMenuAction::Information {}
        ) {
            return HostSurfaceMenuExecution {
                accepted: false,
                removed_surface_ids: Vec::new(),
                snapshot: self.surface_switcher_snapshot(),
            };
        }
        if let lingxia_shell::SurfaceMenuAction::Lxapp { action } = &intent.action {
            let action = *action;
            let snapshot = self.surface_switcher_snapshot();
            let target = (snapshot.revision == intent.revision)
                .then(|| self.main_surface_content(&intent.surface_id))
                .flatten()
                .and_then(|content| match content {
                    lingxia_surface::SurfaceContent::Lxapp { app_id, .. } => {
                        crate::try_get(&app_id)
                    }
                    _ => None,
                });
            let accepted = target.is_some_and(|app| {
                let clear_cache = matches!(
                    action,
                    lingxia_shell::LxappSurfaceMenuAction::CleanCacheRestart
                );
                let app_id = app.appid.clone();
                std::thread::Builder::new()
                    .name(format!("lingxia-lxapp-restart-{app_id}"))
                    .spawn(move || {
                        let result = (|| {
                            if clear_cache {
                                app.clear_user_cache()?;
                            }
                            app.restart_in_place()
                        })();
                        if let Err(error) = result {
                            warn!("Failed to run lxapp surface maintenance: {error}")
                                .with_appid(app_id);
                        }
                    })
                    .is_ok()
            });
            return HostSurfaceMenuExecution {
                accepted,
                removed_surface_ids: Vec::new(),
                snapshot,
            };
        }
        if let lingxia_shell::SurfaceMenuAction::External {
            namespace,
            generation,
            action_id,
        } = &intent.action
        {
            let snapshot = self.surface_switcher_snapshot();
            let accepted = snapshot.revision == intent.revision
                && self
                    .main_surface_content(&intent.surface_id)
                    .is_some_and(|content| {
                        matches!(
                            content,
                            lingxia_surface::SurfaceContent::Lxapp { app_id, .. }
                                if app_id == *namespace
                        )
                    })
                && action_id
                    .parse::<usize>()
                    .ok()
                    .zip(crate::try_get(namespace))
                    .is_some_and(|(index, app)| app.activate_more_action(*generation, index));
            return HostSurfaceMenuExecution {
                accepted,
                removed_surface_ids: Vec::new(),
                snapshot,
            };
        }
        window_controller(PRIMARY_WINDOW, &self.runtime).perform_surface_menu_intent(intent)
    }

    #[doc(hidden)]
    pub fn perform_shell_surface_menu_intent_deferred(
        &self,
        intent: lingxia_shell::SurfaceMenuIntent,
    ) -> HostSurfaceMenuExecution {
        window_controller(PRIMARY_WINDOW, &self.runtime)
            .perform_surface_menu_intent_deferred(intent)
    }

    #[doc(hidden)]
    pub fn commit_shell_surface_layout(&self) -> bool {
        window_controller(PRIMARY_WINDOW, &self.runtime).commit()
    }

    pub fn close_main_surface(
        &self,
        surface_id: &str,
        reason: &str,
    ) -> lingxia_surface::CloseOutcome {
        let surface_id = surface_id.trim();
        if surface_id.is_empty() {
            return lingxia_surface::CloseOutcome::NotFound;
        }
        window_controller(PRIMARY_WINDOW, &self.runtime).close(surface_id, reason)
    }

    /// Remove a main from the graph while the platform presents its successor.
    /// The caller must finish by activating the successfully presented main.
    #[doc(hidden)]
    pub fn close_main_surface_deferred(
        &self,
        surface_id: &str,
        reason: &str,
    ) -> lingxia_surface::CloseOutcome {
        let surface_id = surface_id.trim();
        if surface_id.is_empty() {
            return lingxia_surface::CloseOutcome::NotFound;
        }
        window_controller(PRIMARY_WINDOW, &self.runtime).close_deferred(surface_id, reason)
    }

    pub fn close_other_main_surfaces(&self, keeping: &str) -> Vec<String> {
        let keeping = keeping.trim();
        if keeping.is_empty() {
            return Vec::new();
        }
        window_controller(PRIMARY_WINDOW, &self.runtime).close_other_mains(keeping)
    }

    pub fn close_main_surfaces_after(&self, surface_id: &str) -> Vec<String> {
        let surface_id = surface_id.trim();
        if surface_id.is_empty() {
            return Vec::new();
        }
        window_controller(PRIMARY_WINDOW, &self.runtime).close_mains_after(surface_id)
    }

    pub fn update_shell_surface_automatic_title(
        &self,
        surface_id: &str,
        title: Option<&str>,
    ) -> bool {
        let surface_id = surface_id.trim();
        !surface_id.is_empty()
            && window_controller(PRIMARY_WINDOW, &self.runtime)
                .update_surface_automatic_title(surface_id, title)
    }

    pub fn rename_shell_surface(&self, surface_id: &str, title: Option<&str>) -> bool {
        let surface_id = surface_id.trim();
        !surface_id.is_empty()
            && window_controller(PRIMARY_WINDOW, &self.runtime).rename_surface(surface_id, title)
    }

    /// Remove a host-declared aside from the surface graph.
    pub fn unregister_host_aside(&self, surface_id: &str) {
        let surface_id = surface_id.trim();
        if surface_id.is_empty() {
            return;
        }
        window_controller(PRIMARY_WINDOW, &self.runtime).unregister_host_aside(surface_id);
    }

    /// Mirror a platform-shell visibility change into the shared graph.
    pub fn mark_shell_surface_hidden(&self, surface_id: &str) -> bool {
        let surface_id = surface_id.trim();
        !surface_id.is_empty()
            && window_controller(PRIMARY_WINDOW, &self.runtime)
                .mark_surface_hidden_from_shell(surface_id)
    }

    pub fn shell_surface_presentation(&self, surface_id: &str) -> Option<&'static str> {
        window_controller(PRIMARY_WINDOW, &self.runtime).surface_presentation(surface_id)
    }

    pub fn shell_surface_role(&self, surface_id: &str) -> Option<SurfaceRole> {
        let surface_id = surface_id.trim();
        (!surface_id.is_empty())
            .then(|| {
                window_controller(PRIMARY_WINDOW, &self.runtime).managed_surface_role(surface_id)
            })
            .flatten()
    }

    /// Height of the drag strip a `chrome: 'full'` window keeps above `page`.
    /// The runtime owns that strip, so the page can lay out beneath it without
    /// being able to remove it.
    pub(crate) fn full_chrome_drag_strip_inset(&self, page: &PageInstance) -> f64 {
        let instance_id = page.instance_id_string();
        let Ok(state) = self.state.lock() else {
            return 0.0;
        };
        let surfaces = state
            .surfaces
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        surfaces
            .values()
            .any(|record| {
                record.chrome == WindowChrome::Full
                    && record.content_page_instance_id.as_deref() == Some(instance_id.as_str())
            })
            .then(full_chrome_drag_strip_height)
            .unwrap_or(0.0)
    }

    pub fn shell_surface_visible(&self, surface_id: &str) -> Option<bool> {
        let surface_id = surface_id.trim();
        (!surface_id.is_empty())
            .then(|| {
                window_controller(PRIMARY_WINDOW, &self.runtime).managed_surface_visible(surface_id)
            })
            .flatten()
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

    /// Collapse or restore a whole aside slot from the shell — the region's
    /// "put it away" control. Nothing closes: the slot's children stay open
    /// and reappear when the app opens or focuses one of them again.
    pub fn set_shell_slot_collapsed(&self, kind: &str, collapsed: bool) -> bool {
        let Some(kind) = shell_slot_kind(kind) else {
            return false;
        };
        window_controller(PRIMARY_WINDOW, &self.runtime).set_slot_collapsed(kind, collapsed)
    }

    pub fn shell_slot_collapsed(&self, kind: &str) -> bool {
        shell_slot_kind(kind).is_some_and(|kind| {
            window_controller(PRIMARY_WINDOW, &self.runtime).slot_collapsed(kind)
        })
    }

    pub fn forget_surface(&self, id: &str) -> bool {
        self.forget_surface_with_reason(id, "user")
    }

    /// Remove a non-root shell Surface while preserving the initiating close
    /// reason for retained JS handles.
    pub fn forget_surface_with_reason(&self, id: &str, reason: &str) -> bool {
        let id = id.trim();
        if id.is_empty() {
            return false;
        }
        let controller = window_controller(PRIMARY_WINDOW, &self.runtime);
        if controller.is_root_main(id) {
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
        let closed = matches!(
            controller.close(id, reason),
            lingxia_surface::CloseOutcome::Closed { .. }
        );
        removed || closed
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
            content: SurfaceContent::Lxapp {
                app_id: self.appid.clone(),
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
            SurfaceContent::Page => LxContent::Page {
                app_id: self.appid.clone(),
                path: page_path.clone().unwrap_or_else(|| path_or_url.to_string()),
            },
            SurfaceContent::Url => LxContent::Browser {
                initial_url: path_or_url.to_string(),
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

fn lxapp_surface_menu_header(
    app_id: &str,
    app_name: &str,
    version: &str,
    release_type: &str,
) -> String {
    let mut header = if app_name.trim().is_empty() {
        app_id.to_string()
    } else {
        app_name.trim().to_string()
    };
    if !version.trim().is_empty() {
        header.push_str(" · ");
        header.push_str(version.trim());
    }
    match release_type.trim().to_ascii_lowercase().as_str() {
        "developer" => header.push_str(" [DEV]"),
        "preview" => header.push_str(" [PRE]"),
        _ => {}
    }
    header
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

/// One strip height for every platform, published to the page as a chrome
/// inset. Mirrored by `LxAppSurface.fullChromeDragStripHeight` on macOS, which
/// cannot read a Rust constant.
pub(crate) fn full_chrome_drag_strip_height() -> f64 {
    WindowChrome::FULL_DRAG_STRIP_HEIGHT
}

/// Map a core-arbitrated role (+ resolved edge) back to the platform present
/// parameters, so native presentation follows the core's decision. A float keeps
/// its requested position (popup at that edge/center); an aside docks at its
/// edge; a main is a window.
fn present_params_for_role(
    role: lingxia_surface::Role,
    edge: Option<lingxia_surface::Edge>,
    requested_position: SurfacePosition,
) -> (SurfaceKind, SurfacePosition) {
    use lingxia_surface::{Edge as LxEdge, Role as LxRole};
    match role {
        LxRole::Main => (SurfaceKind::Window, SurfacePosition::Center),
        LxRole::Float => (SurfaceKind::Overlay, requested_position),
        LxRole::Aside => {
            let position = match edge {
                Some(LxEdge::Left) => SurfacePosition::Left,
                Some(LxEdge::Right) => SurfacePosition::Right,
                Some(LxEdge::Top) => SurfacePosition::Top,
                Some(LxEdge::Bottom) => SurfacePosition::Bottom,
                None => requested_position,
            };
            (SurfaceKind::Overlay, position)
        }
    }
}

fn surface_edge_name(edge: lingxia_surface::Edge) -> &'static str {
    match edge {
        lingxia_surface::Edge::Left => "left",
        lingxia_surface::Edge::Right => "right",
        lingxia_surface::Edge::Top => "top",
        lingxia_surface::Edge::Bottom => "bottom",
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

fn surface_presentation(
    kind: SurfaceKind,
    role: lingxia_surface::Role,
    overlay: bool,
) -> &'static str {
    match (role, kind, overlay) {
        (lingxia_surface::Role::Main, _, _) => "main",
        (lingxia_surface::Role::Aside, _, true) => "overlay",
        (lingxia_surface::Role::Aside, _, false) => "dock",
        (lingxia_surface::Role::Float, _, _) => "popover",
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

fn surface_role_str(role: lingxia_surface::Role) -> &'static str {
    match role {
        lingxia_surface::Role::Main => "main",
        lingxia_surface::Role::Aside => "aside",
        lingxia_surface::Role::Float => "float",
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

    #[test]
    fn only_the_stable_lxapp_root_lacks_workspace_close_controls() {
        let home = lingxia_surface::Surface::lxapp("home", lingxia_surface::Role::Main, "home");
        let chat = lingxia_surface::Surface::lxapp("chat", lingxia_surface::Role::Main, "chat");
        let mut manager = lingxia_surface::SurfaceManager::new(1200.0);
        manager
            .open_main(home.clone(), lxapp_workspace_presentation(&home.content))
            .unwrap();
        manager
            .open_main(chat.clone(), lxapp_workspace_presentation(&chat.content))
            .unwrap();

        let snapshot = manager.switcher_snapshot();
        assert!(snapshot.items[0].root);
        assert!(!snapshot.items[0].closable);
        assert!(!snapshot.items[1].root);
        assert!(snapshot.items[1].closable);
        assert!(!snapshot.items[1].renameable);
    }

    #[test]
    fn main_visibility_publication_ignores_a_surface_migrated_to_aside() {
        let mut manager = lingxia_surface::SurfaceManager::new(1200.0);
        manager.open(lingxia_surface::Surface::lxapp(
            "home",
            lingxia_surface::Role::Main,
            "home",
        ));
        manager.open(lingxia_surface::Surface::native(
            "terminal",
            lingxia_surface::Role::Main,
            "terminal",
        ));
        manager.set_active_main("terminal");

        let mut terminal = manager.graph().get("terminal").unwrap().clone();
        terminal.role = lingxia_surface::Role::Aside;
        terminal.placement.edge = Some(lingxia_surface::Edge::Bottom);
        manager.open(terminal);
        let migrated = manager.presentation_plan();

        assert_eq!(migrated.active_main_id.as_deref(), Some("home"));
        assert_eq!(
            previous_main_for_visibility(&migrated, Some("terminal")),
            None
        );

        manager.open(lingxia_surface::Surface::native(
            "workspace",
            lingxia_surface::Role::Main,
            "terminal",
        ));
        let ordinary_switch = manager.presentation_plan();
        assert_eq!(
            previous_main_for_visibility(&ordinary_switch, Some("home")),
            Some("home")
        );
    }

    #[test]
    fn managed_operations_serialize_per_surface_identity() {
        let serializers = ManagedSurfaceSerializers::default();
        let first = serializers.lock_for("terminal", Some("project-a"));
        let guard = first.try_lock().expect("first caller acquires the lock");
        let same = serializers.lock_for("terminal", Some("project-a"));
        let other = serializers.lock_for("terminal", Some("project-b"));

        assert!(Arc::ptr_eq(&first, &same));
        assert!(same.try_lock().is_none());
        assert!(!Arc::ptr_eq(&first, &other));
        assert!(other.try_lock().is_some());

        drop(guard);
        assert!(same.try_lock().is_some());
    }

    #[test]
    fn page_surface_uses_the_exported_surface_role() {
        let surface = PageSurface {
            id: "settings".to_string(),
            page_path: None,
            page_instance_id: None,
            kind: SurfaceKind::Window,
            role: crate::SurfaceRole::Main,
            position: SurfacePosition::Center,
            presentation: "window".to_string(),
            chrome: WindowChrome::System,
        };
        let role: crate::SurfaceRole = surface.role;
        assert_eq!(role, crate::SurfaceRole::Main);
    }

    #[test]
    fn managed_provider_preserves_native_instance_identity() {
        let native = lingxia_surface::Surface::native_instance(
            "native:terminal:1",
            lingxia_surface::Role::Main,
            "terminal",
            Some("project-a".to_string()),
        );
        assert_eq!(
            managed_provider_for_surface(&native),
            ManagedSurfaceProvider::Native {
                capability: "terminal".to_string(),
                instance_key: Some("project-a".to_string()),
            }
        );

        let lxapp = lingxia_surface::Surface::lxapp(
            "lingxia-chat",
            lingxia_surface::Role::Aside,
            "lingxia-chat",
        );
        assert_eq!(
            managed_provider_for_surface(&lxapp),
            ManagedSurfaceProvider::Declared
        );
    }

    #[test]
    fn lxapp_surface_menu_header_includes_channel_badge() {
        assert_eq!(
            lxapp_surface_menu_header("demo", "Showcase", "1.2.3", "developer"),
            "Showcase · 1.2.3 [DEV]"
        );
        assert_eq!(lxapp_surface_menu_header("demo", "", "", "release"), "demo");
    }

    #[test]
    fn keyed_native_instance_keeps_identity_separate_from_requested_role() {
        let surface = host_aside_node(
            "terminal",
            lingxia_surface::SurfaceContent::Native {
                capability: "terminal".into(),
                instance_key: None,
            },
            "bottom",
        );
        let declaration = NativeSurfaceDeclaration {
            presentation: lingxia_surface::SurfacePresentation::for_content(&surface.content),
            surface,
        };

        let (default_surface, _) =
            instantiate_native_declaration(declaration.clone(), "terminal", None, None, None, None);
        assert_eq!(default_surface.role, lingxia_surface::Role::Aside);
        assert_eq!(
            default_surface.placement.edge,
            Some(lingxia_surface::Edge::Bottom)
        );

        let (workspace, presentation) = instantiate_native_declaration(
            declaration,
            "terminal",
            Some("project-a"),
            Some(7),
            Some(SurfaceRole::Main),
            None,
        );
        assert_eq!(workspace.id, "native:terminal:7");
        assert_eq!(workspace.role, lingxia_surface::Role::Main);
        assert_eq!(workspace.placement.edge, None);
        assert_eq!(
            workspace.content.native_identity(),
            Some(("terminal", Some("project-a")))
        );
        assert!(presentation.capabilities.close);
        assert!(presentation.capabilities.rename);
        assert_eq!(presentation.automatic_title.as_deref(), Some("Terminal"));
    }

    fn url_record(url_callback: bool, ephemeral_web_data: bool) -> SurfaceRecord {
        SurfaceRecord {
            chrome: WindowChrome::System,
            owner_page_instance_id: Some("owner".to_string()),
            content_page_instance_id: None,
            content: SurfaceContent::Url,
            target: "https://example.com/login".to_string(),
            kind: SurfaceKind::Overlay,
            role: lingxia_surface::Role::Aside,
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
        let surface = host_aside_node(
            "shell:terminal",
            lingxia_surface::SurfaceContent::Native {
                capability: "terminal".into(),
                instance_key: None,
            },
            "bottom",
        );

        assert_eq!(surface.id, "shell:terminal");
        assert_eq!(
            surface.content.slot_kind(),
            lingxia_surface::SlotKind::Native
        );
        assert_eq!(surface.placement.edge, Some(lingxia_surface::Edge::Bottom));
    }

    fn switcher_intent(
        revision: u64,
        surface_id: &str,
        action: lingxia_shell::SurfaceMenuBuiltinAction,
    ) -> lingxia_shell::SurfaceMenuIntent {
        lingxia_shell::SurfaceMenuIntent {
            revision,
            surface_id: surface_id.into(),
            action: lingxia_shell::SurfaceMenuAction::Switcher { action },
            value: None,
        }
    }

    #[test]
    fn surface_menu_transaction_rejects_root_close() {
        let mut manager = lingxia_surface::SurfaceManager::new(1200.0);
        manager.open(lingxia_surface::Surface::native(
            "terminal",
            lingxia_surface::Role::Main,
            "terminal",
        ));
        manager.open(lingxia_surface::Surface::browser(
            "browser",
            lingxia_surface::Role::Main,
            "https://example.com",
        ));
        let before = manager.switcher_snapshot();

        let execution = execute_surface_menu_intent(
            &mut manager,
            switcher_intent(
                before.revision,
                "terminal",
                lingxia_shell::SurfaceMenuBuiltinAction::Close,
            ),
        );

        assert!(!execution.accepted);
        assert!(execution.removed_surface_ids.is_empty());
        assert_eq!(execution.snapshot, before);
    }

    #[test]
    fn surface_menu_transaction_closes_non_root_atomically() {
        let mut manager = lingxia_surface::SurfaceManager::new(1200.0);
        manager.open(lingxia_surface::Surface::native(
            "terminal",
            lingxia_surface::Role::Main,
            "terminal",
        ));
        manager.open(lingxia_surface::Surface::browser(
            "browser",
            lingxia_surface::Role::Main,
            "https://example.com",
        ));
        let revision = manager.switcher_snapshot().revision;

        let execution = execute_surface_menu_intent(
            &mut manager,
            switcher_intent(
                revision,
                "browser",
                lingxia_shell::SurfaceMenuBuiltinAction::Close,
            ),
        );

        assert!(execution.accepted);
        assert_eq!(execution.removed_surface_ids, ["browser"]);
        assert_eq!(
            execution.snapshot.root_surface_id.as_deref(),
            Some("terminal")
        );
        assert_eq!(execution.snapshot.items.len(), 1);
    }

    #[test]
    fn surface_menu_transaction_rejects_stale_revision() {
        let mut manager = lingxia_surface::SurfaceManager::new(1200.0);
        manager.open(lingxia_surface::Surface::native(
            "terminal",
            lingxia_surface::Role::Main,
            "terminal",
        ));
        manager.open(lingxia_surface::Surface::browser(
            "browser",
            lingxia_surface::Role::Main,
            "https://example.com",
        ));
        let stale_revision = manager.switcher_snapshot().revision;
        assert!(manager.update_automatic_title("browser", Some("Example")));

        let execution = execute_surface_menu_intent(
            &mut manager,
            switcher_intent(
                stale_revision,
                "browser",
                lingxia_shell::SurfaceMenuBuiltinAction::Close,
            ),
        );

        assert!(!execution.accepted);
        assert!(manager.graph().get("browser").is_some());
    }
}
