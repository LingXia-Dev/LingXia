//! `com.lingxia.lxapp.GpuPage` marshalling for the neutral GPU page binding.
use super::with_env;
use crate::gpu_page::{self, Creation, NativeFactory, NativePage, Token};
use jni::objects::{Global, JClass, JObject, JString};
use jni::{Env, EnvUnowned, errors::ThrowRuntimeExAndDefault, jni_sig, jni_str};
use lxapp::{GpuViewGeneration, LxAppError, PageInstance};
use std::sync::{Arc, OnceLock};

static CLASS: OnceLock<Global<JClass<'static>>> = OnceLock::new();

pub(super) fn initialize(env: &mut Env) -> Result<(), jni::errors::Error> {
    if CLASS.get().is_some() {
        return Ok(());
    }
    let class = env.find_class(jni_str!("com/lingxia/lxapp/GpuPage"))?;
    macro_rules! method {
        ($name:literal, $signature:literal, $function:path) => {
            jni::NativeMethod::from_raw_parts(
                jni_str!($name),
                jni_str!($signature),
                $function as *const () as *mut std::ffi::c_void,
            )
        };
    }
    // Bind the exact Java callback signatures and retain them in the final host library.
    // SAFETY: each function's Rust signature matches its JNI signature.
    unsafe {
        env.register_native_methods(
            &class,
            &[
                method!("message", "(JJLjava/lang/String;)V", message),
                method!(
                    "created",
                    "(JLcom/lingxia/lxapp/GpuPage;Ljava/lang/String;)V",
                    created
                ),
                method!("presented", "(JJ)V", presented),
                method!("restart", "(JJ)Z", restart),
            ],
        )?;
    }
    let global = env.new_global_ref(class)?;
    if CLASS.set(global).is_ok() {
        lxapp::register_gpu_page_view_factory(Arc::new(gpu_page::Binding(Arc::new(Factory))))
            .map_err(|_| jni::errors::Error::JniCall(jni::errors::JniError::Unknown))?;
        lingxia_ui_android::set_image_loader(Arc::new(Images));
    }
    Ok(())
}

fn class() -> &'static Global<JClass<'static>> {
    CLASS.get().expect("GPU factory initialized")
}

fn runtime(error: jni::errors::Error) -> LxAppError {
    LxAppError::Runtime(error.to_string())
}

/// `GpuPage.<name>(token)`.
fn operation(token: Token, name: &'static jni::strings::JNIStr) -> Result<(), LxAppError> {
    with_env(|env| {
        env.call_static_method(class(), name, jni_sig!("(J)V"), &[token.into()])?;
        Ok::<_, jni::errors::Error>(())
    })
    .map_err(runtime)
}

/// `GpuPage.<name>(token, generation, value)`.
fn bridge_operation(
    token: Token,
    generation: GpuViewGeneration,
    value: &str,
    name: &'static jni::strings::JNIStr,
) -> Result<(), LxAppError> {
    with_env(|env| {
        let value = env.new_string(value)?;
        env.call_static_method(
            class(),
            name,
            jni_sig!("(JJLjava/lang/String;)V"),
            &[
                token.into(),
                (generation.get() as i64).into(),
                (&value).into(),
            ],
        )?;
        Ok::<_, jni::errors::Error>(())
    })
    .map_err(runtime)
}

/// `GpuPage.<name>(token, value)`.
fn value_operation(
    token: Token,
    value: &str,
    name: &'static jni::strings::JNIStr,
) -> Result<(), LxAppError> {
    with_env(|env| {
        let value = env.new_string(value)?;
        env.call_static_method(
            class(),
            name,
            jni_sig!("(JLjava/lang/String;)V"),
            &[token.into(), (&value).into()],
        )?;
        Ok::<_, jni::errors::Error>(())
    })
    .map_err(runtime)
}

struct AndroidPage {
    token: Token,
    object: Global<JObject<'static>>,
}

