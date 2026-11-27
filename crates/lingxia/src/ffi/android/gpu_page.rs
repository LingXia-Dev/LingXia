//! `com.lingxia.lxapp.GpuPage` marshalling for the neutral GPU page binding.
use super::with_env;
use crate::gpu_page::{self, AutomationReply, Creation, NativeFactory, NativePage, Token};
use jni::objects::{Global, JClass, JObject, JString};
use jni::{Env, EnvUnowned, errors::ThrowRuntimeExAndDefault, jni_sig, jni_str};
use lxapp::{GpuViewGeneration, PageInstance};
use std::collections::HashMap;
use std::sync::{
    Arc, LazyLock, Mutex, OnceLock,
    atomic::{AtomicI64, Ordering},
};

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
                method!(
                    "created",
                    "(JLcom/lingxia/lxapp/GpuPage;Ljava/lang/String;)V",
                    created
                ),
                method!("presented", "(JJ)Z", presented),
                method!("restart", "(JJ)Z", restart),
                method!("canGoBack", "(J)Z", can_go_back),
                method!("goBack", "(J)Z", go_back),
                method!(
                    "automationReply",
                    "(JLjava/lang/String;)V",
                    automation_reply
                ),
                method!(
                    "embeddedWebView",
                    "(JLjava/lang/String;)Landroid/view/View;",
                    embedded_webview
                ),
            ],
        )?;
    }
    let global = env.new_global_ref(class)?;
    if CLASS.set(global).is_ok() {
        lxapp::register_gpu_page_view_factory(Arc::new(gpu_page::Binding(Arc::new(Factory))))
            .map_err(|_| jni::errors::Error::JniCall(jni::errors::JniError::Unknown))?;
        lingxia_view_android::set_image_loader(Arc::new(Images));
        lingxia_view_android::set_state_listener(Arc::new(States));
    }
    Ok(())
}

fn class() -> &'static Global<JClass<'static>> {
    CLASS.get().expect("GPU factory initialized")
}

fn runtime(error: jni::errors::Error) -> lingxia_error::Error {
    lingxia_error::Error::internal(error.to_string())
}

