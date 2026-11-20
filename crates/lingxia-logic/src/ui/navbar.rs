use super::page_chrome_patch::parse_patch;
use crate::i18n::{js_internal_error, js_invalid_parameter_error, js_service_unavailable_error};
use lxapp::navbar::NavigationBarPatch;
use lxapp::{LxApp, PageInstance};
use rong::{JSObject, JSResult};
use std::sync::Arc;

/// Patch `page`'s navigation bar; unset fields stay as they are.
pub(super) async fn update(app: Arc<LxApp>, page: PageInstance, patch: JSObject) -> JSResult<()> {
    let patch = parse_patch::<NavigationBarPatch>(&patch, "navigationBar")?;
    app.commit_navigation_bar(page, patch)
        .await
        .map_err(|error| match error {
            lxapp::LxAppError::InvalidParameter(_) => js_invalid_parameter_error(error.to_string()),
            lxapp::LxAppError::ResourceNotFound(_) => {
                js_service_unavailable_error(error.to_string())
            }
            _ => js_internal_error(error.to_string()),
        })
}
