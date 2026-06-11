use super::app::Platform;
use super::with_env;
use crate::error::PlatformError;
use crate::traits::media_interaction::{
    ChooseMediaRequest, MediaInteraction, MediaKind, MediaObjectFit, PreviewMediaRequest,
    SaveMediaRequest, ScanCodeRequest, ScanType,
};
use crate::traits::media_runtime::{
    CompressImageRequest, CompressVideoRequest, CompressedVideo, ExtractVideoThumbnailRequest,
    ImageInfo, MediaRuntime, VideoInfo, VideoThumbnail,
};
use jni::objects::{JClass, JObject, JString, JValue};
use jni::strings::JNIString;
use jni::sys::{jboolean, jint, jlong};
use jni::{jni_sig, jni_str};
use serde::Deserialize;
use std::path::{Path, PathBuf};

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

    fn cancel_preview(&self, callback_id: u64) -> Result<(), PlatformError> {
        let media_class_ref =
            super::get_cached_class(super::CachedClass::LxAppMedia).map_err(|e| {
                PlatformError::Platform(format!(
                    "Failed to get cached Java class LxAppMedia: {}",
                    e
                ))
            })?;

        with_env(|env| {
            let class: &JClass = media_class_ref.as_ref();
            env.call_static_method(
                class,
                jni_str!("closePreview"),
                jni_sig!("(J)V"),
                &[JValue::Long(callback_id as jlong)],
            )?;

            Ok::<(), jni::errors::Error>(())
        })
        .map_err(|err| PlatformError::Platform(format!("Failed to cancel previewMedia: {}", err)))
    }

    async fn choose_media(&self, request: ChooseMediaRequest) -> Result<String, PlatformError> {
        crate::rt::native_call(|callback_id| {
            choose_media_impl(request, callback_id)
                .map_err(|e| PlatformError::Platform(format!("Failed to choose media: {}", e)))
        })
        .await
    }

    async fn scan_code(&self, request: ScanCodeRequest) -> Result<String, PlatformError> {
        crate::rt::native_call(|callback_id| {
            scan_code_impl(request, callback_id)
                .map_err(|e| PlatformError::Platform(format!("Failed to start scanCode: {}", e)))
        })
        .await
    }

    async fn save_image_to_photos_album(
        &self,
        request: SaveMediaRequest,
    ) -> Result<(), PlatformError> {
        crate::rt::native_call(|callback_id| {
            save_media_impl(request, "saveImageToPhotosAlbum", callback_id)
        })
        .await
        .map(|_| ())
    }

    async fn save_video_to_photos_album(
        &self,
        request: SaveMediaRequest,
    ) -> Result<(), PlatformError> {
        crate::rt::native_call(|callback_id| {
            save_media_impl(request, "saveVideoToPhotosAlbum", callback_id)
        })
        .await
        .map(|_| ())
    }
}

