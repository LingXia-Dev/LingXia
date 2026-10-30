//! The Android adapter for the GPU View host: `com.lingxia.ui.GpuView` natives
//! over a [`ViewHost`].
//! Java owns the host handle from `nativeHostCreate` to `nativeHostRelease` and
//! calls every native on the main thread.
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JCharArray, JClass, JObject, JString};
use jni::sys::{jboolean, jdouble, jfloat, jint, jlong};
use jni::{Env, EnvUnowned, JValue, jni_sig, jni_str};
use lingxia_ui::interaction::view::{AccessibilityAction, Key, Pointer, PointerPhase};
use lingxia_ui::platform::Task;
use lingxia_ui::session::host::{CaretState, ViewHost, ViewWorker};
use lingxia_ui::session::lifecycle::ConnectionTicket;

mod adapter;
#[cfg(all(feature = "gpu", target_os = "android"))]
mod gpu_surface;

use adapter::AndroidAdapter;
pub use adapter::{ImageLoader, set_image_loader};

/// Memory pressure: GPU pages drop memory kept only to speed up coming frames.
pub fn release_transient() {
    #[cfg(all(feature = "gpu", target_os = "android"))]
    gpu_surface::release_transient_all();
}

type Host = ViewHost<AndroidAdapter>;

#[derive(Debug)]
struct Error(String);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}
impl From<jni::errors::Error> for Error {
    fn from(error: jni::errors::Error) -> Self {
        Self(error.to_string())
    }
}
impl From<String> for Error {
    fn from(error: String) -> Self {
        Self(error)
    }
}
impl From<&str> for Error {
    fn from(error: &str) -> Self {
        Self(error.into())
    }
}

fn run<'a, T: Default>(
    env: &mut EnvUnowned<'a>,
    work: impl FnOnce(&mut Env<'a>) -> Result<T, Error>,
) -> T {
    env.with_env(work).resolve::<ThrowRuntimeExAndDefault>()
}

fn host<'a>(handle: jlong) -> Result<&'a Host, Error> {
    // SAFETY: Java owns the box from create until release and calls nothing afterwards.
    unsafe { (handle as *const Host).as_ref() }.ok_or_else(|| "UI_HOST: released".into())
}

fn text(env: &mut Env, value: &JString) -> Result<String, Error> {
    Ok(value.try_to_string(env)?)
}