impl NativePage for AndroidPage {
    fn connect(&self, generation: GpuViewGeneration, nonce: &str) -> Result<(), LxAppError> {
        bridge_operation(self.token, generation, nonce, jni_str!("connect"))
    }
    fn receive(&self, generation: GpuViewGeneration, frame: &str) -> Result<(), LxAppError> {
        bridge_operation(self.token, generation, frame, jni_str!("receive"))
    }
    fn request_leave(&self, generation: GpuViewGeneration, reason: &str) -> Result<(), LxAppError> {
        bridge_operation(self.token, generation, reason, jni_str!("requestLeave"))
    }
    fn update_environment(&self, update: &serde_json::Value) -> Result<(), LxAppError> {
        value_operation(
            self.token,
            &update.to_string(),
            jni_str!("updateEnvironment"),
        )
    }
    fn reload(&self, source: &str) -> Result<(), LxAppError> {
        value_operation(self.token, source, jni_str!("reload"))
    }
    fn park(&self) -> Result<(), LxAppError> {
        operation(self.token, jni_str!("park"))
    }
    fn retain_surface(&self, retained: bool) -> Result<(), LxAppError> {
        let token = self.token;
        with_env(|env| {
            env.call_static_method(
                class(),
                jni_str!("retainSurface"),
                jni_sig!("(JZ)V"),
                &[token.into(), retained.into()],
            )?;
            Ok::<_, jni::errors::Error>(())
        })
        .map_err(runtime)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct Factory;

impl NativeFactory for Factory {
    fn create(
        &self,
        token: Token,
        page: &PageInstance,
        request: Creation,
    ) -> Result<(), LxAppError> {
        with_env(|env| {
            let appid = env.new_string(page.appid())?;
            let worker = env.new_string(&request.worker)?;
            let path = env.new_string(page.path())?;
            let tag = env.new_string(page.webtag().to_string())?;
            let source = env.new_string(request.source)?;
            env.call_static_method(
                class(),
                jni_str!("create"),
                jni_sig!(
                    "(JLjava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Z)V"
                ),
                &[
                    token.into(),
                    (&appid).into(),
                    (&worker).into(),
                    (&path).into(),
                    (&tag).into(),
                    (&source).into(),
                    request.logic.into(),
                ],
            )?;
            Ok::<_, jni::errors::Error>(())
        })
        .map_err(runtime)
    }

    fn dispose(&self, token: Token) {
        if let Err(error) = operation(token, jni_str!("dispose")) {
            log::warn!("GPU page disposal failed: {error}");
        }
    }
}

/// The Java view of a live GPU page, or null.
pub(super) fn java_view<'a>(
    env: &mut Env<'a>,
    page: &PageInstance,
) -> Result<JObject<'a>, jni::errors::Error> {
    let native = gpu_page::native_view(page);
    match native
        .as_ref()
        .and_then(|native| native.as_any().downcast_ref::<AndroidPage>())
    {
        Some(page) => env.new_local_ref(&page.object),
        None => Ok(JObject::null()),
    }
}

extern "system" fn message<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    token: i64,
    generation: i64,
    message: JString<'a>,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let body = message.try_to_string(env)?;
        gpu_page::message(token, generation as u64, &body);
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>();
}

extern "system" fn created<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    token: i64,
    page: JObject<'a>,
    error: JString<'a>,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let outcome = if !error.is_null() {
            Err(error.try_to_string(env)?)
        } else if page.is_null() {
            Err("GPU creation returned no view".into())
        } else {
            Ok(Arc::new(AndroidPage {
                token,
                object: env.new_global_ref(page)?,
            }) as Arc<dyn NativePage>)
        };
        gpu_page::created(token, outcome);
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>();
}

extern "system" fn restart(
    _: EnvUnowned<'_>,
    _: JClass<'_>,
    token: i64,
    generation: i64,
) -> jni::sys::jboolean {
    gpu_page::restart(token, generation as u64)
}

extern "system" fn presented(_: EnvUnowned<'_>, _: JClass<'_>, token: i64, generation: i64) {
    gpu_page::presented(token, generation as u64);
}

/// Loads for GPU pages; the image owner is the page's token.
struct Images;

impl lingxia_ui_android::ImageLoader for Images {
    fn load(
        &self,
        owner: i64,
        source: String,
        done: Box<dyn FnOnce(Result<Vec<u8>, String>) + Send>,
    ) -> Box<dyn Send> {
        Box::new(gpu_page::load_image(owner, source, done))
    }
}
