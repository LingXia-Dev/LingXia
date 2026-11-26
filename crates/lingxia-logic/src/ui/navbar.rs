use super::page_chrome_patch::parse_patch;
use crate::i18n::{js_internal_error, js_invalid_parameter_error, js_state_error};
use lxapp::navbar::NavigationBarPatch;
use lxapp::{LxApp, PageInstance};
use rong::{JSObject, JSResult};
use std::sync::Arc;

/// Patch `page`'s navigation bar; unset fields stay as they are.
pub(super) async fn update(app: Arc<LxApp>, page: PageInstance, patch: JSObject) -> JSResult<()> {
    let patch = parse_patch::<NavigationBarPatch>(&patch, "navigationBar")?;
    patch
        .check_writer(page.renderer())
        .map_err(js_invalid_parameter_error)?;
    app.commit_navigation_bar(page, patch)
        .await
        .map_err(|error| match error.code() {
            lingxia_error::code::INVALID_ARG => js_invalid_parameter_error(error.message()),
            lingxia_error::code::NOT_FOUND => js_state_error("pageUnloaded", error.message()),
            _ => js_internal_error(error.message()),
        })
}