/// Bind the SDK component explicitly so static linking retains every JNI entrypoint.
pub fn register_natives(env: &mut Env, class: &JClass) -> jni::errors::Result<()> {
    static HOOK: std::sync::Once = std::sync::Once::new();
    HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let thread = std::thread::current();
            adapter::log(&format!(
                "panic on {}: {info}\n{}",
                thread.name().unwrap_or("?"),
                std::backtrace::Backtrace::force_capture()
            ));
            previous(info);
        }));
    });
    macro_rules! method {
        ($name:literal, $signature:literal, $function:path) => {
            // SAFETY: each function's Rust signature matches its JNI signature.
            unsafe {
                jni::NativeMethod::from_raw_parts(
                    jni_str!($name),
                    jni_str!($signature),
                    $function as *const () as *mut std::ffi::c_void,
                )
            }
        };
    }
    let methods = [
        method!(
            "nativeHostCreate",
            "(Lcom/lingxia/ui/GpuPlatform;Ljava/lang/String;ZJ)J",
            host_create
        ),
        method!("nativeHostRelease", "(J)V", host_release),
        method!("nativeRunTask", "(J)V", run_task),
        method!("nativeLoad", "(JLjava/lang/String;)V", load),
        method!("nativeReload", "(J)V", reload),
        method!("nativePark", "(J)V", park),
        method!("nativeSetVisible", "(JZ)V", set_visible),
        method!("nativeSurfaceChanged", "(J)V", surface_changed),
        method!("nativeSurfaceLost", "(J)V", surface_lost),
        method!("nativeRequestFrame", "(J)V", request_frame),
        method!("nativeRedraw", "(J)V", redraw),
        method!("nativeFrameCurrent", "(J)Z", frame_current),
        method!("nativeCloseRequest", "(J)Z", close_request),
        method!("nativeVsync", "(J)V", vsync),
        method!("nativeRetry", "(J)V", retry),
        method!("nativeConnect", "(JJLjava/lang/String;)V", connect),
        method!("nativeReceive", "(JJLjava/lang/String;)V", receive),
        method!(
            "nativeRequestLeave",
            "(JJLjava/lang/String;)V",
            request_leave
        ),
        method!("nativeConnection", "(J)J", connection),
        method!("nativeInputBlocked", "(J)Z", input_blocked),
        method!("nativeWindowFocus", "(JZ)V", window_focus),
        method!("nativeReveal", "(JZ)V", reveal),
        method!("nativeTextMenuAction", "(JI)Z", text_menu_action),
        method!("nativeTextMenuDismissed", "(J)V", text_menu_dismissed),
        method!("nativeTextHandleDrag", "(JZ)V", text_handle_drag),
        method!("nativeWindowVisible", "(JZ)V", window_visible),
        method!("nativeBridgeReady", "(J)Z", bridge_ready),
        method!("nativeFail", "(JLjava/lang/String;)V", fail),
        method!(
            "nativeUpdateEnvironment",
            "(JLjava/lang/String;)V",
            update_environment
        ),
        method!("nativePointer", "(JIFFFFFF)Z", pointer),
        method!("nativeLongPress", "(J)V", long_press),
        method!("nativeKey", "(JIZZ)Z", key),
        method!("nativeCancel", "(J)V", cancel),
        method!("nativeFocusEntered", "(JZ)V", focus_entered),
        method!("nativeFocusLeft", "(J)V", focus_left),
        method!("nativeExplore", "(JZFF)Z", explore),
        method!(
            "nativeAccessibility",
            "(JIIDLjava/lang/String;)Z",
            accessibility
        ),
        method!("nativeScrollBy", "(JLjava/lang/String;F)Z", scroll_by),
        method!("nativeCanScrollVertically", "(JI)Z", can_scroll_vertically),
        method!(
            "nativeSemanticRoots",
            "(J)Ljava/lang/String;",
            semantic_roots
        ),
        method!(
            "nativeSemanticNode",
            "(JI)Ljava/lang/String;",
            semantic_node
        ),
        method!(
            "nativeAutomate",
            "(JLjava/lang/String;)Ljava/lang/String;",
            automate
        ),
        method!(
            "nativeMoveTextSelection",
            "(JLjava/lang/String;IFF)V",
            move_text_selection
        ),
        method!(
            "nativeSetTextSelection",
            "(JLjava/lang/String;II)V",
            set_text_selection
        ),
        method!("nativeCopyTextSelection", "(JJ)V", copy_text_selection),
        method!("nativeClearTextSelection", "(J)V", clear_text_selection),
        method!("nativeInput", "(JLjava/lang/String;J)V", input),
        method!("nativeCaret", "(JZZLjava/lang/String;IIII)V", caret),
        method!("nativeGpuSentinel", "(Ljava/lang/String;)V", gpu_sentinel),
        method!("nativeGpuActive", "(Z)V", gpu_active),
        method!(
            "nativeChromeContains",
            "(Ljava/lang/String;FFZ)Z",
            chrome_contains
        ),
        method!(
            "nativeHandleEndpoint",
            "(Ljava/lang/String;I)[F",
            handle_endpoint
        ),
        method!("nativeHandleHit", "(Ljava/lang/String;FFZ)[F", handle_hit),
        method!("nativeHandleDrag", "(Ljava/lang/String;FF)[F", handle_drag),
        #[cfg(all(feature = "gpu", target_os = "android"))]
        method!("nativeAttach", "(JLandroid/view/Surface;II)V", attach),
        #[cfg(feature = "hot-swap")]
        method!("nativeSwapModule", "(JLjava/lang/String;)V", swap_module),
    ];
    // SAFETY: every entry above names a function with the matching signature.
    unsafe { env.register_native_methods(class, &methods) }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_ui_GpuView_initializeNative<'a>(
    mut env: EnvUnowned<'a>,
    class: JClass<'a>,
) {
    run(&mut env, |env| Ok(register_natives(env, &class)?));
}

extern "system" fn host_create<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    platform: JObject<'a>,
    worker: JString<'a>,
    logic: jboolean,
    image_owner: jlong,
) -> jlong {
    run(&mut env, |env| {
        lingxia_ui::text::fonts::set_system_fonts(system_fonts);
        let platform = env.new_global_ref(platform)?;
        let worker = ViewWorker::shared(&text(env, &worker)?)?;
        let host = ViewHost::new(AndroidAdapter::new(platform, image_owner), worker, logic);
        let _ = host.adapter().host.set(host.downgrade());
        Ok(Box::into_raw(Box::new(host)) as jlong)
    })
}

