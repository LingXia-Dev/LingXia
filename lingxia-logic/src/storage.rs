use lingxia_lxapp::{LxApp, lx};
use rong::{JSContext, JSFunc, JSResult, RongJSError};
use rong_modules::storage::{Storage as RongStorage, StorageOptions};
use std::sync::{Arc, OnceLock};

const STORAGE_MAX_KEY_BYTES: u32 = 1024; // match module defaults
const STORAGE_MAX_VALUE_BYTES: u32 = 5 * 1024 * 1024;
const STORAGE_MAX_DATA_BYTES: u32 = 20 * 1024 * 1024;

fn storage_options() -> &'static StorageOptions {
    static OPTIONS: OnceLock<StorageOptions> = OnceLock::new();
    OPTIONS.get_or_init(|| StorageOptions {
        max_key_size: Some(STORAGE_MAX_KEY_BYTES),
        max_value_size: Some(STORAGE_MAX_VALUE_BYTES),
        max_data_size: Some(STORAGE_MAX_DATA_BYTES),
    })
}

fn get_storage(ctx: JSContext) -> JSResult<RongStorage> {
    let lxapp = ctx
        .get_user_data::<Arc<LxApp>>()
        .ok_or_else(|| RongJSError::Error("Missing LxApp context".into()))?;

    if lxapp.storage_file_path.as_os_str().is_empty() {
        return Err(RongJSError::Error(
            "Storage path is not configured for this app".into(),
        ));
    }

    RongStorage::open_with_options(lxapp.storage_file_path.clone(), storage_options().clone())
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let get_storage_fn = JSFunc::new(ctx, get_storage)?.name("getStorage")?;
    lx::register_js_api(ctx, "getStorage", get_storage_fn)?;
    Ok(())
}
