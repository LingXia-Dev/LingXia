use std::sync::OnceLock;

use async_trait::async_trait;
use jni::objects::{Global, JClass, JObject, JString, JValue};
use jni::{jni_sig, jni_str};
use serde_json::{Value, json};

use super::{Platform, with_env};
use crate::error::PlatformError;
use crate::traits::gesture::{
    AppGesture, AppGestureRequest, AppGestureResult, ChromeElement, FlingDirection,
    GestureDelivery, GestureEnd, GestureInput, GestureTarget, PullRefreshIndicator,
};

const CLASS: &str = "com.lingxia.lxapp.APIs.LxAppGesture";

/// Loaded through the SDK's class loader: JNI threads started from Rust
/// resolve `FindClass` against the system loader, which has no app classes.
fn gesture_class(env: &mut jni::Env) -> Result<&'static Global<JClass<'static>>, PlatformError> {
    static CLASS_REF: OnceLock<Global<JClass<'static>>> = OnceLock::new();
    if let Some(class) = CLASS_REF.get() {
        return Ok(class);
    }
    let anchor: &JClass = super::get_cached_class(super::CachedClass::Lingxia)
        .map_err(|e| PlatformError::Platform(e.to_string()))?;
    let loader = env
        .call_method(
            anchor,
            jni_str!("getClassLoader"),
            jni_sig!("()Ljava/lang/ClassLoader;"),
            &[],
        )?
        .l()?;
    let name = env.new_string(CLASS)?;
    let class = env
        .call_method(
            &loader,
            jni_str!("loadClass"),
            jni_sig!("(Ljava/lang/String;)Ljava/lang/Class;"),
            &[JValue::Object(&name)],
        )?
        .l()?;
    let class = env.cast_local::<JClass>(class)?;
    let global = env.new_global_ref(class)?;
    Ok(CLASS_REF.get_or_init(|| global))
}

fn call(method: &'static str, args: [&str; 3], arity3: bool) -> Result<Value, PlatformError> {
    with_env(|env| -> Result<Value, PlatformError> {
        let class = gesture_class(env)?;
        let a = env.new_string(args[0])?;
        let b = env.new_string(args[1])?;
        let result = if arity3 {
            let c = env.new_string(args[2])?;
            env.call_static_method(
                class,
                jni::strings::JNIString::new(method),
                jni_sig!(
                    "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;"
                ),
                &[JValue::Object(&a), JValue::Object(&b), JValue::Object(&c)],
            )
        } else {
            env.call_static_method(
                class,
                jni::strings::JNIString::new(method),
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;"),
                &[JValue::Object(&a), JValue::Object(&b)],
            )
        };
        if env.exception_check() {
            env.exception_clear();
            return Err(PlatformError::Platform(format!("{CLASS}.{method} threw")));
        }
        let obj: JObject = result?.l()?;
        if obj.is_null() {
            return Err(PlatformError::Platform(format!(
                "{CLASS}.{method} returned null"
            )));
        }
        let text = unsafe { JString::from_raw(env, obj.into_raw() as _) }
            .try_to_string(env)
            .map_err(|e| PlatformError::Platform(e.to_string()))?;
        let value: Value = serde_json::from_str(&text)
            .map_err(|e| PlatformError::Platform(format!("{CLASS}.{method}: {e}")))?;
        match value["error"].as_str() {
            Some(error) => Err(PlatformError::Platform(error.to_owned())),
            None => Ok(value),
        }
    })
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, PlatformError> + Send + 'static,
) -> Result<T, PlatformError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| PlatformError::Platform(format!("gesture worker: {e}")))?
}

fn perform(request: AppGestureRequest) -> Result<AppGestureResult, PlatformError> {
    if let Some(input) = request.input
        && input != GestureInput::Touch
    {
        return Err(PlatformError::NotSupported(format!(
            "{input:?} gestures are not available on Android; use touch"
        )));
    }
    let (app_id, webtag) = match &request.target {
        GestureTarget::Page { app_id, webtag, .. } => (app_id.clone(), webtag.clone()),
        // Window gestures land in the presenting activity, whatever lxapp it shows.
        GestureTarget::Window { .. } => (String::new(), String::new()),
    };
    let points = request
        .shape
        .points()
        .map_err(PlatformError::InvalidParameter)?;
    let end = match request.end {
        GestureEnd::Up => "up",
        GestureEnd::Cancel => "cancel",
        GestureEnd::Hold => "hold",
    };
    let body = json!({
        "points": points.iter().map(|p| [p.x, p.y, p.at_ms]).collect::<Vec<_>>(),
        "end": end,
        "finger": request.finger.unwrap_or(0),
    })
    .to_string();
    let reply = call("perform", [&app_id, &webtag, &body], true)?;
    Ok(AppGestureResult {
        input: GestureInput::Touch,
        delivered: match reply["delivered"].as_str() {
            Some("window") => GestureDelivery::Window,
            _ => GestureDelivery::View,
        },
        events: reply["events"].as_u64().unwrap_or(0) as u32,
        duration_ms: reply["durationMs"].as_f64().unwrap_or(0.0),
        finger: reply["finger"].as_u64(),
    })
}

#[async_trait]
impl AppGesture for Platform {
    fn gesture_inputs(&self) -> Vec<GestureInput> {
        vec![GestureInput::Touch]
    }

    async fn perform_app_gesture(
        &self,
        request: AppGestureRequest,
    ) -> Result<AppGestureResult, PlatformError> {
        blocking(move || perform(request)).await
    }

    async fn gesture_viewport(&self, target: GestureTarget) -> Result<[f64; 4], PlatformError> {
        let (app_id, webtag) = match target {
            GestureTarget::Page { app_id, webtag, .. } => (app_id, webtag),
            GestureTarget::Window { .. } => (String::new(), String::new()),
        };
        let reply = blocking(move || call("viewport", [&app_id, &webtag, ""], false)).await?;
        let field = |name: &str| reply[name].as_f64().unwrap_or(0.0);
        Ok([field("left"), field("top"), field("width"), field("height")])
    }

    async fn chrome_elements(
        &self,
        scope: Option<String>,
    ) -> Result<Vec<ChromeElement>, PlatformError> {
        let scope = scope.unwrap_or_default();
        let reply = blocking(move || call("chrome", ["", &scope, ""], false)).await?;
        serde_json::from_value(reply["elements"].clone())
            .map_err(|e| PlatformError::Platform(format!("chrome elements: {e}")))
    }

    async fn pull_refresh_indicator(
        &self,
        app_id: &str,
        webtag: &str,
    ) -> Result<Option<PullRefreshIndicator>, PlatformError> {
        let (app_id, webtag) = (app_id.to_owned(), webtag.to_owned());
        let reply =
            blocking(move || call("pullRefreshState", [&app_id, &webtag, ""], false)).await?;
        Ok(reply["phase"].as_str().map(|phase| PullRefreshIndicator {
            phase: phase.to_owned(),
            offset: reply["offset"].as_f64().unwrap_or(0.0),
        }))
    }

    async fn perform_edge_back(
        &self,
        window_id: Option<String>,
        edge: FlingDirection,
    ) -> Result<AppGestureResult, PlatformError> {
        let _ = (window_id, edge);
        Err(PlatformError::NotSupported(
            "edge back is a system gesture on Android; an app cannot inject into System UI".into(),
        ))
    }
}
