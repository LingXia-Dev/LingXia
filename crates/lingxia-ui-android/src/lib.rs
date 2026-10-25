//! The Android adapter for the GPU View host: `com.lingxia.ui.GpuView` natives
//! over a [`ViewHost`].
//! Java owns the host handle from `nativeHostCreate` to `nativeHostRelease` and
//! calls every native on the main thread.
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JByteArray, JCharArray, JClass, JObject, JString};
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
            "(Lcom/lingxia/ui/GpuPlatform;Ljava/lang/String;Z)J",
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
        method!("nativeSessionFailed", "(J)Z", session_failed),
        method!("nativeBridgeReady", "(J)Z", bridge_ready),
        method!("nativeFail", "(JLjava/lang/String;)V", fail),
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
        method!("nativeInput", "(JLjava/lang/String;J)V", input),
        method!("nativeCaret", "(JZZLjava/lang/String;IIII)V", caret),
        method!("nativeImageLoaded", "(JJ[B)V", image_loaded),
        method!("nativeGpuSentinel", "(Ljava/lang/String;)V", gpu_sentinel),
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
) -> jlong {
    run(&mut env, |env| {
        let platform = env.new_global_ref(platform)?;
        let worker = ViewWorker::shared(&text(env, &worker)?)?;
        let host = ViewHost::new(AndroidAdapter::new(platform), worker, logic);
        Ok(Box::into_raw(Box::new(host)) as jlong)
    })
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

extern "system" fn session_failed<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
) -> jboolean {
    run(&mut env, |_| Ok(host(handle)?.lifecycle().session_failed()))
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
        let key = match key {
            0 => Key::Tab,
            1 => Key::Up,
            2 => Key::Down,
            3 => Key::Left,
            4 => Key::Right,
            5 => Key::Home,
            6 => Key::End,
            _ => Key::Activate,
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

extern "system" fn image_loaded<'a>(
    mut env: EnvUnowned<'a>,
    _: JClass<'a>,
    handle: jlong,
    id: jlong,
    bytes: JByteArray<'a>,
) {
    run(&mut env, |env| {
        let bytes = if bytes.is_null() {
            Err("UI_IMAGE: load failed".into())
        } else if bytes.len(env)? > lingxia_ui::protocol::limits::IMAGE_BYTES {
            Err("UI_IMAGE: encoded size limit".into())
        } else {
            Ok(env.convert_byte_array(&bytes)?)
        };
        host(handle)?.image_loaded(id as u64, bytes);
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
        let existing = host
            .adapter()
            .gpu
            .load(std::sync::atomic::Ordering::Acquire);
        let (width, height) = (width as u32, height as u32);
        host.worker().clone().run(move |_| {
            let attached = gpu_surface::attach(window, width, height, existing);
            let main = host.clone();
            lingxia_ui::platform::MainThread::post(
                host.adapter(),
                Box::new(move || {
                    let adapter = main.adapter();
                    let ok = match attached {
                        Ok(id) => {
                            adapter.gpu.store(id, std::sync::atomic::Ordering::Release);
                            true
                        }
                        Err(error) => {
                            adapter::log(&format!(
                                "GPU unavailable; using CPU reference renderer: {error}"
                            ));
                            false
                        }
                    };
                    let surface = adapter.gpu.load(std::sync::atomic::Ordering::Acquire);
                    let record = gpu_surface::adapter(surface).unwrap_or_default();
                    let backend = if !ok {
                        "vello-cpu"
                    } else if record.starts_with("Gl|") {
                        "vello-gles"
                    } else {
                        "vello-vulkan"
                    };
                    adapter::info(&format!(
                        "backend={backend} surface={surface} adapter={record}"
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
        });
        Ok(())
    })
}
