//! The name and icon an lxapp set for itself with `lx.shell.setAppDisplay`.
//!
//! Kept per app and per service env next to the registry's answer, which it
//! outranks. Icons live under app data, not beside registry artwork: the
//! registry cache directory may be emptied by the OS, and a chosen brand that
//! silently reverts is worse than none.

use super::metadata::{self, AppDisplayRecord};
use super::{LINGXIA_DIR, LXAPPS_DIR, registry, runtime_registry};
use crate::error::LxAppError;
use lingxia_platform::traits::app_runtime::AppRuntime;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

pub const MAX_NAME_CHARS: usize = 64;
pub const MAX_ICON_BYTES: u64 = 1024 * 1024;
/// A 1 MiB file can still claim a 30000 px canvas, so decoding is bounded too.
const MAX_ICON_SIDE: u32 = 4096;
/// Room for a full-size icon at 16 bits per RGBA channel.
const MAX_ICON_DECODE_BYTES: u64 = MAX_ICON_SIDE as u64 * MAX_ICON_SIDE as u64 * 8;
const ICON_DIR: &str = "display-icons";

/// One key of a `setAppDisplay` patch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldPatch<T> {
    Keep,
    Clear,
    Set(T),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDisplayPatch {
    /// Already normalized with [`normalize_name`].
    pub name: FieldPatch<String>,
    /// A resolved local file; [`set`] validates and copies it.
    pub icon: FieldPatch<PathBuf>,
}

fn invalid(message: impl Into<String>) -> LxAppError {
    LxAppError::InvalidParameter(message.into())
}

/// Trimmed, single line, 1–64 characters. `None` for a blank name, which
/// clears rather than sets.
pub fn normalize_name(raw: &str) -> Result<Option<String>, LxAppError> {
    let name = raw.trim();
    if name.is_empty() {
        return Ok(None);
    }
    if name
        .chars()
        .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
    {
        return Err(invalid("name must be a single line"));
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(invalid(format!(
            "name must be 1 to {MAX_NAME_CHARS} characters"
        )));
    }
    Ok(Some(name.to_string()))
}

/// Stored records by metadata key; the sidebar asks on every layout pass.
fn cache() -> &'static Mutex<HashMap<String, AppDisplayRecord>> {
    static CACHE: OnceLock<Mutex<HashMap<String, AppDisplayRecord>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn write_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn icon_dir() -> Option<PathBuf> {
    let runtime = runtime_registry::get_platform()?;
    Some(
        runtime
            .app_data_dir()
            .join(LINGXIA_DIR)
            .join(LXAPPS_DIR)
            .join(ICON_DIR),
    )
}

/// A store that is not open yet is not cached, so the first frame after
/// bootstrap still sees the record.
fn current(appid: &str) -> Option<AppDisplayRecord> {
    let key = metadata::app_display_key(appid);
    let mut cache = cache().lock().unwrap_or_else(|err| err.into_inner());
    if let Some(record) = cache.get(&key) {
        return Some(record.clone());
    }
    let record = metadata::app_display_get(appid).ok()?.unwrap_or_default();
    cache.insert(key, record.clone());
    Some(record)
}

fn forget(appid: &str) {
    cache()
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .remove(&metadata::app_display_key(appid));
}

/// The name this app set for itself.
pub(crate) fn name(appid: &str) -> Option<String> {
    current(appid)?.name
}

/// Absolute path of the icon this app set for itself, when it is on disk.
pub(crate) fn icon_path(appid: &str) -> Option<String> {
    let file = current(appid)?.icon_file?;
    let path = icon_dir()?.join(file);
    path.is_file().then(|| path.to_string_lossy().into_owned())
}

/// Apply `patch` to `appid`. `Ok(false)` when the result equals what is
/// stored: nothing is written and nothing repaints.
pub fn set(appid: &str, patch: AppDisplayPatch) -> Result<bool, LxAppError> {
    let dir = icon_dir().ok_or_else(|| LxAppError::Runtime("runtime not ready".to_string()))?;
    let _guard = write_lock().lock().unwrap_or_else(|err| err.into_inner());
    sweep_staging(&dir);
    let stored = metadata::app_display_get(appid)?.unwrap_or_default();
    let Some(next) = next_record(&stored, patch, &dir)? else {
        return Ok(false);
    };
    let written = commit_record(
        &dir,
        &stored,
        &next,
        |record| metadata::app_display_put(appid, record),
        metadata::app_display_referenced_icon_files,
    );
    forget(appid);
    written?;
    registry::notify_changed(&[appid.to_string()]);
    Ok(true)
}

