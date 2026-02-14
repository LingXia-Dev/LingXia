use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::media_interaction::{
    ChooseMediaRequest, MediaInteraction, MediaKind, PreviewMediaRequest, SaveMediaRequest,
    ScanCodeRequest, ScanType,
};
use crate::traits::media_runtime::{
    CompressImageRequest, ExtractVideoThumbnailRequest, ImageInfo, MediaRuntime, VideoInfo,
    VideoThumbnail,
};
use jni::JNIEnv;
use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::{jint, jlong};
use lingxia_webview::get_env;
use serde::Deserialize;
use std::path::{Path, PathBuf};

fn with_jni<T, F>(env: &mut JNIEnv<'static>, f: F) -> Result<T, Box<dyn std::error::Error>>
where
    F: FnOnce(&mut JNIEnv<'static>) -> jni::errors::Result<T>,
{
    match f(env) {
        Ok(value) => Ok(value),
        Err(err) => {
            if let Ok(true) = env.exception_check() {
                let _ = env.exception_clear();
            }
            Err(Box::new(err))
        }
    }
}

impl MediaInteraction for Platform {
    fn preview_media(&self, request: PreviewMediaRequest) -> Result<(), PlatformError> {
        match preview_media_impl(request) {
            Ok(()) => Ok(()),
            Err(e) => Err(PlatformError::Platform(format!(
                "Failed to preview media: {}",
                e
            ))),
        }
    }

    fn choose_media(&self, request: ChooseMediaRequest) -> Result<(), PlatformError> {
        match choose_media_impl(request) {
            Ok(()) => Ok(()),
            Err(e) => Err(PlatformError::Platform(format!(
                "Failed to choose media: {}",
                e
            ))),
        }
    }

    fn scan_code(&self, request: ScanCodeRequest) -> Result<(), PlatformError> {
        match scan_code_impl(request) {
            Ok(()) => Ok(()),
            Err(e) => Err(PlatformError::Platform(format!(
                "Failed to start scanCode: {}",
                e
            ))),
        }
    }

    fn save_image_to_photos_album(&self, request: SaveMediaRequest) -> Result<(), PlatformError> {
        save_media_impl(request, "saveImageToPhotosAlbum")
    }

    fn save_video_to_photos_album(&self, request: SaveMediaRequest) -> Result<(), PlatformError> {
        save_media_impl(request, "saveVideoToPhotosAlbum")
    }
}

fn preview_media_impl(request: PreviewMediaRequest) -> Result<(), Box<dyn std::error::Error>> {
    let mut env = get_env()?;

    let media_class_ref = super::get_cached_class(super::CachedClass::LxAppMedia)?;
    let payload_class_ref = super::get_cached_class(super::CachedClass::PreviewMediaPayload)?;

    let item_count = request.items.len();
    let payload_array = with_jni(&mut env, |env| {
        let class_ref = env.new_local_ref(payload_class_ref.as_obj())?;
        let class = JClass::from(class_ref);
        env.new_object_array(item_count as i32, class, JObject::null())
    })?;

    for (idx, item) in request.items.iter().enumerate() {
        let path_java = with_jni(&mut env, |env| env.new_string(&item.path))?;
        let path_obj: JObject = path_java.into();

        let media_type_value = match item.media_type {
            MediaKind::Image => 0,
            MediaKind::Video => 1,
            MediaKind::Unknown => -1,
        } as jint;

        let cover_obj = match item.cover_path.as_deref().filter(|s| !s.is_empty()) {
            Some(url) => {
                let cover_java: JString = with_jni(&mut env, |env| env.new_string(url))?;
                cover_java.into()
            }
            None => JObject::null(),
        };

        let payload_obj = with_jni(&mut env, |env| {
            let class_ref = env.new_local_ref(payload_class_ref.as_obj())?;
            let class = JClass::from(class_ref);
            env.new_object(
                class,
                "(Ljava/lang/String;ILjava/lang/String;)V",
                &[
                    JValue::Object(&path_obj),
                    JValue::Int(media_type_value),
                    JValue::Object(&cover_obj),
                ],
            )
        })?;

        with_jni(&mut env, |env| {
            env.set_object_array_element(&payload_array, idx as i32, payload_obj)
        })?;
    }

    let preview_signature = format!(
        "([L{};)V",
        super::CachedClass::PreviewMediaPayload.class_path()
    );

    with_jni(&mut env, |env| {
        let class_ref = env.new_local_ref(media_class_ref.as_obj())?;
        let class = JClass::from(class_ref);
        env.call_static_method(
            class,
            "previewMedia",
            preview_signature.as_str(),
            &[JValue::Object(&payload_array)],
        )
    })?;

    Ok(())
}

fn scan_code_impl(request: ScanCodeRequest) -> Result<(), Box<dyn std::error::Error>> {
    let mut env = get_env()?;

    let media_class_ref = super::get_cached_class(super::CachedClass::LxAppMedia)?;

    // Group codes understood by Kotlin fragment:
    // 1 = QR, 2 = BAR (1D), 3 = DATA_MATRIX, 4 = PDF_417
    let type_codes: Vec<jint> = request
        .scan_types
        .iter()
        .map(|t| match t {
            ScanType::QrCode => 1,
            ScanType::BarCode => 2,
            ScanType::DataMatrix => 3,
            ScanType::Pdf417 => 4,
        })
        .collect();

    let scan_types_array = with_jni(&mut env, |env| {
        let array = env.new_int_array(type_codes.len() as i32)?;
        if !type_codes.is_empty() {
            env.set_int_array_region(&array, 0, &type_codes)?;
        }
        Ok::<_, jni::errors::Error>(JObject::from(array))
    })?;

    with_jni(&mut env, |env| {
        let class_ref = env.new_local_ref(media_class_ref.as_obj())?;
        let class = JClass::from(class_ref);
        env.call_static_method(
            class,
            "scanCode",
            "([IZJ)V",
            &[
                JValue::Object(&scan_types_array),
                JValue::Bool(if request.only_from_camera { 1 } else { 0 }),
                JValue::Long(request.callback_id as jlong),
            ],
        )
    })?;

    Ok(())
}

fn save_media_impl(request: SaveMediaRequest, method: &str) -> Result<(), PlatformError> {
    let mut env =
        get_env().map_err(|e| PlatformError::Platform(format!("Failed to get JNIEnv: {}", e)))?;

    let media_class_ref = super::get_cached_class(super::CachedClass::LxAppMedia).map_err(|e| {
        PlatformError::Platform(format!("Failed to get cached Java class LxAppMedia: {}", e))
    })?;

    let path_java = with_jni(&mut env, |env| env.new_string(&request.file_uri)).map_err(|err| {
        let _ = lingxia_messaging::invoke_callback(request.callback_id, Err(1000));
        PlatformError::Platform(format!(
            "Failed to create Java string for media path: {}",
            err
        ))
    })?;
    let path_obj: JObject = path_java.into();

    let callback_id = request.callback_id as jlong;

    if let Err(err) = with_jni(&mut env, |env| {
        let class_ref = env.new_local_ref(media_class_ref.as_obj())?;
        let class = JClass::from(class_ref);
        env.call_static_method(
            class,
            method,
            "(Ljava/lang/String;J)V",
            &[JValue::Object(&path_obj), JValue::Long(callback_id)],
        )
    }) {
        let _ = lingxia_messaging::invoke_callback(request.callback_id, Err(1000));
        return Err(PlatformError::Platform(format!(
            "Failed to start {}: {}",
            method, err
        )));
    }

    Ok(())
}

fn choose_media_impl(request: ChooseMediaRequest) -> Result<(), Box<dyn std::error::Error>> {
    let mut env = get_env()?;

    let media_class_ref = super::get_cached_class(super::CachedClass::LxAppMedia)?;

    // Map enums to integers expected by Android side
    let mode_value: jint = match request.mode {
        crate::traits::media_interaction::ChooseMediaMode::Images => 0,
        crate::traits::media_interaction::ChooseMediaMode::Videos => 1,
        crate::traits::media_interaction::ChooseMediaMode::Mix => 2,
    };

    let mut has_album = false;
    let mut has_camera = false;
    for source in &request.source_types {
        match source {
            crate::traits::media_interaction::MediaSource::Album => has_album = true,
            crate::traits::media_interaction::MediaSource::Camera => has_camera = true,
        }
    }

    let source_flag: jint = match (has_album, has_camera) {
        (true, false) => 0,
        (false, true) => 1,
        _ => 2, // default to both when unspecified
    };

    let max_duration_value: jint = request
        .max_duration_seconds
        .map(|v| v as jint)
        .unwrap_or(-1);

    let camera_facing_value: jint = request
        .camera_facing
        .map(|c| match c {
            crate::traits::media_interaction::CameraFacing::Front => 0,
            crate::traits::media_interaction::CameraFacing::Back => 1,
        })
        .unwrap_or(-1);

    with_jni(&mut env, |env| {
        let class_ref = env.new_local_ref(media_class_ref.as_obj())?;
        let class = JClass::from(class_ref);
        env.call_static_method(
            class,
            "chooseMedia",
            "(IIIIIJ)V",
            &[
                JValue::Int(request.max_count as jint),
                JValue::Int(mode_value),
                JValue::Int(source_flag),
                JValue::Int(max_duration_value),
                JValue::Int(camera_facing_value),
                JValue::Long(request.callback_id as i64),
            ],
        )
    })?;

    Ok(())
}

impl MediaRuntime for Platform {
    fn copy_album_media_to_file(
        &self,
        uri: &str,
        dest_path: &Path,
        _kind: MediaKind,
    ) -> Result<(), PlatformError> {
        copy_album_media_to_file_impl(uri, dest_path).map_err(|e| {
            PlatformError::Platform(format!("Android copy_album_media_to_file failed: {}", e))
        })
    }

    fn get_image_info(&self, uri: &str) -> Result<ImageInfo, PlatformError> {
        get_image_info_impl(uri)
            .map_err(|e| PlatformError::Platform(format!("get_image_info failed: {}", e)))
    }

    fn compress_image(&self, request: &CompressImageRequest) -> Result<PathBuf, PlatformError> {
        compress_image_impl(request)
            .map_err(|e| PlatformError::Platform(format!("compress_image failed: {}", e)))
    }

    fn get_video_info(&self, uri: &str) -> Result<VideoInfo, PlatformError> {
        get_video_info_impl(uri)
            .map_err(|e| PlatformError::Platform(format!("get_video_info failed: {}", e)))
    }

    fn extract_video_thumbnail(
        &self,
        request: &ExtractVideoThumbnailRequest,
    ) -> Result<VideoThumbnail, PlatformError> {
        extract_video_thumbnail_impl(request)
            .map_err(|e| PlatformError::Platform(format!("extract_video_thumbnail failed: {}", e)))
    }
}

fn copy_album_media_to_file_impl(
    uri: &str,
    dest_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut env = get_env()?;
    let media_class_ref = super::get_cached_class(super::CachedClass::LxAppMedia)?;
    let media_class: &JClass = media_class_ref.as_obj().into();
    let j_uri = env.new_string(uri)?;
    let j_dest = env.new_string(dest_path.to_string_lossy().as_ref())?;
    let res = env.call_static_method(
        media_class,
        "copyAlbumMediaToFile",
        "(Ljava/lang/String;Ljava/lang/String;)Z",
        &[(&j_uri).into(), (&j_dest).into()],
    )?;
    if res.z()? {
        Ok(())
    } else {
        Err("copyAlbumMediaToFile returned false".into())
    }
}

fn get_image_info_impl(uri: &str) -> Result<ImageInfo, Box<dyn std::error::Error>> {
    let mut env = get_env()?;
    let media_class_ref = super::get_cached_class(super::CachedClass::LxAppMedia)?;
    let media_class: &JClass = media_class_ref.as_obj().into();
    let j_uri = env.new_string(uri)?;
    let result = env.call_static_method(
        media_class,
        "getImageInfo",
        "(Ljava/lang/String;)Ljava/lang/String;",
        &[(&j_uri).into()],
    )?;
    let json_obj = result.l()?;
    if json_obj.is_null() {
        return Err("getImageInfo returned null".into());
    }
    let java_str = JString::from(json_obj);
    let json_str: String = env.get_string(&java_str)?.into();
    let parsed: AndroidImageInfoResponse = serde_json::from_str(&json_str)?;
    if !parsed.success {
        return Err(parsed
            .error
            .unwrap_or_else(|| "getImageInfo failed".to_string())
            .into());
    }
    Ok(ImageInfo {
        width: parsed.width.unwrap_or(0),
        height: parsed.height.unwrap_or(0),
        mime_type: parsed.mime_type.filter(|s| !s.is_empty()),
    })
}

fn compress_image_impl(
    request: &CompressImageRequest,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut env = get_env()?;
    let media_class_ref = super::get_cached_class(super::CachedClass::LxAppMedia)?;
    let media_class: &JClass = media_class_ref.as_obj().into();
    let j_uri = env.new_string(&request.source_uri)?;
    let output_path = request.output_path.to_string_lossy();
    let j_output_path = env.new_string(output_path.as_ref())?;
    let quality = i32::from(request.quality);
    let width = request.max_width.unwrap_or(0) as i32;
    let height = request.max_height.unwrap_or(0) as i32;
    let result = env.call_static_method(
        media_class,
        "compressImage",
        "(Ljava/lang/String;Ljava/lang/String;III)Ljava/lang/String;",
        &[
            (&j_uri).into(),
            (&j_output_path).into(),
            JValue::Int(quality),
            JValue::Int(width),
            JValue::Int(height),
        ],
    )?;
    let path_obj = result.l()?;
    if path_obj.is_null() {
        return Err("compressImage returned null".into());
    }
    let java_path = JString::from(path_obj);
    let path: String = env.get_string(&java_path)?.into();
    if let Some(err) = path.strip_prefix("__ERROR__:") {
        return Err(err.to_string().into());
    }
    if path.is_empty() {
        return Err("compressImage failed".into());
    }
    Ok(PathBuf::from(path))
}

fn get_video_info_impl(uri: &str) -> Result<VideoInfo, Box<dyn std::error::Error>> {
    let mut env = get_env()?;
    let media_class_ref = super::get_cached_class(super::CachedClass::LxAppMedia)?;
    let media_class: &JClass = media_class_ref.as_obj().into();
    let j_uri = env.new_string(uri)?;
    let result = env.call_static_method(
        media_class,
        "getVideoInfo",
        "(Ljava/lang/String;)Ljava/lang/String;",
        &[(&j_uri).into()],
    )?;
    let json_obj = result.l()?;
    if json_obj.is_null() {
        return Err("getVideoInfo returned null".into());
    }
    let java_str = JString::from(json_obj);
    let json_str: String = env.get_string(&java_str)?.into();
    let parsed: AndroidVideoInfoResponse = serde_json::from_str(&json_str)?;
    if !parsed.success {
        return Err(parsed
            .error
            .unwrap_or_else(|| "getVideoInfo failed".to_string())
            .into());
    }
    Ok(VideoInfo {
        width: parsed.width.unwrap_or(0),
        height: parsed.height.unwrap_or(0),
        duration_ms: parsed.duration_ms.unwrap_or(0),
        rotation: parsed.rotation,
        bitrate: parsed.bitrate,
        fps: parsed.fps.map(|v| v as f32),
        mime_type: parsed.mime_type.filter(|s| !s.is_empty()),
    })
}

fn extract_video_thumbnail_impl(
    request: &ExtractVideoThumbnailRequest,
) -> Result<VideoThumbnail, Box<dyn std::error::Error>> {
    let mut env = get_env()?;
    let media_class_ref = super::get_cached_class(super::CachedClass::LxAppMedia)?;
    let media_class: &JClass = media_class_ref.as_obj().into();

    let j_uri = env.new_string(&request.source_uri)?;
    let output_path = request.output_path.to_string_lossy();
    let j_output_path = env.new_string(output_path.as_ref())?;
    let quality = i32::from(request.quality);
    let width = request.max_width.unwrap_or(0) as i32;
    let height = request.max_height.unwrap_or(0) as i32;
    let time_ms = request.time_ms.map(|v| v as i64).unwrap_or(-1);

    let result = env.call_static_method(
        media_class,
        "extractVideoThumbnail",
        "(Ljava/lang/String;Ljava/lang/String;IIIJ)Ljava/lang/String;",
        &[
            (&j_uri).into(),
            (&j_output_path).into(),
            JValue::Int(quality),
            JValue::Int(width),
            JValue::Int(height),
            JValue::Long(time_ms),
        ],
    )?;
    let json_obj = result.l()?;
    if json_obj.is_null() {
        return Err("extractVideoThumbnail returned null".into());
    }
    let java_str = JString::from(json_obj);
    let json_str: String = env.get_string(&java_str)?.into();
    let parsed: AndroidVideoThumbnailResponse = serde_json::from_str(&json_str)?;
    if !parsed.success {
        return Err(parsed
            .error
            .unwrap_or_else(|| "extractVideoThumbnail failed".to_string())
            .into());
    }

    let path = parsed.path.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "extractVideoThumbnail missing output path",
        )
    })?;
    Ok(VideoThumbnail {
        path: PathBuf::from(path),
        width: parsed.width.unwrap_or(0),
        height: parsed.height.unwrap_or(0),
        mime_type: parsed.mime_type.filter(|s| !s.is_empty()),
    })
}

#[derive(Deserialize)]
struct AndroidImageInfoResponse {
    success: bool,
    error: Option<String>,
    #[serde(rename = "width")]
    width: Option<u32>,
    #[serde(rename = "height")]
    height: Option<u32>,
    #[serde(rename = "mimeType")]
    mime_type: Option<String>,
}

#[derive(Deserialize)]
struct AndroidVideoInfoResponse {
    success: bool,
    error: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    #[serde(rename = "durationMs")]
    duration_ms: Option<u64>,
    rotation: Option<u16>,
    bitrate: Option<u64>,
    fps: Option<f64>,
    #[serde(rename = "mimeType")]
    mime_type: Option<String>,
}

#[derive(Deserialize)]
struct AndroidVideoThumbnailResponse {
    success: bool,
    error: Option<String>,
    path: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    #[serde(rename = "mimeType")]
    mime_type: Option<String>,
}