fn system_fonts() -> lingxia_ui::text::fonts::SystemFonts {
    let root = std::env::var("ANDROID_ROOT").unwrap_or_else(|_| "/system".into());
    lingxia_ui::text::fonts::SystemFonts::load_android(std::path::Path::new(&root))
}

extern "system" fn host_release<'a>(mut env: EnvUnowned<'a>, _: JClass<'a>, handle: jlong) {
    run(&mut env, |_| {
        if handle == 0 {
            return Ok(());
        }
        // SAFETY: created by `host_create` and released exactly once by Java.
        let host = unsafe { Box::from_raw(handle as *mut Host) };
        host.dispose();
        #[cfg(all(feature = "gpu", target_os = "android"))]
        {
            let surface = host
                .adapter()
                .gpu
                .swap(0, std::sync::atomic::Ordering::AcqRel);
            if surface != 0 {
                gpu_surface::detach(surface);
            }
        }
        Ok(())
    })
}

/// Posted tasks always run exactly once, even after the view is disposed.
extern "system" fn run_task<'a>(mut env: EnvUnowned<'a>, _: JClass<'a>, task: jlong) {
    run(&mut env, |_| {
        if task != 0 {
            // SAFETY: boxed by `AndroidAdapter::post_after` and handed to Java once.
            let task = unsafe { Box::from_raw(task as *mut Task) };
            task();
        }
        Ok(())
    })
}

macro_rules! host_call {
    ($name:ident, $method:ident) => {
        extern "system" fn $name<'a>(mut env: EnvUnowned<'a>, _: JClass<'a>, handle: jlong) {
            run(&mut env, |_| {
                host(handle)?.$method();
                Ok(())
            })
        }
    };
}
host_call!(reload, reload);
host_call!(park, park);
host_call!(request_frame, request_frame);
host_call!(redraw, redraw);
host_call!(vsync, vsync);
host_call!(retry, retry);
host_call!(long_press, long_press);
host_call!(cancel, cancel);
host_call!(focus_left, focus_left);
host_call!(surface_changed, surface_changed);

extern "system" fn surface_lost<'a>(mut env: EnvUnowned<'a>, _: JClass<'a>, handle: jlong) {
    run(&mut env, |_| {
        let host = host(handle)?;
        host.surface_lost();
        #[cfg(all(feature = "gpu", target_os = "android"))]
        gpu_surface::suspend(
            host.adapter()
                .gpu
                .load(std::sync::atomic::Ordering::Acquire),
        );
        Ok(())
    })
}

/// Dev builds: replace the module in place, keeping committed state.
#[cfg(feature = "hot-swap")]
extern "system" fn swap_module<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    source: JString<'a>,
) {
    run(&mut env, |env| {
        let source = text(env, &source)?;
        host(handle)?.swap_module(source);
        Ok(())
    })
}

extern "system" fn load<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    source: JString<'a>,
) {
    run(&mut env, |env| {
        let source = text(env, &source)?;
        host(handle)?.load(source);
        Ok(())
    })
}

extern "system" fn set_visible<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    visible: jboolean,
) {
    run(&mut env, |_| {
        host(handle)?.set_visible(visible);
        Ok(())
    })
}

fn connection_ticket(value: jlong) -> Result<ConnectionTicket, Error> {
    u64::try_from(value)
        .map(ConnectionTicket::new)
        .map_err(|_| "UI_BRIDGE: invalid connection".into())
}

extern "system" fn connect<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    connection: jlong,
    nonce: JString<'a>,
) {
    run(&mut env, |env| {
        let nonce = text(env, &nonce)?;
        host(handle)?.connect(connection_ticket(connection)?, nonce);
        Ok(())
    })
}

extern "system" fn receive<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    connection: jlong,
    frame: JString<'a>,
) {
    run(&mut env, |env| {
        let frame = text(env, &frame)?;
        host(handle)?.receive(connection_ticket(connection)?, frame);
        Ok(())
    })
}

extern "system" fn request_leave<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    connection: jlong,
    reason: JString<'a>,
) {
    run(&mut env, |env| {
        let reason = text(env, &reason)?;
        host(handle)?.request_leave(connection_ticket(connection)?, reason);
        Ok(())
    })
}

