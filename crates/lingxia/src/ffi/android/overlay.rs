//! The Android window overlay surface: `com.lingxia.lxapp.GpuOverlays` shows a see-through GPU
//! page, whose Logic is Rust, in an application panel sub-window over the presenting activity,
//! which is window 1. Java makes only the window and the page's host; Rust loads the module and
//! connects it, then sends what it held meanwhile, in order.
use super::with_env;
use crate::feedback::Feedback;
use jni::objects::{Global, JClass};
use jni::{Env, EnvUnowned, jni_sig, jni_str};
use lingxia_view_android::lingxia_view::platform::{
    OverlayAutomationReply, OverlayInput, OverlayLogic, OverlaySurface, Task, WindowId,
    WindowOverlays,
};
use lingxia_view_android::lingxia_view::session::logic::ToView;
use lingxia_view_android::lingxia_view::session::state::StateWrite;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

/// The presenting activity's window; Java moves the panels when the activity changes.
const WINDOW: WindowId = WindowId(1);
/// The connection every overlay page takes.
const CONNECTION: u64 = 1;

static CLASS: OnceLock<Global<JClass<'static>>> = OnceLock::new();
static FEEDBACK: OnceLock<Arc<Feedback>> = OnceLock::new();
/// Negative, so overlay tokens never meet lxapp page tokens.
static NEXT: AtomicI64 = AtomicI64::new(-1);
/// Live overlays' Logic by page token, for the View worker's messages.
static OVERLAYS: LazyLock<Mutex<HashMap<i64, OverlayLogic>>> = LazyLock::new(Default::default);
/// Overlays whose host Java has not made yet: their module, and what they were sent meanwhile.
static HELD: LazyLock<Mutex<HashMap<i64, Pending>>> = LazyLock::new(Default::default);

struct Pending {
    /// Taken once the page starts connecting; writes keep queuing until it is connected.
    module: Option<&'static str>,
    queue: Vec<Held>,
}

enum Held {
    State(StateWrite),
    Message(ToView),
}

fn overlays() -> MutexGuard<'static, HashMap<i64, OverlayLogic>> {
    OVERLAYS.lock().unwrap_or_else(|p| p.into_inner())
}

fn held() -> MutexGuard<'static, HashMap<i64, Pending>> {
    HELD.lock().unwrap_or_else(|p| p.into_inner())
}

/// Bind `GpuOverlays` and install the window feedback layer over it; from a Java thread, once.
pub(super) fn initialize(env: &mut Env) -> Result<(), jni::errors::Error> {
    if CLASS.get().is_some() {
        return Ok(());
    }
    let class = env.find_class(jni_str!("com/lingxia/lxapp/GpuOverlays"))?;
    macro_rules! method {
        ($name:literal, $signature:literal, $function:path) => {
            jni::NativeMethod::from_raw_parts(
                jni_str!($name),
                jni_str!($signature),
                $function as *const () as *mut std::ffi::c_void,
            )
        };
    }
    // SAFETY: each function's Rust signature matches its JNI signature.
    unsafe {
        env.register_native_methods(
            &class,
            &[
                method!("opened", "(J)V", opened),
                method!("closed", "(J)V", closed),
                method!("run", "(J)V", run),
            ],
        )?;
    }
    if CLASS.set(env.new_global_ref(class)?).is_ok() {
        let _ = FEEDBACK.set(crate::feedback::install(Arc::new(AndroidOverlays)));
    }
    Ok(())
}

/// View worker: overlay `token`'s Logic, or `None` when `token` is an lxapp page.
pub(super) fn logic_of(token: i64) -> Option<OverlayLogic> {
    overlays().get(&token).cloned()
}

/// `GpuOverlays.<name>(token, flag)`.
fn call(name: &'static jni::strings::JNIStr, token: i64, flag: Option<bool>) {
    let Some(class) = CLASS.get() else { return };
    let sent = with_env(|env| {
        match flag {
            Some(flag) => env.call_static_method(
                class,
                name,
                jni_sig!("(JZ)V"),
                &[token.into(), flag.into()],
            )?,
            None => env.call_static_method(class, name, jni_sig!("(J)V"), &[token.into()])?,
        };
        Ok::<_, jni::errors::Error>(())
    });
    if let Err(error) = sent {
        log::warn!("window overlay call failed: {error}");
    }
}

struct AndroidOverlays;

impl WindowOverlays for AndroidOverlays {
    fn window_of(&self, _: Option<&str>) -> Option<WindowId> {
        Some(WINDOW)
    }

