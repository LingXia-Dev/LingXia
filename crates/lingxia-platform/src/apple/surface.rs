use super::app::Platform;
use super::ffi::{
    close_surface, hide_surface, present_layout, present_surface, set_managed_surface_visible,
    show_surface, toggle_managed_surface,
};
use crate::error::PlatformError;
#[cfg(target_os = "ios")]
use crate::traits::ui::SurfaceKind;
use crate::traits::ui::{SurfacePosition, SurfacePresenter, SurfaceRequest};
use lingxia_surface::LayoutPresentationPlan;

impl SurfacePresenter for Platform {
    fn present_layout(
        &self,
        window_id: &str,
        plan: &LayoutPresentationPlan,
    ) -> Result<(), PlatformError> {
        // Serialize exactly as the JS API (`surfaceDerivedLayout`) does so the
        // skin reconciler and `lx.surface.derivedLayout()` see identical JSON.
        let plan_json = serde_json::to_string(plan).map_err(|e| {
            PlatformError::Platform(format!("failed to serialize layout plan: {e}"))
        })?;
        if present_layout(window_id, &plan_json) {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "Failed to present layout: window_id={window_id}"
            )))
        }
    }

    fn present_surface(&self, request: SurfaceRequest) -> Result<(), PlatformError> {
        #[cfg(target_os = "ios")]
        if request.kind == SurfaceKind::Window {
            return Err(PlatformError::NotSupported(
                "lx.surface window is not supported on this platform".to_string(),
            ));
        }

        if present_surface(
            &request.id,
            &request.app_id,
            &request.path,
            request.session_id,
            &request.page_instance_id,
            request.content as i32,
            request.kind as i32,
            request.width,
            request.height,
            request.width_ratio,
            request.height_ratio,
            match request.position {
                SurfacePosition::Center => 0,
                SurfacePosition::Bottom => 1,
                SurfacePosition::Left => 2,
                SurfacePosition::Right => 3,
                SurfacePosition::Top => 4,
            },
            request.role as i32,
            request.ephemeral_web_data,
            request.url_callback,
        ) {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "Failed to present surface: id={}, appid={}, path={}, kind={:?}",
                request.id, request.app_id, request.path, request.kind
            )))
        }
    }

    fn close_surface(&self, app_id: &str, id: &str, reason: &str) -> Result<(), PlatformError> {
        if close_surface(id, app_id, reason) {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "Failed to close surface: id={}, appid={}",
                id, app_id
            )))
        }
    }

    fn show_surface(&self, app_id: &str, id: &str) -> Result<(), PlatformError> {
        if show_surface(id, app_id) {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "Failed to show surface: id={}, appid={}",
                id, app_id
            )))
        }
    }

    fn hide_surface(&self, app_id: &str, id: &str) -> Result<(), PlatformError> {
        if hide_surface(id, app_id) {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "Failed to hide surface: id={}, appid={}",
                id, app_id
            )))
        }
    }

    fn set_managed_surface_visible(
        &self,
        id: &str,
        visible: bool,
        edge: Option<&str>,
    ) -> Result<(), PlatformError> {
        if set_managed_surface_visible(id, visible, edge.unwrap_or("")) {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "cannot manage surface (no host shell or unknown surface): id={id} (visible={visible})"
            )))
        }
    }

    fn toggle_managed_surface(&self, id: &str) -> Result<(), PlatformError> {
        if toggle_managed_surface(id) {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "cannot manage surface (no host shell or unknown surface): id={id}"
            )))
        }
    }
}