extern "system" fn connection<'a>(mut env: EnvUnowned<'a>, _: JClass<'a>, handle: jlong) -> jlong {
    run(&mut env, |_| {
        Ok(host(handle)?.lifecycle().connection().get() as jlong)
    })
}

extern "system" fn frame_current<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
) -> jboolean {
    run(&mut env, |_| Ok(host(handle)?.frame_is_current()))
}

/// `GpuTextMenu.ACTIONS` order.
extern "system" fn text_menu_action<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    action: jint,
) -> jboolean {
    use lingxia_ui::session::host::MenuAction;
    run(&mut env, |_| {
        let action = match action {
            0 => MenuAction::Cut,
            1 => MenuAction::Copy,
            2 => MenuAction::Paste,
            3 => MenuAction::SelectAll,
            4 => MenuAction::Undo,
            5 => MenuAction::Redo,
            _ => return Err("UI_MENU: unknown action".into()),
        };
        Ok(host(handle)?.text_menu_action(action))
    })
}

extern "system" fn text_menu_dismissed<'a>(mut env: EnvUnowned<'a>, _: JClass<'a>, handle: jlong) {
    run(&mut env, |_| {
        host(handle)?.text_menu_dismissed();
        Ok(())
    })
}

extern "system" fn text_handle_drag<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    active: jboolean,
) {
    run(&mut env, |_| {
        host(handle)?.text_handle_drag(active);
        Ok(())
    })
}

extern "system" fn reveal<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    surface_holds_frame: jboolean,
) {
    run(&mut env, |_| {
        host(handle)?.reveal(surface_holds_frame);
        Ok(())
    })
}

extern "system" fn window_visible<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    visible: jboolean,
) {
    run(&mut env, |_| {
        host(handle)?.window_visible(visible);
        Ok(())
    })
}

extern "system" fn window_focus<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    focused: jboolean,
) {
    run(&mut env, |_| {
        host(handle)?.window_focus(focused);
        Ok(())
    })
}

extern "system" fn close_request<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
) -> jboolean {
    run(&mut env, |_| Ok(host(handle)?.close_request()))
}

extern "system" fn input_blocked<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
) -> jboolean {
    run(&mut env, |_| Ok(host(handle)?.input_blocked()))
}

extern "system" fn bridge_ready<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
) -> jboolean {
    run(&mut env, |_| Ok(host(handle)?.lifecycle().bridge_ready()))
}

extern "system" fn fail<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    error: JString<'a>,
) {
    run(&mut env, |env| {
        let error = text(env, &error)?;
        host(handle)?.fail(error);
        Ok(())
    })
}

extern "system" fn update_environment<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    update: JString<'a>,
) {
    run(&mut env, |env| {
        let update = text(env, &update)?;
        if let Err(error) = host(handle)?.update_environment(&update) {
            adapter::log(&error);
        }
        Ok(())
    })
}

#[allow(clippy::too_many_arguments)]
extern "system" fn pointer<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    phase: jint,
    x: jfloat,
    y: jfloat,
    slop: jfloat,
    velocity_x: jfloat,
    velocity_y: jfloat,
    min_fling: jfloat,
) -> jboolean {
    run(&mut env, |_| {
        let phase = match phase {
            0 => PointerPhase::Down,
            1 => PointerPhase::Move,
            2 => PointerPhase::Up,
            _ => PointerPhase::Cancel,
        };
        Ok(host(handle)?.pointer(Pointer {
            phase,
            x,
            y,
            slop,
            velocity: [velocity_x, velocity_y],
            min_fling,
        }))
    })
}

extern "system" fn key<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    key: jint,
    shift: jboolean,
    repeat: jboolean,
) -> jboolean {
    run(&mut env, |_| {
        let Some(key) = Key::from_code(i64::from(key)) else {
            return Ok(false);
        };
        Ok(host(handle)?.key(key, shift, repeat))
    })
}

extern "system" fn focus_entered<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    backward: jboolean,
) {
    run(&mut env, |_| {
        host(handle)?.focus_entered(backward);
        Ok(())
    })
}

extern "system" fn explore<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    active: jboolean,
    x: jfloat,
    y: jfloat,
) -> jboolean {
    run(&mut env, |_| {
        Ok(host(handle)?.explore(active.then_some([x, y])))
    })
}

