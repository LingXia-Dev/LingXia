use super::types::MediaKey;
use crate::i18n::{js_error_from_business_code_with_detail, js_internal_error};
use lingxia_service::storage;
use lxapp::LxApp;
use rong::RongJSError;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn ensure_temp_media_path<F>(
    lxapp: &LxApp,
    key: &MediaKey,
    ext: &str,
    incoming_bytes: Option<u64>,
    write: F,
) -> Result<PathBuf, RongJSError>
where
    F: FnOnce(&Path) -> Result<(), RongJSError>,
{
    let dest_path = lxapp
        .temp_output_path(&format!("media-{}", key.kind), Some(ext))
        .map_err(|e| js_internal_error(format!("temp unavailable: {}", e)))?;
    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            js_internal_error(format!(
                "chooseMedia failed to create temp directory {}: {}",
                parent.display(),
                e
            ))
        })?;
    }
    if let Some(incoming_bytes) = incoming_bytes {
        storage::ensure_temp_quota(&lxapp.temp_dir, &dest_path, incoming_bytes)
            .map_err(temp_quota_error_to_js)?;
    }
    if let Err(error) = write(&dest_path) {
        let _ = fs::remove_file(&dest_path);
        return Err(error);
    }
    let written_size = storage::path_size(&dest_path);
    if let Err(error) = storage::ensure_temp_quota(&lxapp.temp_dir, &dest_path, written_size) {
        let _ = fs::remove_file(&dest_path);
        return Err(temp_quota_error_to_js(error));
    }
    Ok(dest_path)
}

fn temp_quota_error_to_js(err: storage::StorageQuotaError) -> RongJSError {
    js_error_from_business_code_with_detail(1002, err.detail())
}
