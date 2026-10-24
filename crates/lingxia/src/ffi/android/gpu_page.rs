use super::with_env;
use jni::objects::{Global, JClass, JObject, JString};
use jni::{Env, EnvUnowned, errors::ThrowRuntimeExAndDefault, jni_sig, jni_str};
use lxapp::{
    GpuPageView, GpuPageViewFactory, GpuViewCreation, GpuViewGeneration, LxAppError, PageInstance,
    PageViewEnvironment,
};
use std::{
    collections::HashMap,
    sync::{
        Arc, LazyLock, Mutex, OnceLock, Weak,
        atomic::{AtomicBool, AtomicI64, Ordering},
    },
};
use tokio::sync::oneshot;

static CLASS: OnceLock<Global<JClass<'static>>> = OnceLock::new();
struct JavaPage {
    object: Global<JObject<'static>>,
}
type Created = Result<Arc<JavaPage>, String>;
static PENDING: LazyLock<Mutex<HashMap<i64, oneshot::Sender<Created>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static LIVE: LazyLock<Mutex<HashMap<String, (i64, Weak<Endpoint>)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static NEXT: AtomicI64 = AtomicI64::new(1);
type ImageCancellation = (oneshot::Sender<()>, Option<oneshot::Receiver<()>>);
static IMAGE_REQUESTS: LazyLock<Mutex<HashMap<i64, ImageCancellation>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static NEXT_IMAGE: AtomicI64 = AtomicI64::new(1);

pub(super) fn initialize(env: &mut Env) -> Result<(), jni::errors::Error> {
    if CLASS.get().is_some() {
        return Ok(());
    }
    let class = env.find_class(jni_str!("com/lingxia/lxapp/GpuPage"))?;
    // Bind the exact Java callback signature and retain it in the final host library.
    unsafe {
        env.register_native_methods(
            &class,
            &[
                jni::NativeMethod::from_raw_parts(
                    jni_str!("readImage"),
                    jni_str!("(JJLjava/lang/String;)[B"),
                    Java_com_lingxia_lxapp_GpuPage_readImage as *const () as *mut std::ffi::c_void,
                ),
                jni::NativeMethod::from_raw_parts(
                    jni_str!("beginImage"),
                    jni_str!("()J"),
                    begin_image as *const () as *mut std::ffi::c_void,
                ),
                jni::NativeMethod::from_raw_parts(
                    jni_str!("cancelImage"),
                    jni_str!("(J)V"),
                    cancel_image as *const () as *mut std::ffi::c_void,
                ),
                jni::NativeMethod::from_raw_parts(
                    jni_str!("message"),
                    jni_str!("(JJLjava/lang/String;)V"),
                    Java_com_lingxia_lxapp_GpuPage_message as *const () as *mut std::ffi::c_void,
                ),
                jni::NativeMethod::from_raw_parts(
                    jni_str!("created"),
                    jni_str!("(JLcom/lingxia/lxapp/GpuPage;Ljava/lang/String;)V"),
                    Java_com_lingxia_lxapp_GpuPage_created as *const () as *mut std::ffi::c_void,
                ),
                jni::NativeMethod::from_raw_parts(
                    jni_str!("presented"),
                    jni_str!("(JJ)V"),
                    Java_com_lingxia_lxapp_GpuPage_presented as *const () as *mut std::ffi::c_void,
                ),
                jni::NativeMethod::from_raw_parts(
                    jni_str!("restart"),
                    jni_str!("(JJ)Z"),
                    Java_com_lingxia_lxapp_GpuPage_restart as *const () as *mut std::ffi::c_void,
                ),
            ],
        )?;
    }
    let global = env.new_global_ref(class)?;
    if CLASS.set(global).is_ok() {
        lxapp::register_gpu_page_view_factory(Arc::new(Factory))
            .map_err(|_| jni::errors::Error::JniCall(jni::errors::JniError::Unknown))?;
    }
    Ok(())
}

fn operation(token: i64, name: &str) -> Result<(), LxAppError> {
    with_env(|env| {
        env.call_static_method(
            CLASS.get().expect("GPU factory initialized"),
            jni::strings::JNIString::new(name),
            jni_sig!("(J)V"),
            &[token.into()],
        )?;
        Ok::<_, jni::errors::Error>(())
    })
    .map_err(|error| LxAppError::Runtime(error.to_string()))
}

