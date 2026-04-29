use crate::error::PlatformError;
use crate::traits::stream_decoder::{
    AudioCodec, AudioFrame, AudioStreamConfig, VideoCodec, VideoFormat, VideoFrame,
    VideoStreamConfig, VideoStreamDecoderHandle, VideoStreamDecoderManager,
};
use crate::traits::video_player::{
    VideoPlayerCommand, VideoPlayerHandle, VideoPlayerHandleImpl, VideoPlayerManager,
};
use jni::objects::{JClass, JThrowable, JValue};
use jni::signature::MethodSignature;
use jni::strings::JNIStr;
use jni::sys::jboolean;
use jni::{Env, jni_sig, jni_str};
use super::with_env;
use serde_json::json;

use super::Platform;

fn platform_error(context: &str, err: impl std::fmt::Display) -> PlatformError {
    PlatformError::Platform(format!("{}: {}", context, err))
}

fn with_env_and_class<T>(
    context: &str,
    f: impl FnOnce(&mut Env, &JClass) -> Result<T, PlatformError>,
) -> Result<T, PlatformError> {
    with_env(|env| {
        let class: &JClass = super::get_cached_class(super::CachedClass::ComponentRouter)
            .map_err(|e| platform_error(context, e))?;
        f(env, class)
    })
    .map_err(|e| platform_error(context, e))
}

/// Extract Java exception message if one is pending
fn extract_exception_message(env: &mut Env) -> Option<String> {
    if env.exception_check() {
        let exception = env.exception_occurred()?;
        env.exception_clear();

        if !exception.is_null() {
            // Try to get the exception message via toString()
            let throwable: JThrowable = exception;
            if let Ok(msg_obj) = env.call_method(
                &throwable,
                jni_str!("toString"),
                jni_sig!(() -> java.lang.String),
                &[],
            ) {
                if let Ok(msg_jstring) = msg_obj.l() {
                    if !msg_jstring.is_null() {
                        let msg_jstring = unsafe {
                            jni::objects::JString::from_raw(env, msg_jstring.into_raw() as _)
                        };
                        if let Ok(msg) = msg_jstring.try_to_string(env) {
                            return Some(msg);
                        }
                    }
                }
            }
        }
    }
    None
}

fn call_video_static_method<'sig, 'sig_args>(
    env: &mut Env,
    lxapp_video_class: &JClass,
    method: &JNIStr,
    signature: MethodSignature<'sig, 'sig_args>,
    args: &[JValue],
    failure_context: &str,
) -> Result<(), PlatformError> {
    env.call_static_method(lxapp_video_class, method, signature, args)
        .map_err(|e| platform_error(failure_context, e))?;

    if let Some(ex_msg) = extract_exception_message(env) {
        return Err(platform_error(
            failure_context,
            format!("Java exception: {}", ex_msg),
        ));
    }

    Ok(())
}

fn call_component_router_bool<'sig, 'sig_args>(
    env: &mut Env,
    lxapp_video_class: &JClass,
    method: &JNIStr,
    signature: MethodSignature<'sig, 'sig_args>,
    args: &[JValue],
    failure_context: &str,
) -> Result<bool, PlatformError> {
    let result = env
        .call_static_method(lxapp_video_class, method, signature, args)
        .map_err(|e| platform_error(failure_context, e))?;

    if let Some(ex_msg) = extract_exception_message(env) {
        return Err(platform_error(
            failure_context,
            format!("Java exception: {}", ex_msg),
        ));
    }

    result
        .z()
        .map_err(|e| platform_error(failure_context, format!("Bad result: {}", e)))
}

fn ensure_component_ok(component_id: &str, method: &str, ok: bool) -> Result<(), PlatformError> {
    if ok {
        Ok(())
    } else {
        Err(PlatformError::Platform(format!(
            "{} rejected for {}",
            method, component_id
        )))
    }
}

fn with_component_id<T>(
    component_id: &str,
    context: &str,
    f: impl FnOnce(&mut Env, &JClass, &jni::objects::JString) -> Result<T, PlatformError>,
) -> Result<T, PlatformError> {
    with_env_and_class(context, |env, lxapp_video_class| {
        let component_id_jstring = env
            .new_string(component_id)
            .map_err(|e| platform_error(context, e))?;
        f(env, lxapp_video_class, &component_id_jstring)
    })
}

