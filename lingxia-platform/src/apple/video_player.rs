use crate::error::PlatformError;
#[cfg(target_os = "ios")]
use crate::traits::stream_decoder::{AudioFrame, AudioStreamConfig, VideoFrame, VideoStreamConfig};
use crate::traits::stream_decoder::{VideoStreamDecoderHandle, VideoStreamDecoderManager};
use crate::traits::video_player::{VideoPlayerHandle, VideoPlayerManager};

use super::Platform;

#[cfg(target_os = "ios")]
use crate::traits::video_player::{VideoPlayerCommand, VideoPlayerHandleImpl};

#[cfg(target_os = "ios")]
use super::ffi;

#[cfg(target_os = "ios")]
use base64::{Engine as _, engine::general_purpose};
#[cfg(target_os = "ios")]
use serde_json::json;

#[cfg(target_os = "ios")]
/// iOS implementation delegates to native component video players.
/// Native player is created by UI layer; this returns a handle.
impl VideoPlayerManager for Platform {
    fn bind_player(&self, component_id: &str) -> Result<Box<dyn VideoPlayerHandle>, PlatformError> {
        let cid = component_id.to_string();
        let handle = VideoPlayerHandleImpl::new(move |command| {
            let (name, params_json) = map_command_to_ios(command);
            ffi::dispatch_video_command(&cid, &name, &params_json)
                .then_some(())
                .ok_or_else(|| PlatformError::Platform(format!("Failed to dispatch {}", name)))
        });
        Ok(Box::new(handle))
    }

    fn set_player_callback(
        &self,
        component_id: &str,
        callback_id: u64,
    ) -> Result<(), PlatformError> {
        ffi::set_video_player_callback(component_id, callback_id)
            .then_some(())
            .ok_or_else(|| {
                PlatformError::Platform(format!(
                    "Failed to set video player callback for {}",
                    component_id
                ))
            })
    }
}

