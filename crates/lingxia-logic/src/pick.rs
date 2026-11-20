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
    use std::time::Duration;

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