struct Lease {
    token: i64,
    page: String,
    disposed: AtomicBool,
}
impl Lease {
    fn dispose(&self) {
        if self.disposed.swap(true, Ordering::AcqRel) {
            return;
        }
        PENDING.lock().unwrap().remove(&self.token);
        {
            let mut live = LIVE.lock().unwrap();
            if live
                .get(&self.page)
                .is_some_and(|(token, _)| *token == self.token)
            {
                live.remove(&self.page);
            }
        }
        if let Err(error) = operation(self.token, "dispose") {
            log::warn!("GPU page disposal failed: {error}");
        }
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.dispose();
    }
}

struct Endpoint {
    lease: Lease,
    object: Arc<JavaPage>,
    generation: Mutex<Option<GpuViewGeneration>>,
}
fn bridge_operation(
    token: i64,
    generation: GpuViewGeneration,
    value: &str,
    name: &str,
) -> Result<(), LxAppError> {
    with_env(|env| {
        let value = env.new_string(value)?;
        env.call_static_method(
            CLASS.get().expect("GPU factory initialized"),
            jni::strings::JNIString::new(name),
            jni_sig!("(JJLjava/lang/String;)V"),
            &[
                token.into(),
                (generation.get() as i64).into(),
                (&value).into(),
            ],
        )?;
        Ok::<_, jni::errors::Error>(())
    })
    .map_err(|error| LxAppError::Runtime(error.to_string()))
}
impl GpuPageView for Endpoint {
    fn post_message(&self, _: &str) -> Result<(), LxAppError> {
        Err(LxAppError::Runtime(
            "GPU messages require a captured connection generation".into(),
        ))
    }
    fn post_message_for_generation(
        &self,
        generation: GpuViewGeneration,
        message: &str,
    ) -> Result<(), LxAppError> {
        if self.lease.disposed.load(Ordering::Acquire)
            || *self.generation.lock().unwrap() != Some(generation)
        {
            return Ok(());
        }
        bridge_operation(self.lease.token, generation, message, "receive")
    }
    fn connect(&self, generation: GpuViewGeneration, nonce: &str) -> Result<(), LxAppError> {
        {
            let mut current = self.generation.lock().unwrap();
            if current.is_some_and(|current| current.get() > generation.get()) {
                return Err(LxAppError::Bridge("GPU connection was superseded".into()));
            }
            *current = Some(generation);
        }
        bridge_operation(self.lease.token, generation, nonce, "connect")
    }
    fn request_leave(&self, reason: lxapp::LeaveReason) -> Result<(), LxAppError> {
        if self.lease.disposed.load(Ordering::Acquire) {
            return Ok(());
        }
        let Some(generation) = *self.generation.lock().unwrap() else {
            return Ok(());
        };
        let reason = match reason {
            lxapp::LeaveReason::Back => "back",
            lxapp::LeaveReason::Home => "home",
        };
        bridge_operation(self.lease.token, generation, reason, "requestLeave")
    }
    fn update_environment(&self, update: PageViewEnvironment) -> Result<(), LxAppError> {
        if self.lease.disposed.load(Ordering::Acquire) {
            return Ok(());
        }
        let update = match update {
            PageViewEnvironment::DisplayLanguage { language, revision } => {
                serde_json::json!({"kind":"language","revision":revision.to_string(),"language":language})
            }
            PageViewEnvironment::SurfaceContext { context, revision } => {
                serde_json::json!({"kind":"surface","revision":revision.to_string(),"context":context})
            }
        };
        with_env(|env| {
            let update = env.new_string(update.to_string())?;
            env.call_static_method(
                CLASS.get().expect("GPU factory initialized"),
                jni_str!("updateEnvironment"),
                jni_sig!("(JLjava/lang/String;)V"),
                &[self.lease.token.into(), (&update).into()],
            )?;
            Ok::<_, jni::errors::Error>(())
        })
        .map_err(|error| LxAppError::Runtime(error.to_string()))
    }
    fn reload(&self) -> Result<(), LxAppError> {
        let page = lxapp::find_page_by_instance_id(&self.lease.page)
            .ok_or_else(|| LxAppError::Runtime("GPU page was removed".into()))?;
        let source = page.gpu_module_source()?;
        *self.generation.lock().unwrap() = None;
        with_env(|env| {
            let source = env.new_string(source)?;
            env.call_static_method(
                CLASS.get().expect("GPU factory initialized"),
                jni_str!("reload"),
                jni_sig!("(JLjava/lang/String;)V"),
                &[self.lease.token.into(), (&source).into()],
            )?;
            Ok::<_, jni::errors::Error>(())
        })
        .map_err(|error| LxAppError::Runtime(error.to_string()))
    }
    fn park(&self) -> Result<(), LxAppError> {
        *self.generation.lock().unwrap() = None;
        operation(self.lease.token, "park")
    }
    fn dispose(&self) {
        self.lease.dispose();
    }
}

