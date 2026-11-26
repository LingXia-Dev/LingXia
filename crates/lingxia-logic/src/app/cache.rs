use rong::{JSContext, JSFunc, JSObject, JSResult};

use crate::authorization::{self, LogicRoute};
use crate::i18n::js_error;

/// `lx.host.cache` — the product-wide cache a settings screen reports and
/// clears.
///
/// Control app only, checked per call: the figure covers every lxapp the host
/// has run, so another lxapp is refused with `E_PERMISSION_DENIED`.
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
            js_error(lingxia_error::Error::internal(format!(
                "cache size task failed: {err}"
            )))
        })?;
    Ok(bytes as f64)
}

/// Report completed, skipped and failed work without concealing partial cleanup.
async fn cache_clear(ctx: JSContext) -> JSResult<JSObject> {
    authorization::require(&ctx, LogicRoute::AppCacheClear)?;
    let report = lxapp::clear_product_cache().await.map_err(js_error)?;
    let result = JSObject::new(&ctx);
    result.set("freedBytes", report.freed_bytes as f64)?;
    result.set("skippedActivePaths", report.skipped_active_paths as f64)?;
    result.set("webview", report.webview)?;
    result.set("failures", report.failures)?;
    Ok(result)
}
