use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::MediaKind;
use crate::traits::{
    ChooseMediaRequest, MediaInteraction, PreviewMediaRequest, SaveMediaRequest, ScanCodeRequest,
};
use jni::JNIEnv;
use jni::objects::JIntArray;
use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::jint;
use lingxia_webview::get_env;

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

    fn scan_code(&self, _request: ScanCodeRequest) -> Result<(), PlatformError> {
        Err(PlatformError::Platform(
            "scan_code is not implemented on Android".to_string(),
        ))
    }

    fn save_image_to_photos_album(&self, request: SaveMediaRequest) -> Result<(), PlatformError> {
        match save_media_impl(request, "saveImageToPhotosAlbum") {
            Ok(true) => Ok(()),
            Ok(false) => Err(PlatformError::Platform(
                "Failed to save image to photos album".to_string(),
            )),
            Err(e) => Err(PlatformError::Platform(format!(
                "Failed to save image to photos album: {}",
                e
            ))),
        }
    }

    fn save_video_to_photos_album(&self, request: SaveMediaRequest) -> Result<(), PlatformError> {
        match save_media_impl(request, "saveVideoToPhotosAlbum") {
            Ok(true) => Ok(()),
            Ok(false) => Err(PlatformError::Platform(
                "Failed to save video to photos album".to_string(),
            )),
            Err(e) => Err(PlatformError::Platform(format!(
                "Failed to save video to photos album: {}",
                e
            ))),
        }
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

    with_jni(&mut env, |env| {
        let class_ref = env.new_local_ref(media_class_ref.as_obj())?;
        let class = JClass::from(class_ref);
        env.call_static_method(
            class,
            "previewMedia",
            "([Lcom/lingxia/lxapp/media/PreviewMediaPayload;)V",
            &[JValue::Object(&payload_array)],
        )
    })?;

    Ok(())
}

fn save_media_impl(
    request: SaveMediaRequest,
    method: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let mut env = get_env()?;

    let media_class_ref = super::get_cached_class(super::CachedClass::LxAppMedia)?;

    let path_java = with_jni(&mut env, |env| env.new_string(&request.file_uri))?;
    let path_obj: JObject = path_java.into();

    let result = with_jni(&mut env, |env| {
        let class_ref = env.new_local_ref(media_class_ref.as_obj())?;
        let class = JClass::from(class_ref);
        env.call_static_method(
            class,
            method,
            "(Ljava/lang/String;)Z", // (String) returns boolean
            &[JValue::Object(&path_obj)],
        )
    })?;

    // Extract the boolean result using .z() method
    let success = result.z()?;

    Ok(success)
}

fn choose_media_impl(request: ChooseMediaRequest) -> Result<(), Box<dyn std::error::Error>> {
    let mut env = get_env()?;

    let media_class_ref = super::get_cached_class(super::CachedClass::LxAppMedia)?;

    // Map enums to integers expected by Android side
    let mode_value: jint = match request.mode {
        crate::traits::ChooseMediaMode::Images => 0,
        crate::traits::ChooseMediaMode::Videos => 1,
        crate::traits::ChooseMediaMode::Mix => 2,
    };

    let source_values: Vec<jint> = request
        .source_types
        .iter()
        .map(|s| match s {
            crate::traits::MediaSource::Album => 0,
            crate::traits::MediaSource::Camera => 1,
        })
        .collect();

    let sources_array: JIntArray = with_jni(&mut env, |env| {
        let arr = env.new_int_array(source_values.len() as i32)?;
        if !source_values.is_empty() {
            env.set_int_array_region(&arr, 0, &source_values)?;
        }
        Ok(arr)
    })?;

    let max_duration_value: jint = request
        .max_duration_seconds
        .map(|v| v as jint)
        .unwrap_or(-1);

    let camera_facing_value: jint = request
        .camera_facing
        .map(|c| match c {
            crate::traits::CameraFacing::Front => 0,
            crate::traits::CameraFacing::Back => 1,
        })
        .unwrap_or(-1);

    with_jni(&mut env, |env| {
        let class_ref = env.new_local_ref(media_class_ref.as_obj())?;
        let class = JClass::from(class_ref);
        env.call_static_method(
            class,
            "chooseMedia",
            "(II[IZZIIJ)V",
            &[
                JValue::Int(request.max_count as jint),
                JValue::Int(mode_value),
                JValue::Object(&sources_array),
                JValue::Bool(request.allow_original as u8),
                JValue::Bool(request.allow_compressed as u8),
                JValue::Int(max_duration_value),
                JValue::Int(camera_facing_value),
                JValue::Long(request.callback_id as i64),
            ],
        )
    })?;

    Ok(())
}
