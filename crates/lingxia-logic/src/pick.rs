//! `lx.pick`: what the user chooses. Every picked file is built here, from its
//! name alone, so `name`, `size` and `type` are the same on every platform.

use crate::dismissal::completed;
use crate::i18n::js_internal_error;
use rong::{JSContext, JSObject, JSResult, JSValue, JsonToJSValue};
use serde::Serialize;
use std::io::Read;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn pick_namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    match lx.get::<_, JSObject>("pick") {
        Ok(namespace) => Ok(namespace),
        Err(_) => {
            let namespace = JSObject::new(ctx);
            lx.set("pick", namespace.clone())?;
            Ok(namespace)
        }
    }
}

rong::js_api! {
    fn register_pick_property(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        const pick: "PickApi" = pick_namespace(ctx)?;
    }
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_pick_property(ctx)
}

/// Run a system picker until it answers, `signal` aborts, or its page (`owner`) unloads. An abort
/// rejects `E_ABORT` at once; the page unloading settles it as `canceled()`, as the person closing
/// it would. Either way the picker stays until the person closes it, and what it returns then is
/// dropped, with the copies it made in the app's temp area deleted. An already-aborted signal
/// rejects first.
pub(crate) async fn abortable<T: 'static>(
    ctx: &JSContext,
    signal: Option<JSObject>,
    owner: Option<crate::ui::OwnerEnd>,
    work: impl std::future::Future<Output = JSResult<T>> + 'static,
    canceled: impl FnOnce() -> T,
    discard: impl FnOnce(T) + 'static,
) -> JSResult<T> {
    use futures::future::{Either, select};
    let mut listener = None;
    let mut aborted = None;
    if let Some(signal) = signal {
        crate::task_object::validate_abort_signal(Some(&signal))?;
        if signal.get::<_, bool>("aborted")? {
            return Err(crate::task_object::abort_error());
        }
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let tx = std::cell::RefCell::new(Some(tx));
        let on_abort = rong::JSFunc::new(ctx, move || {
            if let Some(tx) = tx.borrow_mut().take() {
                let _ = tx.send(());
            }
        })?;
        let options = JSObject::new(ctx);
        options.set("once", true)?;
        let add: rong::JSFunc = signal.get("addEventListener")?;
        add.call::<_, ()>(Some(signal.clone()), ("abort", on_abort.clone(), options))?;
        listener = Some((signal, on_abort));
        aborted = Some(rx);
    }
    if aborted.is_none() && owner.is_none() {
        return work.await;
    }
    let abort = Box::pin(async move {
        match aborted {
            Some(rx) => {
                let _ = rx.await;
            }
            None => std::future::pending().await,
        }
    });
    let unloaded = Box::pin(async move {
        match owner {
            Some(end) => end.await,
            None => std::future::pending().await,
        }
    });
    let running = rong::spawn_local(work);
    let outcome = match select(running, select(abort, unloaded)).await {
        Either::Left((done, _)) => {
            done.unwrap_or_else(|error| Err(js_internal_error(format!("pick failed: {error}"))))
        }
        Either::Right((ended, running)) => {
            drop(rong::spawn_local(async move {
                if let Ok(Ok(late)) = running.await {
                    discard(late);
                }
            }));
            match ended {
                Either::Left(_) => Err(crate::task_object::abort_error()),
                Either::Right(_) => Ok(canceled()),
            }
        }
    };
    if let Some((signal, on_abort)) = listener {
        let remove: rong::JSFunc = signal.get("removeEventListener")?;
        let _ = remove.call::<_, ()>(Some(signal), ("abort", on_abort));
    }
    outcome
}

/// Delete the files of a pick nobody waits for any more.
pub(crate) fn discard_files(app: &lxapp::LxApp, files: impl IntoIterator<Item = PickedFile>) {
    let paths = files
        .into_iter()
        .filter_map(|file| app.resolve_accessible_path(&file.path).ok());
    remove_copies(&app.temp_dir, paths);
}

