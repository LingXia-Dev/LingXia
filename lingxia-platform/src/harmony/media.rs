use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::media_interaction::{
    CameraFacing, ChooseMediaMode, ChooseMediaRequest, MediaInteraction, MediaKind, MediaSource,
    PreviewMediaRequest, SaveMediaRequest, ScanCodeRequest, ScanType,
};
use crate::traits::media_runtime::{
    CompressImageRequest, ExtractVideoThumbnailRequest, ImageInfo, MediaRuntime, VideoInfo,
    VideoThumbnail,
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
            })
            .collect();

        let json = serde_json::to_string(&payloads).map_err(|e| {
            PlatformError::Platform(format!("Failed to serialize preview media payload: {}", e))
        })?;

        let safe_json = json.replace('|', "%7C");

        lingxia_webview::tsfn::call_arkts("previewMedia", &[safe_json.as_str()])
            .map_err(|e| PlatformError::Platform(format!("Failed to preview media: {}", e)))
    }

    fn choose_media(&self, request: ChooseMediaRequest) -> Result<(), PlatformError> {
        if request.max_count == 0 {
            return Err(PlatformError::Platform(
                "chooseMedia requires max_count to be greater than 0".to_string(),
            ));
        }

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
            callback_id: request.callback_id.to_string(),
            max_count: request.max_count,
            mode: mode_str.to_string(),
            allow_album,
            allow_camera: request
                .source_types
                .iter()
                .any(|source| matches!(source, MediaSource::Camera)),
            max_duration_seconds: None,
            camera_facing: None,
        };

        // Attach optional duration and facing
        let payload = ChooseMediaPayload {
            max_duration_seconds: request.max_duration_seconds,
            camera_facing: request.camera_facing.as_ref().map(|f| match f {
                CameraFacing::Front => "front".to_string(),
                CameraFacing::Back => "back".to_string(),
            }),
            ..payload
        };

        let payload_json = serde_json::to_string(&payload).map_err(|e| {
            PlatformError::Platform(format!("Failed to serialize chooseMedia payload: {}", e))
        })?;

        lingxia_webview::tsfn::call_arkts("chooseMedia", &[payload_json.as_str()]).map_err(|e| {
            let message = format!("Failed to start chooseMedia flow: {}", e);
            lingxia_messaging::invoke_callback(request.callback_id, Err(1000));
            PlatformError::Platform(message)
        })
    }

    fn scan_code(&self, request: ScanCodeRequest) -> Result<(), PlatformError> {
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
            callback_id: request.callback_id.to_string(),
        };

        let payload_json = serde_json::to_string(&payload).map_err(|e| {
            PlatformError::Platform(format!("Failed to serialize scanCode payload: {}", e))
        })?;

        lingxia_webview::tsfn::call_arkts("scanCode", &[payload_json.as_str()]).map_err(|e| {
            let message = format!("Failed to start scanCode flow: {}", e);
            lingxia_messaging::invoke_callback(request.callback_id, Err(1000));
            PlatformError::Platform(message)
        })
    }

    fn save_image_to_photos_album(&self, request: SaveMediaRequest) -> Result<(), PlatformError> {
        save_media_resource(
            &request.file_uri,
            MEDIA_LIBRARY_IMAGE_RESOURCE,
            request.callback_id,
        )
    }

    fn save_video_to_photos_album(&self, request: SaveMediaRequest) -> Result<(), PlatformError> {
        save_media_resource(
            &request.file_uri,
            MEDIA_LIBRARY_VIDEO_RESOURCE,
            request.callback_id,
        )
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
}

