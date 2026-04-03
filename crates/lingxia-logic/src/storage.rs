use crate::i18n::js_service_unavailable_error;
use lxapp::{LxApp, lx};
use rong::{JSContext, JSContextService, JSFunc, JSResult};
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
}

impl JSContextService for LxStorageService {
    fn on_shutdown(&self) {
        // Explicitly close the underlying database so that the process
        // can safely reopen the same path on the next LxApp restart,
        // even if JS still holds Storage objects from the old context.
        self.storage.close();
    }
}

fn get_storage(ctx: JSContext) -> JSResult<RongStorage> {
    // If a Storage instance has already been created for this JSContext,
    // return a clone so getStorage() can be called multiple times safely.
    if let Some(existing) = ctx.get_service::<LxStorageService>() {
        return Ok(existing.storage.clone());
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
        storage: storage.clone(),
    });

    Ok(storage)
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let get_storage_fn = JSFunc::new(ctx, get_storage)?.name("getStorage")?;
    lx::register_js_api(ctx, "getStorage", get_storage_fn)?;
    Ok(())
}