/// Store `next`, then drop the icon file the outcome left unnamed: the
/// replaced one when the write landed, the new one when it failed.
fn commit_record(
    dir: &Path,
    stored: &AppDisplayRecord,
    next: &AppDisplayRecord,
    put: impl FnOnce(&AppDisplayRecord) -> Result<(), LxAppError>,
    referenced: impl FnOnce() -> Result<BTreeSet<String>, LxAppError>,
) -> Result<(), LxAppError> {
    let written = put(next);
    let (kept, dropped) = if written.is_ok() {
        (next, stored)
    } else {
        (stored, next)
    };
    if let Some(file) = dropped
        .icon_file
        .as_ref()
        .filter(|file| kept.icon_file.as_ref() != Some(*file))
        && let Ok(referenced) = referenced()
    {
        remove_unreferenced(dir, std::slice::from_ref(file), &referenced);
    }
    written
}

/// Writes are serialized by `write_lock`, so a staging file seen here was
/// left by a write that crashed.
fn sweep_staging(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "part") {
            let _ = fs::remove_file(path);
        }
    }
}

/// Drop `appid`'s choice under the running service env, on uninstall.
pub(crate) fn clear(appid: &str) {
    let _guard = write_lock().lock().unwrap_or_else(|err| err.into_inner());
    let removed = metadata::app_display_remove(appid);
    forget(appid);
    match removed {
        Ok(Some(record)) => {
            if let (Some(file), Some(dir)) = (record.icon_file, icon_dir()) {
                sweep_unreferenced(&dir, &[file]);
            }
        }
        Ok(None) => {}
        Err(err) => {
            crate::warn!("Failed to clear display choice for {}: {}", appid, err);
        }
    }
}

/// Icon files are content-addressed and shared across apps and envs, so a
/// file goes only when no record names it.
fn sweep_unreferenced(dir: &Path, candidates: &[String]) {
    if let Ok(referenced) = metadata::app_display_referenced_icon_files() {
        remove_unreferenced(dir, candidates, &referenced);
    }
}

fn remove_unreferenced(dir: &Path, candidates: &[String], referenced: &BTreeSet<String>) {
    for file in candidates {
        if !referenced.contains(file) {
            let _ = fs::remove_file(dir.join(file));
        }
    }
}

/// The record a patch produces, or `None` when it changes nothing. A new
/// icon is on disk before the record that names it is stored.
fn next_record(
    stored: &AppDisplayRecord,
    patch: AppDisplayPatch,
    dir: &Path,
) -> Result<Option<AppDisplayRecord>, LxAppError> {
    let name = match patch.name {
        FieldPatch::Keep => stored.name.clone(),
        FieldPatch::Clear => None,
        FieldPatch::Set(name) => Some(name),
    };
    let icon_file = match patch.icon {
        FieldPatch::Keep => stored.icon_file.clone(),
        FieldPatch::Clear => None,
        FieldPatch::Set(source) => Some(store_icon(&source, dir)?),
    };
    if name == stored.name && icon_file == stored.icon_file {
        return Ok(None);
    }
    Ok(Some(AppDisplayRecord { name, icon_file }))
}

/// Validate `source` and copy it into `dir` under its content hash, so the
/// same image set again names the same file.
fn store_icon(source: &Path, dir: &Path) -> Result<String, LxAppError> {
    let (bytes, extension) = read_icon(source)?;
    let digest = ring::digest::digest(&ring::digest::SHA256, &bytes);
    let hex: String = digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let file = format!("{hex}.{extension}");
    let destination = dir.join(&file);
    // A crash can leave a short file under the right name; rewrite it.
    if fs::read(&destination).is_ok_and(|existing| existing == bytes) {
        return Ok(file);
    }
    fs::create_dir_all(dir)?;
    let staging = dir.join(format!("{}.part", uuid::Uuid::new_v4()));
    let written = fs::File::create(&staging).and_then(|mut out| {
        out.write_all(&bytes)?;
        out.sync_all()
    });
    if let Err(err) = written {
        let _ = fs::remove_file(&staging);
        return Err(err.into());
    }
    // Windows refuses to rename onto an existing file.
    let _ = fs::remove_file(&destination);
    if let Err(err) = fs::rename(&staging, &destination) {
        let _ = fs::remove_file(&staging);
        if !fs::read(&destination).is_ok_and(|existing| existing == bytes) {
            return Err(err.into());
        }
    }
    Ok(file)
}

