//! The native half of the page-scoped members `definePage` hands to `setup`:
//! `page.navigationBar`, `page.refresh()`, `page.video()`, `page.orientation` and `page.env`. Each call names
//! its page by instance id, so it reaches that page whether or not it is on top.

use crate::i18n::js_service_unavailable_error;
use lxapp::{LxApp, PageInstance};
use rong::{JSContext, JSFunc, JSObject, JSResult};
use std::sync::Arc;

const NATIVE_SLOT: &str = "__LX_PAGE_NATIVE__";

fn page_of(ctx: &JSContext, instance_id: &str) -> JSResult<(Arc<LxApp>, PageInstance)> {
    let app = LxApp::from_ctx(ctx)?;
    let page = app
        .get_page_by_instance_id_str(instance_id)
        .ok_or_else(|| js_service_unavailable_error("the page has unloaded"))?;
    Ok((app, page))
}

async fn navigation_bar_update(
    ctx: JSContext,
    instance_id: String,
    patch: JSObject,
) -> JSResult<()> {
    let (app, page) = page_of(&ctx, &instance_id)?;
    super::navbar::update(app, page, patch).await
}

fn start_refresh(ctx: JSContext, instance_id: String) -> JSResult<()> {
    let (app, page) = page_of(&ctx, &instance_id)?;
    super::pull_to_refresh::start(&app, &page)
}

fn stop_refresh(ctx: JSContext, instance_id: String) -> JSResult<()> {
    let (app, page) = page_of(&ctx, &instance_id)?;
    super::pull_to_refresh::stop(&app, &page)
}

fn video(
    ctx: JSContext,
    instance_id: String,
    name: String,
) -> JSResult<crate::media::VideoContextHandle> {
    let (_, page) = page_of(&ctx, &instance_id)?;
    crate::media::video_for_page(&ctx, page, name)
}

fn orientation_lock(
    ctx: JSContext,
    instance_id: String,
    orientation: rong::function::Optional<String>,
) -> JSResult<()> {
    let (app, page) = page_of(&ctx, &instance_id)?;
    crate::display::apply_page_orientation_lock(&app, &page, orientation.0)
}

fn env(ctx: JSContext) -> JSResult<JSObject> {
    crate::surface::page_env(&ctx)
}

fn env_watch(ctx: JSContext, handler: JSFunc) -> JSResult<JSFunc> {
    crate::surface::page_env_watch(&ctx, handler)
}

pub(super) fn init(ctx: &JSContext) -> JSResult<()> {
    let native = JSObject::new(ctx);
    native.set(
        "navigationBarUpdate",
        JSFunc::new(ctx, navigation_bar_update)?,
    )?;
    native.set("startRefresh", JSFunc::new(ctx, start_refresh)?)?;
    native.set("stopRefresh", JSFunc::new(ctx, stop_refresh)?)?;
    native.set("video", JSFunc::new(ctx, video)?)?;
    native.set("orientationLock", JSFunc::new(ctx, orientation_lock)?)?;
    native.set("env", JSFunc::new(ctx, env)?)?;
    native.set("envWatch", JSFunc::new(ctx, env_watch)?)?;
    ctx.global().set(NATIVE_SLOT, native)?;
    Ok(())
}
