use rong::{JSContext, JSFunc, JSObject, JSResult};

use crate::authorization::{self, LogicRoute};
use crate::i18n::js_error_from_lxapp_error;

/// `lx.app.cache` — the product-wide cache a settings screen reports and
/// clears.
///
/// Restricted to the Control app, and app-scoped rather than lxapp-scoped: the
/// figure a user is shown covers the whole product, so it spans every lxapp the
/// host has run, not just the one asking. An ordinary lxapp clearing every
/// other lxapp's cache is not a capability it should have.
pub(super) fn init(ctx: &JSContext, app: &JSObject) -> JSResult<()> {
    let cache = JSObject::new(ctx);
    cache.set("size", JSFunc::new(ctx, cache_size)?.name("size")?)?;
    cache.set("clear", JSFunc::new(ctx, cache_clear)?.name("clear")?)?;
    app.set("cache", cache)?;
    Ok(())
}

/// Estimated reclaimable managed bytes, excluding live session storage and WebView cache.
async fn cache_size(ctx: JSContext) -> JSResult<f64> {
    authorization::require(&ctx, LogicRoute::AppCacheSize)?;
    let bytes = tokio::task::spawn_blocking(lxapp::product_cache_usage_bytes)
        .await
        .map_err(|err| {
            js_error_from_lxapp_error(&lxapp::LxAppError::Runtime(format!(
                "cache size task failed: {err}"
            )))
        })?;
    Ok(bytes as f64)
}

/// Report completed, skipped and failed work without concealing partial cleanup.
async fn cache_clear(ctx: JSContext) -> JSResult<JSObject> {
    authorization::require(&ctx, LogicRoute::AppCacheClear)?;
    let report = lxapp::clear_product_cache()
        .await
        .map_err(|err| js_error_from_lxapp_error(&err))?;
    let result = JSObject::new(&ctx);
    result.set("freedBytes", report.freed_bytes as f64)?;
    result.set("skippedActivePaths", report.skipped_active_paths as f64)?;
    result.set("webview", report.webview)?;
    result.set("failures", report.failures)?;
    Ok(result)
}