fn save_media_resource(
    file_uri: &str,
    resource_type: i32,
    callback_id: u64,
) -> Result<(), PlatformError> {
    let safe_file_uri = file_uri.replace('|', "%7C");
    let media_type_str = resource_type.to_string();
    let callback_id_str = callback_id.to_string();
    lingxia_webview::tsfn::call_arkts(
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
    use core::ffi::{c_char, c_void};
    use std::ffi::CString;
    use std::fs::{self, OpenOptions};
    use std::os::fd::{AsRawFd, RawFd};
    use std::ptr;
    use std::slice;

    const IMAGE_SUCCESS: i32 = 0;
    const JPEG_MIME: &str = "image/jpeg";

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
        if request.source_uri.is_empty() {
            return Err(PlatformError::Platform("source_uri is empty".to_string()));
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
        let packing_options = PackingOptions::new()?;
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
            let code = unsafe {
                OH_ImageSourceNative_CreateFromUri(
                    c_uri.as_ptr() as *mut c_char,
                    uri.len(),
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

    struct PackingOptions(*mut OH_PackingOptions);

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
            Ok(PackingOptions(ptr))
        }

        fn as_ptr(&self) -> *mut OH_PackingOptions {
            self.0
        }

        fn set_quality(&self, quality: u32) -> Result<(), PlatformError> {
            check(
                unsafe { OH_PackingOptions_SetQuality(self.0, quality) },
                "OH_PackingOptions_SetQuality",
            )
        }

        fn set_mime_type(&self, mime: &str) -> Result<(), PlatformError> {
            let c_mime = CString::new(mime).map_err(|_| {
                PlatformError::Platform("Mime type contains invalid characters".to_string())
            })?;
            let mut mime_string = ImageString {
                data: c_mime.as_ptr() as *mut c_char,
                size: mime.len(),
            };
            check(
                unsafe { OH_PackingOptions_SetMimeType(self.0, &mut mime_string) },
                "OH_PackingOptions_SetMimeType",
            )
        }
    }

    impl Drop for PackingOptions {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    OH_PackingOptions_Release(self.0);
                }
            }
        }
    }

    fn take_image_string(value: &mut ImageString) -> Option<String> {
        if value.data.is_null() || value.size == 0 {
            return None;
        }

        let len = unsafe {
            let bytes = slice::from_raw_parts(value.data as *const u8, value.size);
            bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len())
        };

        let text = unsafe {
            let bytes = slice::from_raw_parts(value.data as *const u8, len);
            String::from_utf8_lossy(bytes).trim().to_string()
        };

        unsafe { c_free(value.data as *mut c_void) };
        value.data = ptr::null_mut();
        value.size = 0;

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
        #[link_name = "free"]
        fn c_free(ptr: *mut c_void);
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
    use crate::traits::media_runtime::{ExtractVideoThumbnailRequest, VideoInfo, VideoThumbnail};
    use core::ffi::c_char;
    use std::ffi::{CStr, CString};
    use std::fs::{self, OpenOptions};
    use std::os::fd::{AsRawFd, RawFd};
    use std::path::Path;
    use std::ptr;

    const AV_SUCCESS: i32 = 0;
    const IMAGE_SUCCESS: i32 = 0;
    const JPEG_MIME: &str = "image/jpeg";

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
        let final_pixelmap = if let Some(size) = desired_size {
            if size.width != original_width || size.height != original_height {
                let scale_x = size.width as f32 / original_width as f32;
                let scale_y = size.height as f32 / original_height as f32;
                pixelmap.create_scaled(scale_x, scale_y)?
            } else {
                pixelmap
            }
        } else {
            pixelmap
        };
        let (final_width, final_height) = final_pixelmap.dimensions()?;

        let packer = ImagePacker::new()?;
        let packing_options = PackingOptions::new()?;
        packing_options.set_mime_type(JPEG_MIME)?;
        packing_options.set_quality(u32::from(request.quality.clamp(0, 100)))?;

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

        if let Err(err) = packer.pack_to_file(&final_pixelmap, &packing_options, file.as_raw_fd()) {
            drop(file);
            let _ = fs::remove_file(&request.output_path);
            return Err(err);
        }

        Ok(VideoThumbnail {
            path: request.output_path.clone(),
            width: final_width,
            height: final_height,
            mime_type: Some(JPEG_MIME.to_string()),
        })
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

    struct PackingOptions(*mut OH_PackingOptions);

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
            Ok(PackingOptions(ptr))
        }

        fn as_ptr(&self) -> *mut OH_PackingOptions {
            self.0
        }

        fn set_quality(&self, quality: u32) -> Result<(), PlatformError> {
            check_image(
                unsafe { OH_PackingOptions_SetQuality(self.0, quality) },
                "OH_PackingOptions_SetQuality(video)",
            )
        }

        fn set_mime_type(&self, mime: &str) -> Result<(), PlatformError> {
            let c_mime = CString::new(mime).map_err(|_| {
                PlatformError::Platform("Mime type contains invalid characters".to_string())
            })?;
            let mut mime_string = ImageString {
                data: c_mime.as_ptr() as *mut c_char,
                size: mime.len(),
            };
            check_image(
                unsafe { OH_PackingOptions_SetMimeType(self.0, &mut mime_string) },
                "OH_PackingOptions_SetMimeType(video)",
            )
        }
    }

    impl Drop for PackingOptions {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    OH_PackingOptions_Release(self.0);
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
