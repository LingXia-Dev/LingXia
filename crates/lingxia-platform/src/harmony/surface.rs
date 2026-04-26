use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::ui::{SurfaceKind, SurfacePresenter, SurfaceRequest};

impl SurfacePresenter for Platform {
    fn present_surface(&self, request: SurfaceRequest) -> Result<(), PlatformError> {
        if request.kind != SurfaceKind::Popup {
            return Err(PlatformError::NotSupported(
                "window surface is not supported on Harmony".to_string(),
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
        ];
        let arg_refs = args.iter().map(String::as_str).collect::<Vec<_>>();

        lingxia_webview::platform::harmony::tsfn::call_arkts("presentSurface", &arg_refs)
            .map_err(|e| PlatformError::Platform(format!("Failed to present surface: {e}")))
    }

    fn close_surface(&self, app_id: &str, id: &str, reason: &str) -> Result<(), PlatformError> {
        lingxia_webview::platform::harmony::tsfn::call_arkts("closeSurface", &[id, app_id, reason])
            .map_err(|e| PlatformError::Platform(format!("Failed to close surface: {e}")))
    }
}

fn double_arg(value: f64) -> String {
    if value.is_nan() {
        "NaN".to_string()
    } else {
        value.to_string()
    }
}