fn begin_creation(page: &PageInstance) -> Result<(Lease, oneshot::Receiver<Created>), LxAppError> {
    let source = page.gpu_module_source()?;
    let token = NEXT.fetch_add(1, Ordering::Relaxed);
    let lease = Lease {
        token,
        page: page.instance_id_string(),
        disposed: AtomicBool::new(false),
    };
    let (tx, rx) = oneshot::channel();
    PENDING.lock().unwrap().insert(token, tx);
    with_env(|env| {
        let appid = env.new_string(page.appid())?;
        let path = env.new_string(page.path())?;
        let tag = env.new_string(page.webtag().to_string())?;
        let source = env.new_string(source)?;
        env.call_static_method(
            CLASS.get().expect("GPU factory initialized"),
            jni_str!("create"),
            jni_sig!(
                "(JLjava/lang/String;JLjava/lang/String;Ljava/lang/String;Ljava/lang/String;Z)V"
            ),
            &[
                token.into(),
                (&appid).into(),
                (page.app_session_id() as i64).into(),
                (&path).into(),
                (&tag).into(),
                (&source).into(),
                page.logic_enabled().into(),
            ],
        )?;
        Ok::<_, jni::errors::Error>(())
    })
    .map_err(|error| LxAppError::Runtime(error.to_string()))?;

    Ok((lease, rx))
}