/// `GpuPage.<name>(token)`.
fn operation(
    token: Token,
    name: &'static jni::strings::JNIStr,
) -> Result<(), lingxia_error::Error> {
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
) -> Result<(), lingxia_error::Error> {
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
) -> Result<(), lingxia_error::Error> {
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
    fn connect(&self, generation: GpuViewGeneration) -> Result<(), lingxia_error::Error> {
        with_env(|env| {
            env.call_static_method(
                class(),
                jni_str!("connect"),
                jni_sig!("(JJ)V"),
                &[self.token.into(), (generation.get() as i64).into()],
            )?;
            Ok::<_, jni::errors::Error>(())
        })
        .map_err(runtime)
    }
    fn deliver(
        &self,
        generation: GpuViewGeneration,
        message: lxapp::GpuViewMessage,
    ) -> Result<(), lingxia_error::Error> {
        lingxia_view_android::deliver(
            self.token,
            generation.get(),
            gpu_page::state::message_to_view(message),
        );
        Ok(())
    }
    fn write_state(
        &self,
        generation: GpuViewGeneration,
        write: lxapp::GpuStateWrite,
    ) -> Result<(), lingxia_error::Error> {
        lingxia_view_android::write_state(
            self.token,
            generation.get(),
            gpu_page::state::to_view(write),
        );
        Ok(())
    }
    fn request_leave(
        &self,
        generation: GpuViewGeneration,
        reason: &str,
    ) -> Result<(), lingxia_error::Error> {
        bridge_operation(self.token, generation, reason, jni_str!("requestLeave"))
    }
    fn update_environment(&self, update: &serde_json::Value) -> Result<(), lingxia_error::Error> {
        value_operation(
            self.token,
            &update.to_string(),
            jni_str!("updateEnvironment"),
        )
    }
    fn reload(&self, source: &str) -> Result<(), lingxia_error::Error> {
        lingxia_view_android::mark_stale(self.token);
        value_operation(self.token, source, jni_str!("reload"))
    }
    fn swap_module(&self, source: &str) -> Result<(), lingxia_error::Error> {
        value_operation(self.token, source, jni_str!("swapModule"))
    }
    fn park(&self) -> Result<(), lingxia_error::Error> {
        lingxia_view_android::mark_stale(self.token);
        operation(self.token, jni_str!("park"))
    }
    fn page_left(&self) -> Result<(), lingxia_error::Error> {
        lingxia_view_android::page_left(self.token);
        Ok(())
    }
    fn retain_surface(&self, retained: bool) -> Result<(), lingxia_error::Error> {
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
    fn automation(&self, request: serde_json::Value, reply: AutomationReply) {
        let id = keep_reply(reply);
        let token = self.token;
        let sent = with_env(|env| {
            let request = env.new_string(request.to_string())?;
            env.call_static_method(
                class(),
                jni_str!("automate"),
                jni_sig!("(JLjava/lang/String;J)V"),
                &[token.into(), (&request).into(), id.into()],
            )?;
            Ok::<_, jni::errors::Error>(())
        });
        if let Err(error) = sent
            && let Some(reply) = take_reply(id)
        {
            reply(Err(format!("page is not active: {error}")));
        }
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

static NEXT_REPLY: AtomicI64 = AtomicI64::new(1);

/// Automation replies awaiting their page's UI thread.
fn replies() -> std::sync::MutexGuard<'static, HashMap<i64, AutomationReply>> {
    static REPLIES: LazyLock<Mutex<HashMap<i64, AutomationReply>>> =
        LazyLock::new(Default::default);
    REPLIES.lock().unwrap_or_else(|p| p.into_inner())
}

/// Keeps `reply` for `GpuPage.automationReply`; the id Java answers with.
pub(super) fn keep_reply(reply: AutomationReply) -> i64 {
    let id = NEXT_REPLY.fetch_add(1, Ordering::Relaxed);
    replies().insert(id, reply);
    id
}

/// A kept reply Java will never answer.
pub(super) fn take_reply(id: i64) -> Option<AutomationReply> {
    replies().remove(&id)
}

extern "system" fn automation_reply<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    id: i64,
    reply: JString<'a>,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let reply = reply.try_to_string(env)?;
        // Take it out before running host code.
        let pending = replies().remove(&id);
        if let Some(pending) = pending {
            pending(gpu_page::decode_reply(&reply));
        }
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>();
}

/// Hears the View's typed reports and events for Logic, by page token.
struct States;

impl lingxia_view_android::StateListener for States {
    fn state(
        &self,
        owner: i64,
        connection: u64,
        signal: lingxia_view_android::lingxia_view::session::state::StateSignal,
    ) {
        gpu_page::state_signal(owner, connection, gpu_page::state::from_view(signal));
    }
    fn logic(
        &self,
        owner: i64,
        connection: u64,
        messages: Vec<lingxia_view_android::lingxia_view::session::logic::ToLogic>,
    ) {
        #[cfg(feature = "standard")]
        if let Some(logic) = super::overlay::logic_of(owner) {
            return logic(messages);
        }
        gpu_page::view_events(owner, connection, messages);
    }
    fn video_orientation(
        &self,
        owner: i64,
        orientation: Option<lingxia_view_android::lingxia_view::scene::video::Orientation>,
    ) {
        gpu_page::video_orientation(owner, orientation.map(gpu_page::state::page_orientation));
    }
    fn console(&self, owner: i64, level: log::Level, message: &str) {
        gpu_page::console(owner, level, message);
    }
    fn failed(&self, owner: i64, error: &str) {
        gpu_page::failed(owner, error);
    }
    fn sync_webviews(
        &self,
        owner: i64,
        mounts: &[lingxia_view_android::lingxia_view::scene::webview::WebViewMount],
    ) {
        gpu_page::webviews::sync_webviews(
            owner,
            mounts,
            lingxia_view_android::webview_event,
            sync_android_webviews_ui,
        );
    }
    fn exit_webview_fullscreen(&self, owner: i64, web_id: &str) {
        gpu_page::webviews::exit_webview_fullscreen(owner, web_id);
    }
    fn clear_webviews(&self, owner: i64) {
        gpu_page::webviews::clear_webviews(owner, Some(clear_android_webviews_ui));
    }
}

fn sync_android_webviews_ui(
    token: Token,
    mounts: &[lingxia_view_android::lingxia_view::scene::webview::WebViewMount],
) {
    let items: Vec<serde_json::Value> = mounts
        .iter()
        .map(|m| {
            let key =
                lingxia_view_android::lingxia_view::scene::webview::make_key(m.scope, &m.web_id);
            let ready = gpu_page::webviews::is_ready(token, &m.web_id)
                || lingxia_view_android::lingxia_view::scene::webview::is_ready_key(&key);
            serde_json::json!({
                "key": key,
                "webId": m.web_id,
                "src": m.src,
                "bounds": m.bounds,
                "size": m.size,
                "visibleBounds": m.visible_bounds,
                "degraded": m.degraded,
                "accessibilityHidden": m.accessibility_hidden,
                "handlesDrags": m.handles_drags,
                "dark": m.dark,
                "ready": ready,
            })
        })
        .collect();
    let json = serde_json::Value::Array(items).to_string();
    let _ = value_operation(token, &json, jni_str!("syncWebViews"));
}

fn clear_android_webviews_ui(token: Token) {
    let _ = value_operation(token, "[]", jni_str!("syncWebViews"));
}

extern "system" fn embedded_webview<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    token: i64,
    web_id: JString<'a>,
) -> JObject<'a> {
    env.with_env(|env| -> Result<JObject<'a>, jni::errors::Error> {
        let web_id = web_id.try_to_string(env)?;
        gpu_page::webviews::java_webview(env, token, &web_id)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

struct Factory;

impl NativeFactory for Factory {
    fn create(
        &self,
        token: Token,
        page: &PageInstance,
        request: Creation,
    ) -> Result<(), lingxia_error::Error> {
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

extern "system" fn can_go_back(_: EnvUnowned<'_>, _: JClass<'_>, token: i64) -> jni::sys::jboolean {
    gpu_page::can_go_back(token)
}

extern "system" fn go_back(_: EnvUnowned<'_>, _: JClass<'_>, token: i64) -> jni::sys::jboolean {
    gpu_page::go_back(token)
}

extern "system" fn presented(
    _: EnvUnowned<'_>,
    _: JClass<'_>,
    token: i64,
    generation: i64,
) -> jni::sys::jboolean {
    gpu_page::presented(token, generation as u64)
}

/// Loads for GPU pages; the image owner is the page's token.
struct Images;

impl lingxia_view_android::ImageLoader for Images {
    fn load(
        &self,
        owner: i64,
        source: String,
        done: Box<dyn FnOnce(Result<Vec<u8>, String>) + Send>,
    ) -> Box<dyn Send> {
        Box::new(gpu_page::load_image(owner, source, done))
    }
    fn video_url(&self, owner: i64, source: &str) -> Result<String, String> {
        gpu_page::video_url(owner, source)
    }
}
