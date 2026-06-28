use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::ui::{SurfaceKind, SurfacePresenter, SurfaceRequest};
use jni::objects::{JClass, JObject, JValue};
use jni::{jni_sig, jni_str};
use lingxia_surface::LayoutPresentationPlan;

impl SurfacePresenter for Platform {
    fn present_layout(
        &self,
        window_id: &str,
        plan: &LayoutPresentationPlan,
    ) -> Result<(), PlatformError> {
        let surface_class: &JClass = super::get_cached_class(super::CachedClass::LxAppSurface)
            .map_err(|e| PlatformError::Platform(e.to_string()))?;
        let plan_json = serde_json::to_string(plan).map_err(|e| {
            PlatformError::Platform(format!("failed to serialize layout plan: {e}"))
        })?;

        super::with_env(|env| -> Result<(), PlatformError> {
            let window_id = env.new_string(window_id)?;
            let plan_json = env.new_string(&plan_json)?;
            let ok = env
                .call_static_method(
                    surface_class,
                    jni_str!("presentLayout"),
                    jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Z"),
                    &[
                        JValue::Object(&JObject::from(window_id)),
                        JValue::Object(&JObject::from(plan_json)),
                    ],
                )?
                .z()?;
            if ok {
                Ok(())
            } else {
                Err(PlatformError::Platform(
                    "Failed to present layout".to_string(),
                ))
            }
        })
        .map_err(|e| PlatformError::Platform(format!("Failed to present layout: {e}")))
    }

    fn present_surface(&self, request: SurfaceRequest) -> Result<(), PlatformError> {
        if request.kind == SurfaceKind::Window {
            return Err(PlatformError::NotSupported(
                "lx.surface window is not supported on this platform".to_string(),
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
                    jni_sig!("(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;JLjava/lang/String;IIDDDDII)Z"),
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
                        JValue::Int(request.role as i32),
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

    fn show_surface(&self, app_id: &str, id: &str) -> Result<(), PlatformError> {
        let surface_class: &JClass = super::get_cached_class(super::CachedClass::LxAppSurface)
            .map_err(|e| PlatformError::Platform(e.to_string()))?;
        super::with_env(|env| -> Result<(), PlatformError> {
            let id = env.new_string(id)?;
            let app_id = env.new_string(app_id)?;
            let ok = env
                .call_static_method(
                    surface_class,
                    jni_str!("show"),
                    jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Z"),
                    &[
                        JValue::Object(&JObject::from(id)),
                        JValue::Object(&JObject::from(app_id)),
                    ],
                )?
                .z()?;
            if ok {
                Ok(())
            } else {
                Err(PlatformError::Platform(
                    "Failed to show surface".to_string(),
                ))
            }
        })
        .map_err(|e| PlatformError::Platform(format!("Failed to show surface: {e}")))
    }

    fn hide_surface(&self, app_id: &str, id: &str) -> Result<(), PlatformError> {
        let surface_class: &JClass = super::get_cached_class(super::CachedClass::LxAppSurface)
            .map_err(|e| PlatformError::Platform(e.to_string()))?;
        super::with_env(|env| -> Result<(), PlatformError> {
            let id = env.new_string(id)?;
            let app_id = env.new_string(app_id)?;
            let ok = env
                .call_static_method(
                    surface_class,
                    jni_str!("hide"),
                    jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Z"),
                    &[
                        JValue::Object(&JObject::from(id)),
                        JValue::Object(&JObject::from(app_id)),
                    ],
                )?
                .z()?;
            if ok {
                Ok(())
            } else {
                Err(PlatformError::Platform(
                    "Failed to hide surface".to_string(),
                ))
            }
        })
        .map_err(|e| PlatformError::Platform(format!("Failed to hide surface: {e}")))
    }
}
