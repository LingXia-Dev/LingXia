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
                let created = self.create_page_instance(
                    owner,
                    target,
                    request.query,
                    presentation_kind,
                    Some(Duration::from_millis(SURFACE_DISPOSE_TTL_MS)),
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
        if let Ok(state) = self.state.lock() {
            state.surfaces.lock().unwrap().insert(
                id.clone(),
                SurfaceRecord {
                    owner_page_instance_id: owner_page_instance_id.map(|id| id.to_string()),
                    content_page_instance_id,
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

    pub fn forget_surface(&self, id: &str) -> bool {
        let id = id.trim();
        if id.is_empty() {
            return false;
        }
        self.state
            .lock()
            .ok()
            .map(|state| state.surfaces.lock().unwrap().remove(id))
            .flatten()
            .is_some()
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
