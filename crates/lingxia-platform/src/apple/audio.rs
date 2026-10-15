use super::Platform;
use crate::traits::audio::AudioBackend;

#[cfg(any(target_os = "ios", target_os = "macos"))]
use crate::{error::PlatformError, traits::audio::AudioDeviceCapabilities};

#[cfg(any(target_os = "ios", target_os = "macos"))]
impl AudioBackend for Platform {
    fn audio_capabilities(&self) -> AudioDeviceCapabilities {
        let bits = super::ffi::audio_capabilities();
        AudioDeviceCapabilities {
            earpiece: bits & 1 != 0,
            proximity: bits & 2 != 0,
        }
    }

    fn audio_attach(&self, event_callback_id: u64) -> Result<(), PlatformError> {
        super::ffi::audio_attach(event_callback_id);
        Ok(())
    }

    fn audio_send(&self, command_json: &str) -> Result<(), PlatformError> {
        super::ffi::audio_send(command_json);
        Ok(())
    }
}

#[cfg(not(any(target_os = "ios", target_os = "macos")))]
impl AudioBackend for Platform {}