    fn open(
        &self,
        window: WindowId,
        module: &'static str,
        logic: OverlayLogic,
    ) -> Result<Box<dyn OverlaySurface>, String> {
        let class = CLASS.get().ok_or("the window overlay is not bound")?;
        let token = NEXT.fetch_sub(1, Ordering::Relaxed);
        overlays().insert(token, logic);
        held().insert(
            token,
            Pending {
                module: Some(module),
                queue: Vec::new(),
            },
        );
        let sent = with_env(|env| {
            env.call_static_method(
                class,
                jni_str!("open"),
                jni_sig!("(JJ)V"),
                &[token.into(), (window.0 as i64).into()],
            )?;
            Ok::<_, jni::errors::Error>(())
        });
        if let Err(error) = sent {
            overlays().remove(&token);
            held().remove(&token);
            return Err(format!("the window overlay did not open: {error}"));
        }
        Ok(Box::new(AndroidSurface { token }))
    }

    fn after(&self, delay: Duration, task: Task) {
        let Some(class) = CLASS.get() else { return };
        let raw = Box::into_raw(Box::new(task)) as i64;
        let delay = i64::try_from(delay.as_millis()).unwrap_or(i64::MAX);
        let sent = with_env(|env| {
            env.call_static_method(
                class,
                jni_str!("after"),
                jni_sig!("(JJ)V"),
                &[raw.into(), delay.into()],
            )?;
            Ok::<_, jni::errors::Error>(())
        });
        if let Err(error) = sent {
            // SAFETY: Java never took `raw`, so it is still only ours.
            drop(unsafe { Box::from_raw(raw as *mut Task) });
            log::warn!("window overlay timer dropped: {error}");
        }
    }
}

struct AndroidSurface {
    token: i64,
}

impl AndroidSurface {
    /// Held until the page is connected, so nothing goes ahead of what came before it.
    fn deliver(&self, item: Held) {
        let mut held = held();
        match held.get_mut(&self.token) {
            Some(pending) => pending.queue.push(item),
            None => apply(self.token, item),
        }
    }
}

fn apply(token: i64, item: Held) {
    match item {
        Held::State(write) => lingxia_view_android::write_state(token, CONNECTION, write),
        Held::Message(message) => lingxia_view_android::deliver(token, CONNECTION, message),
    };
}

impl OverlaySurface for AndroidSurface {
    fn write_state(&self, write: StateWrite) {
        self.deliver(Held::State(write));
    }

    fn send(&self, message: ToView) {
        self.deliver(Held::Message(message));
    }

    fn set_input(&self, input: OverlayInput) {
        call(
            jni_str!("input"),
            self.token,
            Some(input == OverlayInput::Block),
        );
    }

    fn set_focus(&self, focused: bool) {
        call(jni_str!("focus"), self.token, Some(focused));
    }

    fn automation(&self, request: serde_json::Value, reply: OverlayAutomationReply) {
        let id = super::gpu_page::keep_reply(reply);
        let sent = CLASS
            .get()
            .ok_or_else(|| "unbound".to_string())
            .and_then(|class| {
                with_env(|env| {
                    let request = env.new_string(request.to_string())?;
                    env.call_static_method(
                        class,
                        jni_str!("automate"),
                        jni_sig!("(JLjava/lang/String;J)V"),
                        &[self.token.into(), (&request).into(), id.into()],
                    )?;
                    Ok::<_, jni::errors::Error>(())
                })
                .map_err(|error| error.to_string())
            });
        if let Err(error) = sent
            && let Some(reply) = super::gpu_page::take_reply(id)
        {
            reply(Err(format!("the window overlay is not active: {error}")));
        }
    }
}

impl Drop for AndroidSurface {
    fn drop(&mut self) {
        overlays().remove(&self.token);
        held().remove(&self.token);
        call(jni_str!("close"), self.token, None);
    }
}

/// Main thread: overlay `token`'s host exists. The module loads from Rust, never crossing JNI;
/// then the page connects and what was held for it goes, in order.
extern "system" fn opened(_: EnvUnowned<'_>, _: JClass<'_>, token: i64) {
    let Some(module) = held()
        .get_mut(&token)
        .and_then(|pending| pending.module.take())
    else {
        return;
    };
    // No lock across the load: the page may answer through this surface while it runs.
    if !lingxia_view_android::load_and_connect(token, module.to_owned(), CONNECTION) {
        log::warn!("window overlay {token}: its page has no host");
        held().remove(&token);
        return;
    }
    let mut held = held();
    if let Some(pending) = held.remove(&token) {
        for item in pending.queue {
            apply(token, item);
        }
    }
}

/// Main thread: `window` can show no overlay; what it holds settles.
extern "system" fn closed(_: EnvUnowned<'_>, _: JClass<'_>, window: i64) {
    if let Some(feedback) = FEEDBACK.get() {
        feedback.window_closed(WindowId(window as u64));
    }
}

/// Main thread: a task `AndroidOverlays::after` boxed, run once.
extern "system" fn run(_: EnvUnowned<'_>, _: JClass<'_>, task: i64) {
    if task != 0 {
        // SAFETY: boxed by `AndroidOverlays::after` and handed to Java once.
        let task = unsafe { Box::from_raw(task as *mut Task) };
        task();
    }
}