#[cfg(target_os = "ios")]
fn map_command_to_ios(command: VideoPlayerCommand) -> (String, String) {
    const EMPTY: &str = "{}";

    match command {
        VideoPlayerCommand::Play => ("play".into(), EMPTY.into()),
        VideoPlayerCommand::Pause => ("pause".into(), EMPTY.into()),
        VideoPlayerCommand::Stop => ("stop".into(), EMPTY.into()),
        VideoPlayerCommand::NotifyEnded => ("notifyEnded".into(), EMPTY.into()),
        VideoPlayerCommand::Seek { position } => {
            ("seek".into(), format!(r#"{{"time":{}}}"#, position))
        }
        VideoPlayerCommand::SetDuration { duration } => (
            "setDuration".into(),
            format!(r#"{{"duration":{}}}"#, duration),
        ),
        VideoPlayerCommand::EnterFullscreen => ("enterFullscreen".into(), EMPTY.into()),
        VideoPlayerCommand::ExitFullscreen => ("exitFullscreen".into(), EMPTY.into()),
    }
}

#[cfg(target_os = "ios")]
fn ios_video_config_json(config: &VideoStreamConfig) -> Result<String, PlatformError> {
    let codec = match config.codec {
        crate::traits::stream_decoder::VideoCodec::H264 => "h264",
        crate::traits::stream_decoder::VideoCodec::H265 => "h265",
    };
    let format = match config.format {
        crate::traits::stream_decoder::VideoFormat::AnnexB => "annexb",
        crate::traits::stream_decoder::VideoFormat::Avcc => "avcc",
    };

    let sps_b64 = general_purpose::STANDARD.encode(&config.sps);
    let pps_b64 = general_purpose::STANDARD.encode(&config.pps);
    let vps_b64 = general_purpose::STANDARD.encode(&config.vps);

    serde_json::to_string(&json!({
        "codec": codec,
        "format": format,
        "sps": sps_b64,
        "pps": pps_b64,
        "vps": vps_b64,
        "nalLengthSize": config.nal_length_size,
        "width": config.width,
        "height": config.height,
    }))
    .map_err(|e| PlatformError::Platform(format!("Failed to serialize video config: {}", e)))
}

#[cfg(target_os = "ios")]
fn ios_audio_config_json(config: &AudioStreamConfig) -> Result<String, PlatformError> {
    let codec = match config.codec {
        crate::traits::stream_decoder::AudioCodec::Aac => "aac",
        crate::traits::stream_decoder::AudioCodec::PcmS16le => "pcm_s16le",
    };

    let asc_b64 = general_purpose::STANDARD.encode(&config.audio_specific_config);

    serde_json::to_string(&json!({
        "codec": codec,
        "audioSpecificConfig": asc_b64,
        "sampleRate": config.sample_rate,
        "channels": config.channels,
        "aacIsAdts": config.aac_is_adts,
    }))
    .map_err(|e| PlatformError::Platform(format!("Failed to serialize audio config: {}", e)))
}

#[cfg(target_os = "ios")]
struct IosStreamDecoderHandle {
    component_id: String,
}

#[cfg(target_os = "ios")]
impl VideoStreamDecoderHandle for IosStreamDecoderHandle {
    fn supports_soft_reset(&self) -> bool {
        true
    }

    fn supports_in_place_hard_reset(&self) -> bool {
        true
    }

    fn reset_stream(&self, hard: bool) -> Result<(), PlatformError> {
        let params_json = serde_json::to_string(&json!({ "hard": hard })).map_err(|e| {
            PlatformError::Platform(format!("Failed to serialize reset params: {}", e))
        })?;
        ffi::dispatch_video_command(&self.component_id, "resetStream", &params_json)
            .then_some(())
            .ok_or_else(|| {
                PlatformError::Platform(format!("resetStream rejected for {}", self.component_id))
            })
    }

    fn configure_video(&self, config: VideoStreamConfig) -> Result<(), PlatformError> {
        let config_json = ios_video_config_json(&config)?;
        ffi::configure_stream_video(&self.component_id, &config_json)
            .then_some(())
            .ok_or_else(|| {
                PlatformError::Platform(format!(
                    "configureStreamVideo rejected for {}",
                    self.component_id
                ))
            })
    }

    fn configure_audio(&self, config: AudioStreamConfig) -> Result<(), PlatformError> {
        let config_json = ios_audio_config_json(&config)?;
        ffi::configure_stream_audio(&self.component_id, &config_json)
            .then_some(())
            .ok_or_else(|| {
                PlatformError::Platform(format!(
                    "configureStreamAudio rejected for {}",
                    self.component_id
                ))
            })
    }

    fn push_video(&self, frame: VideoFrame) -> Result<(), PlatformError> {
        ffi::push_stream_video(
            &self.component_id,
            frame.data,
            frame.dts_ms,
            frame.pts_ms,
            frame.keyframe,
        )
        .then_some(())
        .ok_or_else(|| {
            PlatformError::Platform(format!(
                "pushStreamVideo rejected for {}",
                self.component_id
            ))
        })
    }

    fn push_audio(&self, frame: AudioFrame) -> Result<(), PlatformError> {
        ffi::push_stream_audio(&self.component_id, frame.data, frame.dts_ms, frame.pts_ms)
            .then_some(())
            .ok_or_else(|| {
                PlatformError::Platform(format!(
                    "pushStreamAudio rejected for {}",
                    self.component_id
                ))
            })
    }

    fn stop(&self) -> Result<(), PlatformError> {
        ffi::stop_stream_decoder(&self.component_id)
            .then_some(())
            .ok_or_else(|| {
                PlatformError::Platform(format!(
                    "stopStreamDecoder rejected for {}",
                    self.component_id
                ))
            })
    }
}

#[cfg(not(target_os = "ios"))]
impl VideoPlayerManager for Platform {
    fn bind_player(
        &self,
        _component_id: &str,
    ) -> Result<Box<dyn VideoPlayerHandle>, PlatformError> {
        Err(PlatformError::Platform(
            "Video player control is not supported on this platform".to_string(),
        ))
    }
}

#[cfg(target_os = "ios")]
impl VideoStreamDecoderManager for Platform {
    fn create_stream_decoder(
        &self,
        component_id: &str,
    ) -> Result<Box<dyn VideoStreamDecoderHandle>, PlatformError> {
        if !ffi::create_stream_decoder(component_id) {
            return Err(PlatformError::Platform(format!(
                "Failed to create stream decoder for {}",
                component_id
            )));
        }
        Ok(Box::new(IosStreamDecoderHandle {
            component_id: component_id.to_string(),
        }))
    }
}

#[cfg(not(target_os = "ios"))]
impl VideoStreamDecoderManager for Platform {
    fn create_stream_decoder(
        &self,
        _component_id: &str,
    ) -> Result<Box<dyn VideoStreamDecoderHandle>, PlatformError> {
        Err(PlatformError::Platform(
            "Stream decoder is not supported on this platform".to_string(),
        ))
    }
}
