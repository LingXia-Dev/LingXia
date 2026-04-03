use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::media_interaction::{
    CameraFacing, ChooseMediaMode, ChooseMediaRequest, MediaInteraction, MediaKind, MediaObjectFit,
    MediaSource, PreviewMediaRequest, SaveMediaRequest, ScanCodeRequest, ScanType,
};
use crate::traits::media_runtime::{
    CompressImageRequest, CompressVideoRequest, CompressedVideo, ExtractVideoThumbnailRequest,
    ImageInfo, MediaRuntime, VideoInfo, VideoThumbnail,
};
use serde::Serialize;
use std::path::{Path, PathBuf};

const MEDIA_LIBRARY_IMAGE_RESOURCE: i32 = 1;
const MEDIA_LIBRARY_VIDEO_RESOURCE: i32 = 2;

#[derive(Serialize)]
struct PreviewMediaPayload<'a> {
    path: &'a str,
    media_type: i32,
    cover_path: &'a str,
    rotate: Option<u16>,
    object_fit: Option<&'static str>,
    #[serde(rename = "durationMs")]
    duration_ms: Option<u64>,
}

#[derive(Serialize)]
struct PreviewMediaRequestPayload<'a> {
    sources: Vec<PreviewMediaPayload<'a>>,
    #[serde(rename = "startIndex")]
    start_index: i32,
    advance: &'static str,
    #[serde(rename = "showIndexIndicator")]
    show_index_indicator: bool,
    #[serde(rename = "callbackId")]
    callback_id: String,
}

#[derive(Serialize)]
struct ScanCodePayload {
    #[serde(rename = "scanTypes")]
    scan_types: Vec<String>,
    #[serde(rename = "onlyFromCamera")]
    only_from_camera: bool,
    #[serde(rename = "callbackId")]
    callback_id: String,
}

impl MediaInteraction for Platform {
    fn preview_media(&self, request: PreviewMediaRequest) -> Result<(), PlatformError> {
        if request.items.is_empty() {
            return Err(PlatformError::Platform(
                "previewMedia requires at least one item".to_string(),
            ));
        }

        let payloads: Vec<PreviewMediaPayload> = request
            .items
            .iter()
            .map(|item| PreviewMediaPayload {
                path: item.path.as_str(),
                media_type: match item.media_type {
                    MediaKind::Image => 0,
                    MediaKind::Video => 1,
                    MediaKind::Unknown => -1,
                },
                cover_path: item.cover_path.as_deref().unwrap_or_default(),
                rotate: item.rotate,
                object_fit: item.object_fit.map(|fit| match fit {
                    MediaObjectFit::Cover => "cover",
                    MediaObjectFit::Contain => "contain",
                    MediaObjectFit::Fill => "fill",
                    MediaObjectFit::Fit => "fit",
                }),
                duration_ms: item.duration_ms,
            })
            .collect();

        let payload = PreviewMediaRequestPayload {
            sources: payloads,
            start_index: request.start_index,
            advance: request.advance.as_str(),
            show_index_indicator: request.show_index_indicator,
            callback_id: request.callback_id.to_string(),
        };

        let json = serde_json::to_string(&payload).map_err(|e| {
            PlatformError::Platform(format!("Failed to serialize preview media payload: {}", e))
        })?;

        let safe_json = json.replace('|', "%7C");

        lingxia_webview::platform::harmony::tsfn::call_arkts("previewMedia", &[safe_json.as_str()])
            .map_err(|e| PlatformError::Platform(format!("Failed to preview media: {}", e)))
    }

    fn cancel_preview(&self, callback_id: u64) -> Result<(), PlatformError> {
        let callback_id_str = callback_id.to_string();
        lingxia_webview::platform::harmony::tsfn::call_arkts(
            "closePreview",
            &[callback_id_str.as_str()],
        )
        .map_err(|e| PlatformError::Platform(format!("Failed to cancel preview media: {}", e)))
    }

    async fn choose_media(&self, request: ChooseMediaRequest) -> Result<String, PlatformError> {
        if request.max_count == 0 {
            return Err(PlatformError::Platform(
                "chooseMedia requires max_count to be greater than 0".to_string(),
            ));
        }

        crate::rt::native_call(|callback_id| {
            let mode_str = match request.mode {
                ChooseMediaMode::Images => "images",
                ChooseMediaMode::Videos => "videos",
                ChooseMediaMode::Mix => "mix",
            };

            let allow_album = request
                .source_types
                .iter()
                .any(|source| matches!(source, MediaSource::Album));

            let payload = ChooseMediaPayload {
                callback_id: callback_id.to_string(),
                max_count: request.max_count,
                mode: mode_str.to_string(),
                allow_album,
                allow_camera: request
                    .source_types
                    .iter()
                    .any(|source| matches!(source, MediaSource::Camera)),
                max_duration_seconds: request.max_duration_seconds,
                camera_facing: request.camera_facing.as_ref().map(|f| match f {
                    CameraFacing::Front => "front".to_string(),
                    CameraFacing::Back => "back".to_string(),
                }),
            };

            let payload_json = serde_json::to_string(&payload).map_err(|e| {
                PlatformError::Platform(format!("Failed to serialize chooseMedia payload: {}", e))
            })?;

            lingxia_webview::platform::harmony::tsfn::call_arkts(
                "chooseMedia",
                &[payload_json.as_str()],
            )
            .map_err(|e| {
                PlatformError::Platform(format!("Failed to start chooseMedia flow: {}", e))
            })
        })
        .await
    }

    async fn scan_code(&self, request: ScanCodeRequest) -> Result<String, PlatformError> {
        crate::rt::native_call(|callback_id| {
            let scan_types: Vec<String> = request
                .scan_types
                .iter()
                .map(|scan_type| match scan_type {
                    ScanType::QrCode => "qrCode".to_string(),
                    ScanType::BarCode => "barCode".to_string(),
                    ScanType::DataMatrix => "datamatrix".to_string(),
                    ScanType::Pdf417 => "pdf417".to_string(),
                })
                .collect();

            let payload = ScanCodePayload {
                scan_types,
                only_from_camera: request.only_from_camera,
                callback_id: callback_id.to_string(),
            };

            let payload_json = serde_json::to_string(&payload).map_err(|e| {
                PlatformError::Platform(format!("Failed to serialize scanCode payload: {}", e))
            })?;

            lingxia_webview::platform::harmony::tsfn::call_arkts(
                "scanCode",
                &[payload_json.as_str()],
            )
            .map_err(|e| PlatformError::Platform(format!("Failed to start scanCode flow: {}", e)))
        })
        .await
    }

    async fn save_image_to_photos_album(
        &self,
        request: SaveMediaRequest,
    ) -> Result<(), PlatformError> {
        crate::rt::native_call(|callback_id| {
            save_media_resource(&request.file_uri, MEDIA_LIBRARY_IMAGE_RESOURCE, callback_id)
        })
        .await
        .map(|_| ())
    }

    async fn save_video_to_photos_album(
        &self,
        request: SaveMediaRequest,
    ) -> Result<(), PlatformError> {
        crate::rt::native_call(|callback_id| {
            save_media_resource(&request.file_uri, MEDIA_LIBRARY_VIDEO_RESOURCE, callback_id)
        })
        .await
        .map(|_| ())
    }
}

impl MediaRuntime for Platform {
    fn copy_album_media_to_file(
        &self,
        uri: &str,
        dest_path: &Path,
        kind: MediaKind,
    ) -> Result<(), PlatformError> {
        self.copy_album_media_to_file_impl(uri, dest_path, kind)
    }

    fn get_image_info(&self, uri: &str) -> Result<ImageInfo, PlatformError> {
        image_native::get_image_info(uri)
    }

    fn compress_image(&self, request: &CompressImageRequest) -> Result<PathBuf, PlatformError> {
        image_native::compress_image(request)
    }

    fn get_video_info(&self, uri: &str) -> Result<VideoInfo, PlatformError> {
        video_native::get_video_info(uri)
    }

    fn extract_video_thumbnail(
        &self,
        request: &ExtractVideoThumbnailRequest,
    ) -> Result<VideoThumbnail, PlatformError> {
        video_native::extract_video_thumbnail(request)
    }

    fn compress_video(
        &self,
        request: &CompressVideoRequest,
    ) -> Result<CompressedVideo, PlatformError> {
        video_native::compress_video(request)
    }
}

fn save_media_resource(
    file_uri: &str,
    resource_type: i32,
    callback_id: u64,
) -> Result<(), PlatformError> {
    let safe_file_uri = file_uri.replace('|', "%7C");
    let media_type_str = resource_type.to_string();
    let callback_id_str = callback_id.to_string();
    lingxia_webview::platform::harmony::tsfn::call_arkts(
        "saveMedia",
        &[safe_file_uri.as_str(), &media_type_str, &callback_id_str],
    )
    .map_err(|e| {
        let _ = lingxia_messaging::invoke_callback(callback_id, Err(1000));
        PlatformError::Platform(format!("Failed to start save media: {}", e))
    })
}

#[derive(Serialize)]
struct ChooseMediaPayload {
    #[serde(rename = "callbackId")]
    callback_id: String,
    #[serde(rename = "maxCount")]
    max_count: u32,
    mode: String,
    #[serde(rename = "allowAlbum")]
    allow_album: bool,
    #[serde(rename = "allowCamera")]
    allow_camera: bool,
    #[serde(rename = "maxDurationSeconds")]
    max_duration_seconds: Option<u32>,
    #[serde(rename = "cameraFacing")]
    camera_facing: Option<String>,
}

mod image_native {
    use super::PlatformError;
    use crate::traits::media_runtime::{CompressImageRequest, ImageInfo};
    use core::ffi::c_char;
    use std::ffi::CString;
    use std::fs::{self, OpenOptions};
    use std::os::fd::{AsRawFd, RawFd};
    use std::path::Path;
    use std::ptr;
    use std::slice;
    use std::sync::{Mutex, OnceLock};

    const IMAGE_SUCCESS: i32 = 0;
    const JPEG_MIME: &str = "image/jpeg";

    fn image_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[repr(C)]
    struct ImageString {
        data: *mut c_char,
        size: usize,
    }

    #[repr(C)]
    struct ImageSize {
        width: u32,
        height: u32,
    }

    #[repr(C)]
    struct OH_ImageSourceNative {
        _private: [u8; 0],
    }
    #[repr(C)]
    struct OH_ImageSource_Info {
        _private: [u8; 0],
    }
    #[repr(C)]
    struct OH_DecodingOptions {
        _private: [u8; 0],
    }
    #[repr(C)]
    struct OH_PixelmapNative {
        _private: [u8; 0],
    }
    #[repr(C)]
    struct OH_ImagePackerNative {
        _private: [u8; 0],
    }
    #[repr(C)]
    struct OH_PackingOptions {
        _private: [u8; 0],
    }

    pub(super) fn get_image_info(uri: &str) -> Result<ImageInfo, PlatformError> {
        // Harmony image native APIs are not guaranteed to be thread-safe.
        let _guard = image_lock().lock().unwrap_or_else(|e| e.into_inner());

        let normalized_uri = normalize_uri(uri);
        let source = NativeImageSource::from_uri(&normalized_uri)?;
        let info = NativeImageInfo::new()?;

        check(
            unsafe { OH_ImageSourceNative_GetImageInfo(source.as_ptr(), 0, info.as_ptr()) },
            "OH_ImageSourceNative_GetImageInfo",
        )?;

        let width = info.width()?;
        let height = info.height()?;
        let mime_type = info.mime_type();

        Ok(ImageInfo {
            width,
            height,
            mime_type,
        })
    }

