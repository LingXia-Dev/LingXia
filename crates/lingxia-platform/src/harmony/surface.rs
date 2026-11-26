use super::app::Platform;
use crate::traits::ui::{SurfaceKind, SurfacePresenter, SurfaceRequest};
use lingxia_error::Error;
use lingxia_surface::LayoutPresentationPlan;
use lingxia_webview::WebTag;

impl SurfacePresenter for Platform {
    fn present_layout(&self, window_id: &str, plan: &LayoutPresentationPlan) -> Result<(), Error> {
        let plan_json = serde_json::to_string(plan)
            .map_err(|e| Error::internal(format!("failed to serialize layout plan: {e}")))?;
        lingxia_webview::platform::harmony::tsfn::call_arkts(
            "presentLayout",
            &[window_id, &plan_json],
        )
        .map_err(|e| Error::internal(format!("Failed to present layout: {e}")))
    }

    fn present_surface(&self, request: SurfaceRequest) -> Result<(), Error> {
        if request.kind == SurfaceKind::Window {
            return Err(Error::not_supported(
                "lx.surface window is not supported on this platform".to_string(),
            ));
        }

        let webtag = WebTag::from(request.content.webtag());
        let args = vec![
            request.id,
            request.app_id,
            request.owner_session_id.to_string(),
            (request.content.kind() as i32).to_string(),
            webtag.key().to_string(),
            webtag.page_instance_id().unwrap_or("").to_string(),
            request.content.url().to_string(),
            (request.kind as i32).to_string(),
            double_arg(request.width),
            double_arg(request.height),
            double_arg(request.width_ratio),
            double_arg(request.height_ratio),
            (request.position as i32).to_string(),
            (request.role as i32).to_string(),
            request.interaction.close_button.to_string(),
            (request.interaction.dismiss == lingxia_surface::FloatDismiss::TapOutside).to_string(),
            request.interaction.modal.to_string(),
            request.ephemeral_web_data.to_string(),
            request.url_callback.to_string(),
        ];
        let arg_refs = args.iter().map(String::as_str).collect::<Vec<_>>();

        lingxia_webview::platform::harmony::tsfn::call_arkts("presentSurface", &arg_refs)
            .map_err(|e| Error::internal(format!("Failed to present surface: {e}")))
    }

    fn close_surface(&self, app_id: &str, id: &str, reason: &str) -> Result<(), Error> {
        lingxia_webview::platform::harmony::tsfn::call_arkts("closeSurface", &[id, app_id, reason])
            .map_err(|e| Error::internal(format!("Failed to close surface: {e}")))
    }

    fn show_surface(&self, app_id: &str, id: &str) -> Result<(), Error> {
        lingxia_webview::platform::harmony::tsfn::call_arkts("showSurface", &[id, app_id])
            .map_err(|e| Error::internal(format!("Failed to show surface: {e}")))
    }

    fn hide_surface(&self, app_id: &str, id: &str) -> Result<(), Error> {
        lingxia_webview::platform::harmony::tsfn::call_arkts("hideSurface", &[id, app_id])
            .map_err(|e| Error::internal(format!("Failed to hide surface: {e}")))
    }
}

fn double_arg(value: f64) -> String {
    if value.is_nan() {
        "NaN".to_string()
    } else {
        value.to_string()
    }
}