const UNSUPPORTED: &str = "icon must be a PNG, JPEG, WebP or SVG image";

fn read_icon(source: &Path) -> Result<(Vec<u8>, &'static str), LxAppError> {
    let metadata = fs::metadata(source).map_err(|_| invalid("icon file not found"))?;
    if !metadata.is_file() {
        return Err(invalid("icon must be a file"));
    }
    let too_large = || invalid(format!("icon must be at most {MAX_ICON_BYTES} bytes"));
    if metadata.len() > MAX_ICON_BYTES {
        return Err(too_large());
    }
    let bytes = fs::read(source).map_err(|_| invalid("icon file is not readable"))?;
    if bytes.len() as u64 > MAX_ICON_BYTES {
        return Err(too_large());
    }
    if is_svg(&bytes) {
        return Ok((bytes, "svg"));
    }
    let format = image::guess_format(&bytes).map_err(|_| invalid(UNSUPPORTED))?;
    let extension = match format {
        image::ImageFormat::Png => "png",
        image::ImageFormat::Jpeg => "jpg",
        image::ImageFormat::WebP => "webp",
        _ => return Err(invalid(UNSUPPORTED)),
    };
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_ICON_SIDE);
    limits.max_image_height = Some(MAX_ICON_SIDE);
    limits.max_alloc = Some(MAX_ICON_DECODE_BYTES);
    let mut reader = image::ImageReader::with_format(Cursor::new(&bytes), format);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|err| invalid(format!("icon could not be decoded: {err}")))?;
    if image.width() == 0 || image.height() == 0 {
        return Err(invalid("icon has no pixels"));
    }
    Ok((bytes, extension))
}

