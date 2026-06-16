use super::*;
use lingxia_platform::traits::ui::{
    SurfaceContent, SurfaceKind, SurfacePosition, SurfacePresenter,
    SurfaceRequest as PlatformSurfaceRequest,
};
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;

const SURFACE_DISPOSE_TTL_MS: u64 = 30_000;
static SURFACE_CLOSE_OBSERVER: OnceLock<fn(&str, &str) -> bool> = OnceLock::new();

pub fn register_surface_close_observer(observer: fn(&str, &str) -> bool) {
    let _ = SURFACE_CLOSE_OBSERVER.set(observer);
}

fn notify_surface_close_observer(id: &str, reason: &str) {
    if let Some(observer) = SURFACE_CLOSE_OBSERVER.get() {
        let _ = observer(id, reason);
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

#[derive(Debug, Clone)]
pub(crate) struct SurfaceRecord {
    pub owner_page_instance_id: Option<String>,
    /// The page instance hosted inside this surface (when content is a page).
    /// Used to close the surface when its inner page is disposed (e.g. SDK
    /// reclaim after long hide) so the owner's `Surface` handle reliably
    /// receives an onClose event.
    pub content_page_instance_id: Option<String>,
}

impl LxApp {
    pub fn open_surface(&self, request: PageSurfaceRequest) -> Result<PageSurface, LxAppError> {
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
        if let Ok(state) = self.state.lock() {
            state.surfaces.lock().unwrap().insert(
                id.clone(),
                SurfaceRecord {
                    owner_page_instance_id: owner_pid.clone(),
                    content_page_instance_id,
                },
            );
            // Mirror into the Adaptive Surface Layout core (authoritative model).
            let node = self.build_surface_node(
                &id,
                content,
                request.kind,
                request.position,
                &path,
                &page_path,
                owner_pid.as_deref(),
            );
            let _ = state.surface_manager.lock().unwrap().open(node);
        }

        let present_result = self.runtime.present_surface(PlatformSurfaceRequest {
            id: id.clone(),
            app_id: self.appid.clone(),
            path,
            session_id: self.session_id(),
            page_instance_id: page_instance_id.clone(),
            content,
            kind: request.kind,
            width: finite_or_nan(request.width),
            height: finite_or_nan(request.height),
            width_ratio: finite_or_nan(request.width_ratio),
            height_ratio: finite_or_nan(request.height_ratio),
            position: request.position,
        });
        if let Err(err) = present_result {
            self.forget_surface(&id);
            if !page_instance_id.is_empty() {
                let _ = dispose_page_instance_by_id(&page_instance_id, CloseReason::Programmatic);
            }
            return Err(err.into());
        }

        Ok(PageSurface {
            id,
            page_path,
            page_instance_id: (!page_instance_id.is_empty()).then_some(page_instance_id),
            kind: request.kind,
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

    pub fn show_surface(&self, id: &str) -> Result<(), LxAppError> {
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
        let is_known = self
            .state
            .lock()
            .ok()
            .map(|state| state.surfaces.lock().unwrap().contains_key(id))
            .unwrap_or(false);
        if !is_known {
            return Err(LxAppError::InvalidParameter(format!(
                "unknown surface: {id}"
            )));
        }
        self.runtime
            .hide_surface(&self.appid, id)
            .map_err(Into::into)
    }

    /// `lx.shell.open` / `close`: show or hide a host-declared top-level
    /// surface (e.g. the AI-chat panel or terminal). Delegates to the platform
    /// host shell; platforms without one return an error.
    pub fn set_shell_surface_visible(&self, id: &str, visible: bool) -> Result<(), LxAppError> {
        let id = id.trim();
        if id.is_empty() {
            return Err(LxAppError::InvalidParameter(
                "shell surface id must not be empty".to_string(),
            ));
        }
        self.runtime
            .set_managed_surface_visible(id, visible)
            .map_err(Into::into)
    }

    /// `lx.shell.toggle`: flip a host-declared top-level surface's visibility.
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
        self.state
            .lock()
            .ok()
            .and_then(|state| {
                // Keep the Adaptive Surface Layout core in sync with removals.
                let _ = state.surface_manager.lock().unwrap().close(id);
                state.surfaces.lock().unwrap().remove(id)
            })
            .is_some()
    }

    /// Report the container width so the core resolves the right `sizeClass`
    /// (with hysteresis). Returns `true` when the `sizeClass` flipped.
    ///
    /// Also seeds the app's root `main` surface into the graph if absent — the
    /// app's own primary content must be the `main`, otherwise asides have no
    /// primary to dock to and arbitration promotes them.
    pub fn set_surface_width(&self, width: f64) -> bool {
        let root = self.root_main_node();
        self.state
            .lock()
            .ok()
            .map(|state| {
                let mut manager = state.surface_manager.lock().unwrap();
                if manager.graph().mains().is_empty() {
                    manager.open(root);
                }
                manager.set_width(width)
            })
            .unwrap_or(false)
    }

    /// The app's root primary, represented as a `main` surface (id = appid).
    fn root_main_node(&self) -> lingxia_surface::Surface {
        use lingxia_surface::{Role, Surface as LxSurface, SurfaceContent, SurfaceOwner, SurfaceState};
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

    /// Snapshot the core's `DerivedLayout` for this app's window (new model).
    pub fn surface_derived_layout(&self) -> Option<lingxia_surface::DerivedLayout> {
        self.state
            .lock()
            .ok()
            .map(|state| state.surface_manager.lock().unwrap().derive())
    }

    /// Map a legacy surface request into an Adaptive Surface Layout node
    /// (§10.1 migration: Window→main, Overlay+Center→float, Overlay+edge→aside).
    fn build_surface_node(
        &self,
        id: &str,
        content: SurfaceContent,
        kind: SurfaceKind,
        position: SurfacePosition,
        path_or_url: &str,
        page_path: &Option<String>,
        owner_page_instance_id: Option<&str>,
    ) -> lingxia_surface::Surface {
        use lingxia_surface::{
            Edge as LxEdge, FloatSpec, Placement, Role as LxRole, Surface as LxSurface,
            SurfaceContent as LxContent, SurfaceOwner as LxOwner, SurfaceState as LxState,
        };
        let role = match (kind, position) {
            (SurfaceKind::Window, _) => LxRole::Main,
            (SurfaceKind::Overlay, SurfacePosition::Center) => LxRole::Float,
            (SurfaceKind::Overlay, _) => LxRole::Aside,
        };
        let edge = match position {
            SurfacePosition::Left => Some(LxEdge::Left),
            SurfacePosition::Right => Some(LxEdge::Right),
            SurfacePosition::Top => Some(LxEdge::Top),
            SurfacePosition::Bottom => Some(LxEdge::Bottom),
            SurfacePosition::Center => None,
        };
        let node_content = match content {
            SurfaceContent::Page => LxContent::Entry {
                id: self.appid.clone(),
                path: page_path.clone(),
            },
            SurfaceContent::Url => LxContent::Web {
                url: path_or_url.to_string(),
                chrome: false,
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

fn finite_or_nan(value: Option<f64>) -> f64 {
    match value {
        Some(value) if value.is_finite() => value,
        _ => f64::NAN,
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