    pub(super) fn compress_image(
        request: &CompressImageRequest,
    ) -> Result<std::path::PathBuf, PlatformError> {
        // Harmony image native APIs are not guaranteed to be thread-safe.
        let _guard = image_lock().lock().unwrap_or_else(|e| e.into_inner());

        if request.source_uri.is_empty() {
            return Err(PlatformError::Platform("source_uri is empty".to_string()));
        }

        // Avoid crashing native decoders/packers on invalid absolute filesystem paths.
        // Note: Harmony picker URIs may look like `file://media/...` and do not necessarily map to
        // a direct filesystem path, so we only validate real absolute paths.
        let source_path = request
            .source_uri
            .strip_prefix("file://")
            .unwrap_or(&request.source_uri);
        let source_path_ref = Path::new(source_path);
        if source_path_ref.is_absolute() && !source_path_ref.exists() {
            return Err(PlatformError::Platform(format!(
                "source image does not exist: {}",
                source_path
            )));
        }

        let normalized_uri = normalize_uri(&request.source_uri);
        let source = NativeImageSource::from_uri(&normalized_uri)?;
        let info = NativeImageInfo::new()?;
        check(
            unsafe { OH_ImageSourceNative_GetImageInfo(source.as_ptr(), 0, info.as_ptr()) },
            "OH_ImageSourceNative_GetImageInfo",
        )?;
        let original_width = info.width()?;
        let original_height = info.height()?;
        let desired_size = compute_desired_size(
            original_width,
            original_height,
            request.max_width,
            request.max_height,
        );

        let decoding_options = DecodingOptions::new()?;
        if let Some(size) = desired_size {
            decoding_options.set_desired_size(size)?;
        }

        let pixelmap = source.create_pixelmap(&decoding_options)?;
        let packer = ImagePacker::new()?;
        let mut packing_options = PackingOptions::new()?;
        packing_options.set_mime_type(JPEG_MIME)?;
        packing_options.set_quality(u32::from(request.quality))?;

        if let Some(parent) = request.output_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                PlatformError::Platform(format!(
                    "Failed to prepare directory {}: {}",
                    parent.display(),
                    err
                ))
            })?;
        }

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&request.output_path)
            .map_err(|err| {
                PlatformError::Platform(format!(
                    "Failed to open {}: {}",
                    request.output_path.display(),
                    err
                ))
            })?;

        if let Err(err) = packer.pack_to_file(&pixelmap, &packing_options, file.as_raw_fd()) {
            drop(file);
            let _ = fs::remove_file(&request.output_path);
            return Err(err);
        }

        Ok(request.output_path.clone())
    }

    struct NativeImageSource(*mut OH_ImageSourceNative);

    impl NativeImageSource {
        fn from_uri(uri: &str) -> Result<Self, PlatformError> {
            let c_uri = CString::new(uri).map_err(|_| {
                PlatformError::Platform("Image URI contains invalid characters".to_string())
            })?;
            let mut handle = ptr::null_mut();
            // Some Harmony native APIs expect the buffer length to include the trailing NUL.
            let uri_size = c_uri.as_bytes_with_nul().len();
            let code = unsafe {
                OH_ImageSourceNative_CreateFromUri(
                    c_uri.as_ptr() as *mut c_char,
                    uri_size,
                    &mut handle,
                )
            };
            check(code, "OH_ImageSourceNative_CreateFromUri")?;
            if handle.is_null() {
                return Err(PlatformError::Platform(
                    "ImageSource handle is null".to_string(),
                ));
            }
            Ok(NativeImageSource(handle))
        }

        fn as_ptr(&self) -> *mut OH_ImageSourceNative {
            self.0
        }

        fn create_pixelmap(
            &self,
            options: &DecodingOptions,
        ) -> Result<NativePixelMap, PlatformError> {
            let mut pixelmap = ptr::null_mut();
            check(
                unsafe {
                    OH_ImageSourceNative_CreatePixelmap(self.0, options.as_ptr(), &mut pixelmap)
                },
                "OH_ImageSourceNative_CreatePixelmap",
            )?;
            if pixelmap.is_null() {
                return Err(PlatformError::Platform(
                    "Pixelmap handle is null".to_string(),
                ));
            }
            Ok(NativePixelMap(pixelmap))
        }
    }

    impl Drop for NativeImageSource {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    OH_ImageSourceNative_Release(self.0);
                }
            }
        }
    }

    struct NativeImageInfo(*mut OH_ImageSource_Info);

    impl NativeImageInfo {
        fn new() -> Result<Self, PlatformError> {
            let mut ptr = ptr::null_mut();
            check(
                unsafe { OH_ImageSourceInfo_Create(&mut ptr) },
                "OH_ImageSourceInfo_Create",
            )?;
            if ptr.is_null() {
                return Err(PlatformError::Platform(
                    "ImageSourceInfo handle is null".to_string(),
                ));
            }
            Ok(NativeImageInfo(ptr))
        }

        fn as_ptr(&self) -> *mut OH_ImageSource_Info {
            self.0
        }

        fn width(&self) -> Result<u32, PlatformError> {
            let mut width = 0;
            check(
                unsafe { OH_ImageSourceInfo_GetWidth(self.0, &mut width) },
                "OH_ImageSourceInfo_GetWidth",
            )?;
            Ok(width)
        }

        fn height(&self) -> Result<u32, PlatformError> {
            let mut height = 0;
            check(
                unsafe { OH_ImageSourceInfo_GetHeight(self.0, &mut height) },
                "OH_ImageSourceInfo_GetHeight",
            )?;
            Ok(height)
        }

        fn mime_type(&self) -> Option<String> {
            let mut mime = ImageString {
                data: ptr::null_mut(),
                size: 0,
            };
            let code =
                unsafe { OH_ImageSourceInfo_GetMimeType(self.0, &mut mime as *mut ImageString) };
            if code != IMAGE_SUCCESS {
                return None;
            }
            take_image_string(&mut mime)
        }
    }

    impl Drop for NativeImageInfo {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    OH_ImageSourceInfo_Release(self.0);
                }
            }
        }
    }

    struct DecodingOptions(*mut OH_DecodingOptions);

    impl DecodingOptions {
        fn new() -> Result<Self, PlatformError> {
            let mut ptr = ptr::null_mut();
            check(
                unsafe { OH_DecodingOptions_Create(&mut ptr) },
                "OH_DecodingOptions_Create",
            )?;
            if ptr.is_null() {
                return Err(PlatformError::Platform(
                    "DecodingOptions handle is null".to_string(),
                ));
            }
            Ok(DecodingOptions(ptr))
        }

        fn as_ptr(&self) -> *mut OH_DecodingOptions {
            self.0
        }

        fn set_desired_size(&self, size: ImageSize) -> Result<(), PlatformError> {
            let mut desired = size;
            check(
                unsafe {
                    OH_DecodingOptions_SetDesiredSize(self.0, &mut desired as *mut ImageSize)
                },
                "OH_DecodingOptions_SetDesiredSize",
            )
        }
    }

    impl Drop for DecodingOptions {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    OH_DecodingOptions_Release(self.0);
                }
            }
        }
    }

    struct NativePixelMap(*mut OH_PixelmapNative);

    impl NativePixelMap {
        fn as_ptr(&self) -> *mut OH_PixelmapNative {
            self.0
        }
    }

    impl Drop for NativePixelMap {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    OH_PixelmapNative_Release(self.0);
                }
            }
        }
    }

    struct ImagePacker(*mut OH_ImagePackerNative);

    impl ImagePacker {
        fn new() -> Result<Self, PlatformError> {
            let mut ptr = ptr::null_mut();
            check(
                unsafe { OH_ImagePackerNative_Create(&mut ptr) },
                "OH_ImagePackerNative_Create",
            )?;
            if ptr.is_null() {
                return Err(PlatformError::Platform(
                    "ImagePacker handle is null".to_string(),
                ));
            }
            Ok(ImagePacker(ptr))
        }

        fn pack_to_file(
            &self,
            pixelmap: &NativePixelMap,
            options: &PackingOptions,
            fd: RawFd,
        ) -> Result<(), PlatformError> {
            check(
                unsafe {
                    OH_ImagePackerNative_PackToFileFromPixelmap(
                        self.0,
                        options.as_ptr(),
                        pixelmap.as_ptr(),
                        fd,
                    )
                },
                "OH_ImagePackerNative_PackToFileFromPixelmap",
            )
        }
    }

    impl Drop for ImagePacker {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    OH_ImagePackerNative_Release(self.0);
                }
            }
        }
    }

    struct PackingOptions {
        handle: *mut OH_PackingOptions,
        // Keep the mime type C string alive in case the platform stores the pointer.
        mime_type: Option<CString>,
    }

    impl PackingOptions {
        fn new() -> Result<Self, PlatformError> {
            let mut ptr = ptr::null_mut();
            check(
                unsafe { OH_PackingOptions_Create(&mut ptr) },
                "OH_PackingOptions_Create",
            )?;
            if ptr.is_null() {
                return Err(PlatformError::Platform(
                    "PackingOptions handle is null".to_string(),
                ));
            }
            Ok(PackingOptions {
                handle: ptr,
                mime_type: None,
            })
        }

        fn as_ptr(&self) -> *mut OH_PackingOptions {
            self.handle
        }

        fn set_quality(&self, quality: u32) -> Result<(), PlatformError> {
            check(
                unsafe { OH_PackingOptions_SetQuality(self.handle, quality) },
                "OH_PackingOptions_SetQuality",
            )
        }

        fn set_mime_type(&mut self, mime: &str) -> Result<(), PlatformError> {
            let c_mime = CString::new(mime).map_err(|_| {
                PlatformError::Platform("Mime type contains invalid characters".to_string())
            })?;
            let mut mime_string = ImageString {
                data: c_mime.as_ptr() as *mut c_char,
                size: c_mime.as_bytes_with_nul().len(),
            };
            check(
                unsafe { OH_PackingOptions_SetMimeType(self.handle, &mut mime_string) },
                "OH_PackingOptions_SetMimeType",
            )?;
            self.mime_type = Some(c_mime);
            Ok(())
        }
    }

    impl Drop for PackingOptions {
        fn drop(&mut self) {
            if !self.handle.is_null() {
                unsafe {
                    OH_PackingOptions_Release(self.handle);
                }
            }
        }
    }

    fn take_image_string(value: &mut ImageString) -> Option<String> {
        if value.data.is_null() || value.size == 0 {
            return None;
        }

        // `OH_ImageSourceInfo_GetMimeType` may return memory owned by image_source internals.
        // Do not free `value.data` here; releasing `OH_ImageSource_Info` will handle cleanup.
        let len = unsafe {
            let bytes = slice::from_raw_parts(value.data as *const u8, value.size);
            bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len())
        };

        let text = unsafe {
            let bytes = slice::from_raw_parts(value.data as *const u8, len);
            String::from_utf8_lossy(bytes).trim().to_string()
        };

        if text.is_empty() { None } else { Some(text) }
    }

    fn compute_desired_size(
        original_width: u32,
        original_height: u32,
        requested_width: Option<u32>,
        requested_height: Option<u32>,
    ) -> Option<ImageSize> {
        if original_width == 0 || original_height == 0 {
            return None;
        }

        let width_opt = requested_width.filter(|w| *w > 0);
        let height_opt = requested_height.filter(|h| *h > 0);

        match (width_opt, height_opt) {
            (Some(w), Some(h)) => {
                if w == original_width && h == original_height {
                    None
                } else {
                    Some(ImageSize {
                        width: w,
                        height: h,
                    })
                }
            }
            (Some(w), None) => {
                let ratio = original_height as f64 / original_width as f64;
                let mut derived_h = (w as f64 * ratio).round() as u32;
                if derived_h == 0 {
                    derived_h = 1;
                }
                if w == original_width && derived_h == original_height {
                    None
                } else {
                    Some(ImageSize {
                        width: w,
                        height: derived_h,
                    })
                }
            }
            (None, Some(h)) => {
                let ratio = original_width as f64 / original_height as f64;
                let mut derived_w = (h as f64 * ratio).round() as u32;
                if derived_w == 0 {
                    derived_w = 1;
                }
                if derived_w == original_width && h == original_height {
                    None
                } else {
                    Some(ImageSize {
                        width: derived_w,
                        height: h,
                    })
                }
            }
            (None, None) => None,
        }
    }

    fn normalize_uri(uri: &str) -> String {
        if uri.starts_with("file://") {
            uri.to_string()
        } else {
            format!("file://{}", uri)
        }
    }

    fn check(code: i32, context: &str) -> Result<(), PlatformError> {
        if code == IMAGE_SUCCESS {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "{} failed: code {}",
                context, code
            )))
        }
    }

    #[link(name = "image_source")]
    unsafe extern "C" {
        fn OH_ImageSourceNative_CreateFromUri(
            uri: *mut c_char,
            uri_size: usize,
            res: *mut *mut OH_ImageSourceNative,
        ) -> i32;
        fn OH_ImageSourceNative_GetImageInfo(
            source: *mut OH_ImageSourceNative,
            index: i32,
            info: *mut OH_ImageSource_Info,
        ) -> i32;
        fn OH_ImageSourceNative_CreatePixelmap(
            source: *mut OH_ImageSourceNative,
            options: *mut OH_DecodingOptions,
            pixelmap: *mut *mut OH_PixelmapNative,
        ) -> i32;
        fn OH_ImageSourceNative_Release(source: *mut OH_ImageSourceNative) -> i32;

        fn OH_ImageSourceInfo_Create(info: *mut *mut OH_ImageSource_Info) -> i32;
        fn OH_ImageSourceInfo_GetWidth(info: *mut OH_ImageSource_Info, width: *mut u32) -> i32;
        fn OH_ImageSourceInfo_GetHeight(info: *mut OH_ImageSource_Info, height: *mut u32) -> i32;
        fn OH_ImageSourceInfo_GetMimeType(
            info: *mut OH_ImageSource_Info,
            mime: *mut ImageString,
        ) -> i32;
        fn OH_ImageSourceInfo_Release(info: *mut OH_ImageSource_Info) -> i32;

        fn OH_DecodingOptions_Create(options: *mut *mut OH_DecodingOptions) -> i32;
        fn OH_DecodingOptions_SetDesiredSize(
            options: *mut OH_DecodingOptions,
            desired: *mut ImageSize,
        ) -> i32;
        fn OH_DecodingOptions_Release(options: *mut OH_DecodingOptions) -> i32;
    }

    #[link(name = "pixelmap")]
    unsafe extern "C" {
        fn OH_PixelmapNative_Release(pixelmap: *mut OH_PixelmapNative) -> i32;
    }

    #[link(name = "image_packer")]
    unsafe extern "C" {
        fn OH_ImagePackerNative_Create(packer: *mut *mut OH_ImagePackerNative) -> i32;
        fn OH_ImagePackerNative_PackToFileFromPixelmap(
            packer: *mut OH_ImagePackerNative,
            options: *mut OH_PackingOptions,
            pixelmap: *mut OH_PixelmapNative,
            fd: i32,
        ) -> i32;
        fn OH_ImagePackerNative_Release(packer: *mut OH_ImagePackerNative) -> i32;

        fn OH_PackingOptions_Create(options: *mut *mut OH_PackingOptions) -> i32;
        fn OH_PackingOptions_SetMimeType(
            options: *mut OH_PackingOptions,
            format: *mut ImageString,
        ) -> i32;
        fn OH_PackingOptions_SetQuality(options: *mut OH_PackingOptions, quality: u32) -> i32;
        fn OH_PackingOptions_Release(options: *mut OH_PackingOptions) -> i32;
    }
}

