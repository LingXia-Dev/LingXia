use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::{
    ChooseMediaMode, ChooseMediaRequest, CompressImageRequest, ImageInfo, MediaInteraction,
    MediaKind, MediaRuntime, MediaSource, PreviewMediaRequest, SaveMediaRequest, ScanCodeRequest,
    ScanType,
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
                crate::traits::CameraFacing::Front => "front".to_string(),
                crate::traits::CameraFacing::Back => "back".to_string(),
            }),
            ..payload
        };

        let payload_json = serde_json::to_string(&payload).map_err(|e| {
            PlatformError::Platform(format!("Failed to serialize chooseMedia payload: {}", e))
        })?;

        lingxia_webview::tsfn::call_arkts("chooseMedia", &[payload_json.as_str()]).map_err(|e| {
            let message = format!("Failed to start chooseMedia flow: {}", e);
            lingxia_messaging::invoke_callback(request.callback_id, false, message.clone());
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
            lingxia_messaging::invoke_callback(request.callback_id, false, message.clone());
            PlatformError::Platform(message)
        })
    }

    fn save_image_to_photos_album(&self, request: SaveMediaRequest) -> Result<(), PlatformError> {
        save_media_resource(&request.file_uri, MEDIA_LIBRARY_IMAGE_RESOURCE)
    }

    fn save_video_to_photos_album(&self, request: SaveMediaRequest) -> Result<(), PlatformError> {
        save_media_resource(&request.file_uri, MEDIA_LIBRARY_VIDEO_RESOURCE)
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
}

fn save_media_resource(file_uri: &str, resource_type: i32) -> Result<(), PlatformError> {
    let media_type_str = resource_type.to_string();
    lingxia_webview::tsfn::call_arkts("saveMedia", &[file_uri, &media_type_str])
        .map_err(|e| PlatformError::Platform(format!("Failed to save media: {}", e)))
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
    use crate::traits::{CompressImageRequest, ImageInfo};
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
                        fd as i32,
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
                size: mime.as_bytes().len(),
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