/// Delete the paths inside `temp_dir`, the copies a picker made; a file the person picked in
/// place is theirs and stays.
pub(crate) fn remove_copies(
    temp_dir: &Path,
    paths: impl IntoIterator<Item = std::path::PathBuf>,
) -> Vec<std::path::PathBuf> {
    // Compare real paths: a `..` or a symlinked prefix (macOS `/var` → `/private/var`) must not
    // pass or fail the prefix test by spelling.
    let Ok(temp_dir) = temp_dir.canonicalize() else {
        return Vec::new();
    };
    paths
        .into_iter()
        .filter_map(|path| path.canonicalize().ok())
        .filter(|path| path.starts_with(&temp_dir) && std::fs::remove_file(path).is_ok())
        .collect()
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct PickedFile {
    pub(crate) path: String,
    pub(crate) name: String,
    pub(crate) size: u64,
    #[serde(rename = "type")]
    pub(crate) mime: &'static str,
}

impl PickedFile {
    /// `local` is the file on disk when the core can read it; `name` and
    /// `size` are what the platform reported for a reference it cannot.
    pub(crate) fn new(
        path: String,
        local: Option<&Path>,
        name: Option<String>,
        size: Option<u64>,
    ) -> Self {
        let mut name = name
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .or_else(|| {
                local
                    .and_then(Path::file_name)
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| last_segment(&path));
        if extension(&name).is_none()
            && let Some(ext) = local.and_then(sniff_extension)
        {
            name = format!("{name}.{ext}");
        }
        let size = local
            .and_then(|path| std::fs::metadata(path).ok())
            .map(|meta| meta.len())
            .or(size)
            .unwrap_or(0);
        let mime = mime_for_name(&name);
        Self {
            path,
            name,
            size,
            mime,
        }
    }
}

/// `{ status: 'ok', files }`.
pub(crate) fn files_result(ctx: &JSContext, files: &[PickedFile]) -> JSResult<JSObject> {
    let json = serde_json::to_string(files)
        .map_err(|e| js_internal_error(format!("failed to serialize picked files: {e}")))?;
    let files: JSValue = json
        .as_str()
        .json_to_js_value(ctx)
        .map_err(|e| js_internal_error(format!("failed to materialize picked files: {e}")))?;
    let result = completed(ctx)?;
    result.set("files", files)?;
    Ok(result)
}

/// `{ status: 'ok', file }`.
pub(crate) fn file_result(ctx: &JSContext, file: &PickedFile) -> JSResult<JSObject> {
    let json = serde_json::to_string(file)
        .map_err(|e| js_internal_error(format!("failed to serialize picked file: {e}")))?;
    let file: JSValue = json
        .as_str()
        .json_to_js_value(ctx)
        .map_err(|e| js_internal_error(format!("failed to materialize picked file: {e}")))?;
    let result = completed(ctx)?;
    result.set("file", file)?;
    Ok(result)
}

/// A captured or library item with no file name of its own:
/// `IMG_20261120_103000.jpg`, `VID_…`, with `_2`, `_3`… within one pick.
pub(crate) fn generated_media_name(
    video: bool,
    ext: &str,
    index: usize,
    now: SystemTime,
) -> String {
    let secs = now
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    let t = secs % 86_400;
    let stem = format!(
        "{}_{y:04}{m:02}{d:02}_{:02}{:02}{:02}",
        if video { "VID" } else { "IMG" },
        t / 3600,
        t / 60 % 60,
        t % 60
    );
    let suffix = if index == 0 {
        String::new()
    } else {
        format!("_{}", index + 1)
    };
    let ext = ext.trim_start_matches('.').to_ascii_lowercase();
    format!("{stem}{suffix}.{ext}")
}

// Howard Hinnant's days-to-civil; UTC keeps the name the same on every platform.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

fn last_segment(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    trimmed
        .rsplit(['/', '\\'])
        .next()
        .filter(|segment| !segment.is_empty())
        .unwrap_or(trimmed)
        .to_string()
}

fn extension(name: &str) -> Option<&str> {
    let (stem, ext) = name.rsplit_once('.')?;
    (!stem.is_empty() && !ext.is_empty()).then_some(ext)
}

/// A local file's type: from its extension, else from its leading bytes.
pub(crate) fn mime_for_path(path: &Path) -> &'static str {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    match extension(&name) {
        Some(_) => mime_for_name(&name),
        None => sniff_extension(path).map_or("application/octet-stream", |ext| {
            mime_for_name(&format!("f.{ext}"))
        }),
    }
}