mod video_native {
    use super::PlatformError;
    use crate::traits::media_runtime::{
        CompressVideoRequest, CompressedVideo, ExtractVideoThumbnailRequest, VideoInfo,
        VideoThumbnail,
    };
    use core::ffi::{c_char, c_void};
    use image::{
        ExtendedColorType, ImageBuffer, Rgba, codecs::jpeg::JpegEncoder, imageops::FilterType,
    };
    use std::collections::HashSet;
    use std::ffi::{CStr, CString};
    use std::fs::{self, OpenOptions};
    use std::os::fd::{AsRawFd, RawFd};
    use std::path::Path;
    use std::ptr;
    use std::sync::{Arc, Condvar, Mutex, OnceLock};
    use std::time::Duration;

    const AV_SUCCESS: i32 = 0;
    const IMAGE_SUCCESS: i32 = 0;
    const JPEG_MIME: &str = "image/jpeg";
    // Matches ArkTS `media.ContainerFormatType.CFT_MPEG_4`.
    const AV_OUTPUT_FORMAT_MPEG_4: i32 = 2;
    const TRANSCODE_TIMEOUT: Duration = Duration::from_secs(180);
    const TRANSCODER_AUDIO_BITRATE_DEFAULT: i32 = 128_000;
    const TRANSCODER_AUDIO_BITRATE_MAX: i32 = 500_000;
    const TRANSCODER_VIDEO_WIDTH_MIN: i32 = 240;
    const TRANSCODER_VIDEO_WIDTH_MAX: i32 = 3840;
    const TRANSCODER_VIDEO_HEIGHT_MIN: i32 = 240;
    const TRANSCODER_VIDEO_HEIGHT_MAX: i32 = 2160;

    fn video_image_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[repr(C)]
    struct OH_AVPlayer {
        _private: [u8; 0],
    }

    #[repr(C)]
    struct OH_AVMetadataExtractor {
        _private: [u8; 0],
    }

    #[repr(C)]
    struct OH_AVFormat {
        _private: [u8; 0],
    }

    #[repr(C)]
    struct OH_AVImageGenerator {
        _private: [u8; 0],
    }
    #[repr(C)]
    struct OH_AVTranscoder {
        _private: [u8; 0],
    }
    #[repr(C)]
    struct OH_AVTranscoder_Config {
        _private: [u8; 0],
    }
    #[repr(C)]
    struct OH_PixelmapNative {
        _private: [u8; 0],
    }
    #[repr(C)]
    struct OH_Pixelmap_ImageInfo {
        _private: [u8; 0],
    }
    #[repr(C)]
    struct OH_ImagePackerNative {
        _private: [u8; 0],
    }
    #[repr(C)]
    struct OH_PackingOptions {
        _private: [u8; 0],
    }
    #[repr(C)]
    struct ImageString {
        data: *mut c_char,
        size: usize,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct ImageSize {
        width: u32,
        height: u32,
    }

    #[repr(i32)]
    #[derive(Clone, Copy)]
    enum OhAvImageGeneratorQueryOptions {
        ClosestSync = 2,
        Closest = 3,
    }

    // Avoid using a Rust enum in FFI callback signatures: the native runtime could theoretically
    // pass an unknown value, which would be UB for a `repr(i32)` enum.
    const OH_AVTRANSCODER_STATE_CANCELLED: i32 = 4;
    const OH_AVTRANSCODER_STATE_COMPLETED: i32 = 5;

    enum TranscodeState {
        Pending,
        Completed,
        Error(String),
    }

    struct TranscodeSignal {
        state: Mutex<TranscodeState>,
        cv: Condvar,
    }

    impl TranscodeSignal {
        fn new() -> Self {
            Self {
                state: Mutex::new(TranscodeState::Pending),
                cv: Condvar::new(),
            }
        }

        fn mark_completed(&self) {
            if let Ok(mut state) = self.state.lock() {
                if matches!(*state, TranscodeState::Pending) {
                    *state = TranscodeState::Completed;
                    self.cv.notify_all();
                }
            }
        }

        fn mark_error(&self, message: String) {
            if let Ok(mut state) = self.state.lock() {
                if matches!(*state, TranscodeState::Pending) {
                    *state = TranscodeState::Error(message);
                    self.cv.notify_all();
                }
            }
        }

        fn wait(&self, timeout: Duration) -> Result<(), PlatformError> {
            let state = self
                .state
                .lock()
                .map_err(|_| PlatformError::Platform("transcode state poisoned".to_string()))?;
            let (state, timeout_result) = self
                .cv
                .wait_timeout_while(state, timeout, |s| matches!(*s, TranscodeState::Pending))
                .map_err(|_| PlatformError::Platform("transcode wait failed".to_string()))?;

            if timeout_result.timed_out() && matches!(*state, TranscodeState::Pending) {
                return Err(PlatformError::Platform(
                    "compressVideo timed out".to_string(),
                ));
            }

            match &*state {
                TranscodeState::Completed => Ok(()),
                TranscodeState::Error(message) => Err(PlatformError::Platform(message.clone())),
                TranscodeState::Pending => Err(PlatformError::Platform(
                    "compressVideo wait aborted unexpectedly".to_string(),
                )),
            }
        }
    }

    struct TranscodeCallbackContext {
        signal: Arc<TranscodeSignal>,
    }

    struct ScopedFd(i32);

    impl ScopedFd {
        fn raw(&self) -> i32 {
            self.0
        }
    }

    impl Drop for ScopedFd {
        fn drop(&mut self) {
            if self.0 >= 0 {
                unsafe { libc::close(self.0) };
                self.0 = -1;
            }
        }
    }

    struct NativeTranscoderConfig {
        handle: *mut OH_AVTranscoder_Config,
        // Keep codec mime C strings alive in case the platform stores the pointer.
        video_mime: Option<CString>,
        audio_mime: Option<CString>,
    }

    impl NativeTranscoderConfig {
        fn new() -> Result<Self, PlatformError> {
            let handle = unsafe { OH_AVTranscoderConfig_Create() };
            if handle.is_null() {
                return Err(PlatformError::Platform(
                    "OH_AVTranscoderConfig_Create returned null".to_string(),
                ));
            }
            Ok(Self {
                handle,
                video_mime: None,
                audio_mime: None,
            })
        }

        fn as_ptr(&self) -> *mut OH_AVTranscoder_Config {
            self.handle
        }

        fn set_src_fd(&self, fd: i32, size: i64) -> Result<(), PlatformError> {
            check_av(
                unsafe { OH_AVTranscoderConfig_SetSrcFD(self.as_ptr(), fd, 0, size) },
                "OH_AVTranscoderConfig_SetSrcFD",
            )
        }

        fn set_dst_fd(&self, fd: i32) -> Result<(), PlatformError> {
            check_av(
                unsafe { OH_AVTranscoderConfig_SetDstFD(self.as_ptr(), fd) },
                "OH_AVTranscoderConfig_SetDstFD",
            )
        }

        fn set_dst_file_type(&self, file_type: i32) -> Result<(), PlatformError> {
            check_av(
                unsafe { OH_AVTranscoderConfig_SetDstFileType(self.as_ptr(), file_type) },
                "OH_AVTranscoderConfig_SetDstFileType",
            )
        }

        fn set_dst_video_type_avc(&mut self) -> Result<(), PlatformError> {
            let c_mime = CString::new("video/avc").map_err(|_| {
                PlatformError::Platform("video codec mime contains null byte".to_string())
            })?;
            check_av(
                unsafe { OH_AVTranscoderConfig_SetDstVideoType(self.as_ptr(), c_mime.as_ptr()) },
                "OH_AVTranscoderConfig_SetDstVideoType",
            )?;
            // Keep alive in case the platform stores the pointer.
            self.video_mime = Some(c_mime);
            Ok(())
        }

        fn set_dst_audio_type_aac(&mut self) -> Result<(), PlatformError> {
            // NOTE: ArkTS uses `media.CodecMimeType.AUDIO_AAC`; OpenHarmony reports AAC as
            // `audio/mp4a-latm` in many places. Empirically, `audio/aac` also works on recent
            // releases, but `audio/mp4a-latm` is more broadly compatible.
            let c_mime = CString::new("audio/mp4a-latm").map_err(|_| {
                PlatformError::Platform("audio codec mime contains null byte".to_string())
            })?;
            check_av(
                unsafe { OH_AVTranscoderConfig_SetDstAudioType(self.as_ptr(), c_mime.as_ptr()) },
                "OH_AVTranscoderConfig_SetDstAudioType",
            )?;
            self.audio_mime = Some(c_mime);
            Ok(())
        }

        fn set_video_bitrate(&self, bitrate: i32) -> Result<(), PlatformError> {
            check_av(
                unsafe { OH_AVTranscoderConfig_SetDstVideoBitrate(self.as_ptr(), bitrate) },
                "OH_AVTranscoderConfig_SetDstVideoBitrate",
            )
        }

        fn set_audio_bitrate(&self, bitrate: i32) -> Result<(), PlatformError> {
            check_av(
                unsafe { OH_AVTranscoderConfig_SetDstAudioBitrate(self.as_ptr(), bitrate) },
                "OH_AVTranscoderConfig_SetDstAudioBitrate",
            )
        }

        fn set_video_resolution(&self, width: i32, height: i32) -> Result<(), PlatformError> {
            check_av(
                unsafe {
                    OH_AVTranscoderConfig_SetDstVideoResolution(self.as_ptr(), width, height)
                },
                "OH_AVTranscoderConfig_SetDstVideoResolution",
            )
        }
    }

    impl Drop for NativeTranscoderConfig {
        fn drop(&mut self) {
            if !self.handle.is_null() {
                unsafe {
                    let _ = OH_AVTranscoderConfig_Release(self.handle);
                }
                self.handle = ptr::null_mut();
            }
        }
    }

    struct NativeAvTranscoder(*mut OH_AVTranscoder);

