use std::collections::HashMap;
use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;

use serde::Serialize;

use super::{Platform, file, not_supported};
use crate::error::PlatformError;
use crate::traits::media_interaction::{
    ChooseMediaMode, ChooseMediaRequest, MediaInteraction, PreviewMediaRequest, SaveMediaRequest,
    ScanCodeRequest,
};
use crate::traits::media_runtime::{
    CompressImageRequest, CompressVideoRequest, ExtractVideoThumbnailRequest,
    ImageInfo, MediaRuntime, VideoInfo, VideoThumbnail,
};

impl MediaInteraction for Platform {
    fn preview_media(&self, request: PreviewMediaRequest) -> Result<(), PlatformError> {
        super::media_preview::open_preview(request).map_err(PlatformError::Platform)
    }

    fn cancel_preview(&self, callback_id: u64) -> Result<(), PlatformError> {
        super::media_preview::cancel_preview(callback_id).map_err(PlatformError::Platform)
    }

    fn choose_media(
        &self,
        request: ChooseMediaRequest,
    ) -> impl Future<Output = Result<String, PlatformError>> + Send {
        async move {
            // Camera capture has no pipeline on desktop; like the macOS
            // chooser, camera requests fall back to the file dialog.
            let handle = crate::rt::spawn_blocking(move || pick_media_files(&request));
            match handle {
                Some(task) => task.await.map_err(|err| {
                    PlatformError::Platform(format!("choose_media task panicked: {err}"))
                })?,
                None => Err(PlatformError::Platform(
                    "choose_media: async runtime not initialized".into(),
                )),
            }
        }
    }

    fn scan_code(
        &self,
        request: ScanCodeRequest,
    ) -> impl Future<Output = Result<String, PlatformError>> + Send {
        crate::desktop::scan::scan_code_desktop(request)
    }

    fn save_image_to_photos_album(
        &self,
        _request: SaveMediaRequest,
    ) -> impl Future<Output = Result<(), PlatformError>> + Send {
        async { not_supported("save_image_to_photos_album") }
    }

    fn save_video_to_photos_album(
        &self,
        _request: SaveMediaRequest,
    ) -> impl Future<Output = Result<(), PlatformError>> + Send {
        async { not_supported("save_video_to_photos_album") }
    }
}

const IMAGE_EXTENSIONS: [&str; 8] = ["png", "jpg", "jpeg", "gif", "bmp", "webp", "tif", "tiff"];
const VIDEO_EXTENSIONS: [&str; 7] = ["mp4", "mov", "avi", "mkv", "webm", "m4v", "wmv"];

/// Album picking on Windows is a file dialog over the media types; the
/// chosen paths go back as the `[{uri, fileType, isOriginal}]` array the
/// logic layer copies into the app cache. Cancel yields an empty list.
fn pick_media_files(request: &ChooseMediaRequest) -> Result<String, PlatformError> {
    let dialog = rfd::FileDialog::new();
    let dialog = match request.mode {
        ChooseMediaMode::Images => dialog
            .set_title("Choose Images")
            .add_filter("Images", &IMAGE_EXTENSIONS),
        ChooseMediaMode::Videos => dialog
            .set_title("Choose Videos")
            .add_filter("Videos", &VIDEO_EXTENSIONS),
        ChooseMediaMode::Mix => {
            let mut all: Vec<&str> = Vec::new();
            all.extend_from_slice(&IMAGE_EXTENSIONS);
            all.extend_from_slice(&VIDEO_EXTENSIONS);
            dialog
                .set_title("Choose Media")
                .add_filter("Media", &all)
                .add_filter("Images", &IMAGE_EXTENSIONS)
                .add_filter("Videos", &VIDEO_EXTENSIONS)
        }
    };
    let picked = if request.max_count > 1 {
        dialog.pick_files().unwrap_or_default()
    } else {
        dialog
            .pick_file()
            .map(|path| vec![path])
            .unwrap_or_default()
    };
    let entries: Vec<serde_json::Value> = picked
        .into_iter()
        .take(request.max_count.max(1) as usize)
        .map(|path| {
            let ext = path
                .extension()
                .map(|ext| ext.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            let kind = if VIDEO_EXTENSIONS.contains(&ext.as_str()) {
                "video"
            } else {
                "image"
            };
            serde_json::json!({
                "uri": path.to_string_lossy(),
                "fileType": kind,
                "isOriginal": true,
                "fileExt": ext,
            })
        })
        .collect();
    serde_json::to_string(&entries)
        .map_err(|err| PlatformError::Platform(format!("choose_media: {err}")))
}

impl MediaRuntime for Platform {
    fn copy_album_media_to_file(
        &self,
        uri: &str,
        dest_path: &Path,
        _kind: crate::traits::media_interaction::MediaKind,
    ) -> Result<(), PlatformError> {
        let source = file::normalize_file_uri(uri)?;
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                PlatformError::Platform(format!(
                    "failed to create destination directory {}: {err}",
                    parent.display()
                ))
            })?;
        }
        fs::copy(&source, dest_path).map_err(|err| {
            PlatformError::Platform(format!(
                "failed to copy media {} -> {}: {err}",
                source.display(),
                dest_path.display()
            ))
        })?;
        Ok(())
    }

    fn get_image_info(&self, uri: &str) -> Result<ImageInfo, PlatformError> {
        crate::desktop::image::get_image_info_desktop(uri)
    }

    fn compress_image(&self, request: &CompressImageRequest) -> Result<PathBuf, PlatformError> {
        crate::desktop::image::compress_image_desktop(request)
    }

    fn compress_video(&self, request: &CompressVideoRequest) -> Result<(), PlatformError> {
        // Resolve the source up front so a bad URI fails synchronously (the
        // caller treats `Err` as "job rejected"); the transcode itself runs on
        // a worker thread and reports back through the request's callbacks.
        let source = file::normalize_file_uri(&request.source_uri)?;
        let request = request.clone();
        let callback_id = request.callback_id;
        let progress_id = request.progress_callback_id;

        let cancel = Arc::new(AtomicBool::new(false));
        compress_cancel_registry()
            .lock()
            .unwrap()
            .insert(callback_id, cancel.clone());

        let spawned = thread::Builder::new()
            .name("lx-compress-video".to_string())
            .spawn(move || run_compress_video(request, source, progress_id, callback_id, cancel));

        if let Err(err) = spawned {
            compress_cancel_registry().lock().unwrap().remove(&callback_id);
            return Err(PlatformError::Platform(format!(
                "failed to start compress_video worker: {err}"
            )));
        }
        Ok(())
    }

    fn cancel_compress_video(&self, callback_id: u64) -> Result<(), PlatformError> {
        if let Some(cancel) = compress_cancel_registry().lock().unwrap().get(&callback_id) {
            cancel.store(true, Ordering::SeqCst);
        }
        Ok(())
    }

    fn get_video_info(&self, uri: &str) -> Result<VideoInfo, PlatformError> {
        let path = file::normalize_file_uri(uri)?;
        super::video_info::read_video_info(&path)
    }

    fn extract_video_thumbnail(
        &self,
        request: &ExtractVideoThumbnailRequest,
    ) -> Result<VideoThumbnail, PlatformError> {
        let source = file::normalize_file_uri(&request.source_uri)?;
        super::video_info::extract_thumbnail(request, &source).inspect_err(|err| {
            log::warn!("extract_video_thumbnail({}): {err}", source.display());
        })
    }
}