extern "system" fn accessibility<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    id: jint,
    action: jint,
    number: jdouble,
    argument: JString<'a>,
) -> jboolean {
    run(&mut env, |env| {
        let action = match action {
            0 => AccessibilityAction::Focus,
            1 => AccessibilityAction::ClearFocus,
            2 => AccessibilityAction::Click,
            3 => AccessibilityAction::ShowOnScreen,
            4 => AccessibilityAction::ScrollForward,
            5 => AccessibilityAction::ScrollBackward,
            6 => AccessibilityAction::SetProgress(number),
            7 => AccessibilityAction::SetText(text(env, &argument)?),
            8 => AccessibilityAction::SetSelection(
                number as i32,
                text(env, &argument)?.parse().unwrap_or(-1),
            ),
            9 => AccessibilityAction::ClearSelection,
            10 => AccessibilityAction::SelectItem(number as usize),
            11 => AccessibilityAction::Copy,
            12 => AccessibilityAction::Cut,
            13 => AccessibilityAction::Paste(text(env, &argument)?),
            _ => return Err("UI_ACCESSIBILITY: unknown action".into()),
        };
        Ok(host(handle)?.accessibility(id as u32, action))
    })
}

extern "system" fn scroll_by<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    region: JString<'a>,
    delta: jfloat,
) -> jboolean {
    run(&mut env, |env| {
        let region = text(env, &region)?;
        Ok(host(handle)?.scroll_by(&region, delta))
    })
}

extern "system" fn can_scroll_vertically<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    direction: jint,
) -> jboolean {
    run(&mut env, |_| {
        Ok(host(handle)?.can_scroll_vertically(direction))
    })
}

extern "system" fn semantic_roots<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
) -> JString<'a> {
    run(&mut env, |env| {
        let roots = host(handle)?.semantics(|projection, focus| match projection {
            Some(projection) => serde_json::json!({
                "roots": projection.roots(),
                "inputFocus": projection.input_focus(),
                "accessibilityFocus": focus,
            }),
            None => serde_json::Value::Null,
        });
        Ok(env.new_string(roots.to_string())?)
    })
}

extern "system" fn semantic_node<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    id: jint,
) -> JString<'a> {
    run(&mut env, |env| {
        let node = host(handle)?.semantics(|projection, _| {
            projection
                .and_then(|projection| projection.node(id as u32))
                .map(|node| serde_json::to_value(node).unwrap_or_default())
                .unwrap_or_default()
        });
        Ok(env.new_string(node.to_string())?)
    })
}

/// One automation request (`op` query or act) on the presented frame; answers
/// `{"value": ...}` or `{"error": "..."}` with the core's message verbatim.
extern "system" fn automate<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    request: JString<'a>,
) -> JString<'a> {
    run(&mut env, |env| {
        let request: serde_json::Value =
            serde_json::from_str(&request.try_to_string(env)?).map_err(|e| e.to_string())?;
        let reply = match host(handle)?.automation_request(&request) {
            Ok(value) => serde_json::json!({ "value": value }),
            Err(error) => serde_json::json!({ "error": error }),
        };
        Ok(env.new_string(reply.to_string())?)
    })
}

/// Drag a static-text selection end (0 anchor, 1 focus) to a logical point.
extern "system" fn move_text_selection<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    identity: JString<'a>,
    which: jint,
    x: jfloat,
    y: jfloat,
) {
    run(&mut env, |env| {
        let identity = text(env, &identity)?;
        host(handle)?.move_text_selection(identity, which.clamp(0, 1) as u8, [x, y]);
        Ok(())
    })
}

extern "system" fn set_text_selection<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    identity: JString<'a>,
    start: jint,
    end: jint,
) {
    run(&mut env, |env| {
        let identity = text(env, &identity)?;
        host(handle)?.set_text_selection(identity, start.max(0) as u32, end.max(0) as u32);
        Ok(())
    })
}

/// The selected text arrives through `inputReply(callback, {"text": ...})`; the selection clears.
extern "system" fn copy_text_selection<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    callback: jlong,
) {
    run(&mut env, |_| {
        let host = host(handle)?;
        let replier = host.clone();
        host.copy_text_selection(move |selected| {
            replier.adapter().java(|env, platform| {
                let reply = env.new_string(serde_json::json!({ "text": selected }).to_string())?;
                env.call_method(
                    platform,
                    jni_str!("inputReply"),
                    jni_sig!("(JLjava/lang/String;)V"),
                    &[JValue::Long(callback), JValue::Object(&reply)],
                )?;
                Ok(())
            })
        });
        Ok(())
    })
}

