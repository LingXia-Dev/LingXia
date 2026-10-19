use super::{CachedClass, Platform, get_cached_class, with_env};
use crate::error::PlatformError;
use crate::traits::audio::{AudioBackend, AudioDeviceCapabilities};
use jni::objects::{JClass, JValue};
use jni::{jni_sig, jni_str};

fn audio_class() -> Result<&'static JClass<'static>, PlatformError> {
    get_cached_class(CachedClass::LxAppAudio)
        .map(|class| class.as_ref())
        .map_err(|e| PlatformError::Platform(e.to_string()))
}

impl AudioBackend for Platform {
    fn audio_capabilities(&self) -> AudioDeviceCapabilities {
        let bits = audio_class()
            .and_then(|class| {
                with_env(|env| -> Result<i32, PlatformError> {
                    Ok(env
                        .call_static_method(class, jni_str!("capabilities"), jni_sig!("()I"), &[])?
                        .i()?)
                })
            })
            .unwrap_or_else(|error| {
                log::warn!("audio capabilities unavailable: {error}");
                0
            });
        AudioDeviceCapabilities {
            earpiece: bits & 1 != 0,
            proximity: bits & 2 != 0,
        }
    }

    fn audio_attach(&self, event_callback_id: u64) -> Result<(), PlatformError> {
        let class = audio_class()?;
        with_env(|env| -> Result<(), PlatformError> {
            env.call_static_method(
                class,
                jni_str!("attach"),
                jni_sig!("(J)V"),
                &[JValue::Long(event_callback_id as i64)],
            )?;
            Ok(())
        })
    }

    fn audio_send(&self, command_json: &str) -> Result<(), PlatformError> {
        let class = audio_class()?;
        with_env(|env| -> Result<(), PlatformError> {
            let json = env.new_string(command_json)?;
            env.call_static_method(
                class,
                jni_str!("send"),
                jni_sig!("(Ljava/lang/String;)V"),
                &[JValue::Object(&json)],
            )?;
            Ok(())
        })
    }
}