/// Live compress-video jobs keyed by their completion `callback_id`, each
/// holding the cancellation flag the worker polls. `cancel_compress_video`
/// flips the flag; the worker removes its entry when it finishes.
fn compress_cancel_registry() -> &'static Mutex<HashMap<u64, Arc<AtomicBool>>> {
    static REGISTRY: OnceLock<Mutex<HashMap<u64, Arc<AtomicBool>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Completion payload matching the JS-side `NativeCompressVideoResult`.
#[derive(Serialize)]
struct CompressVideoCompletion {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    height: Option<u32>,
    #[serde(rename = "durationMs", skip_serializing_if = "Option::is_none")]
    duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<u64>,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    mime_type: Option<String>,
}

/// Worker body for a single transcode: runs the Media Foundation pipeline,
/// streams progress, and fires the completion callback — unless the job was
/// cancelled, in which case it drops the partial output and stays silent
/// (the caller has already removed the callbacks).
fn run_compress_video(
    request: CompressVideoRequest,
    source: PathBuf,
    progress_id: u64,
    callback_id: u64,
    cancel: Arc<AtomicBool>,
) {
    // The transcode runs on this fresh worker thread; give it a COM apartment
    // (Media Foundation startup itself is process-wide and handled lazily by
    // the source reader). `CoInitializeEx` is idempotent per thread.
    unsafe {
        use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    let on_progress = move |pct: u8| {
        let _ = lingxia_messaging::invoke_callback(progress_id, Ok(format!("{{\"progress\":{pct}}}")));
    };
    let ctrl = super::video_compress::CompressControl {
        cancel: &cancel,
        on_progress: &on_progress,
    };
    let result = super::video_compress::compress_video(&request, &source, &ctrl);

    compress_cancel_registry().lock().unwrap().remove(&callback_id);

    if cancel.load(Ordering::SeqCst) {
        // Cancelled: discard any partial output and do not fire completion.
        let _ = fs::remove_file(&request.output_path);
        return;
    }

    let completion = match result {
        Ok(compressed) => CompressVideoCompletion {
            success: true,
            error: None,
            path: Some(compressed.path.to_string_lossy().into_owned()),
            width: Some(compressed.width),
            height: Some(compressed.height),
            duration_ms: Some(compressed.duration_ms),
            size: Some(compressed.size),
            mime_type: compressed
                .mime_type
                .or_else(|| Some("video/mp4".to_string())),
        },
        Err(err) => CompressVideoCompletion {
            success: false,
            error: Some(err.to_string()),
            path: None,
            width: None,
            height: None,
            duration_ms: None,
            size: None,
            mime_type: None,
        },
    };

    let payload = serde_json::to_string(&completion).unwrap_or_else(|_| {
        "{\"success\":false,\"error\":\"compress_video failed to serialize result\"}".to_string()
    });
    let _ = lingxia_messaging::invoke_callback(callback_id, Ok(payload));
}
