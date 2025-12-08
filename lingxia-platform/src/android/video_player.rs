use crate::error::PlatformError;
use crate::traits::{
    VideoPlayerCommand, VideoPlayerHandle, VideoPlayerHandleImpl, VideoPlayerManager,
};
use jni::JNIEnv;
use jni::objects::{JClass, JThrowable, JValue};
use lingxia_webview::get_env;

use super::Platform;

fn platform_error(context: &str, err: impl std::fmt::Display) -> PlatformError {
    PlatformError::Platform(format!("{}: {}", context, err))
}

fn with_env_and_class<T>(
    context: &str,
    f: impl FnOnce(&mut JNIEnv, &JClass) -> Result<T, PlatformError>,
) -> Result<T, PlatformError> {
    let mut env = get_env().map_err(|e| platform_error(context, e))?;
    let class: &JClass = super::get_cached_class(super::CachedClass::LxAppVideo)
        .map_err(|e| platform_error(context, e))?
        .as_obj()
        .into();

    f(&mut env, class)
}

/// Extract Java exception message if one is pending
fn extract_exception_message(env: &mut JNIEnv) -> Option<String> {
    if env.exception_check().unwrap_or(false) {
        let exception = env.exception_occurred().ok()?;
        env.exception_clear().ok()?;

        if !exception.is_null() {
            // Try to get the exception message via toString()
            let throwable: JThrowable = exception;
            if let Ok(msg_obj) =
                env.call_method(&throwable, "toString", "()Ljava/lang/String;", &[])
            {
                if let Ok(msg_jstring) = msg_obj.l() {
                    if !msg_jstring.is_null() {
                        if let Ok(msg) = env.get_string((&msg_jstring).into()) {
                            return Some(msg.into());
                        }
                    }
                }
            }
        }
    }
    None
}

fn call_video_static_method(
    env: &mut JNIEnv,
    lxapp_video_class: &JClass,
    method: &str,
    signature: &str,
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

fn dispatch_command_android(
    component_id: &str,
    command: VideoPlayerCommand,
) -> Result<(), PlatformError> {
    let (name, params_json) = map_command_to_android(command);

    let failure_context = format!(
        "dispatchVideoCommand for component {}, command {}",
        component_id, name
    );

    with_env_and_class(&failure_context, |env, lxapp_video_class| {
        let component_id_jstring = env
            .new_string(component_id)
            .map_err(|e| platform_error(&failure_context, e))?;
        let name_jstring = env
            .new_string(&name)
            .map_err(|e| platform_error(&failure_context, e))?;
        let params_jstring = env
            .new_string(&params_json)
            .map_err(|e| platform_error(&failure_context, e))?;

        call_video_static_method(
            env,
            lxapp_video_class,
            "dispatchVideoCommand",
            "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",
            &[
                JValue::Object(&component_id_jstring),
                JValue::Object(&name_jstring),
                JValue::Object(&params_jstring),
            ],
            &failure_context,
        )
    })
}

impl VideoPlayerManager for Platform {
    fn bind_player(
        &self,
        component_id: &str,
        event_callback_id: u64,
    ) -> Result<Box<dyn VideoPlayerHandle>, PlatformError> {
        // Register callback for this component (used by VideoPlayerRegistry for command routing)
        let failure_context = format!("setVideoPlayerCallback for component {}", component_id);

        with_env_and_class(&failure_context, |env, lxapp_video_class| {
            let component_id_jstring = env
                .new_string(component_id)
                .map_err(|e| platform_error(&failure_context, e))?;

            call_video_static_method(
                env,
                lxapp_video_class,
                "setVideoPlayerCallback",
                "(Ljava/lang/String;J)V",
                &[
                    JValue::Object(&component_id_jstring),
                    JValue::Long(event_callback_id as i64),
                ],
                &failure_context,
            )
        })?;

        let cid = component_id.to_string();
        let handle =
            VideoPlayerHandleImpl::new(move |command| dispatch_command_android(&cid, command));
        Ok(Box::new(handle))
    }
}

fn map_command_to_android(command: VideoPlayerCommand) -> (String, String) {
    use serde_json::json;

    match command {
        VideoPlayerCommand::Play => ("play".to_string(), "{}".to_string()),
        VideoPlayerCommand::Pause => ("pause".to_string(), "{}".to_string()),
        VideoPlayerCommand::Stop => ("stop".to_string(), "{}".to_string()),
        VideoPlayerCommand::Seek { position } => {
            ("seek".to_string(), json!({ "time": position }).to_string())
        }
        VideoPlayerCommand::EnterFullscreen => ("enterFullscreen".to_string(), "{}".to_string()),
        VideoPlayerCommand::ExitFullscreen => ("exitFullscreen".to_string(), "{}".to_string()),
    }
}