    impl NativeAvTranscoder {
        fn new() -> Result<Self, PlatformError> {
            let handle = unsafe { OH_AVTranscoder_Create() };
            if handle.is_null() {
                return Err(PlatformError::Platform(
                    "OH_AVTranscoder_Create returned null".to_string(),
                ));
            }
            Ok(Self(handle))
        }

        fn as_ptr(&self) -> *mut OH_AVTranscoder {
            self.0
        }

        fn set_callbacks(&self, user_data: *mut c_void) -> Result<(), PlatformError> {
            check_av(
                unsafe {
                    OH_AVTranscoder_SetStateCallback(
                        self.as_ptr(),
                        Some(on_transcoder_state_changed),
                        user_data,
                    )
                },
                "OH_AVTranscoder_SetStateCallback",
            )?;
            check_av(
                unsafe {
                    OH_AVTranscoder_SetErrorCallback(
                        self.as_ptr(),
                        Some(on_transcoder_error),
                        user_data,
                    )
                },
                "OH_AVTranscoder_SetErrorCallback",
            )?;
            check_av(
                unsafe {
                    OH_AVTranscoder_SetProgressUpdateCallback(
                        self.as_ptr(),
                        Some(on_transcoder_progress),
                        user_data,
                    )
                },
                "OH_AVTranscoder_SetProgressUpdateCallback",
            )
        }

        fn prepare(&self, config: &NativeTranscoderConfig) -> Result<(), PlatformError> {
            check_av(
                unsafe { OH_AVTranscoder_Prepare(self.as_ptr(), config.as_ptr()) },
                "OH_AVTranscoder_Prepare",
            )
        }

        fn start(&self) -> Result<(), PlatformError> {
            check_av(
                unsafe { OH_AVTranscoder_Start(self.as_ptr()) },
                "OH_AVTranscoder_Start",
            )
        }

        fn cancel(&self) -> Result<(), PlatformError> {
            check_av(
                unsafe { OH_AVTranscoder_Cancel(self.as_ptr()) },
                "OH_AVTranscoder_Cancel",
            )
        }

        fn release(&mut self) -> Result<(), PlatformError> {
            if self.0.is_null() {
                return Ok(());
            }
            let result = check_av(
                unsafe { OH_AVTranscoder_Release(self.as_ptr()) },
                "OH_AVTranscoder_Release",
            );
            self.0 = ptr::null_mut();
            result
        }
    }