fn dispatch_command_android(
    component_id: &str,
    command: VideoPlayerCommand,
) -> Result<(), PlatformError> {
    let (name, params_json) = map_command_to_android(command);

    let failure_context = format!(
        "dispatchVideoCommand for component {}, command {}",
        component_id, name
    );

    dispatch_video_command(component_id, &name, &params_json, &failure_context)
}

fn json_video_config(config: &VideoStreamConfig) -> Result<String, PlatformError> {
    let codec = match config.codec {
        VideoCodec::H264 => "h264",
        VideoCodec::H265 => "h265",
    };
    let format = match config.format {
        VideoFormat::AnnexB => "annexb",
        VideoFormat::Avcc => "avcc",
    };
    serde_json::to_string(&json!({
        "codec": codec,
        "format": format,
        "sps": config.sps,
        "pps": config.pps,
        "vps": config.vps,
        "nalLengthSize": config.nal_length_size,
        "width": config.width,
        "height": config.height,
    }))
    .map_err(|e| PlatformError::Platform(format!("Failed to serialize video config: {}", e)))
}

fn json_audio_config(config: &AudioStreamConfig) -> Result<String, PlatformError> {
    let codec = match config.codec {
        AudioCodec::Aac => "aac",
        AudioCodec::PcmS16le => "pcm_s16le",
    };
    serde_json::to_string(&json!({
        "codec": codec,
        "audioSpecificConfig": config.audio_specific_config,
        "sampleRate": config.sample_rate,
        "channels": config.channels,
        "aacIsAdts": config.aac_is_adts,
    }))
    .map_err(|e| PlatformError::Platform(format!("Failed to serialize audio config: {}", e)))
}

fn to_i32(value: u32, field: &str) -> Result<i32, PlatformError> {
    i32::try_from(value).map_err(|_| {
        PlatformError::Platform(format!("Stream decoder {} out of range: {}", field, value))
    })
}

struct AndroidStreamDecoderHandle {
    component_id: String,
}

impl VideoStreamDecoderHandle for AndroidStreamDecoderHandle {
    fn supports_soft_reset(&self) -> bool {
        true
    }

    fn supports_in_place_hard_reset(&self) -> bool {
        false
    }

    fn reset_stream(&self, hard: bool) -> Result<(), PlatformError> {
        let params_json = serde_json::to_string(&json!({"hard": hard})).map_err(|e| {
            PlatformError::Platform(format!("Failed to serialize reset params: {}", e))
        })?;
        let failure_context = format!(
            "dispatchVideoCommand(resetStream) for component {}",
            self.component_id
        );
        dispatch_video_command(
            &self.component_id,
            "resetStream",
            &params_json,
            &failure_context,
        )
    }

    fn configure_video(&self, config: VideoStreamConfig) -> Result<(), PlatformError> {
        let config_json = json_video_config(&config)?;
        let method = "configureStreamVideo";
        let ok = with_component_id(
            &self.component_id,
            method,
            |env, lxapp_video_class, component_id_jstring| {
                let config_jstring = env
                    .new_string(&config_json)
                    .map_err(|e| platform_error(method, e))?;
                call_component_router_bool(
                    env,
                    lxapp_video_class,
                    jni_str!("configureStreamVideo"),
                    jni_sig!((java.lang.String, java.lang.String) -> boolean),
                    &[
                        JValue::Object(component_id_jstring),
                        JValue::Object(&config_jstring),
                    ],
                    method,
                )
            },
        )?;
        ensure_component_ok(&self.component_id, method, ok)
    }

    fn configure_audio(&self, config: AudioStreamConfig) -> Result<(), PlatformError> {
        let config_json = json_audio_config(&config)?;
        let method = "configureStreamAudio";
        let ok = with_component_id(
            &self.component_id,
            method,
            |env, lxapp_video_class, component_id_jstring| {
                let config_jstring = env
                    .new_string(&config_json)
                    .map_err(|e| platform_error(method, e))?;
                call_component_router_bool(
                    env,
                    lxapp_video_class,
                    jni_str!("configureStreamAudio"),
                    jni_sig!((java.lang.String, java.lang.String) -> boolean),
                    &[
                        JValue::Object(component_id_jstring),
                        JValue::Object(&config_jstring),
                    ],
                    method,
                )
            },
        )?;
        ensure_component_ok(&self.component_id, method, ok)
    }

