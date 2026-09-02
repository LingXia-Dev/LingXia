use lxapp::LxApp;
use rong::{JSContext, JSFunc, JSObject, JSResult};

use crate::i18n::js_error_from_lxapp_error;

/// `lx.app.cache` — the product-wide cache a settings screen reports and
/// clears.
///
/// Restricted to the home lxapp, and app-scoped rather than lxapp-scoped: the
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

/// Bytes currently held by LingXia-managed caches: every lxapp's usercache,
/// every idle session's temp, and shared runtime artwork under the cache dir.
///
/// The WebView's own HTTP cache is excluded, because the platform stores report
/// a site count rather than a byte total and any figure here would be invented.
/// [`cache_clear`] still drops it — a settings screen should say "at least".
async fn cache_size(ctx: JSContext) -> JSResult<f64> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    super::ensure_home_lxapp(&lxapp, "lx.app.cache.size")?;
    let bytes = tokio::task::spawn_blocking(lxapp::product_cache_usage_bytes)
        .await
        .unwrap_or(0);
    Ok(bytes as f64)
}

/// Drops those caches plus the WebView's regenerable cache, and resolves with
/// the bytes freed from LingXia-managed storage.
///
/// Never touches userdata, the `lx.getStorage` key-value store, the user's
/// downloads, or installed lxapp packages: none of those are regenerable, so
/// removing them behind a "clear cache" control is data loss. Cookies and
/// logins survive too — this clears caches, it does not sign anyone out. A
/// partial LingXia-managed clear rejects after attempting every category.
async fn cache_clear(ctx: JSContext) -> JSResult<f64> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    super::ensure_home_lxapp(&lxapp, "lx.app.cache.clear")?;
    lxapp::clear_product_cache()
        .await
        .map(|bytes| bytes as f64)
        .map_err(|err| js_error_from_lxapp_error(&err))
}