fn preview_media_impl(request: PreviewMediaRequest) -> Result<(), Box<dyn std::error::Error>> {
    let media_class_ref = super::get_cached_class(super::CachedClass::LxAppMedia)?;
    let payload_class_ref = super::get_cached_class(super::CachedClass::PreviewMediaPayload)?;

    let item_count = request.items.len();
    let start_index = request.start_index;
    let advance = request.advance.as_str();
    let show_index_indicator = request.show_index_indicator;
    let callback_id = request.callback_id as jlong;
    let presented_callback_id = request.presented_callback_id as jlong;
    let change_callback_id = request.change_callback_id as jlong;

    with_env(|env| {
        let payload_class: &JClass = payload_class_ref.as_ref();
        let payload_array =
            env.new_object_array(item_count as i32, payload_class, JObject::null())?;

        for (idx, item) in request.items.iter().enumerate() {
            let path_java = env.new_string(&item.path)?;
            let path_obj: JObject = path_java.into();

            let media_type_value = match item.media_type {
                MediaKind::Image => 0,
                MediaKind::Video => 1,
                MediaKind::Unknown => -1,
            } as jint;

            let rotate_obj = match item.rotate {
                Some(rotate) => env.new_object(
                    jni_str!("java/lang/Integer"),
                    jni_sig!("(I)V"),
                    &[JValue::Int(rotate as jint)],
                )?,
                None => JObject::null(),
            };

            let object_fit_obj = match item.object_fit {
                Some(fit) => {
                    let value = match fit {
                        MediaObjectFit::Cover => "cover",
                        MediaObjectFit::Contain => "contain",
                        MediaObjectFit::Fill => "fill",
                        MediaObjectFit::Fit => "fit",
                    };
                    let fit_java: JString = env.new_string(value)?;
                    fit_java.into()
                }
                None => JObject::null(),
            };

            let duration_obj = match item.duration_ms {
                Some(duration_ms) => env.new_object(
                    jni_str!("java/lang/Long"),
                    jni_sig!("(J)V"),
                    &[JValue::Long(duration_ms.min(jlong::MAX as u64) as jlong)],
                )?,
                None => JObject::null(),
            };

            let payload_obj = env.new_object(
                payload_class,
                jni_sig!(
                    "(Ljava/lang/String;ILjava/lang/Integer;Ljava/lang/String;Ljava/lang/Long;)V"
                ),
                &[
                    JValue::Object(&path_obj),
                    JValue::Int(media_type_value),
                    JValue::Object(&rotate_obj),
                    JValue::Object(&object_fit_obj),
                    JValue::Object(&duration_obj),
                ],
            )?;

            payload_array.set_element(env, idx, &payload_obj)?;
        }

        let class: &JClass = media_class_ref.as_ref();
        let advance_java = env.new_string(advance)?;
        let advance_obj: JObject = advance_java.into();
        env.call_static_method(
            class,
            jni_str!("previewMedia"),
            jni_sig!(
                "([Lcom/lingxia/lxapp/APIs/media/PreviewMediaPayload;ILjava/lang/String;ZJJJ)V"
            ),
            &[
                JValue::Object(&payload_array),
                JValue::Int(start_index),
                JValue::Object(&advance_obj),
                JValue::Bool(jboolean::from(show_index_indicator)),
                JValue::Long(callback_id),
                JValue::Long(presented_callback_id),
                JValue::Long(change_callback_id),
            ],
        )?;

        Ok::<(), jni::errors::Error>(())
    })?;

    Ok(())
}

fn scan_code_impl(
    request: ScanCodeRequest,
    callback_id: u64,
) -> Result<(), Box<dyn std::error::Error>> {
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

    with_env(|env| {
        let array = env.new_int_array(type_codes.len())?;
        if !type_codes.is_empty() {
            array.set_region(env, 0, &type_codes)?;
        }
        let scan_types_array = JObject::from(array);

        let class: &JClass = media_class_ref.as_ref();
        env.call_static_method(
            class,
            jni_str!("scanCode"),
            jni_sig!("([IZJ)V"),
            &[
                JValue::Object(&scan_types_array),
                JValue::Bool(request.only_from_camera),
                JValue::Long(callback_id as jlong),
            ],
        )?;

        Ok::<(), jni::errors::Error>(())
    })?;

    Ok(())
}

fn save_media_impl(
    request: SaveMediaRequest,
    method: &str,
    callback_id: u64,
) -> Result<(), PlatformError> {
    let media_class_ref = super::get_cached_class(super::CachedClass::LxAppMedia).map_err(|e| {
        PlatformError::Platform(format!("Failed to get cached Java class LxAppMedia: {}", e))
    })?;

    let method_str = method.to_string();
    let method_jni = JNIString::new(method_str.as_str());
    let file_uri = request.file_uri.clone();

    with_env(move |env| {
        let path_java = env.new_string(&file_uri)?;
        let path_obj: JObject = path_java.into();

        let class: &JClass = media_class_ref.as_ref();
        env.call_static_method(
            class,
            &method_jni,
            jni_sig!("(Ljava/lang/String;J)V"),
            &[
                JValue::Object(&path_obj),
                JValue::Long(callback_id as jlong),
            ],
        )?;

        Ok::<(), jni::errors::Error>(())
    })
    .map_err(|err| PlatformError::Platform(format!("Failed to start {}: {}", method, err)))?;

    Ok(())
}