    fn push_video(&self, frame: VideoFrame) -> Result<(), PlatformError> {
        let VideoFrame {
            data,
            dts_ms,
            pts_ms,
            keyframe,
        } = frame;
        let dts_ms = to_i32(dts_ms, "dts_ms")?;
        let pts_ms = to_i32(pts_ms, "pts_ms")?;
        let method = "pushStreamVideo";
        let ok = with_component_id(
            &self.component_id,
            method,
            |env, lxapp_video_class, component_id_jstring| {
                let data_array = env
                    .byte_array_from_slice(&data)
                    .map_err(|e| platform_error(method, e))?;
                call_component_router_bool(
                    env,
                    lxapp_video_class,
                    jni_str!("pushStreamVideo"),
                    jni_sig!((java.lang.String, [byte], int, int, boolean) -> boolean),
                    &[
                        JValue::Object(component_id_jstring),
                        JValue::Object(&data_array),
                        JValue::Int(dts_ms),
                        JValue::Int(pts_ms),
                        JValue::Bool(jboolean::from(keyframe)),
                    ],
                    method,
                )
            },
        )?;
        ensure_component_ok(&self.component_id, method, ok)
    }

    fn push_audio(&self, frame: AudioFrame) -> Result<(), PlatformError> {
        let AudioFrame {
            data,
            dts_ms,
            pts_ms,
        } = frame;
        let dts_ms = to_i32(dts_ms, "dts_ms")?;
        let pts_ms = to_i32(pts_ms, "pts_ms")?;
        let method = "pushStreamAudio";
        let ok = with_component_id(
            &self.component_id,
            method,
            |env, lxapp_video_class, component_id_jstring| {
                let data_array = env
                    .byte_array_from_slice(&data)
                    .map_err(|e| platform_error(method, e))?;
                call_component_router_bool(
                    env,
                    lxapp_video_class,
                    jni_str!("pushStreamAudio"),
                    jni_sig!((java.lang.String, [byte], int, int) -> boolean),
                    &[
                        JValue::Object(component_id_jstring),
                        JValue::Object(&data_array),
                        JValue::Int(dts_ms),
                        JValue::Int(pts_ms),
                    ],
                    method,
                )
            },
        )?;
        ensure_component_ok(&self.component_id, method, ok)
    }

    fn stop(&self) -> Result<(), PlatformError> {
        let method = "stopStreamDecoder";
        let ok = with_component_id(
            &self.component_id,
            method,
            |env, lxapp_video_class, component_id_jstring| {
                call_component_router_bool(
                    env,
                    lxapp_video_class,
                    jni_str!("stopStreamDecoder"),
                    jni_sig!((java.lang.String) -> boolean),
                    &[JValue::Object(component_id_jstring)],
                    method,
                )
            },
        )?;
        ensure_component_ok(&self.component_id, method, ok)
    }
}

impl VideoPlayerManager for Platform {
    fn bind_player(&self, component_id: &str) -> Result<Box<dyn VideoPlayerHandle>, PlatformError> {
        let failure_context = format!("hasComponent for component {}", component_id);
        let exists = with_env_and_class(&failure_context, |env, component_router_class| {
            let component_id_jstring = env
                .new_string(component_id)
                .map_err(|e| platform_error(&failure_context, e))?;

            let result = env
                .call_static_method(
                    component_router_class,
                    jni_str!("hasComponent"),
                    jni_sig!((java.lang.String) -> boolean),
                    &[JValue::Object(&component_id_jstring)],
                )
                .map_err(|e| platform_error(&failure_context, e))?;

            if let Some(ex_msg) = extract_exception_message(env) {
                return Err(platform_error(
                    &failure_context,
                    format!("Java exception: {}", ex_msg),
                ));
            }

            result
                .z()
                .map_err(|e| platform_error(&failure_context, format!("Bad result: {}", e)))
        })?;

        if !exists {
            return Err(PlatformError::Platform(format!(
                "Video component not found: {}",
                component_id
            )));
        }

        let cid = component_id.to_string();
        let handle =
            VideoPlayerHandleImpl::new(move |command| dispatch_command_android(&cid, command));
        Ok(Box::new(handle))
    }

