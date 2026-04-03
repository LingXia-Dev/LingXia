use super::types::MediaKey;
use crate::i18n::js_internal_error;
use lxapp::LxApp;
use rong::RongJSError;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn ensure_cached_media_path<F>(
    lxapp: &LxApp,
    key: &MediaKey,
    ext: &str,
    write: F,
) -> Result<PathBuf, RongJSError>
where
    F: FnOnce(&Path) -> Result<(), RongJSError>,
{
    let cache = lxapp
        .cache()
        .map_err(|e| js_internal_error(format!("cache unavailable: {}", e)))?;

    match cache.resolve_path_with_ext(key, ext) {
        lxapp::ResolveResult::Exists(path) => Ok(path),
        lxapp::ResolveResult::NonExists(dest_path) => {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    js_internal_error(format!(
                        "chooseMedia failed to create cache directory {}: {}",
                        parent.display(),
                        e
                    ))
                })?;
            }
            write(&dest_path)?;
            Ok(dest_path)
        }
    }
}
