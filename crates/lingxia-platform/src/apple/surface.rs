use super::app::Platform;
use super::ffi::{
    close_surface, destroy_managed_surface, hide_surface, open_managed_native_surface,
    present_layout, present_surface, set_managed_surface_visible, show_surface,
};
use crate::error::PlatformError;
#[cfg(target_os = "ios")]
use crate::traits::ui::SurfaceKind;
use crate::traits::ui::{
    ManagedSurfaceFuture, ManagedSurfaceProvider, ManagedSurfaceProviderDestroyRequest,
    ManagedSurfaceProviderRequest, SurfacePosition, SurfacePresenter, SurfaceRequest, SurfaceRole,
};
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
            request.interaction.close_button,
            request.interaction.dismiss == lingxia_surface::FloatDismiss::TapOutside,
            request.interaction.modal,
            request.ephemeral_web_data,
            request.url_callback,
            request.chrome as i32,
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

    fn ensure_managed_surface_provider(
        &self,
        request: ManagedSurfaceProviderRequest,
    ) -> ManagedSurfaceFuture {
        Box::pin(async move {
            let accepted = match request.provider {
                ManagedSurfaceProvider::Declared => set_managed_surface_visible(
                    &request.surface_id,
                    true,
                    request.role.map_or("", SurfaceRole::as_str),
                    request.edge.as_deref().unwrap_or(""),
                ),
                ManagedSurfaceProvider::Native {
                    capability,
                    instance_key,
                } => open_managed_native_surface(
                    &request.surface_id,
                    &capability,
                    instance_key.as_deref().unwrap_or(""),
                    request.role.map_or("", SurfaceRole::as_str),
                    request.edge.as_deref().unwrap_or(""),
                ),
            };
            accepted.then_some(()).ok_or_else(|| {
                PlatformError::AssetNotFound(format!(
                    "cannot ensure managed surface provider: id={}",
                    request.surface_id
                ))
            })
        })
    }

    fn destroy_managed_surface_provider(
        &self,
        request: ManagedSurfaceProviderDestroyRequest,
    ) -> ManagedSurfaceFuture {
        Box::pin(async move {
            if destroy_managed_surface(
                &request.surface_id,
                request.role.map_or("", SurfaceRole::as_str),
            ) {
                Ok(())
            } else {
                Err(PlatformError::AssetNotFound(format!(
                    "cannot destroy managed surface provider: id={}",
                    request.surface_id
                )))
            }
        })
    }
}
