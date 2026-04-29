use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::ui::{SurfaceKind, SurfacePresenter, SurfaceRequest};
use jni::objects::{JClass, JObject, JValue};
use jni::{jni_sig, jni_str};

impl SurfacePresenter for Platform {
    fn present_surface(&self, request: SurfaceRequest) -> Result<(), PlatformError> {
        if request.kind != SurfaceKind::Popup {
            return Err(PlatformError::NotSupported(
                "window surface is not supported on Android".to_string(),
            ));
        }

        let surface_class: &JClass = super::get_cached_class(super::CachedClass::LxAppSurface)
            .map_err(|e| PlatformError::Platform(e.to_string()))?;

        super::with_env(|env| -> Result<(), PlatformError> {
            let id = env.new_string(&request.id)?;
            let app_id = env.new_string(&request.app_id)?;
            let path = env.new_string(&request.path)?;
            let page_instance_id = env.new_string(&request.page_instance_id)?;

            let ok = env
                .call_static_method(
                    surface_class,
                    jni_str!("present"),
                    jni_sig!("(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;JLjava/lang/String;IIDDDDI)Z"),
                    &[
                        JValue::Object(&JObject::from(id)),
                        JValue::Object(&JObject::from(app_id)),
                        JValue::Object(&JObject::from(path)),
                        JValue::Long(request.session_id as i64),
                        JValue::Object(&JObject::from(page_instance_id)),
                        JValue::Int(request.content as i32),
                        JValue::Int(request.kind as i32),
                        JValue::Double(request.width),
                        JValue::Double(request.height),
                        JValue::Double(request.width_ratio),
                        JValue::Double(request.height_ratio),
                        JValue::Int(request.position as i32),
                    ],
                )?
                .z()?;
            if ok {
                Ok(())
            } else {
                Err(PlatformError::Platform(format!(
                    "Failed to present surface: id={}, appid={}, path={}",
                    request.id, request.app_id, request.path
                )))
            }
        })
        .map_err(|e| PlatformError::Platform(format!("Failed to present surface: {e}")))
    }

    fn close_surface(&self, app_id: &str, id: &str, reason: &str) -> Result<(), PlatformError> {
        let surface_class: &JClass = super::get_cached_class(super::CachedClass::LxAppSurface)
            .map_err(|e| PlatformError::Platform(e.to_string()))?;

        super::with_env(|env| -> Result<(), PlatformError> {
            let id = env.new_string(id)?;
            let app_id = env.new_string(app_id)?;
            let reason = env.new_string(reason)?;
            let ok = env
                .call_static_method(
                    surface_class,
                    jni_str!("close"),
                    jni_sig!("(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)Z"),
                    &[
                        JValue::Object(&JObject::from(id)),
                        JValue::Object(&JObject::from(app_id)),
                        JValue::Object(&JObject::from(reason)),
                    ],
                )?
                .z()?;
            if ok {
                Ok(())
            } else {
                Err(PlatformError::Platform(
                    "Failed to close surface".to_string(),
                ))
            }
        })
        .map_err(|e| PlatformError::Platform(format!("Failed to close surface: {e}")))
    }
}