    fn set_player_callback(
        &self,
        component_id: &str,
        callback_id: u64,
    ) -> Result<(), PlatformError> {
        let failure_context = format!("setVideoPlayerCallback for component {}", component_id);
        with_env_and_class(&failure_context, |env, component_router_class| {
            let component_id_jstring = env
                .new_string(component_id)
                .map_err(|e| platform_error(&failure_context, e))?;

            let result = env
                .call_static_method(
                    component_router_class,
                    jni_str!("setVideoPlayerCallback"),
                    jni_sig!((java.lang.String, long) -> boolean),
                    &[
                        JValue::Object(&component_id_jstring),
                        JValue::Long(callback_id as i64),
                    ],
                )
                .map_err(|e| platform_error(&failure_context, e))?;

            if let Some(ex_msg) = extract_exception_message(env) {
                return Err(platform_error(
                    &failure_context,
                    format!("Java exception: {}", ex_msg),
                ));
            }

            result
                .z()
                .map_err(|e| platform_error(&failure_context, format!("Bad result: {}", e)))
                .and_then(|ok| ensure_component_ok(component_id, "setVideoPlayerCallback", ok))
        })?;
        Ok(())
    }
}

fn map_command_to_android(command: VideoPlayerCommand) -> (String, String) {
    match command {
        VideoPlayerCommand::Play => ("play".to_string(), "{}".to_string()),
        VideoPlayerCommand::Pause => ("pause".to_string(), "{}".to_string()),
        VideoPlayerCommand::Stop => ("stop".to_string(), "{}".to_string()),
        VideoPlayerCommand::NotifyEnded => ("notifyEnded".to_string(), "{}".to_string()),
        VideoPlayerCommand::Seek { position } => {
            ("seek".to_string(), json!({ "time": position }).to_string())
        }
        VideoPlayerCommand::SetDuration { duration } => (
            "setDuration".to_string(),
            json!({ "duration": duration }).to_string(),
        ),
        VideoPlayerCommand::EnterFullscreen => ("enterFullscreen".to_string(), "{}".to_string()),
        VideoPlayerCommand::ExitFullscreen => ("exitFullscreen".to_string(), "{}".to_string()),
    }
}

impl VideoStreamDecoderManager for Platform {
    fn create_stream_decoder(
        &self,
        component_id: &str,
    ) -> Result<Box<dyn VideoStreamDecoderHandle>, PlatformError> {
        let ok = with_component_id(
            component_id,
            "createStreamDecoder",
            |env, lxapp_video_class, component_id_jstring| {
                call_component_router_bool(
                    env,
                    lxapp_video_class,
                    jni_str!("createStreamDecoder"),
                    jni_sig!((java.lang.String) -> boolean),
                    &[JValue::Object(component_id_jstring)],
                    "createStreamDecoder",
                )
            },
        )?;
        if !ok {
            return Err(PlatformError::Platform(format!(
                "Failed to create stream decoder for {}",
                component_id
            )));
        }
        Ok(Box::new(AndroidStreamDecoderHandle {
            component_id: component_id.to_string(),
        }))
    }
}

fn dispatch_video_command(
    component_id: &str,
    name: &str,
    params_json: &str,
    failure_context: &str,
) -> Result<(), PlatformError> {
    with_env_and_class(failure_context, |env, lxapp_video_class| {
        let component_id_jstring = env
            .new_string(component_id)
            .map_err(|e| platform_error(failure_context, e))?;
        let name_jstring = env
            .new_string(name)
            .map_err(|e| platform_error(failure_context, e))?;
        let params_jstring = env
            .new_string(params_json)
            .map_err(|e| platform_error(failure_context, e))?;

        call_video_static_method(
            env,
            lxapp_video_class,
            jni_str!("dispatchVideoCommand"),
            jni_sig!((java.lang.String, java.lang.String, java.lang.String) -> void),
            &[
                JValue::Object(&component_id_jstring),
                JValue::Object(&name_jstring),
                JValue::Object(&params_jstring),
            ],
            failure_context,
        )
    })
}