fn choose_media_impl(
    request: ChooseMediaRequest,
    callback_id: u64,
) -> Result<(), Box<dyn std::error::Error>> {
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

    with_env(|env| {
        let class: &JClass = media_class_ref.as_ref();
        env.call_static_method(
            class,
            jni_str!("chooseMedia"),
            jni_sig!("(IIIIIJ)V"),
            &[
                JValue::Int(request.max_count as jint),
                JValue::Int(mode_value),
                JValue::Int(source_flag),
                JValue::Int(max_duration_value),
                JValue::Int(camera_facing_value),
                JValue::Long(callback_id as i64),
            ],
        )?;

        Ok::<(), jni::errors::Error>(())
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

    fn compress_video(
        &self,
        request: &CompressVideoRequest,
    ) -> Result<CompressedVideo, PlatformError> {
        compress_video_impl(request)
            .map_err(|e| PlatformError::Platform(format!("compress_video failed: {}", e)))
    }
}

fn copy_album_media_to_file_impl(
    uri: &str,
    dest_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let media_class_ref = super::get_cached_class(super::CachedClass::LxAppMedia)?;
    let uri = uri.to_string();
    let dest_path_str = dest_path.to_string_lossy().to_string();

    with_env(|env| {
        let media_class: &JClass = media_class_ref.as_ref();
        let j_uri = env.new_string(&uri)?;
        let j_dest = env.new_string(&dest_path_str)?;
        let res = env.call_static_method(
            media_class,
            jni_str!("copyAlbumMediaToFile"),
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Z"),
            &[(&j_uri).into(), (&j_dest).into()],
        )?;
        if res.z()? {
            Ok(())
        } else {
            Err("copyAlbumMediaToFile returned false".into())
        }
    })
}

fn get_image_info_impl(uri: &str) -> Result<ImageInfo, Box<dyn std::error::Error>> {
    let media_class_ref = super::get_cached_class(super::CachedClass::LxAppMedia)?;
    let uri = uri.to_string();

    with_env(|env| {
        let media_class: &JClass = media_class_ref.as_ref();
        let j_uri = env.new_string(&uri)?;
        let result = env.call_static_method(
            media_class,
            jni_str!("getImageInfo"),
            jni_sig!("(Ljava/lang/String;)Ljava/lang/String;"),
            &[(&j_uri).into()],
        )?;
        let json_obj = result.l()?;
        if json_obj.is_null() {
            return Err("getImageInfo returned null".into());
        }
        let java_str = unsafe { JString::from_raw(env, json_obj.into_raw() as _) };
        let json_str: String = java_str.try_to_string(env)?;
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
    })
}

fn compress_image_impl(
    request: &CompressImageRequest,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let media_class_ref = super::get_cached_class(super::CachedClass::LxAppMedia)?;
    let source_uri = request.source_uri.clone();
    let output_path = request.output_path.to_string_lossy().to_string();
    let quality = i32::from(request.quality);
    let width = request.max_width.unwrap_or(0) as i32;
    let height = request.max_height.unwrap_or(0) as i32;

    with_env(|env| {
        let media_class: &JClass = media_class_ref.as_ref();
        let j_uri = env.new_string(&source_uri)?;
        let j_output_path = env.new_string(&output_path)?;
        let result = env.call_static_method(
            media_class,
            jni_str!("compressImage"),
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;III)Ljava/lang/String;"),
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
        let java_path = unsafe { JString::from_raw(env, path_obj.into_raw() as _) };
        let path: String = java_path.try_to_string(env)?;
        if let Some(err) = path.strip_prefix("__ERROR__:") {
            return Err(err.to_string().into());
        }
        if path.is_empty() {
            return Err("compressImage failed".into());
        }
        Ok(PathBuf::from(path))
    })
}

fn get_video_info_impl(uri: &str) -> Result<VideoInfo, Box<dyn std::error::Error>> {
    let media_class_ref = super::get_cached_class(super::CachedClass::LxAppMedia)?;
    let uri = uri.to_string();

    with_env(|env| {
        let media_class: &JClass = media_class_ref.as_ref();
        let j_uri = env.new_string(&uri)?;
        let result = env.call_static_method(
            media_class,
            jni_str!("getVideoInfo"),
            jni_sig!("(Ljava/lang/String;)Ljava/lang/String;"),
            &[(&j_uri).into()],
        )?;
        let json_obj = result.l()?;
        if json_obj.is_null() {
            return Err("getVideoInfo returned null".into());
        }
        let java_str = unsafe { JString::from_raw(env, json_obj.into_raw() as _) };
        let json_str: String = java_str.try_to_string(env)?;
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
    })
}

fn extract_video_thumbnail_impl(
    request: &ExtractVideoThumbnailRequest,
) -> Result<VideoThumbnail, Box<dyn std::error::Error>> {
    let media_class_ref = super::get_cached_class(super::CachedClass::LxAppMedia)?;
    let source_uri = request.source_uri.clone();
    let output_path = request.output_path.to_string_lossy().to_string();
    let quality = i32::from(request.quality);
    let width = request.max_width.unwrap_or(0) as i32;
    let height = request.max_height.unwrap_or(0) as i32;
    let time_ms = request.time_ms.map(|v| v as i64).unwrap_or(-1);

    with_env(|env| {
        let media_class: &JClass = media_class_ref.as_ref();

        let j_uri = env.new_string(&source_uri)?;
        let j_output_path = env.new_string(&output_path)?;

        let result = env.call_static_method(
            media_class,
            jni_str!("extractVideoThumbnail"),
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;IIIJ)Ljava/lang/String;"),
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
        let java_str = unsafe { JString::from_raw(env, json_obj.into_raw() as _) };
        let json_str: String = java_str.try_to_string(env)?;
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
    })
}

fn compress_video_impl(
    request: &CompressVideoRequest,
) -> Result<CompressedVideo, Box<dyn std::error::Error>> {
    let media_class_ref = super::get_cached_class(super::CachedClass::LxAppMedia)?;
    let source_uri = request.source_uri.clone();
    let output_path = request.output_path.to_string_lossy().to_string();
    let quality = request.quality.map_or_else(String::new, |q| match q {
        crate::traits::media_runtime::VideoCompressQuality::Low => "low".to_string(),
        crate::traits::media_runtime::VideoCompressQuality::Medium => "medium".to_string(),
        crate::traits::media_runtime::VideoCompressQuality::High => "high".to_string(),
    });
    let bitrate = request.bitrate_kbps.unwrap_or(0) as i32;
    let fps = request.fps.unwrap_or(0) as i32;
    let resolution = request.resolution_ratio.unwrap_or(0.0f32);

    with_env(|env| {
        let media_class: &JClass = media_class_ref.as_ref();

        let j_uri = env.new_string(&source_uri)?;
        let j_output_path = env.new_string(&output_path)?;
        let j_quality = env.new_string(&quality)?;

        let result = env.call_static_method(
            media_class,
            jni_str!("compressVideo"),
            jni_sig!(
                "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;IIF)Ljava/lang/String;"
            ),
            &[
                (&j_uri).into(),
                (&j_output_path).into(),
                (&j_quality).into(),
                JValue::Int(bitrate),
                JValue::Int(fps),
                JValue::Float(resolution),
            ],
        )?;
        let json_obj = result.l()?;
        if json_obj.is_null() {
            return Err("compressVideo returned null".into());
        }
        let java_str = unsafe { JString::from_raw(env, json_obj.into_raw() as _) };
        let json_str: String = java_str.try_to_string(env)?;
        let parsed: AndroidCompressVideoResponse = serde_json::from_str(&json_str)?;
        if !parsed.success {
            return Err(parsed
                .error
                .unwrap_or_else(|| "compressVideo failed".to_string())
                .into());
        }

        let path = parsed.path.ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "compressVideo missing output path",
            )
        })?;

        Ok(CompressedVideo {
            path: PathBuf::from(path),
            width: parsed.width.unwrap_or(0),
            height: parsed.height.unwrap_or(0),
            duration_ms: parsed.duration_ms.unwrap_or(0),
            size: parsed.size.unwrap_or(0),
            mime_type: parsed.mime_type.filter(|s| !s.is_empty()),
        })
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

#[derive(Deserialize)]
struct AndroidCompressVideoResponse {
    success: bool,
    error: Option<String>,
    path: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    #[serde(rename = "durationMs")]
    duration_ms: Option<u64>,
    size: Option<u64>,
    #[serde(rename = "mimeType")]
    mime_type: Option<String>,
}