extern "system" fn clear_text_selection<'a>(mut env: EnvUnowned<'a>, _: JClass<'a>, handle: jlong) {
    run(&mut env, |_| {
        host(handle)?.clear_text_selection();
        Ok(())
    })
}

/// Unpaired surrogates must reach the session as an error, not as replacement characters.
fn utf16(env: &mut Env, value: &JString, limit: usize) -> Result<String, Error> {
    let chars = env
        .call_method(value, jni_str!("toCharArray"), jni_sig!("()[C"), &[])?
        .l()?;
    let chars = env.cast_local::<JCharArray>(chars)?;
    let length = chars.len(env)?;
    if length > limit {
        return Err("UI_INPUT: request budget exceeded".into());
    }
    let mut units = vec![0; length];
    chars.get_region(env, 0, &mut units)?;
    String::from_utf16(&units).map_err(|_| "UI_INPUT: invalid Unicode".into())
}

extern "system" fn input<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    request: JString<'a>,
    callback: jlong,
) {
    run(&mut env, |env| {
        let request = utf16(
            env,
            &request,
            lingxia_ui::session::host::MAX_INPUT_REQUEST_BYTES,
        )?;
        let host = host(handle)?;
        let replier = host.clone();
        host.input(request, move |reply| {
            replier.adapter().java(|env, platform| {
                let reply = match reply {
                    Some(reply) => JObject::from(env.new_string(reply)?),
                    None => JObject::null(),
                };
                env.call_method(
                    platform,
                    jni_str!("inputReply"),
                    jni_sig!("(JLjava/lang/String;)V"),
                    &[JValue::Long(callback), JValue::Object(&reply)],
                )?;
                Ok(())
            })
        });
        Ok(())
    })
}

#[allow(clippy::too_many_arguments)]
extern "system" fn caret<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    eligible: jboolean,
    connected: jboolean,
    value: JString<'a>,
    anchor: jint,
    focus: jint,
    composition_start: jint,
    composition_end: jint,
) {
    run(&mut env, |env| {
        let state = if connected {
            Some(CaretState {
                text: text(env, &value)?,
                anchor,
                focus,
                composition: (composition_start, composition_end),
            })
        } else {
            None
        };
        host(handle)?.caret(eligible, state);
        Ok(())
    })
}

use lingxia_ui::interaction::chrome::{self, ChromeNode};

fn chrome_node(env: &mut Env, node: &JString) -> Result<ChromeNode, Error> {
    Ok(ChromeNode::parse(&text(env, node)?)?)
}

/// A float array, or null for `None`.
fn floats<'a>(
    env: &mut Env<'a>,
    values: Option<&[f32]>,
) -> Result<jni::objects::JFloatArray<'a>, Error> {
    let Some(values) = values else {
        return Ok(jni::objects::JFloatArray::null());
    };
    let array = env.new_float_array(values.len())?;
    array.set_region(env, 0, values)?;
    Ok(array)
}

extern "system" fn chrome_contains<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    node: JString<'a>,
    x: jfloat,
    y: jfloat,
    own_radius: jboolean,
) -> jboolean {
    run(&mut env, |env| {
        Ok(chrome::contains(
            &chrome_node(env, &node)?,
            f64::from(x),
            f64::from(y),
            own_radius,
        ))
    })
}

/// `[center x, line bottom, line middle]` of the anchor (0) or caret (1) handle, or null.
extern "system" fn handle_endpoint<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    node: JString<'a>,
    which: jint,
) -> jni::objects::JFloatArray<'a> {
    run(&mut env, |env| {
        let end = chrome::endpoint(&chrome_node(env, &node)?, usize::from(which != 0));
        floats(
            env,
            end.map(|e| e.map(|v| v as f32))
                .as_ref()
                .map(|e| e.as_slice()),
        )
    })
}

/// `[which, center x, line bottom, line middle]` of the grabbed handle, or null.
extern "system" fn handle_hit<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    node: JString<'a>,
    x: jfloat,
    y: jfloat,
    collapsed: jboolean,
) -> jni::objects::JFloatArray<'a> {
    run(&mut env, |env| {
        let hit = chrome::handle_hit(
            &chrome_node(env, &node)?,
            f64::from(x),
            f64::from(y),
            collapsed,
        )
        .map(|(which, [cx, bottom, middle])| {
            [which as f32, cx as f32, bottom as f32, middle as f32]
        });
        floats(env, hit.as_ref().map(|h| h.as_slice()))
    })
}

