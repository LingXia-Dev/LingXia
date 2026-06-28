use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::ui::{SurfaceKind, SurfacePresenter, SurfaceRequest};
use lingxia_surface::LayoutPresentationPlan;

impl SurfacePresenter for Platform {
    fn present_layout(
        &self,
        window_id: &str,
        plan: &LayoutPresentationPlan,
    ) -> Result<(), PlatformError> {
        let plan_json = serde_json::to_string(plan).map_err(|e| {
            PlatformError::Platform(format!("failed to serialize layout plan: {e}"))
        })?;
        lingxia_webview::platform::harmony::tsfn::call_arkts(
            "presentLayout",
            &[window_id, &plan_json],
        )
        .map_err(|e| PlatformError::Platform(format!("Failed to present layout: {e}")))
    }

    fn present_surface(&self, request: SurfaceRequest) -> Result<(), PlatformError> {
        if request.kind == SurfaceKind::Window {
            return Err(PlatformError::NotSupported(
                "lx.surface window is not supported on this platform".to_string(),
            ));
        }

        let args = vec![
            request.id,
            request.app_id,
            request.path,
            request.session_id.to_string(),
            request.page_instance_id,
            (request.content as i32).to_string(),
            (request.kind as i32).to_string(),
            double_arg(request.width),
            double_arg(request.height),
            double_arg(request.width_ratio),
            double_arg(request.height_ratio),
            (request.position as i32).to_string(),
            (request.role as i32).to_string(),
        ];
        let arg_refs = args.iter().map(String::as_str).collect::<Vec<_>>();

        lingxia_webview::platform::harmony::tsfn::call_arkts("presentSurface", &arg_refs)
            .map_err(|e| PlatformError::Platform(format!("Failed to present surface: {e}")))
    }

    fn close_surface(&self, app_id: &str, id: &str, reason: &str) -> Result<(), PlatformError> {
        lingxia_webview::platform::harmony::tsfn::call_arkts("closeSurface", &[id, app_id, reason])
            .map_err(|e| PlatformError::Platform(format!("Failed to close surface: {e}")))
    }

    fn show_surface(&self, app_id: &str, id: &str) -> Result<(), PlatformError> {
        lingxia_webview::platform::harmony::tsfn::call_arkts("showSurface", &[id, app_id])
            .map_err(|e| PlatformError::Platform(format!("Failed to show surface: {e}")))
    }

    fn hide_surface(&self, app_id: &str, id: &str) -> Result<(), PlatformError> {
        lingxia_webview::platform::harmony::tsfn::call_arkts("hideSurface", &[id, app_id])
            .map_err(|e| PlatformError::Platform(format!("Failed to hide surface: {e}")))
    }
}

fn double_arg(value: f64) -> String {
    if value.is_nan() {
        "NaN".to_string()
    } else {
        value.to_string()
    }
}
