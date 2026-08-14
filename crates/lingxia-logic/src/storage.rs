use crate::i18n::js_service_unavailable_error;
use lxapp::LxApp;
use rong::function::Optional;
use rong::{
    FromJSValue, IntoJSValue, JSContext, JSContextService, JSFunc, JSObject, JSResult, Promise,
};
use rong_storage::{Storage as RongStorage, StorageOptions};
use std::cell::RefCell;
use std::rc::Rc;

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
    /// The JS object handed to `lx.getStorage()`. Cached so repeated calls
    /// return the same object — `getStorage() === getStorage()` — and so the
    /// shimmed `list` is installed exactly once per context.
    exposed: Rc<RefCell<Option<JSObject>>>,
}

impl JSContextService for LxStorageService {
    fn on_shutdown(&self) {
        // Explicitly close the underlying database so that the process
        // can safely reopen the same path on the next LxApp restart,
        // even if JS still holds Storage objects from the old context.
        self.storage.close();
    }
}

/// Drains a JS iterator into the resolved key list.
fn collect_iterator_keys(iterator: &JSObject) -> JSResult<Vec<String>> {
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

/// The storage module resolves `list` to a JS iterator; `Storage.list` is an
/// array, so shadow the prototype method on the instance.
///
/// `backing` is a second instance of the same store, deliberately not the
/// object the shim is installed on: capturing that object would make it
/// reachable only through its own property, and a Rust closure is opaque to
/// the JS cycle collector, so the pair could never be reclaimed.
fn install_list_array_shim(ctx: &JSContext, storage: &JSObject, backing: JSObject) -> JSResult<()> {
    let inner: JSFunc = backing.get("list")?;
    let shim = JSFunc::new(ctx, move |prefix: Optional<String>| {
        let inner = inner.clone();
        let target = backing.clone();
        async move {
            let pending: Promise = match prefix.0 {
                Some(prefix) => inner.call(Some(target), (prefix,))?,
                None => inner.call(Some(target), ())?,
            };
            let iterator: JSObject = pending.into_future().await?;
            collect_iterator_keys(&iterator)
        }
    })?;
    storage.set("list", shim)?;
    Ok(())
}

/// Open this lxapp's asynchronous persistent key-value store. `get` asserts the
/// value shape at the call site and resolves `undefined` for a missing key. Use
/// `lx.fs` instead for path-based data.
fn get_storage(ctx: JSContext) -> JSResult<JSObject> {
    // If a Storage instance has already been created for this JSContext,
    // return a clone so getStorage() can be called multiple times safely.
    if let Some(existing) = ctx.get_service::<LxStorageService>() {
        return expose_storage(&ctx, existing);
    }

    let lxapp = LxApp::from_ctx(&ctx)?;

    if lxapp.storage_file_path.as_os_str().is_empty() {
        return Err(js_service_unavailable_error(
            "Storage path is not configured for this app",
        ));
    }

    let options = storage_options();
    let storage = RongStorage::new(lxapp.storage_file_path.clone(), options)?;

    // Cache Storage instance on JSContext so that:
    // - Subsequent getStorage() calls reuse the same database handle.
    // - When JSContext is dropped, JSContextService::on_shutdown is invoked
    //   and LxStorageService is dropped, closing the database.
    ctx.set_service::<LxStorageService>(LxStorageService {
        storage,
        exposed: Rc::new(RefCell::new(None)),
    });
    let service = ctx
        .get_service::<LxStorageService>()
        .expect("storage service was inserted above");

    expose_storage(&ctx, service)
}

fn expose_storage(ctx: &JSContext, service: &LxStorageService) -> JSResult<JSObject> {
    if let Some(existing) = service.exposed.borrow().as_ref() {
        return Ok(existing.clone());
    }
    let object = JSObject::from_js_value(ctx, service.storage.clone().into_js_value(ctx))?;
    let backing = JSObject::from_js_value(ctx, service.storage.clone().into_js_value(ctx))?;
    install_list_array_shim(ctx, &object, backing)?;
    *service.exposed.borrow_mut() = Some(object.clone());
    Ok(object)
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        fn getStorage(ts_return = "Storage") = get_storage;
    }
}
