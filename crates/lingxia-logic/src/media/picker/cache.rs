use super::types::MediaKey;
use crate::i18n::js_internal_error;
use lxapp::LxApp;
use rong::RongJSError;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn ensure_temp_media_path<F>(
    lxapp: &LxApp,
    key: &MediaKey,
    ext: &str,
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
    write(&dest_path)?;
    Ok(dest_path)
}
