use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::UIUpdate;
use jni::objects::{JClass, JValue};

impl UIUpdate for Platform {
    fn update_navbar_ui(&self, appid: String) -> Result<(), PlatformError> {
        // Call Android updateNavBarUI method to trigger immediate UI update
        match || -> Result<(), Box<dyn std::error::Error>> {
            let mut env = lingxia_webview::get_env()?;

            // Get the LxApp class
            let lxapp_class: &JClass = super::get_lxapp_class()?.as_obj().into();

            // Convert parameters to Java objects
            let appid_jstring = env.new_string(&appid)?;

            // Call the static updateNavBarUI method on LxApp
            let result = env.call_static_method(
                lxapp_class,
                "updateNavBarUI",
                "(Ljava/lang/String;)Z",
                &[JValue::Object(&appid_jstring.into())],
            )?;

            let success = result.z()?;
            if success {
                Ok(())
            } else {
                Err("updateNavBarUI returned false".into())
            }
        }() {
            Ok(()) => Ok(()),
            Err(e) => Err(PlatformError::Platform(format!(
                "Failed to update NavigationBar UI for appId: {}: {}",
                appid, e
            ))),
        }
    }

    fn update_tabbar_ui(&self, appid: String) -> Result<(), PlatformError> {
        match || -> Result<(), Box<dyn std::error::Error>> {
            let mut env = lingxia_webview::get_env()?;

            // Get the LxApp class
            let lxapp_class: &JClass = super::get_lxapp_class()?.as_obj().into();

            // Convert parameters to Java objects
            let appid_jstring = env.new_string(&appid)?;

            // Call the static updateTabBarUI method on LxApp
            let result = env.call_static_method(
                lxapp_class,
                "updateTabBarUI",
                "(Ljava/lang/String;)Z",
                &[JValue::Object(&appid_jstring.into())],
            )?;

            let success = result.z()?;
            if success {
                Ok(())
            } else {
                Err("updateTabBarUI returned false".into())
            }
        }() {
            Ok(()) => Ok(()),
            Err(e) => Err(PlatformError::Platform(format!(
                "Failed to update TabBar UI for appId: {}: {}",
                appid, e
            ))),
        }
    }
}