/// The one extension → MIME table: every file type LingXia reports.
pub(crate) fn mime_for_name(name: &str) -> &'static str {
    let Some(ext) = extension(name) else {
        return "application/octet-stream";
    };
    match ext.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "heic" => "image/heic",
        "heif" => "image/heif",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "tif" | "tiff" => "image/tiff",
        "ico" => "image/x-icon",
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "avi" => "video/x-msvideo",
        "3gp" => "video/3gpp",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "aac" => "audio/aac",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        "pdf" => "application/pdf",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "txt" | "log" => "text/plain",
        "md" => "text/markdown",
        "csv" => "text/csv",
        "html" | "htm" => "text/html",
        "json" => "application/json",
        "xml" => "application/xml",
        "zip" => "application/zip",
        "gz" => "application/gzip",
        "7z" => "application/x-7z-compressed",
        "rar" => "application/vnd.rar",
        "epub" => "application/epub+zip",
        _ => "application/octet-stream",
    }
}

/// An extension for a file whose name has none, from its first bytes.
fn sniff_extension(path: &Path) -> Option<&'static str> {
    let mut head = [0u8; 16];
    let read = std::fs::File::open(path).ok()?.read(&mut head).ok()?;
    let head = &head[..read];
    if head.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if head.starts_with(b"\x89PNG") {
        Some("png")
    } else if head.starts_with(b"GIF8") {
        Some("gif")
    } else if head.starts_with(b"%PDF") {
        Some("pdf")
    } else if head.len() >= 12 && &head[..4] == b"RIFF" && &head[8..12] == b"WEBP" {
        Some("webp")
    } else if head.len() >= 12 && &head[4..8] == b"ftyp" {
        match &head[8..12] {
            b"heic" | b"heix" | b"mif1" => Some("heic"),
            b"qt  " => Some("mov"),
            _ => Some("mp4"),
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rong::{JSEngine, Source};
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::Duration;

    fn signal(ctx: &JSContext, aborted: bool) -> JSObject {
        ctx.eval(Source::from_bytes(format!(
            "globalThis.signal = {{ aborted: {aborted}, listeners: [], \
             addEventListener(_, f) {{ this.listeners.push(f); }}, \
             removeEventListener(_, f) {{ this.listeners = this.listeners.filter(g => g !== f); }} }}"
        )))
        .unwrap()
    }

    fn on_local<T>(work: impl std::future::Future<Output = T>) -> T {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        tokio::task::LocalSet::new().block_on(&runtime, work)
    }

    #[test]
    fn an_aborted_signal_rejects_before_the_picker_opens() {
        let runtime = <rong::RongJS as JSEngine>::runtime();
        let ctx = runtime.context();
        let opened = Rc::new(RefCell::new(false));
        let seen = opened.clone();
        let work = async move {
            *seen.borrow_mut() = true;
            Ok(1)
        };
        let result = on_local(abortable(
            &ctx,
            Some(signal(&ctx, true)),
            None,
            work,
            || 0,
            |_| {},
        ));
        assert!(result.unwrap_err().is_code(rong::error::E_ABORT));
        assert!(!*opened.borrow());
    }

    #[test]
    fn an_abort_rejects_at_once_and_drops_the_later_answer() {
        let runtime = <rong::RongJS as JSEngine>::runtime();
        let ctx = runtime.context();
        let (answer, answered) = tokio::sync::oneshot::channel::<u32>();
        let dropped = Rc::new(RefCell::new(Vec::new()));
        let into = dropped.clone();
        let work = async move { Ok(answered.await.unwrap()) };
        on_local(async {
            let call = abortable(
                &ctx,
                Some(signal(&ctx, false)),
                None,
                work,
                || 0,
                move |late| into.borrow_mut().push(late),
            );
            let (result, ()) = futures::join!(call, async {
                tokio::time::sleep(Duration::from_millis(20)).await;
                let _: rong::JSValue = ctx
                    .eval(Source::from_bytes(
                        "signal.aborted = true; signal.listeners.slice().forEach(f => f());",
                    ))
                    .unwrap();
            });
            assert!(result.unwrap_err().is_code(rong::error::E_ABORT));
            assert!(
                dropped.borrow().is_empty(),
                "the picker has not answered yet"
            );
            answer.send(7).unwrap();
            tokio::time::sleep(Duration::from_millis(20)).await;
        });
        assert_eq!(*dropped.borrow(), vec![7]);
        let listeners: i32 = ctx
            .eval(Source::from_bytes("signal.listeners.length"))
            .unwrap();
        assert_eq!(listeners, 0);
    }

    #[test]
    fn its_page_unloading_settles_the_pick_as_canceled_and_drops_the_later_answer() {
        let runtime = <rong::RongJS as JSEngine>::runtime();
        let ctx = runtime.context();
        let (answer, answered) = tokio::sync::oneshot::channel::<u32>();
        let (unload, unloaded) = tokio::sync::oneshot::channel::<()>();
        let owner: crate::ui::OwnerEnd = Box::pin(async move {
            let _ = unloaded.await;
        });
        let dropped = Rc::new(RefCell::new(Vec::new()));
        let into = dropped.clone();
        let work = async move { Ok(answered.await.unwrap()) };
        on_local(async {
            let call = abortable(
                &ctx,
                None,
                Some(owner),
                work,
                || 0,
                move |late| into.borrow_mut().push(late),
            );
            let (result, ()) = futures::join!(call, async {
                tokio::time::sleep(Duration::from_millis(20)).await;
                unload.send(()).unwrap();
            });
            assert_eq!(result.unwrap(), 0, "settles as canceled, not E_ABORT");
            answer.send(7).unwrap();
            tokio::time::sleep(Duration::from_millis(20)).await;
        });
        assert_eq!(*dropped.borrow(), vec![7]);
    }

    #[test]
    fn only_the_pickers_copies_are_deleted() {
        let temp = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let copy = temp.path().join("IMG_1.jpg");
        let original = elsewhere.path().join("mine.jpg");
        std::fs::write(&copy, b"x").unwrap();
        std::fs::write(&original, b"x").unwrap();
        let escape = temp
            .path()
            .join("..")
            .join(elsewhere.path().file_name().unwrap())
            .join("mine.jpg");
        let real_copy = copy.canonicalize().unwrap();
        let deleted = remove_copies(temp.path(), [copy.clone(), original.clone(), escape]);
        assert_eq!(deleted, vec![real_copy]);
        assert!(!copy.exists());
        assert!(
            original.exists(),
            "a `..` out of the temp dir is not a copy"
        );
    }

    #[test]
    fn type_follows_the_name_not_the_platform() {
        let file = PickedFile::new(
            "content://com.android.providers/document/42".into(),
            None,
            Some("Report.DOCX".into()),
            Some(48_213),
        );
        assert_eq!(file.name, "Report.DOCX");
        assert_eq!(file.size, 48_213);
        assert_eq!(
            file.mime,
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        );
        assert_eq!(mime_for_name(".bashrc"), "application/octet-stream");
    }

    #[test]
    fn a_nameless_local_file_gets_its_extension_from_its_bytes() {
        let dir = std::env::temp_dir().join(format!("lx-pick-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("1000000021");
        std::fs::write(&path, b"%PDF-1.7 rest").unwrap();
        let file = PickedFile::new("lx://temp/1000000021".into(), Some(&path), None, None);
        assert_eq!(file.name, "1000000021.pdf");
        assert_eq!(file.mime, "application/pdf");
        assert_eq!(file.size, 13);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn generated_names_are_utc_and_numbered_within_a_pick() {
        let at = UNIX_EPOCH + Duration::from_secs(1_795_170_600); // 2026-11-20 10:30:00 UTC
        assert_eq!(
            generated_media_name(false, "JPG", 0, at),
            "IMG_20261120_103000.jpg"
        );
        assert_eq!(
            generated_media_name(true, ".mp4", 1, at),
            "VID_20261120_103000_2.mp4"
        );
    }
}