/// `[x, y, edge x, edge y, visible]` for a handle drag query, or null to end the drag.
extern "system" fn handle_drag<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    node: JString<'a>,
    x: jfloat,
    y: jfloat,
) -> jni::objects::JFloatArray<'a> {
    run(&mut env, |env| {
        let drag =
            chrome::drag_point(&chrome_node(env, &node)?, f64::from(x), f64::from(y)).map(|d| {
                [
                    d.point[0] as f32,
                    d.point[1] as f32,
                    d.edge[0] as f32,
                    d.edge[1] as f32,
                    f32::from(u8::from(d.visible)),
                ]
            });
        floats(env, drag.as_ref().map(|d| d.as_slice()))
    })
}

/// Back the GPU crash sentinel with a file in app-private storage; the first call wins.
extern "system" fn gpu_sentinel<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    directory: JString<'a>,
) {
    run(&mut env, |env| {
        let directory = text(env, &directory)?;
        lingxia_ui::renderer::sentinel::install(Box::new(
            lingxia_ui::renderer::sentinel::FileStore(
                std::path::Path::new(&directory).join("lingxia-gpu-sentinel"),
            ),
        ));
        Ok(())
    })
}

/// A page stopped or resumed on the platform's normal path; a process killed while
/// stopped did not crash in the driver.
extern "system" fn gpu_active(_: EnvUnowned<'_>, _: JClass<'_>, active: jboolean) {
    if active {
        lingxia_ui::renderer::sentinel::resumed();
    } else {
        lingxia_ui::renderer::sentinel::interrupted();
    }
}

/// Bind the Surface to a GPU presenter on the View worker; Java hears back
/// through `GpuPlatform.gpuAttached` and renders on the CPU until then.
#[cfg(all(feature = "gpu", target_os = "android"))]
extern "system" fn attach<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    surface: JObject<'a>,
    width: jint,
    height: jint,
) {
    run(&mut env, |env| {
        // SAFETY: `env` is this thread's environment and `surface` a live Surface argument.
        let window =
            unsafe { gpu_surface::NativeWindow::from_surface(env.get_raw(), surface.as_raw())? };
        let host = host(handle)?.clone();
        let adapter = host.adapter();
        let existing = adapter.gpu.load(std::sync::atomic::Ordering::Acquire);
        adapter.set_target(adapter::Target::Waiting);
        let (width, height) = (width as u32, height as u32);
        let fallback = host.clone();
        // Off the shared View worker: an attach must never queue behind another page's frames.
        std::thread::Builder::new()
            .name("lingxia-gpu-attach".into())
            .spawn(move || {
                let attached = gpu_surface::attach(window, width, height, existing);
                let main = host.clone();
                lingxia_ui::platform::MainThread::post(
                    host.adapter(),
                    Box::new(move || {
                        let adapter = main.adapter();
                        let ok = match attached {
                            Ok(id) => {
                                adapter.gpu.store(id, std::sync::atomic::Ordering::Release);
                                adapter.set_target(adapter::Target::Ready);
                                true
                            }
                            Err(error) => {
                                adapter.set_target(adapter::Target::Unavailable);
                                adapter::log(&format!(
                                    "GPU unavailable; using CPU reference renderer: {error}"
                                ));
                                false
                            }
                        };
                        let surface = adapter.gpu.load(std::sync::atomic::Ordering::Acquire);
                        let (record, backend) = match gpu_surface::describe(surface) {
                            Some(described) if ok => described,
                            _ => (String::new(), "vello-cpu"),
                        };
                        let reason = match lingxia_ui::renderer::sentinel::fallback() {
                            Some(_) => " fallback=driver (refused by the GPU crash sentinel)",
                            None => "",
                        };
                        adapter::info(&format!(
                            "backend={backend} surface={surface} adapter={record}{reason}"
                        ));
                        adapter.java(|env, platform| {
                            env.call_method(
                                platform,
                                jni_str!("gpuAttached"),
                                jni_sig!("(Z)V"),
                                &[JValue::Bool(ok)],
                            )?;
                            Ok(())
                        });
                    }),
                );
            })
            .map_err(|error| {
                fallback.adapter().set_target(adapter::Target::Unavailable);
                Error::from(format!("UI_SURFACE: {error}"))
            })?;
        Ok(())
    })
}
