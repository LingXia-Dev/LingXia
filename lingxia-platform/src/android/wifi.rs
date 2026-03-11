use crate::error::PlatformError;
use crate::traits::wifi::{Wifi, WifiConnectRequest, WifiGetConnectedRequest};
use jni::objects::{JClass, JValue};
use jni::strings::JNIString;
use jni::{jni_sig, jni_str};

use super::Platform;

// Helper function to get WiFi class reference
fn get_wifi_class() -> Result<&'static JClass<'static>, PlatformError> {
    super::get_cached_class(super::CachedClass::LxAppWifi)
        .map(|class_ref| class_ref.as_ref())
        .map_err(|e| PlatformError::Platform(format!("Failed to get LxAppWifi class: {}", e)))
}

// Helper to call simple JNI methods with callback_id
fn call_wifi_method(method_name: &str, callback_id: u64) -> Result<(), PlatformError> {
    let wifi_class = get_wifi_class()?;
    let method = JNIString::new(method_name);

    lingxia_webview::platform::android::with_env(|env| {
        env.call_static_method(
            wifi_class,
            &method,
            jni_sig!("(J)V"),
            &[JValue::Long(callback_id as i64)],
        )
        .map_err(|e| {
            PlatformError::Platform(format!("JNI call to {} failed: {}", method_name, e))
        })?;

        Ok(())
    })
}

impl Wifi for Platform {
    fn start_wifi(&self, callback_id: u64) -> Result<(), PlatformError> {
        call_wifi_method("startWifi", callback_id)
    }

    fn stop_wifi(&self, callback_id: u64) -> Result<(), PlatformError> {
        call_wifi_method("stopWifi", callback_id)
    }

    fn connect_wifi(&self, request: WifiConnectRequest) -> Result<(), PlatformError> {
        let wifi_class = get_wifi_class()?;

        lingxia_webview::platform::android::with_env(|env| -> Result<(), PlatformError> {
            let ssid_jstring = env.new_string(&request.ssid).map_err(|e| {
                PlatformError::Platform(format!("Failed to create SSID string: {}", e))
            })?;
            let password_jstring = env
                .new_string(request.password.as_deref().unwrap_or(""))
                .map_err(|e| {
                    PlatformError::Platform(format!("Failed to create password string: {}", e))
                })?;

            env.call_static_method(
                wifi_class,
                jni_str!("connectWifi"),
                jni_sig!("(JLjava/lang/String;Ljava/lang/String;)V"),
                &[
                    JValue::Long(request.callback_id as i64),
                    JValue::Object(&ssid_jstring),
                    JValue::Object(&password_jstring),
                ],
            )
            .map_err(|e| PlatformError::Platform(format!("Failed to connect WiFi: {}", e)))?;

            Ok(())
        })
    }

    fn get_wifi_list(&self, callback_id: u64) -> Result<(), PlatformError> {
        call_wifi_method("getWifiList", callback_id)
    }

    fn get_connected_wifi(&self, request: WifiGetConnectedRequest) -> Result<(), PlatformError> {
        call_wifi_method("getConnectedWifi", request.callback_id)
    }

    fn is_wifi_enabled(&self) -> Result<bool, PlatformError> {
        let wifi_class = get_wifi_class()?;

        lingxia_webview::platform::android::with_env(|env| -> Result<bool, PlatformError> {
            let result = env
                .call_static_method(wifi_class, jni_str!("isWifiEnabled"), jni_sig!("()Z"), &[])
                .map_err(|e| {
                    PlatformError::Platform(format!("Failed to check WiFi status: {}", e))
                })?;

            Ok(result.z().map_err(|e| {
                PlatformError::Platform(format!("Failed to parse WiFi status: {}", e))
            })?)
        })
    }

    fn add_wifi_state_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        call_wifi_method("addWifiStateListener", callback_id)
    }

    fn remove_wifi_state_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        call_wifi_method("removeWifiStateListener", callback_id)
    }
}