    impl Drop for NativeAvTranscoder {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    let _ = OH_AVTranscoder_Release(self.0);
                }
                self.0 = ptr::null_mut();
            }
        }
    }

    unsafe extern "C" fn on_transcoder_state_changed(
        _transcoder: *mut OH_AVTranscoder,
        state: i32,
        user_data: *mut c_void,
    ) {
        if user_data.is_null() {
            return;
        }
        let context = unsafe { &*(user_data as *const TranscodeCallbackContext) };
        match state {
            OH_AVTRANSCODER_STATE_COMPLETED => context.signal.mark_completed(),
            OH_AVTRANSCODER_STATE_CANCELLED => {
                context
                    .signal
                    .mark_error("compressVideo cancelled unexpectedly".to_string());
            }
            _ => {}
        }
    }

    unsafe extern "C" fn on_transcoder_error(
        _transcoder: *mut OH_AVTranscoder,
        error_code: i32,
        error_msg: *const c_char,
        user_data: *mut c_void,
    ) {
        if user_data.is_null() {
            return;
        }
        let context = unsafe { &*(user_data as *const TranscodeCallbackContext) };
        let detail = if error_msg.is_null() {
            String::new()
        } else {
            unsafe { CStr::from_ptr(error_msg) }
                .to_string_lossy()
                .into_owned()
        };
        if detail.is_empty() {
            context
                .signal
                .mark_error(format!("compressVideo failed with code {}", error_code));
        } else {
            context.signal.mark_error(format!(
                "compressVideo failed with code {}: {}",
                error_code, detail
            ));
        }
    }

    unsafe extern "C" fn on_transcoder_progress(
        _transcoder: *mut OH_AVTranscoder,
        _progress: i32,
        _user_data: *mut c_void,
    ) {
    }

    pub(super) fn compress_video(
        request: &CompressVideoRequest,
    ) -> Result<CompressedVideo, PlatformError> {
        if request.source_uri.trim().is_empty() {
            return Err(PlatformError::Platform("source_uri is empty".to_string()));
        }

        let source_path = normalize_to_path(&request.source_uri)?;
        let output_path = request.output_path.clone();
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                PlatformError::Platform(format!(
                    "Failed to prepare output directory {}: {}",
                    parent.display(),
                    err
                ))
            })?;
        }

        let source_info_result = get_video_info(&source_path);
        if let Err(err) = &source_info_result {
            log::info!(
                "[LingXia.Media] compressVideo source={} get_video_info failed: {}",
                source_path,
                err
            );
        }
        let source_info = source_info_result.ok();
        let bitrate = resolve_bitrate_bps(request);
        let detected_resolution = detect_source_resolution(source_info.as_ref(), &source_path);
        let scaled_resolution = resolve_output_resolution(request, detected_resolution);
        let min_safe_resolution = ensure_min_transcode_resolution(scaled_resolution);
        let aligned_resolution = align_resolution(min_safe_resolution, 16);
        let scaled_aligned_resolution = align_resolution(scaled_resolution, 16);
        let source_audio_detected = source_has_audio_track(&source_path);

        let (src_fd, src_size) = open_file_descriptor(&source_path)?;
        let src_fd = ScopedFd(src_fd);
        let source_size = src_size.max(0) as u64;

        let mut attempts = Vec::<(bool, Option<i32>, Option<(i32, i32)>)>::new();
        let mut seen = HashSet::<String>::new();
        let mut push_attempt =
            |include_audio: bool,
             attempt_bitrate: Option<i32>,
             attempt_resolution: Option<(i32, i32)>| {
                let key = format!(
                    "a:{}|b:{}|w:{}|h:{}",
                    include_audio,
                    attempt_bitrate.unwrap_or(-1),
                    attempt_resolution.map(|(w, _)| w).unwrap_or(-1),
                    attempt_resolution.map(|(_, h)| h).unwrap_or(-1)
                );
                if seen.insert(key) {
                    attempts.push((include_audio, attempt_bitrate, attempt_resolution));
                }
            };

        // Prefer explicit audio settings first to avoid the native default audio bitrate
        // becoming INT_MAX on some devices.
        for include_audio in [source_audio_detected, !source_audio_detected] {
            push_attempt(include_audio, bitrate, min_safe_resolution);
            push_attempt(include_audio, bitrate, aligned_resolution);
            push_attempt(include_audio, bitrate, scaled_resolution);
            push_attempt(include_audio, bitrate, scaled_aligned_resolution);
            push_attempt(include_audio, bitrate, None);
            push_attempt(include_audio, None, min_safe_resolution);
            push_attempt(include_audio, None, aligned_resolution);
            push_attempt(include_audio, None, scaled_resolution);
            push_attempt(include_audio, None, scaled_aligned_resolution);
            push_attempt(include_audio, None, None);
        }

        log::info!(
            "[LingXia.Media] compressVideo source={} sourceAudioDetected={} bitrate={:?} detectedResolution={:?} resolution={:?} minSafeResolution={:?} alignedResolution={:?} attempts={}",
            source_path,
            source_audio_detected,
            bitrate,
            detected_resolution,
            scaled_resolution,
            min_safe_resolution,
            aligned_resolution,
            attempts.len()
        );

        let mut last_error: Option<PlatformError> = None;
        let mut transcode_ok = false;
        for (index, (include_audio, attempt_bitrate, attempt_resolution)) in
            attempts.iter().enumerate()
        {
            log::info!(
                "[LingXia.Media] compressVideo attempt={}/{} includeAudio={} bitrate={:?} resolution={:?}",
                index + 1,
                attempts.len(),
                include_audio,
                attempt_bitrate,
                attempt_resolution
            );
            match run_transcode_attempt(
                src_fd.raw(),
                src_size,
                &output_path,
                *include_audio,
                *attempt_bitrate,
                *attempt_resolution,
            ) {
                Ok(()) => {
                    transcode_ok = true;
                    break;
                }
                Err(err) => {
                    log::info!(
                        "[LingXia.Media] compressVideo attempt={}/{} failed: {}",
                        index + 1,
                        attempts.len(),
                        err
                    );
                    let _ = fs::remove_file(&output_path);
                    last_error = Some(err);
                }
            }
        }

        // Ensure source descriptor is closed before probing output metadata.
        drop(src_fd);

        if !transcode_ok {
            log::info!(
                "[LingXia.Media] compressVideo fallback to passthrough after transcode failure: {}",
                last_error
                    .as_ref()
                    .map(|err| err.to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            );
            copy_source_to_output(&source_path, &output_path)?;
        }

        if request.fps.unwrap_or(0) > 0 {
            log::debug!("Harmony AVTranscoder does not support fps override; ignoring request.fps");
        }

        let mut size = fs::metadata(&output_path).map(|m| m.len()).unwrap_or(0);
        if transcode_ok && source_size > 0 && size >= source_size {
            log::info!(
                "[LingXia.Media] compressVideo fallback to passthrough because output is not smaller: outputSize={} sourceSize={}",
                size,
                source_size
            );
            copy_source_to_output(&source_path, &output_path)?;
            size = fs::metadata(&output_path)
                .map(|m| m.len())
                .unwrap_or(source_size);
        }

        let output_path_str = output_path.to_string_lossy().into_owned();
        let output_info = get_video_info(&output_path_str).ok();

        Ok(CompressedVideo {
            path: output_path,
            width: output_info.as_ref().map(|v| v.width).unwrap_or(0),
            height: output_info.as_ref().map(|v| v.height).unwrap_or(0),
            duration_ms: output_info.as_ref().map(|v| v.duration_ms).unwrap_or(0),
            size,
            mime_type: output_info
                .and_then(|v| v.mime_type)
                .or_else(|| infer_video_mime_type(&output_path_str)),
        })
    }

    fn resolve_bitrate_bps(request: &CompressVideoRequest) -> Option<i32> {
        let by_quality = request.quality.map(|quality| match quality {
            crate::traits::media_runtime::VideoCompressQuality::Low => 800_000,
            crate::traits::media_runtime::VideoCompressQuality::Medium => 1_500_000,
            crate::traits::media_runtime::VideoCompressQuality::High => 3_000_000,
        });
        let by_input = request
            .bitrate_kbps
            .filter(|v| *v > 0)
            .map(|v| v.saturating_mul(1_000).min(i32::MAX as u32) as i32);
        by_input.or(by_quality)
    }

    fn detect_source_resolution(
        source_info: Option<&VideoInfo>,
        source_path: &str,
    ) -> Option<(i32, i32)> {
        if let Some(info) = source_info {
            if let Some(size) = sanitize_resolution(info.width as i32, info.height as i32) {
                return Some(size);
            }
        }

        if let Ok((width, height)) = read_video_dimensions_with_generator(source_path) {
            if let Some(size) = sanitize_resolution(width as i32, height as i32) {
                return Some(size);
            }
        }

        if let Ok((width, height, _duration_ms)) = read_basic_video_info_with_player(source_path) {
            if let Some(size) = sanitize_resolution(width as i32, height as i32) {
                return Some(size);
            }
        }

        None
    }

    fn resolve_output_resolution(
        request: &CompressVideoRequest,
        source_resolution: Option<(i32, i32)>,
    ) -> Option<(i32, i32)> {
        let (source_width, source_height) = source_resolution?;

        let (raw_width, raw_height) = match request.resolution_ratio {
            Some(ratio) if (0.0..1.0).contains(&ratio) => (
                ((source_width as f32) * ratio).round() as i32,
                ((source_height as f32) * ratio).round() as i32,
            ),
            _ => (source_width, source_height),
        };

        sanitize_resolution(
            raw_width.clamp(2, source_width),
            raw_height.clamp(2, source_height),
        )
    }

    fn sanitize_resolution(width: i32, height: i32) -> Option<(i32, i32)> {
        let clamped_width = width.clamp(2, TRANSCODER_VIDEO_WIDTH_MAX);
        let clamped_height = height.clamp(2, TRANSCODER_VIDEO_HEIGHT_MAX);
        let even_width = even_int(clamped_width);
        let even_height = even_int(clamped_height);
        if even_width < 2 || even_height < 2 {
            return None;
        }
        Some((even_width, even_height))
    }

    fn ensure_min_transcode_resolution(resolution: Option<(i32, i32)>) -> Option<(i32, i32)> {
        let (width, height) = resolution?;
        if width >= TRANSCODER_VIDEO_WIDTH_MIN && height >= TRANSCODER_VIDEO_HEIGHT_MIN {
            return Some((width, height));
        }

        let scale_w = TRANSCODER_VIDEO_WIDTH_MIN as f32 / width as f32;
        let scale_h = TRANSCODER_VIDEO_HEIGHT_MIN as f32 / height as f32;
        let scale = scale_w.max(scale_h).max(1.0);
        let scaled_width = ((width as f32) * scale).ceil() as i32;
        let scaled_height = ((height as f32) * scale).ceil() as i32;
        let normalized = sanitize_resolution(scaled_width, scaled_height)?;
        if normalized.0 < TRANSCODER_VIDEO_WIDTH_MIN || normalized.1 < TRANSCODER_VIDEO_HEIGHT_MIN {
            return None;
        }
        Some(normalized)
    }

    fn align_resolution(resolution: Option<(i32, i32)>, alignment: i32) -> Option<(i32, i32)> {
        let (width, height) = resolution?;
        let aligned_width = align_down(width, alignment);
        let aligned_height = align_down(height, alignment);
        let normalized = sanitize_resolution(aligned_width, aligned_height)?;
        if normalized.0 == width && normalized.1 == height {
            return None;
        }
        Some(normalized)
    }

    fn align_down(value: i32, alignment: i32) -> i32 {
        if alignment <= 1 {
            return value;
        }
        (value / alignment) * alignment
    }

    fn run_transcode_attempt(
        src_fd: i32,
        src_size: i64,
        output_path: &Path,
        include_audio: bool,
        bitrate: Option<i32>,
        resolution: Option<(i32, i32)>,
    ) -> Result<(), PlatformError> {
        let dst_fd = open_output_descriptor(output_path)?;
        let dst_fd = ScopedFd(dst_fd);

        let mut config = NativeTranscoderConfig::new()?;
        config.set_src_fd(src_fd, src_size)?;
        config.set_dst_fd(dst_fd.raw())?;
        config.set_dst_file_type(AV_OUTPUT_FORMAT_MPEG_4)?;
        config.set_dst_video_type_avc()?;
        if include_audio {
            config.set_dst_audio_type_aac()?;
        }
        // Always set an explicit audio bitrate to avoid vendor defaults becoming INT_MAX.
        config.set_audio_bitrate(clamp_audio_bitrate(TRANSCODER_AUDIO_BITRATE_DEFAULT))?;
        if let Some(video_bitrate) = bitrate {
            config.set_video_bitrate(video_bitrate)?;
        }
        if let Some((width, height)) = resolution {
            config.set_video_resolution(width, height)?;
        }

        let mut transcoder = NativeAvTranscoder::new()?;
        let signal = Arc::new(TranscodeSignal::new());
        let callback_ptr = Box::into_raw(Box::new(TranscodeCallbackContext {
            signal: Arc::clone(&signal),
        }));

        let transcode_result = (|| {
            transcoder.set_callbacks(callback_ptr as *mut c_void)?;
            transcoder.prepare(&config)?;
            transcoder.start()?;
            signal.wait(TRANSCODE_TIMEOUT)
        })();

        if transcode_result.is_err() {
            let _ = transcoder.cancel();
        }
        let release_result = transcoder.release();
        unsafe { drop(Box::from_raw(callback_ptr)) };

        // Ensure output descriptor is closed before probing or deleting output file.
        drop(dst_fd);

        if let Err(err) = transcode_result {
            let _ = release_result;
            return Err(err);
        }

        release_result
    }

    fn copy_source_to_output(source_path: &str, output_path: &Path) -> Result<(), PlatformError> {
        let source = Path::new(source_path);
        if source == output_path {
            return Ok(());
        }
        fs::copy(source, output_path).map_err(|err| {
            PlatformError::Platform(format!(
                "Failed to copy source video to {}: {}",
                output_path.display(),
                err
            ))
        })?;
        Ok(())
    }

    fn source_has_audio_track(path: &str) -> bool {
        let Ok(mut extractor) = NativeMetadataExtractor::new() else {
            return false;
        };
        if extractor.set_file_source(path).is_err() {
            return false;
        }
        let Ok(format) = NativeAvFormat::new() else {
            return false;
        };
        if extractor.fetch_metadata(format.as_ptr()).is_err() {
            return false;
        }
        // If we can read an audio sample rate, the source has an audio track.
        format
            .get_int_compatible(unsafe { OH_MD_KEY_AUD_SAMPLE_RATE })
            .map(|v| v > 0)
            .unwrap_or(false)
    }

    fn clamp_audio_bitrate(value: i32) -> i32 {
        if value <= 0 {
            return 1;
        }
        value.min(TRANSCODER_AUDIO_BITRATE_MAX)
    }

    fn even_int(value: i32) -> i32 {
        if value <= 0 {
            return 0;
        }
        if value % 2 == 0 { value } else { value - 1 }
    }

    pub(super) fn get_video_info(uri: &str) -> Result<VideoInfo, PlatformError> {
        let source_path = normalize_to_path(uri)?;
        let mut info = VideoInfo {
            width: 0,
            height: 0,
            duration_ms: 0,
            rotation: None,
            bitrate: None,
            fps: None,
            mime_type: infer_video_mime_type(&source_path),
        };

        let metadata_error = match read_video_metadata(&source_path) {
            Ok(metadata) => {
                if let Some(width) = metadata.width {
                    info.width = width;
                }
                if let Some(height) = metadata.height {
                    info.height = height;
                }
                if let Some(duration_ms) = metadata.duration_ms {
                    info.duration_ms = duration_ms;
                }
                info.rotation = metadata.rotation;
                info.bitrate = metadata.bitrate;
                info.fps = metadata.fps;
                if metadata.mime_type.is_some() {
                    info.mime_type = metadata.mime_type;
                }
                None
            }
            Err(err) => Some(err),
        };

        let generator_error = if info.width == 0 || info.height == 0 {
            match read_video_dimensions_with_generator(&source_path) {
                Ok((width, height)) => {
                    if info.width == 0 {
                        info.width = width;
                    }
                    if info.height == 0 {
                        info.height = height;
                    }
                    None
                }
                Err(err) => Some(err),
            }
        } else {
            None
        };

        let should_probe_player =
            info.width == 0 || info.height == 0 || info.duration_ms == 0 || info.duration_ms < 1000;
        let player_probe = if should_probe_player {
            Some(read_basic_video_info_with_player(&source_path))
        } else {
            None
        };
        let mut player_error = None;
        if let Some(probe) = player_probe {
            match probe {
                Ok((width, height, duration_ms)) => {
                    if info.width == 0 {
                        info.width = width;
                    }
                    if info.height == 0 {
                        info.height = height;
                    }
                    info.duration_ms = reconcile_duration_ms(info.duration_ms, duration_ms);
                }
                Err(err) => {
                    player_error = Some(err);
                }
            }
        };

        if info.width == 0 && info.height == 0 && info.duration_ms == 0 {
            return Err(player_error
                .or(generator_error)
                .or(metadata_error)
                .unwrap_or_else(|| {
                    PlatformError::Platform("Failed to read video metadata".to_string())
                }));
        }

        Ok(info)
    }

    struct RawVideoMetadata {
        width: Option<u32>,
        height: Option<u32>,
        duration_ms: Option<u64>,
        rotation: Option<u16>,
        bitrate: Option<u64>,
        fps: Option<f32>,
        mime_type: Option<String>,
    }

    fn read_video_metadata(path: &str) -> Result<RawVideoMetadata, PlatformError> {
        let mut extractor = NativeMetadataExtractor::new()?;
        extractor.set_file_source(path)?;
        let format = NativeAvFormat::new()?;
        extractor.fetch_metadata(format.as_ptr())?;

        let width = format
            .get_int_compatible(unsafe { OH_MD_KEY_WIDTH })
            .map(|v| v.max(0) as u32);
        let height = format
            .get_int_compatible(unsafe { OH_MD_KEY_HEIGHT })
            .map(|v| v.max(0) as u32);

        let duration_ms = format
            .get_long_compatible(unsafe { OH_MD_KEY_DURATION })
            .map(normalize_metadata_duration_ms);
        let rotation = format
            .get_int_compatible(unsafe { OH_MD_KEY_ROTATION })
            .map(normalize_rotation_degrees);
        let bitrate = format
            .get_long_compatible(unsafe { OH_MD_KEY_BITRATE })
            .map(|v| v.max(0) as u64);
        let fps = format
            .get_double_compatible(unsafe { OH_MD_KEY_FRAME_RATE })
            .and_then(|v| {
                if v.is_finite() && v > 0.0 {
                    Some(v as f32)
                } else {
                    None
                }
            });
        let mime_type = format
            .get_string(unsafe { OH_MD_KEY_CODEC_MIME })
            .and_then(|v| if v.is_empty() { None } else { Some(v) });

        Ok(RawVideoMetadata {
            width,
            height,
            duration_ms,
            rotation,
            bitrate,
            fps,
            mime_type,
        })
    }

    fn read_basic_video_info_with_player(path: &str) -> Result<(u32, u32, u64), PlatformError> {
        let mut player = NativeAvPlayer::new()?;
        player.set_file_source(path)?;
        player.prepare()?;
        let (width, height) = player.get_video_size()?;
        let duration_ms = player.get_duration()?;
        player.release()?;
        Ok((
            width.max(0) as u32,
            height.max(0) as u32,
            duration_ms.max(0) as u64,
        ))
    }

    fn read_video_dimensions_with_generator(path: &str) -> Result<(u32, u32), PlatformError> {
        let mut generator = NativeAvImageGenerator::new()?;
        generator.set_file_source(path)?;
        let pixelmap = generator.fetch_frame_by_time(0)?;
        pixelmap.dimensions()
    }

    fn normalize_metadata_duration_ms(raw: i64) -> u64 {
        raw.max(0) as u64
    }

    fn reconcile_duration_ms(metadata_duration_ms: u64, player_duration_ms: u64) -> u64 {
        if metadata_duration_ms == 0 {
            return player_duration_ms;
        }
        if player_duration_ms == 0 {
            return metadata_duration_ms;
        }

        if metadata_duration_ms > player_duration_ms.saturating_mul(100)
            && metadata_duration_ms % 1000 == 0
        {
            let us_to_ms = metadata_duration_ms / 1000;
            if us_to_ms > 0 {
                return us_to_ms;
            }
        }

        if metadata_duration_ms < 1000 && player_duration_ms >= 1000 {
            return player_duration_ms;
        }

        metadata_duration_ms
    }

    fn normalize_rotation_degrees(raw: i32) -> u16 {
        let normalized = raw.rem_euclid(360);
        normalized as u16
    }

    pub(super) fn extract_video_thumbnail(
        request: &ExtractVideoThumbnailRequest,
    ) -> Result<VideoThumbnail, PlatformError> {
        // Harmony image/pixelmap native APIs are not guaranteed to be thread-safe.
        let _guard = video_image_lock().lock().unwrap_or_else(|e| e.into_inner());

        if request.source_uri.is_empty() {
            return Err(PlatformError::Platform("source_uri is empty".to_string()));
        }

        let source_path = normalize_to_path(&request.source_uri)?;
        let mut generator = NativeAvImageGenerator::new()?;
        generator.set_file_source(&source_path)?;
        let time_ms = request.time_ms.unwrap_or(0);
        let pixelmap = generator.fetch_frame_by_time(time_ms)?;

        let (original_width, original_height) = pixelmap.dimensions()?;
        let desired_size = compute_desired_size(
            original_width,
            original_height,
            request.max_width,
            request.max_height,
        );
        let (final_width, final_height) = match pack_thumbnail_with_pixelmap(
            &pixelmap,
            desired_size,
            request.quality,
            &request.output_path,
        ) {
            Ok(size) => size,
            Err(primary_err) => {
                match encode_thumbnail_with_rust_jpeg(
                    &pixelmap,
                    desired_size,
                    request.quality,
                    &request.output_path,
                ) {
                    Ok(size) => size,
                    Err(rust_jpeg_err) => {
                        return Err(PlatformError::Platform(format!(
                            "extractVideoThumbnail failed; pack(native): {}; jpeg(rust): {}",
                            primary_err, rust_jpeg_err
                        )));
                    }
                }
            }
        };

        Ok(VideoThumbnail {
            path: request.output_path.clone(),
            width: final_width,
            height: final_height,
            mime_type: Some(JPEG_MIME.to_string()),
        })
    }

    fn pack_thumbnail_with_pixelmap(
        pixelmap: &NativePixelMap,
        desired_size: Option<ImageSize>,
        quality: u8,
        output_path: &Path,
    ) -> Result<(u32, u32), PlatformError> {
        let (source_width, source_height) = pixelmap.dimensions()?;
        let mut scaled_pixelmap = None;
        if let Some(size) = desired_size {
            if size.width != source_width || size.height != source_height {
                let scale_x = size.width as f32 / source_width as f32;
                let scale_y = size.height as f32 / source_height as f32;
                scaled_pixelmap = Some(pixelmap.create_scaled(scale_x, scale_y)?);
            }
        }
        let pixelmap_for_pack = scaled_pixelmap.as_ref().unwrap_or(pixelmap);
        let (final_width, final_height) = pixelmap_for_pack.dimensions()?;

        let packer = ImagePacker::new()?;
        let mut packing_options = PackingOptions::new()?;
        packing_options.set_mime_type(JPEG_MIME)?;
        packing_options.set_quality(u32::from(quality.clamp(0, 100)))?;

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                PlatformError::Platform(format!(
                    "Failed to prepare directory {}: {}",
                    parent.display(),
                    err
                ))
            })?;
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(output_path)
            .map_err(|err| {
                PlatformError::Platform(format!(
                    "Failed to open {}: {}",
                    output_path.display(),
                    err
                ))
            })?;

        if let Err(err) =
            packer.pack_to_file(&pixelmap_for_pack, &packing_options, file.as_raw_fd())
        {
            drop(file);
            let _ = fs::remove_file(output_path);
            return Err(err);
        }

        Ok((final_width, final_height))
    }

    fn encode_thumbnail_with_rust_jpeg(
        pixelmap: &NativePixelMap,
        desired_size: Option<ImageSize>,
        quality: u8,
        output_path: &Path,
    ) -> Result<(u32, u32), PlatformError> {
        let (source_width, source_height) = pixelmap.dimensions()?;
        // Use OH_PixelmapNative_GetArgbPixels so Harmony handles source format
        // conversion (e.g. NV12/YUV -> ARGB), then normalize to RGBA.
        let source_argb = pixelmap.read_pixels_argb()?;
        let source_rgba = argb_to_rgba(&source_argb);
        let source_image =
            ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(source_width, source_height, source_rgba)
                .ok_or_else(|| {
                    PlatformError::Platform(
                        "Failed to create RGBA image buffer for thumbnail encoding".to_string(),
                    )
                })?;

        let final_image = if let Some(size) = desired_size {
            if size.width != source_width || size.height != source_height {
                image::imageops::resize(
                    &source_image,
                    size.width,
                    size.height,
                    FilterType::Triangle,
                )
            } else {
                source_image
            }
        } else {
            source_image
        };
        let final_width = final_image.width();
        let final_height = final_image.height();
        let final_rgb = rgba_to_rgb(final_image.as_raw());

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                PlatformError::Platform(format!(
                    "Failed to prepare directory {}: {}",
                    parent.display(),
                    err
                ))
            })?;
        }

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(output_path)
            .map_err(|err| {
                PlatformError::Platform(format!(
                    "Failed to open {}: {}",
                    output_path.display(),
                    err
                ))
            })?;

        let clamped_quality = quality.clamp(1, 100);
        let mut encoder = JpegEncoder::new_with_quality(&mut file, clamped_quality);
        encoder
            .encode(
                &final_rgb,
                final_width,
                final_height,
                ExtendedColorType::Rgb8,
            )
            .map_err(|err| {
                PlatformError::Platform(format!(
                    "Rust JPEG encoder failed for {}: {}",
                    output_path.display(),
                    err
                ))
            })?;

        Ok((final_width, final_height))
    }

    fn argb_to_rgba(argb: &[u8]) -> Vec<u8> {
        let mut rgba = Vec::with_capacity(argb.len());
        for px in argb.chunks_exact(4) {
            rgba.push(px[1]);
            rgba.push(px[2]);
            rgba.push(px[3]);
            rgba.push(px[0]);
        }
        rgba
    }

    fn rgba_to_rgb(rgba: &[u8]) -> Vec<u8> {
        let mut rgb = Vec::with_capacity((rgba.len() / 4) * 3);
        for px in rgba.chunks_exact(4) {
            rgb.push(px[0]);
            rgb.push(px[1]);
            rgb.push(px[2]);
        }
        rgb
    }

    struct NativeMetadataExtractor {
        handle: *mut OH_AVMetadataExtractor,
        source_fd: Option<i32>,
    }

    impl NativeMetadataExtractor {
        fn new() -> Result<Self, PlatformError> {
            let handle = unsafe { OH_AVMetadataExtractor_Create() };
            if handle.is_null() {
                return Err(PlatformError::Platform(
                    "OH_AVMetadataExtractor_Create returned null".to_string(),
                ));
            }
            Ok(Self {
                handle,
                source_fd: None,
            })
        }

        fn set_file_source(&mut self, path: &str) -> Result<(), PlatformError> {
            let (fd, size) = open_file_descriptor(path)?;
            if let Err(err) = check_av(
                unsafe { OH_AVMetadataExtractor_SetFDSource(self.handle, fd, 0, size) },
                "OH_AVMetadataExtractor_SetFDSource",
            ) {
                unsafe { libc::close(fd) };
                return Err(err);
            }
            if let Some(existing) = self.source_fd.take() {
                unsafe { libc::close(existing) };
            }
            self.source_fd = Some(fd);
            Ok(())
        }

        fn fetch_metadata(&self, format: *mut OH_AVFormat) -> Result<(), PlatformError> {
            check_av(
                unsafe { OH_AVMetadataExtractor_FetchMetadata(self.handle, format) },
                "OH_AVMetadataExtractor_FetchMetadata",
            )
        }
    }

    impl Drop for NativeMetadataExtractor {
        fn drop(&mut self) {
            if !self.handle.is_null() {
                unsafe {
                    let _ = OH_AVMetadataExtractor_Release(self.handle);
                }
                self.handle = ptr::null_mut();
            }
            if let Some(fd) = self.source_fd.take() {
                unsafe { libc::close(fd) };
            }
        }
    }

    struct NativeAvFormat(*mut OH_AVFormat);

    impl NativeAvFormat {
        fn new() -> Result<Self, PlatformError> {
            let handle = unsafe { OH_AVFormat_Create() };
            if handle.is_null() {
                return Err(PlatformError::Platform(
                    "OH_AVFormat_Create returned null".to_string(),
                ));
            }
            Ok(Self(handle))
        }

        fn as_ptr(&self) -> *mut OH_AVFormat {
            self.0
        }

        fn get_int(&self, key: *const c_char) -> Option<i32> {
            let mut value = 0i32;
            let ok = unsafe { OH_AVFormat_GetIntValue(self.0, key, &mut value) };
            if ok { Some(value) } else { None }
        }

        fn get_long(&self, key: *const c_char) -> Option<i64> {
            let mut value = 0i64;
            let ok = unsafe { OH_AVFormat_GetLongValue(self.0, key, &mut value) };
            if ok { Some(value) } else { None }
        }

        fn get_float(&self, key: *const c_char) -> Option<f32> {
            let mut value = 0f32;
            let ok = unsafe { OH_AVFormat_GetFloatValue(self.0, key, &mut value) };
            if ok { Some(value) } else { None }
        }

        fn get_double(&self, key: *const c_char) -> Option<f64> {
            let mut value = 0f64;
            let ok = unsafe { OH_AVFormat_GetDoubleValue(self.0, key, &mut value) };
            if ok { Some(value) } else { None }
        }

        fn get_string(&self, key: *const c_char) -> Option<String> {
            let mut value = ptr::null();
            let ok = unsafe { OH_AVFormat_GetStringValue(self.0, key, &mut value) };
            if !ok || value.is_null() {
                return None;
            }
            let value = unsafe { CStr::from_ptr(value) }
                .to_string_lossy()
                .trim()
                .to_string();
            if value.is_empty() { None } else { Some(value) }
        }

        fn get_int_compatible(&self, key: *const c_char) -> Option<i32> {
            self.get_int(key)
                .or_else(|| self.get_long(key).map(|v| v as i32))
                .or_else(|| self.get_string(key).and_then(|v| v.parse::<i32>().ok()))
        }

        fn get_long_compatible(&self, key: *const c_char) -> Option<i64> {
            self.get_long(key)
                .or_else(|| self.get_int(key).map(i64::from))
                .or_else(|| self.get_string(key).and_then(|v| v.parse::<i64>().ok()))
        }

        fn get_double_compatible(&self, key: *const c_char) -> Option<f64> {
            self.get_double(key)
                .or_else(|| self.get_float(key).map(f64::from))
                .or_else(|| self.get_int(key).map(f64::from))
                .or_else(|| self.get_long(key).map(|v| v as f64))
                .or_else(|| self.get_string(key).and_then(|v| v.parse::<f64>().ok()))
        }
    }

    impl Drop for NativeAvFormat {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    OH_AVFormat_Destroy(self.0);
                }
            }
        }
    }

    struct NativeAvPlayer {
        handle: *mut OH_AVPlayer,
        source_fd: Option<i32>,
    }

    impl NativeAvPlayer {
        fn new() -> Result<Self, PlatformError> {
            let handle = unsafe { OH_AVPlayer_Create() };
            if handle.is_null() {
                return Err(PlatformError::Platform(
                    "OH_AVPlayer_Create returned null".to_string(),
                ));
            }
            Ok(Self {
                handle,
                source_fd: None,
            })
        }

        fn set_file_source(&mut self, path: &str) -> Result<(), PlatformError> {
            let (fd, size) = open_file_descriptor(path)?;

            if let Err(err) = check_av(
                unsafe { OH_AVPlayer_SetFDSource(self.handle, fd, 0, size) },
                "OH_AVPlayer_SetFDSource",
            ) {
                unsafe { libc::close(fd) };
                return Err(err);
            }

            if let Some(existing) = self.source_fd.take() {
                unsafe { libc::close(existing) };
            }
            self.source_fd = Some(fd);
            Ok(())
        }

        fn prepare(&self) -> Result<(), PlatformError> {
            check_av(
                unsafe { OH_AVPlayer_Prepare(self.handle) },
                "OH_AVPlayer_Prepare",
            )
        }

        fn get_video_size(&self) -> Result<(i32, i32), PlatformError> {
            let mut width = 0i32;
            let mut height = 0i32;
            check_av(
                unsafe { OH_AVPlayer_GetVideoWidth(self.handle, &mut width) },
                "OH_AVPlayer_GetVideoWidth",
            )?;
            check_av(
                unsafe { OH_AVPlayer_GetVideoHeight(self.handle, &mut height) },
                "OH_AVPlayer_GetVideoHeight",
            )?;
            Ok((width, height))
        }

        fn get_duration(&self) -> Result<i32, PlatformError> {
            let mut duration = 0i32;
            check_av(
                unsafe { OH_AVPlayer_GetDuration(self.handle, &mut duration) },
                "OH_AVPlayer_GetDuration",
            )?;
            Ok(duration)
        }

        fn release(&mut self) -> Result<(), PlatformError> {
            if !self.handle.is_null() {
                check_av(
                    unsafe { OH_AVPlayer_Release(self.handle) },
                    "OH_AVPlayer_Release",
                )?;
                self.handle = ptr::null_mut();
            }
            if let Some(fd) = self.source_fd.take() {
                unsafe { libc::close(fd) };
            }
            Ok(())
        }
    }

    impl Drop for NativeAvPlayer {
        fn drop(&mut self) {
            if !self.handle.is_null() {
                unsafe {
                    let _ = OH_AVPlayer_Release(self.handle);
                }
                self.handle = ptr::null_mut();
            }
            if let Some(fd) = self.source_fd.take() {
                unsafe { libc::close(fd) };
            }
        }
    }

    struct NativeAvImageGenerator {
        handle: *mut OH_AVImageGenerator,
        source_fd: Option<i32>,
    }

    impl NativeAvImageGenerator {
        fn new() -> Result<Self, PlatformError> {
            let handle = unsafe { OH_AVImageGenerator_Create() };
            if handle.is_null() {
                return Err(PlatformError::Platform(
                    "OH_AVImageGenerator_Create returned null".to_string(),
                ));
            }
            Ok(Self {
                handle,
                source_fd: None,
            })
        }

        fn set_file_source(&mut self, path: &str) -> Result<(), PlatformError> {
            let (fd, size) = open_file_descriptor(path)?;
            if let Err(err) = check_av(
                unsafe { OH_AVImageGenerator_SetFDSource(self.handle, fd, 0, size) },
                "OH_AVImageGenerator_SetFDSource",
            ) {
                unsafe { libc::close(fd) };
                return Err(err);
            }

            if let Some(existing) = self.source_fd.take() {
                unsafe { libc::close(existing) };
            }
            self.source_fd = Some(fd);
            Ok(())
        }

        fn fetch_frame_by_time(&self, time_ms: u64) -> Result<NativePixelMap, PlatformError> {
            let time_us = time_ms.saturating_mul(1000) as i64;
            let mut pixelmap = ptr::null_mut();
            let primary = unsafe {
                OH_AVImageGenerator_FetchFrameByTime(
                    self.handle,
                    time_us,
                    OhAvImageGeneratorQueryOptions::Closest,
                    &mut pixelmap,
                )
            };
            if primary != AV_SUCCESS || pixelmap.is_null() {
                pixelmap = ptr::null_mut();
                check_av(
                    unsafe {
                        OH_AVImageGenerator_FetchFrameByTime(
                            self.handle,
                            time_us,
                            OhAvImageGeneratorQueryOptions::ClosestSync,
                            &mut pixelmap,
                        )
                    },
                    "OH_AVImageGenerator_FetchFrameByTime",
                )?;
            }

            if pixelmap.is_null() {
                return Err(PlatformError::Platform(
                    "Fetched pixelmap is null".to_string(),
                ));
            }
            Ok(NativePixelMap(pixelmap))
        }
    }

    impl Drop for NativeAvImageGenerator {
        fn drop(&mut self) {
            if !self.handle.is_null() {
                unsafe {
                    let _ = OH_AVImageGenerator_Release(self.handle);
                }
                self.handle = ptr::null_mut();
            }
            if let Some(fd) = self.source_fd.take() {
                unsafe { libc::close(fd) };
            }
        }
    }

    struct NativePixelMapInfo(*mut OH_Pixelmap_ImageInfo);

    impl NativePixelMapInfo {
        fn new() -> Result<Self, PlatformError> {
            let mut ptr = ptr::null_mut();
            check_image(
                unsafe { OH_PixelmapImageInfo_Create(&mut ptr) },
                "OH_PixelmapImageInfo_Create(video)",
            )?;
            if ptr.is_null() {
                return Err(PlatformError::Platform(
                    "PixelmapImageInfo handle is null".to_string(),
                ));
            }
            Ok(Self(ptr))
        }

        fn as_ptr(&self) -> *mut OH_Pixelmap_ImageInfo {
            self.0
        }

        fn width(&self) -> Result<u32, PlatformError> {
            let mut width = 0;
            check_image(
                unsafe { OH_PixelmapImageInfo_GetWidth(self.0, &mut width) },
                "OH_PixelmapImageInfo_GetWidth(video)",
            )?;
            Ok(width)
        }

        fn height(&self) -> Result<u32, PlatformError> {
            let mut height = 0;
            check_image(
                unsafe { OH_PixelmapImageInfo_GetHeight(self.0, &mut height) },
                "OH_PixelmapImageInfo_GetHeight(video)",
            )?;
            Ok(height)
        }

        fn row_stride(&self) -> Result<u32, PlatformError> {
            let mut row_stride = 0;
            check_image(
                unsafe { OH_PixelmapImageInfo_GetRowStride(self.0, &mut row_stride) },
                "OH_PixelmapImageInfo_GetRowStride(video)",
            )?;
            Ok(row_stride)
        }
    }

    impl Drop for NativePixelMapInfo {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    OH_PixelmapImageInfo_Release(self.0);
                }
            }
        }
    }

    struct NativePixelMap(*mut OH_PixelmapNative);

    impl NativePixelMap {
        fn as_ptr(&self) -> *mut OH_PixelmapNative {
            self.0
        }

        fn dimensions(&self) -> Result<(u32, u32), PlatformError> {
            let info = NativePixelMapInfo::new()?;
            check_image(
                unsafe { OH_PixelmapNative_GetImageInfo(self.0, info.as_ptr()) },
                "OH_PixelmapNative_GetImageInfo(video)",
            )?;
            Ok((info.width()?, info.height()?))
        }

        fn create_scaled(&self, scale_x: f32, scale_y: f32) -> Result<Self, PlatformError> {
            let mut dst = ptr::null_mut();
            check_image(
                unsafe {
                    OH_PixelmapNative_CreateScaledPixelMap(self.0, &mut dst, scale_x, scale_y)
                },
                "OH_PixelmapNative_CreateScaledPixelMap(video)",
            )?;
            if dst.is_null() {
                return Err(PlatformError::Platform(
                    "Scaled pixelmap handle is null".to_string(),
                ));
            }
            Ok(Self(dst))
        }

        fn read_pixels_argb(&self) -> Result<Vec<u8>, PlatformError> {
            let info = NativePixelMapInfo::new()?;
            check_image(
                unsafe { OH_PixelmapNative_GetImageInfo(self.0, info.as_ptr()) },
                "OH_PixelmapNative_GetImageInfo(video)",
            )?;
            let width = info.width()? as usize;
            let height = info.height()? as usize;
            let row_stride = info.row_stride()? as usize;
            let tight_stride = width.checked_mul(4).ok_or_else(|| {
                PlatformError::Platform("Pixelmap width is too large".to_string())
            })?;
            let tight_size = tight_stride.checked_mul(height).ok_or_else(|| {
                PlatformError::Platform("Pixelmap dimensions are too large".to_string())
            })?;
            if tight_size == 0 {
                return Err(PlatformError::Platform(
                    "Cannot read pixels from 0-sized pixelmap".to_string(),
                ));
            }

            // Some implementations may align rows. Start with a conservative
            // capacity and then pack down to tight ARGB rows.
            let alloc_stride = tight_stride.max(row_stride);
            let alloc_size = alloc_stride.checked_mul(height).ok_or_else(|| {
                PlatformError::Platform("Pixel buffer allocation size overflow".to_string())
            })?;
            let mut buf = vec![0u8; alloc_size];
            let mut actual_size = alloc_size;
            check_image(
                unsafe {
                    OH_PixelmapNative_GetArgbPixels(self.0, buf.as_mut_ptr(), &mut actual_size)
                },
                "OH_PixelmapNative_GetArgbPixels(video)",
            )?;

            if actual_size < tight_size {
                return Err(PlatformError::Platform(format!(
                    "OH_PixelmapNative_GetArgbPixels(video) returned truncated buffer: actual={}, expected_at_least={}",
                    actual_size, tight_size
                )));
            }

            buf.truncate(actual_size);
            let inferred_stride = actual_size / height;
            if inferred_stride >= tight_stride && inferred_stride * height == actual_size {
                if inferred_stride == tight_stride {
                    return Ok(buf);
                }
                let mut packed = vec![0u8; tight_size];
                for row in 0..height {
                    let src_start = row * inferred_stride;
                    let src_end = src_start + tight_stride;
                    let dst_start = row * tight_stride;
                    let dst_end = dst_start + tight_stride;
                    packed[dst_start..dst_end].copy_from_slice(&buf[src_start..src_end]);
                }
                return Ok(packed);
            }

            Ok(buf[..tight_size].to_vec())
        }
    }

    impl Drop for NativePixelMap {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    OH_PixelmapNative_Release(self.0);
                }
            }
        }
    }

    struct ImagePacker(*mut OH_ImagePackerNative);

    impl ImagePacker {
        fn new() -> Result<Self, PlatformError> {
            let mut ptr = ptr::null_mut();
            check_image(
                unsafe { OH_ImagePackerNative_Create(&mut ptr) },
                "OH_ImagePackerNative_Create(video)",
            )?;
            if ptr.is_null() {
                return Err(PlatformError::Platform(
                    "ImagePacker handle is null".to_string(),
                ));
            }
            Ok(ImagePacker(ptr))
        }

        fn pack_to_file(
            &self,
            pixelmap: &NativePixelMap,
            options: &PackingOptions,
            fd: RawFd,
        ) -> Result<(), PlatformError> {
            check_image(
                unsafe {
                    OH_ImagePackerNative_PackToFileFromPixelmap(
                        self.0,
                        options.as_ptr(),
                        pixelmap.as_ptr(),
                        fd,
                    )
                },
                "OH_ImagePackerNative_PackToFileFromPixelmap(video)",
            )
        }
    }

    impl Drop for ImagePacker {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    OH_ImagePackerNative_Release(self.0);
                }
            }
        }
    }

    struct PackingOptions {
        handle: *mut OH_PackingOptions,
        // Keep the mime type C string alive in case the platform stores the pointer.
        mime_type: Option<CString>,
    }

    impl PackingOptions {
        fn new() -> Result<Self, PlatformError> {
            let mut ptr = ptr::null_mut();
            check_image(
                unsafe { OH_PackingOptions_Create(&mut ptr) },
                "OH_PackingOptions_Create(video)",
            )?;
            if ptr.is_null() {
                return Err(PlatformError::Platform(
                    "PackingOptions handle is null".to_string(),
                ));
            }
            Ok(PackingOptions {
                handle: ptr,
                mime_type: None,
            })
        }

        fn as_ptr(&self) -> *mut OH_PackingOptions {
            self.handle
        }

        fn set_quality(&self, quality: u32) -> Result<(), PlatformError> {
            check_image(
                unsafe { OH_PackingOptions_SetQuality(self.handle, quality) },
                "OH_PackingOptions_SetQuality(video)",
            )
        }

        fn set_mime_type(&mut self, mime: &str) -> Result<(), PlatformError> {
            let c_mime = CString::new(mime).map_err(|_| {
                PlatformError::Platform("Mime type contains invalid characters".to_string())
            })?;
            let mut mime_string = ImageString {
                data: c_mime.as_ptr() as *mut c_char,
                size: c_mime.as_bytes_with_nul().len(),
            };
            check_image(
                unsafe { OH_PackingOptions_SetMimeType(self.handle, &mut mime_string) },
                "OH_PackingOptions_SetMimeType(video)",
            )?;
            self.mime_type = Some(c_mime);
            Ok(())
        }
    }

    impl Drop for PackingOptions {
        fn drop(&mut self) {
            if !self.handle.is_null() {
                unsafe {
                    OH_PackingOptions_Release(self.handle);
                }
            }
        }
    }

    fn compute_desired_size(
        original_width: u32,
        original_height: u32,
        requested_width: Option<u32>,
        requested_height: Option<u32>,
    ) -> Option<ImageSize> {
        if original_width == 0 || original_height == 0 {
            return None;
        }

        let width_opt = requested_width.filter(|w| *w > 0);
        let height_opt = requested_height.filter(|h| *h > 0);

        match (width_opt, height_opt) {
            (Some(w), Some(h)) => {
                let width_ratio = w as f64 / original_width as f64;
                let height_ratio = h as f64 / original_height as f64;
                let ratio = width_ratio.min(height_ratio);
                if ratio >= 1.0 {
                    None
                } else {
                    Some(ImageSize {
                        width: (original_width as f64 * ratio).round().max(1.0) as u32,
                        height: (original_height as f64 * ratio).round().max(1.0) as u32,
                    })
                }
            }
            (Some(w), None) => {
                if w >= original_width {
                    None
                } else {
                    let ratio = w as f64 / original_width as f64;
                    Some(ImageSize {
                        width: w,
                        height: (original_height as f64 * ratio).round().max(1.0) as u32,
                    })
                }
            }
            (None, Some(h)) => {
                if h >= original_height {
                    None
                } else {
                    let ratio = h as f64 / original_height as f64;
                    Some(ImageSize {
                        width: (original_width as f64 * ratio).round().max(1.0) as u32,
                        height: h,
                    })
                }
            }
            (None, None) => None,
        }
    }

    fn normalize_to_path(uri: &str) -> Result<String, PlatformError> {
        let trimmed = uri.trim();
        if trimmed.is_empty() {
            return Err(PlatformError::Platform("URI is empty".to_string()));
        }
        if let Some(path) = trimmed.strip_prefix("file://") {
            return Ok(path.to_string());
        }
        if trimmed.contains("://") {
            return Err(PlatformError::Platform(format!(
                "Unsupported URI scheme: {}",
                trimmed
            )));
        }
        Ok(trimmed.to_string())
    }

    fn open_file_descriptor(path: &str) -> Result<(i32, i64), PlatformError> {
        let c_path = CString::new(path).map_err(|_| {
            PlatformError::Platform("Video path contains invalid characters".to_string())
        })?;

        let fd = unsafe { libc::open(c_path.as_ptr(), libc::O_RDONLY) };
        if fd < 0 {
            return Err(PlatformError::Platform(format!(
                "Failed to open video file: {}",
                path
            )));
        }

        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstat(fd, &mut stat) } < 0 {
            unsafe { libc::close(fd) };
            return Err(PlatformError::Platform(format!(
                "Failed to stat video file: {}",
                path
            )));
        }

        Ok((fd, stat.st_size))
    }

    fn open_output_descriptor(path: &Path) -> Result<i32, PlatformError> {
        let path_str = path.to_str().ok_or_else(|| {
            PlatformError::Platform(format!(
                "Output path contains invalid UTF-8: {}",
                path.display()
            ))
        })?;
        let c_path = CString::new(path_str)
            .map_err(|_| PlatformError::Platform("Output path contains null byte".to_string()))?;

        let fd = unsafe {
            libc::open(
                c_path.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC,
                0o644,
            )
        };
        if fd < 0 {
            return Err(PlatformError::Platform(format!(
                "Failed to create output file: {}",
                path.display()
            )));
        }
        Ok(fd)
    }

    fn infer_video_mime_type(path: &str) -> Option<String> {
        let ext = Path::new(path)
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let mime = match ext.as_str() {
            "mp4" | "m4v" => "video/mp4",
            "mov" => "video/quicktime",
            "webm" => "video/webm",
            "mkv" => "video/x-matroska",
            "avi" => "video/x-msvideo",
            "3gp" | "3gpp" => "video/3gpp",
            _ => return None,
        };
        Some(mime.to_string())
    }

    fn check_av(code: i32, context: &str) -> Result<(), PlatformError> {
        if code == AV_SUCCESS {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "{} failed: code {}",
                context, code
            )))
        }
    }

    fn check_image(code: i32, context: &str) -> Result<(), PlatformError> {
        if code == IMAGE_SUCCESS {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "{} failed: code {}",
                context, code
            )))
        }
    }

    #[link(name = "avmetadata_extractor")]
    unsafe extern "C" {
        fn OH_AVMetadataExtractor_Create() -> *mut OH_AVMetadataExtractor;
        fn OH_AVMetadataExtractor_SetFDSource(
            extractor: *mut OH_AVMetadataExtractor,
            fd: i32,
            offset: i64,
            size: i64,
        ) -> i32;
        fn OH_AVMetadataExtractor_FetchMetadata(
            extractor: *mut OH_AVMetadataExtractor,
            metadata: *mut OH_AVFormat,
        ) -> i32;
        fn OH_AVMetadataExtractor_Release(extractor: *mut OH_AVMetadataExtractor) -> i32;
    }

    #[link(name = "native_media_core")]
    unsafe extern "C" {
        fn OH_AVFormat_Create() -> *mut OH_AVFormat;
        fn OH_AVFormat_Destroy(format: *mut OH_AVFormat);
        fn OH_AVFormat_GetIntValue(
            format: *mut OH_AVFormat,
            key: *const c_char,
            out: *mut i32,
        ) -> bool;
        fn OH_AVFormat_GetLongValue(
            format: *mut OH_AVFormat,
            key: *const c_char,
            out: *mut i64,
        ) -> bool;
        fn OH_AVFormat_GetFloatValue(
            format: *mut OH_AVFormat,
            key: *const c_char,
            out: *mut f32,
        ) -> bool;
        fn OH_AVFormat_GetDoubleValue(
            format: *mut OH_AVFormat,
            key: *const c_char,
            out: *mut f64,
        ) -> bool;
        fn OH_AVFormat_GetStringValue(
            format: *mut OH_AVFormat,
            key: *const c_char,
            out: *mut *const c_char,
        ) -> bool;
    }

    #[link(name = "native_media_codecbase")]
    unsafe extern "C" {
        static OH_MD_KEY_CODEC_MIME: *const c_char;
        static OH_MD_KEY_DURATION: *const c_char;
        static OH_MD_KEY_BITRATE: *const c_char;
        static OH_MD_KEY_WIDTH: *const c_char;
        static OH_MD_KEY_HEIGHT: *const c_char;
        static OH_MD_KEY_FRAME_RATE: *const c_char;
        static OH_MD_KEY_ROTATION: *const c_char;
        static OH_MD_KEY_AUD_SAMPLE_RATE: *const c_char;
    }

    #[link(name = "avtranscoder")]
    unsafe extern "C" {
        fn OH_AVTranscoderConfig_Create() -> *mut OH_AVTranscoder_Config;
        fn OH_AVTranscoderConfig_Release(config: *mut OH_AVTranscoder_Config) -> i32;
        fn OH_AVTranscoderConfig_SetSrcFD(
            config: *mut OH_AVTranscoder_Config,
            src_fd: i32,
            src_offset: i64,
            length: i64,
        ) -> i32;
        fn OH_AVTranscoderConfig_SetDstFD(config: *mut OH_AVTranscoder_Config, dst_fd: i32) -> i32;
        fn OH_AVTranscoderConfig_SetDstVideoType(
            config: *mut OH_AVTranscoder_Config,
            mime_type: *const c_char,
        ) -> i32;
        fn OH_AVTranscoderConfig_SetDstAudioType(
            config: *mut OH_AVTranscoder_Config,
            mime_type: *const c_char,
        ) -> i32;
        fn OH_AVTranscoderConfig_SetDstFileType(
            config: *mut OH_AVTranscoder_Config,
            mime_type: i32,
        ) -> i32;
        fn OH_AVTranscoderConfig_SetDstAudioBitrate(
            config: *mut OH_AVTranscoder_Config,
            bitrate: i32,
        ) -> i32;
        fn OH_AVTranscoderConfig_SetDstVideoBitrate(
            config: *mut OH_AVTranscoder_Config,
            bitrate: i32,
        ) -> i32;
        fn OH_AVTranscoderConfig_SetDstVideoResolution(
            config: *mut OH_AVTranscoder_Config,
            width: i32,
            height: i32,
        ) -> i32;
        fn OH_AVTranscoder_Create() -> *mut OH_AVTranscoder;
        fn OH_AVTranscoder_Prepare(
            transcoder: *mut OH_AVTranscoder,
            config: *mut OH_AVTranscoder_Config,
        ) -> i32;
        fn OH_AVTranscoder_Start(transcoder: *mut OH_AVTranscoder) -> i32;
        fn OH_AVTranscoder_Cancel(transcoder: *mut OH_AVTranscoder) -> i32;
        fn OH_AVTranscoder_Release(transcoder: *mut OH_AVTranscoder) -> i32;
        fn OH_AVTranscoder_SetStateCallback(
            transcoder: *mut OH_AVTranscoder,
            callback: Option<
                unsafe extern "C" fn(
                    transcoder: *mut OH_AVTranscoder,
                    state: i32,
                    user_data: *mut c_void,
                ),
            >,
            user_data: *mut c_void,
        ) -> i32;
        fn OH_AVTranscoder_SetErrorCallback(
            transcoder: *mut OH_AVTranscoder,
            callback: Option<
                unsafe extern "C" fn(
                    transcoder: *mut OH_AVTranscoder,
                    error_code: i32,
                    error_msg: *const c_char,
                    user_data: *mut c_void,
                ),
            >,
            user_data: *mut c_void,
        ) -> i32;
        fn OH_AVTranscoder_SetProgressUpdateCallback(
            transcoder: *mut OH_AVTranscoder,
            callback: Option<
                unsafe extern "C" fn(
                    transcoder: *mut OH_AVTranscoder,
                    progress: i32,
                    user_data: *mut c_void,
                ),
            >,
            user_data: *mut c_void,
        ) -> i32;
    }

    unsafe extern "C" {
        fn OH_AVPlayer_Create() -> *mut OH_AVPlayer;
        fn OH_AVPlayer_SetFDSource(
            player: *mut OH_AVPlayer,
            fd: i32,
            offset: i64,
            size: i64,
        ) -> i32;
        fn OH_AVPlayer_Prepare(player: *mut OH_AVPlayer) -> i32;
        fn OH_AVPlayer_GetDuration(player: *mut OH_AVPlayer, duration: *mut i32) -> i32;
        fn OH_AVPlayer_GetVideoWidth(player: *mut OH_AVPlayer, width: *mut i32) -> i32;
        fn OH_AVPlayer_GetVideoHeight(player: *mut OH_AVPlayer, height: *mut i32) -> i32;
        fn OH_AVPlayer_Release(player: *mut OH_AVPlayer) -> i32;
    }

    #[link(name = "avimage_generator")]
    unsafe extern "C" {
        fn OH_AVImageGenerator_Create() -> *mut OH_AVImageGenerator;
        fn OH_AVImageGenerator_SetFDSource(
            generator: *mut OH_AVImageGenerator,
            fd: i32,
            offset: i64,
            size: i64,
        ) -> i32;
        fn OH_AVImageGenerator_FetchFrameByTime(
            generator: *mut OH_AVImageGenerator,
            time_us: i64,
            options: OhAvImageGeneratorQueryOptions,
            pixel_map: *mut *mut OH_PixelmapNative,
        ) -> i32;
        fn OH_AVImageGenerator_Release(generator: *mut OH_AVImageGenerator) -> i32;
    }

    #[link(name = "pixelmap")]
    unsafe extern "C" {
        fn OH_PixelmapImageInfo_Create(info: *mut *mut OH_Pixelmap_ImageInfo) -> i32;
        fn OH_PixelmapImageInfo_GetWidth(info: *mut OH_Pixelmap_ImageInfo, width: *mut u32) -> i32;
        fn OH_PixelmapImageInfo_GetHeight(
            info: *mut OH_Pixelmap_ImageInfo,
            height: *mut u32,
        ) -> i32;
        fn OH_PixelmapImageInfo_GetRowStride(
            info: *mut OH_Pixelmap_ImageInfo,
            row_stride: *mut u32,
        ) -> i32;
        fn OH_PixelmapImageInfo_Release(info: *mut OH_Pixelmap_ImageInfo) -> i32;
        fn OH_PixelmapNative_GetImageInfo(
            pixelmap: *mut OH_PixelmapNative,
            image_info: *mut OH_Pixelmap_ImageInfo,
        ) -> i32;
        fn OH_PixelmapNative_CreateScaledPixelMap(
            src_pixelmap: *mut OH_PixelmapNative,
            dst_pixelmap: *mut *mut OH_PixelmapNative,
            scale_x: f32,
            scale_y: f32,
        ) -> i32;
        fn OH_PixelmapNative_Release(pixelmap: *mut OH_PixelmapNative) -> i32;
        fn OH_PixelmapNative_GetArgbPixels(
            pixelmap: *mut OH_PixelmapNative,
            destination: *mut u8,
            buffer_size: *mut usize,
        ) -> i32;
    }

    #[link(name = "image_packer")]
    unsafe extern "C" {
        fn OH_ImagePackerNative_Create(packer: *mut *mut OH_ImagePackerNative) -> i32;
        fn OH_ImagePackerNative_PackToFileFromPixelmap(
            packer: *mut OH_ImagePackerNative,
            options: *mut OH_PackingOptions,
            pixelmap: *mut OH_PixelmapNative,
            fd: i32,
        ) -> i32;
        fn OH_ImagePackerNative_Release(packer: *mut OH_ImagePackerNative) -> i32;

        fn OH_PackingOptions_Create(options: *mut *mut OH_PackingOptions) -> i32;
        fn OH_PackingOptions_SetMimeType(
            options: *mut OH_PackingOptions,
            format: *mut ImageString,
        ) -> i32;
        fn OH_PackingOptions_SetQuality(options: *mut OH_PackingOptions, quality: u32) -> i32;
        fn OH_PackingOptions_Release(options: *mut OH_PackingOptions) -> i32;
    }
}
