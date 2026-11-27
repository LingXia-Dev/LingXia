use crate::i18n::js_service_unavailable_error;
use lxapp::LxApp;
use rong::function::Optional;
use rong::{
    FromJSValue, IntoJSValue, JSContext, JSContextService, JSFunc, JSObject, JSResult, JSValue,
    Promise,
};
use rong_storage::{Storage as RongStorage, StorageOptions};

const STORAGE_MAX_KEY_BYTES: u32 = 1024; // match module defaults
const STORAGE_MAX_VALUE_BYTES: u32 = 5 * 1024 * 1024;
const STORAGE_MAX_DATA_BYTES: u32 = 20 * 1024 * 1024;

fn storage_options() -> StorageOptions {
    StorageOptions {
        max_key_size: Some(STORAGE_MAX_KEY_BYTES),
        max_value_size: Some(STORAGE_MAX_VALUE_BYTES),
        max_data_size: Some(STORAGE_MAX_DATA_BYTES),
    }
}

#[derive(Clone)]
struct LxStorageService {
    storage: RongStorage,
    /// The storage module's JS object; `lx.storage` forwards to it.
    native: JSObject,
}

impl JSContextService for LxStorageService {
    fn on_shutdown(&self) {
        // Close the database so the next LxApp restart can reopen the same
        // path, even if JS still holds the old context's objects.
        self.storage.close();
    }
}

/// Opens the database on first use, so a Logic context that never touches
/// `lx.storage` never opens it.
fn native_storage(ctx: &JSContext) -> JSResult<JSObject> {
    if let Some(existing) = ctx.get_service::<LxStorageService>() {
        return Ok(existing.native.clone());
    }
    let lxapp = LxApp::from_ctx(ctx)?;
    if lxapp.storage_file_path.as_os_str().is_empty() {
        return Err(js_service_unavailable_error(
            "Storage path is not configured for this app",
        ));
    }
    let storage = RongStorage::new(lxapp.storage_file_path.clone(), storage_options())?;
    let native = JSObject::from_js_value(ctx, storage.clone().into_js_value(ctx))?;
    ctx.set_service::<LxStorageService>(LxStorageService {
        storage,
        native: native.clone(),
    });
    Ok(native)
}

fn native_method(ctx: &JSContext, name: &str) -> JSResult<(JSObject, JSFunc)> {
    let native = native_storage(ctx)?;
    let method: JSFunc = native.get(name)?;
    Ok((native, method))
}

async fn storage_get(
    ctx: JSContext,
    key: String,
    options: Optional<JSObject>,
) -> JSResult<JSValue> {
    let decode = match options.0 {
        Some(options) => options
            .get::<_, Option<JSFunc>>("decode")
            .map_err(|error| error.at_field("decode").at_argument(2))?,
        None => None,
    };
    let (native, get) = native_method(&ctx, "get")?;
    let pending: Promise = get.call(Some(native), (key,))?;
    let value: JSValue = pending.into_future().await?;
    match decode {
        Some(decode) if !value.is_undefined() => decode.call(None, (value,)),
        _ => Ok(value),
    }
}

async fn storage_set(ctx: JSContext, key: String, value: JSValue) -> JSResult<()> {
    let (native, set) = native_method(&ctx, "set")?;
    let pending: Promise = set.call(Some(native), (key, value))?;
    let _: JSValue = pending.into_future().await?;
    Ok(())
}

async fn storage_has(ctx: JSContext, key: String) -> JSResult<bool> {
    let (native, has) = native_method(&ctx, "has")?;
    let pending: Promise = has.call(Some(native), (key,))?;
    pending.into_future().await
}

async fn storage_delete(ctx: JSContext, key: String) -> JSResult<()> {
    let (native, delete) = native_method(&ctx, "delete")?;
    let pending: Promise = delete.call(Some(native), (key,))?;
    let _: JSValue = pending.into_future().await?;
    Ok(())
}

async fn storage_clear(ctx: JSContext) -> JSResult<()> {
    let (native, clear) = native_method(&ctx, "clear")?;
    let pending: Promise = clear.call(Some(native), ())?;
    let _: JSValue = pending.into_future().await?;
    Ok(())
}

/// The module resolves `list` to a JS iterator; `Storage.list` is an array.
async fn storage_list(ctx: JSContext, prefix: Optional<String>) -> JSResult<Vec<String>> {
    let (native, list) = native_method(&ctx, "list")?;
    let pending: Promise = match prefix.0 {
        Some(prefix) => list.call(Some(native), (prefix,))?,
        None => list.call(Some(native), ())?,
    };
    let iterator: JSObject = pending.into_future().await?;
    let next: JSFunc = iterator.get("next")?;
    let mut keys = Vec::new();
    loop {
        let step: JSObject = next.call(Some(iterator.clone()), ())?;
        if step.get::<_, bool>("done")? {
            return Ok(keys);
        }
        keys.push(step.get::<_, String>("value")?);
    }
}

async fn storage_info(ctx: JSContext) -> JSResult<JSObject> {
    let (native, info) = native_method(&ctx, "info")?;
    let pending: Promise = info.call(Some(native), ())?;
    let raw: JSObject = pending.into_future().await?;
    let out = JSObject::new(&ctx);
    out.set("usedBytes", raw.get::<_, f64>("currentSize")?)?;
    out.set("limitBytes", raw.get::<_, f64>("limitSize")?)?;
    out.set("keys", raw.get::<_, f64>("keyCount")?)?;
    Ok(out)
}

fn storage_namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let object = JSObject::new(ctx);
    object.set("get", JSFunc::new(ctx, storage_get)?.name("get")?)?;
    object.set("set", JSFunc::new(ctx, storage_set)?.name("set")?)?;
    object.set("has", JSFunc::new(ctx, storage_has)?.name("has")?)?;
    object.set("delete", JSFunc::new(ctx, storage_delete)?.name("delete")?)?;
    object.set("clear", JSFunc::new(ctx, storage_clear)?.name("clear")?)?;
    object.set("list", JSFunc::new(ctx, storage_list)?.name("list")?)?;
    object.set("info", JSFunc::new(ctx, storage_info)?.name("info")?)?;
    Ok(object)
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        const storage: "Storage<LxStorageValues>" = storage_namespace(ctx)?;
    }
}