/// UTF-8 text whose first element is `<svg`, after an optional XML
/// declaration, doctype and comments. Hosts render it; it is not parsed here.
fn is_svg(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let mut rest = text.trim_start_matches('\u{feff}').trim_start();
    loop {
        if let Some(tail) = rest.strip_prefix("<svg") {
            return tail.starts_with(|c: char| c.is_whitespace() || c == '>' || c == '/');
        }
        let close = if rest.starts_with("<?") {
            "?>"
        } else if rest.starts_with("<!--") {
            "-->"
        } else if rest.starts_with("<!") {
            ">"
        } else {
            return false;
        };
        let Some(end) = rest.find(close) else {
            return false;
        };
        rest = rest[end + close.len()..].trim_start();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(color: [u8; 4]) -> Vec<u8> {
        let mut bytes = Vec::new();
        image::RgbaImage::from_pixel(4, 4, image::Rgba(color))
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes
    }

    fn patch(name: FieldPatch<String>, icon: FieldPatch<PathBuf>) -> AppDisplayPatch {
        AppDisplayPatch { name, icon }
    }

    #[test]
    fn identical_values_change_nothing_so_apps_may_set_them_on_every_launch() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join(ICON_DIR);
        let source = root.path().join("logo.png");
        fs::write(&source, png([10, 20, 30, 255])).unwrap();

        let set = |stored: &AppDisplayRecord| {
            next_record(
                stored,
                patch(
                    FieldPatch::Set("Acme".into()),
                    FieldPatch::Set(source.clone()),
                ),
                &dir,
            )
            .unwrap()
        };
        let first = set(&AppDisplayRecord::default()).expect("a first set changes");
        assert_eq!(set(&first), None);

        let cleared = next_record(&first, patch(FieldPatch::Keep, FieldPatch::Clear), &dir)
            .unwrap()
            .expect("clearing the icon changes the record");
        assert_eq!(cleared.name.as_deref(), Some("Acme"));
        assert_eq!(cleared.icon_file, None);
    }

    #[test]
    fn a_truncated_stored_icon_is_rewritten() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join(ICON_DIR);
        let source = root.path().join("logo.png");
        let bytes = png([1, 2, 3, 255]);
        fs::write(&source, &bytes).unwrap();
        let file = store_icon(&source, &dir).unwrap();
        fs::write(dir.join(&file), &bytes[..10]).unwrap();
        assert_eq!(store_icon(&source, &dir).unwrap(), file);
        assert_eq!(fs::read(dir.join(&file)).unwrap(), bytes);
    }

    #[test]
    fn only_drawable_icons_within_bounds_are_stored() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join(ICON_DIR);
        let check = |name: &str, bytes: &[u8]| {
            let source = root.path().join(name);
            fs::write(&source, bytes).unwrap();
            store_icon(&source, &dir)
        };
        let svg = br#"<?xml version="1.0"?><!-- logo --><svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"/>"#;
        assert!(check("logo.svg", svg).unwrap().ends_with(".svg"));
        assert!(
            check("logo.png", &png([0, 0, 0, 255]))
                .unwrap()
                .ends_with(".png")
        );

        let reject = |name: &str, bytes: &[u8]| {
            matches!(check(name, bytes), Err(LxAppError::InvalidParameter(_)))
        };
        assert!(reject("not-svg.svg", b"<svgx/>"));
        assert!(reject("truncated.png", &png([0, 0, 0, 255])[..24]));
        assert!(reject("logo.gif", b"GIF89a\x01\x00\x01\x00\x00\x00\x00;"));
        let mut oversized = svg.to_vec();
        oversized.resize(MAX_ICON_BYTES as usize + 1, b' ');
        assert!(reject("oversized.svg", &oversized));
    }

    #[test]
    fn a_full_size_icon_decodes_and_a_larger_one_is_refused() {
        let root = tempfile::tempdir().unwrap();
        let encode = |side: u32| {
            let mut bytes = Vec::new();
            image::RgbaImage::from_pixel(side, side, image::Rgba([255, 255, 255, 128]))
                .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
                .unwrap();
            let source = root.path().join(format!("{side}.png"));
            fs::write(&source, bytes).unwrap();
            read_icon(&source)
        };
        assert!(encode(MAX_ICON_SIDE).is_ok());
        assert!(matches!(
            encode(MAX_ICON_SIDE + 1),
            Err(LxAppError::InvalidParameter(_))
        ));
    }

    #[test]
    fn a_failed_write_takes_back_the_icon_it_stored() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join(ICON_DIR);
        let source = root.path().join("logo.png");
        fs::write(&source, png([5, 6, 7, 255])).unwrap();
        let file = store_icon(&source, &dir).unwrap();
        let stored = AppDisplayRecord::default();
        let next = AppDisplayRecord {
            name: None,
            icon_file: Some(file.clone()),
        };
        let refuse = |_: &AppDisplayRecord| Err(LxAppError::Runtime("disk full".into()));

        // Another record naming the same content keeps it.
        let shared = BTreeSet::from([file.clone()]);
        assert!(commit_record(&dir, &stored, &next, refuse, || Ok(shared)).is_err());
        assert!(dir.join(&file).is_file());

        assert!(commit_record(&dir, &stored, &next, refuse, || Ok(BTreeSet::new())).is_err());
        assert!(!dir.join(&file).exists());
    }

    #[test]
    fn a_landed_write_drops_the_icon_it_replaced() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join(ICON_DIR);
        let store = |name: &str, color: [u8; 4]| {
            let source = root.path().join(name);
            fs::write(&source, png(color)).unwrap();
            store_icon(&source, &dir).unwrap()
        };
        let old = store("old.png", [1, 1, 1, 255]);
        let new = store("new.png", [2, 2, 2, 255]);
        let record = |file: &str| AppDisplayRecord {
            name: None,
            icon_file: Some(file.to_string()),
        };
        commit_record(
            &dir,
            &record(&old),
            &record(&new),
            |_| Ok(()),
            || Ok(BTreeSet::from([new.clone()])),
        )
        .unwrap();
        assert!(!dir.join(&old).exists());
        assert!(dir.join(&new).is_file());
    }

    #[test]
    fn staging_files_left_by_a_crash_are_swept() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join(ICON_DIR);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("0123.part"), b"partial").unwrap();
        fs::write(dir.join("abcd.png"), png([0, 0, 0, 255])).unwrap();
        sweep_staging(&dir);
        assert!(!dir.join("0123.part").exists());
        assert!(dir.join("abcd.png").is_file());
        // No directory yet: nothing to sweep.
        sweep_staging(&root.path().join("missing"));
    }

    #[test]
    fn a_name_is_one_trimmed_line_and_blank_clears() {
        assert_eq!(
            normalize_name("  Acme Corp ").unwrap().as_deref(),
            Some("Acme Corp")
        );
        assert_eq!(normalize_name("   ").unwrap(), None);
        assert!(normalize_name("Acme\nCorp").is_err());
        assert!(normalize_name(&"灵".repeat(MAX_NAME_CHARS)).is_ok());
        assert!(normalize_name(&"a".repeat(MAX_NAME_CHARS + 1)).is_err());
    }
}
