use crate::traits::app_runtime::{DesktopBannerShow, DesktopBannerSource};
use serde_json::json;

pub(crate) fn present(request: &DesktopBannerShow, source: Option<&DesktopBannerSource>) -> bool {
    let actions = request
        .actions
        .iter()
        .map(|action| {
            json!({
                "id": action.id,
                "label": action.label,
                "style": action.style.as_str(),
            })
        })
        .collect::<Vec<_>>();
    let actions_json = serde_json::to_string(&actions).unwrap_or_else(|_| "[]".into());
    let background = request.background.as_ffi();
    let source_name = source.map(|source| source.name.as_str()).unwrap_or("");
    let source_icon = source
        .and_then(|source| source.icon_path.as_deref())
        .unwrap_or("");
    super::ffi::desktop_banner_show(
        &request.id,
        &request.title,
        &request.body,
        &actions_json,
        &background,
        request.actions.is_empty(),
        source_name,
        source_icon,
    )
}

pub(crate) fn hide() {
    let _ = super::ffi::desktop_banner_hide();
}