struct Factory;
impl GpuPageViewFactory for Factory {
    fn create(&self, page: PageInstance) -> GpuViewCreation {
        Box::pin(async move {
            let (lease, rx) = begin_creation(&page)?;
            let token = lease.token;
            let object = rx
                .await
                .map_err(|_| LxAppError::Runtime("GPU creation cancelled".into()))?
                .map_err(LxAppError::Runtime)?;
            let endpoint = Arc::new(Endpoint {
                lease,
                object,
                generation: Mutex::new(None),
            });
            LIVE.lock().unwrap().insert(
                page.instance_id_string(),
                (token, Arc::downgrade(&endpoint)),
            );
            Ok(endpoint as Arc<dyn GpuPageView>)
        })
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_GpuPage_message<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    token: i64,
    generation: i64,
    message: JString<'a>,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let endpoint = LIVE
            .lock()
            .unwrap()
            .values()
            .find_map(|(id, endpoint)| (*id == token).then(|| endpoint.upgrade()).flatten());
        if let Some(endpoint) = endpoint
            && !endpoint.lease.disposed.load(Ordering::Acquire)
        {
            let current = *endpoint.generation.lock().unwrap();
            if let Some(current) = current
                && current.get() == generation as u64
                && let Some(page) = lxapp::find_page_by_instance_id(&endpoint.lease.page)
            {
                let body = message.try_to_string(env)?;
                if let Err(error) = page.handle_incoming_gpu_message(
                    &(endpoint as Arc<dyn GpuPageView>),
                    current,
                    &body,
                ) {
                    log::warn!("GPU bridge ingress rejected: {error}");
                }
            }
        }
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>();
}

pub(super) fn java_view<'a>(
    env: &mut Env<'a>,
    page: &PageInstance,
) -> Result<JObject<'a>, jni::errors::Error> {
    let endpoint = LIVE
        .lock()
        .unwrap()
        .get(&page.instance_id_string())
        .and_then(|(_, endpoint)| endpoint.upgrade());
    match endpoint {
        Some(endpoint) if !endpoint.lease.disposed.load(Ordering::Acquire) => {
            env.new_local_ref(&endpoint.object.object)
        }
        _ => Ok(JObject::null()),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_GpuPage_created<'a>(
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
            Ok(Arc::new(JavaPage {
                object: env.new_global_ref(page)?,
            }))
        };
        let pending = PENDING.lock().unwrap().remove(&token);
        if let Some(pending) = pending {
            let _ = pending.send(outcome);
        }
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_GpuPage_restart(
    _: EnvUnowned<'_>,
    _: JClass<'_>,
    token: i64,
    generation: i64,
) -> jni::sys::jboolean {
    let endpoint = LIVE
        .lock()
        .unwrap()
        .values()
        .find_map(|(id, endpoint)| (*id == token).then(|| endpoint.upgrade()).flatten());
    let Some(endpoint) = endpoint else {
        return false.into();
    };
    // Release both registry and generation locks before reload calls host code.
    let current = *endpoint.generation.lock().unwrap();
    if endpoint.lease.disposed.load(Ordering::Acquire)
        || !current.map_or(generation == 0, |current| {
            current.get() == generation as u64
        })
    {
        return false.into();
    }
    let Some(page) = lxapp::find_page_by_instance_id(&endpoint.lease.page) else {
        return false.into();
    };
    match page.reload_gpu_view(&(endpoint as Arc<dyn GpuPageView>)) {
        Ok(()) => true.into(),
        Err(error) => {
            log::warn!("GPU page recovery failed: {error}");
            false.into()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_GpuPage_presented(
    _: EnvUnowned<'_>,
    _: JClass<'_>,
    token: i64,
    generation: i64,
) {
    let endpoint = LIVE
        .lock()
        .unwrap()
        .values()
        .find_map(|(id, endpoint)| (*id == token).then(|| endpoint.upgrade()).flatten());
    if let Some(endpoint) = endpoint
        && !endpoint.lease.disposed.load(Ordering::Acquire)
        && endpoint
            .generation
            .lock()
            .unwrap()
            .map_or(generation == 0, |current| {
                current.get() == generation as u64
            })
        && let Some(page) = lxapp::find_page_by_instance_id(&endpoint.lease.page)
    {
        page.gpu_frame_presented(&(endpoint as Arc<dyn GpuPageView>));
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_GpuPage_readImage<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    token: i64,
    request: i64,
    source: JString<'a>,
) -> jni::sys::jbyteArray {
    env.with_env(|env| -> Result<jni::sys::jbyteArray, jni::errors::Error> {
        let cancellation = IMAGE_REQUESTS
            .lock()
            .unwrap()
            .get_mut(&request)
            .and_then(|(_, receiver)| receiver.take());
        let Some(cancellation) = cancellation else {
            return Ok(std::ptr::null_mut());
        };
        let endpoint = LIVE
            .lock()
            .unwrap()
            .values()
            .find_map(|(id, endpoint)| (*id == token).then(|| endpoint.upgrade()).flatten());
        let Some(endpoint) =
            endpoint.filter(|endpoint| !endpoint.lease.disposed.load(Ordering::Acquire))
        else {
            return Ok(std::ptr::null_mut());
        };
        let Some(page) = lxapp::find_page_by_instance_id(&endpoint.lease.page) else {
            return Ok(std::ptr::null_mut());
        };
        let source = source.try_to_string(env)?;
        // Called on the bounded Java image IO pool, never the UI/View thread.
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        std::mem::drop(rong_rt::RongExecutor::global().spawn(async move {
            let result = tokio::select! {
                biased;
                _ = cancellation => return,
                result = page.load_gpu_image_bytes(&source) => result,
            };
            let _ = send.send(result);
        }));
        let Ok(result) = receive.recv() else {
            return Ok(std::ptr::null_mut());
        };
        match result {
            Ok(bytes) => Ok(env.byte_array_from_slice(&bytes)?.into_raw()),
            Err(error) => {
                log::warn!("GPU image resource failed: {error}");
                Ok(std::ptr::null_mut())
            }
        }
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

extern "system" fn begin_image(_: EnvUnowned<'_>, _: JClass<'_>) -> i64 {
    let request = NEXT_IMAGE.fetch_add(1, Ordering::Relaxed);
    let (send, receive) = oneshot::channel();
    IMAGE_REQUESTS
        .lock()
        .unwrap()
        .insert(request, (send, Some(receive)));
    request
}

extern "system" fn cancel_image(_: EnvUnowned<'_>, _: JClass<'_>, request: i64) {
    if let Some((send, _)) = IMAGE_REQUESTS.lock().unwrap().remove(&request) {
        let _ = send.send(());
    }
}
