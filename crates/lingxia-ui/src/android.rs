use jni::{
    JNIEnv,
    objects::{JByteArray, JCharArray, JClass, JString},
    sys::{jboolean, jfloat, jint, jintArray, jlong, jstring},
};
use std::{
    cell::RefCell,
    collections::HashMap,
    sync::atomic::{AtomicI64, Ordering},
};

#[cfg(feature = "gpu")]
mod gpu_surface;
mod image_requests;

/// Bind the SDK component explicitly so static linking retains every JNI entrypoint.
///
/// # Safety
/// `env` must be this thread's live JNI environment and `class` a live reference to
/// `com.lingxia.ui.GpuView` from the host's classloader. Neither reference is retained.
pub unsafe fn register_natives(
    env: *mut std::ffi::c_void,
    class: *mut std::ffi::c_void,
) -> Result<(), String> {
    let mut env = unsafe { JNIEnv::from_raw(env.cast()) }.map_err(|e| e.to_string())?;
    let class = unsafe { JClass::from_raw(class.cast()) };
    macro_rules! method {
        ($name:literal, $signature:literal, $function:path) => {
            jni::NativeMethod {
                name: $name.into(),
                sig: $signature.into(),
                fn_ptr: $function as *const () as *mut std::ffi::c_void,
            }
        };
    }
    let methods = [
        method!("nativeImageQueueCreate", "()J", image_requests::create),
        method!(
            "nativeImageQueueUpdate",
            "(JLjava/lang/String;)V",
            image_requests::update
        ),
        method!(
            "nativeImageQueueStart",
            "(J)Ljava/lang/String;",
            image_requests::start
        ),
        method!("nativeImageQueueCheck", "(JJZ)Z", image_requests::check),
        method!("nativeImageQueueReset", "(JZ)V", image_requests::reset),
        method!(
            "nativeImageStatus",
            "(JJJLjava/lang/String;Z)V",
            image_status
        ),
        method!("nativeImageEvents", "(JJ)Z", image_events),
        method!(
            "nativeInput",
            "(JLjava/lang/String;)Ljava/lang/String;",
            Java_com_lingxia_ui_GpuView_nativeInput
        ),
        method!(
            "nativeImage",
            "(JJJ[B)Z",
            Java_com_lingxia_ui_GpuView_nativeImage
        ),
        method!(
            "nativeBeginBridge",
            "(JLjava/lang/String;)Ljava/lang/String;",
            Java_com_lingxia_ui_GpuView_nativeBeginBridge
        ),
        method!(
            "nativeReceiveBridge",
            "(JLjava/lang/String;)Ljava/lang/String;",
            Java_com_lingxia_ui_GpuView_nativeReceiveBridge
        ),
        method!(
            "nativeDrainActions",
            "(J)Ljava/lang/String;",
            Java_com_lingxia_ui_GpuView_nativeDrainActions
        ),
        method!(
            "nativeRequestLeave",
            "(JLjava/lang/String;)Ljava/lang/String;",
            Java_com_lingxia_ui_GpuView_nativeRequestLeave
        ),
        method!(
            "nativeEnvironment",
            "(JLjava/lang/String;)V",
            Java_com_lingxia_ui_GpuView_nativeEnvironment
        ),
        method!(
            "nativeReset",
            "(J)V",
            Java_com_lingxia_ui_GpuView_nativeReset
        ),
        method!(
            "nativeLoad",
            "(Ljava/lang/String;)J",
            Java_com_lingxia_ui_GpuView_nativeLoad
        ),
        method!(
            "nativeRender",
            "(JIIFZZLjava/lang/String;JJ)[I",
            Java_com_lingxia_ui_GpuView_nativeRender
        ),
        method!(
            "nativeAnimationPark",
            "(JJ)V",
            Java_com_lingxia_ui_GpuView_nativeAnimationPark
        ),
        method!(
            "nativeAnimationPresented",
            "(JJJJ)J",
            Java_com_lingxia_ui_GpuView_nativeAnimationPresented
        ),
        method!(
            "nativeSnapshot",
            "(J)Ljava/lang/String;",
            Java_com_lingxia_ui_GpuView_nativeSnapshot
        ),
        method!(
            "nativeClick",
            "(JJJ)Z",
            Java_com_lingxia_ui_GpuView_nativeClick
        ),
        method!(
            "nativeNumber",
            "(JJJD)Z",
            Java_com_lingxia_ui_GpuView_nativeNumber
        ),
        method!(
            "nativePlot",
            "(JJLjava/lang/String;IFFI)Z",
            Java_com_lingxia_ui_GpuView_nativePlot
        ),
        method!(
            "nativeRelease",
            "(J)V",
            Java_com_lingxia_ui_GpuView_nativeRelease
        ),
        #[cfg(feature = "gpu")]
        method!(
            "nativeAttach",
            "(Landroid/view/Surface;IIJ)J",
            gpu_surface::Java_com_lingxia_ui_GpuView_nativeAttach
        ),
        #[cfg(feature = "gpu")]
        method!(
            "nativeSuspend",
            "(J)V",
            gpu_surface::Java_com_lingxia_ui_GpuView_nativeSuspend
        ),
        #[cfg(feature = "gpu")]
        method!(
            "nativePrepareGpu",
            "(JJIIFZZLjava/lang/String;JJ)V",
            gpu_surface::Java_com_lingxia_ui_GpuView_nativePrepareGpu
        ),
        #[cfg(feature = "gpu")]
        method!(
            "nativePresentGpu",
            "(JJ)Z",
            gpu_surface::Java_com_lingxia_ui_GpuView_nativePresentGpu
        ),
        #[cfg(feature = "gpu")]
        method!(
            "nativeDetach",
            "(J)V",
            gpu_surface::Java_com_lingxia_ui_GpuView_nativeDetach
        ),
    ];
    env.register_native_methods(class, &methods)
        .map_err(|e| e.to_string())
}

struct WorkerPage {
    id: crate::ViewPageId,
    snapshot: String,
}
struct Worker {
    app: crate::ViewApp,
    pages: HashMap<i64, WorkerPage>,
}
thread_local! { static WORKER: RefCell<Option<Worker>> = const { RefCell::new(None) }; }
static NEXT_PAGE: AtomicI64 = AtomicI64::new(1);

fn bridge_update(
    session: &mut crate::ViewSession,
    mut update: crate::state::StateUpdate,
) -> Result<String, String> {
    for request in session.take_action_requests()? {
        update
            .outbound
            .push(serde_json::from_str(&request).map_err(|e| e.to_string())?);
    }
    Ok(serde_json::json!({"ready":session.ready_to_render(),"changed":update.changed,"localChanged":session.has_local_updates()?,"outbound":update.outbound,"errors":session.take_action_errors()?}).to_string())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_ui_GpuView_nativeBeginBridge(
    mut env: JNIEnv,
    _: JClass,
    page: jlong,
    nonce: JString,
) -> jstring {
    guarded(&mut env, |env| {
        let nonce: String = env.get_string(&nonce).map_err(|e| e.to_string())?.into();
        let message = with_page(page, |session, snapshot| {
            snapshot.clear();
            Ok(session.begin_bridge(&nonce)?.to_string())
        })?;
        env.new_string(message)
            .map(|s| s.into_raw())
            .map_err(|e| e.to_string())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_ui_GpuView_nativeReceiveBridge(
    mut env: JNIEnv,
    _: JClass,
    page: jlong,
    frame: JString,
) -> jstring {
    guarded(&mut env, |env| {
        let frame: String = env.get_string(&frame).map_err(|e| e.to_string())?.into();
        let frame = serde_json::from_str(&frame).map_err(|e| e.to_string())?;
        let update = with_page(page, |session, _| {
            let update = session.receive_bridge(&frame)?;
            bridge_update(session, update)
        })?;
        env.new_string(update)
            .map(|s| s.into_raw())
            .map_err(|e| e.to_string())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_ui_GpuView_nativeDrainActions(
    mut env: JNIEnv,
    _: JClass,
    page: jlong,
) -> jstring {
    guarded(&mut env, |env| {
        let update = with_page(page, |session, _| {
            bridge_update(
                session,
                crate::state::StateUpdate {
                    changed: true,
                    outbound: Vec::new(),
                },
            )
        })?;
        env.new_string(update)
            .map(|s| s.into_raw())
            .map_err(|e| e.to_string())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_ui_GpuView_nativeRequestLeave(
    mut env: JNIEnv,
    _: JClass,
    page: jlong,
    reason: JString,
) -> jstring {
    guarded(&mut env, |env| {
        let reason: String = env.get_string(&reason).map_err(|e| e.to_string())?.into();
        let update = with_page(page, |session, _| {
            session.request_leave(&reason)?;
            bridge_update(
                session,
                crate::state::StateUpdate {
                    changed: true,
                    outbound: Vec::new(),
                },
            )
        })?;
        env.new_string(update)
            .map(|s| s.into_raw())
            .map_err(|e| e.to_string())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_ui_GpuView_nativeEnvironment(
    mut env: JNIEnv,
    _: JClass,
    page: jlong,
    value: JString,
) {
    guarded(&mut env, |env| {
        let value: String = env.get_string(&value).map_err(|e| e.to_string())?.into();
        let value = serde_json::from_str(&value).map_err(|e| format!("UI_ENVIRONMENT: {e}"))?;
        with_page(page, |session, _| session.set_environment(value))
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_ui_GpuView_nativeReset(
    mut env: JNIEnv,
    _: JClass,
    page: jlong,
) {
    guarded(&mut env, |_| {
        with_page(page, |session, snapshot| {
            session.reset()?;
            snapshot.clear();
            Ok(())
        })
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_ui_GpuView_initializeNative(
    mut env: JNIEnv,
    class: JClass,
) {
    guarded(&mut env, |env| unsafe {
        register_natives(env.get_native_interface().cast(), class.as_raw().cast())
    });
}

fn with_page<T>(
    id: i64,
    work: impl FnOnce(&mut crate::ViewSession, &mut String) -> Result<T, String>,
) -> Result<T, String> {
    WORKER.with(|worker| {
        let mut worker = worker.borrow_mut();
        let worker = worker.as_mut().ok_or("UI_SESSION: missing app worker")?;
        let page = worker
            .pages
            .get_mut(&id)
            .ok_or("UI_SESSION: discarded page or wrong worker")?;
        work(worker.app.page(page.id)?, &mut page.snapshot)
    })
}

fn guarded<T: Default>(env: &mut JNIEnv, work: impl FnOnce(&mut JNIEnv) -> Result<T, String>) -> T {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(env)))
        .unwrap_or_else(|_| Err("UI_RENDER: native panic".into()));
    match result {
        Ok(value) => value,
        Err(error) => {
            let _ = env.throw_new("java/lang/IllegalStateException", error);
            T::default()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_ui_GpuView_nativeLoad(
    mut env: JNIEnv,
    _: JClass,
    source: JString,
) -> jlong {
    guarded(&mut env, |env| {
        let source: String = env.get_string(&source).map_err(|e| e.to_string())?.into();
        WORKER.with(|worker| {
            let mut worker = worker.borrow_mut();
            if worker.is_none() {
                *worker = Some(Worker {
                    app: crate::ViewApp::new(16)?,
                    pages: HashMap::new(),
                });
            }
            let worker = worker.as_mut().unwrap();
            let id = worker.app.create(&source, false)?;
            let handle = NEXT_PAGE.fetch_add(1, Ordering::Relaxed);
            worker.pages.insert(
                handle,
                WorkerPage {
                    id,
                    snapshot: String::new(),
                },
            );
            Ok(handle)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_ui_GpuView_nativeRender(
    mut env: JNIEnv,
    _: JClass,
    page: jlong,
    width: jint,
    height: jint,
    scale: jfloat,
    rebuild: jboolean,
    caret_visible: jboolean,
    offsets: JString,
    epoch: jlong,
    time: jlong,
) -> jintArray {
    guarded(&mut env, |env| {
        let offsets: String = env.get_string(&offsets).map_err(|e| e.to_string())?.into();
        let offsets = serde_json::from_str(&offsets).map_err(|e| format!("UI_SCROLL: {e}"))?;
        with_page(page, |session, snapshot| {
            let width = u16::try_from(width).map_err(|_| "UI_VIEWPORT: width")?;
            let height = u16::try_from(height).map_err(|_| "UI_VIEWPORT: height")?;
            let now = animation_time(time)?;
            session.set_animation_surface(
                Some(u64::try_from(epoch).map_err(|_| "UI_ANIMATION: invalid epoch")?),
                now,
            )?;
            let frame = session.prepare_view_animated(
                width,
                height,
                scale,
                rebuild != 0,
                Some(&offsets),
                now,
                |mut display| {
                    if caret_visible == 0 {
                        display.hide_caret();
                    }
                    display.rasterize()
                },
            )?;
            *snapshot = serde_json::json!({"generation":session.generation(), "nodes":frame.nodes,"images":session.image_requests(),"metrics":session.frame_metrics()})
                .to_string();
            // The opaque root background makes these premultiplied channels also valid ARGB colors.
            let pixels: Vec<jint> = frame
                .pixels
                .data_as_u8_slice()
                .chunks_exact(4)
                .map(|p| u32::from_be_bytes([p[3], p[0], p[1], p[2]]) as jint)
                .collect();
            let output = env
                .new_int_array(pixels.len() as jint)
                .map_err(|e| e.to_string())?;
            env.set_int_array_region(&output, 0, &pixels)
                .map_err(|e| e.to_string())?;
            Ok(output.into_raw())
        })
    })
}

fn animation_time(time: jlong) -> Result<std::time::Duration, String> {
    Ok(std::time::Duration::from_millis(
        u64::try_from(time).map_err(|_| "UI_ANIMATION: invalid time")?,
    ))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_ui_GpuView_nativeAnimationPark(
    mut env: JNIEnv,
    _: JClass,
    page: jlong,
    time: jlong,
) {
    guarded(&mut env, |_| {
        with_page(page, |session, _| {
            session.set_animation_surface(None, animation_time(time)?)
        })
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_ui_GpuView_nativeAnimationPresented(
    mut env: JNIEnv,
    _: JClass,
    page: jlong,
    epoch: jlong,
    generation: jlong,
    time: jlong,
) -> jlong {
    guarded(&mut env, |_| {
        with_page(page, |session, _| {
            let now = animation_time(time)?;
            let epoch = u64::try_from(epoch).map_err(|_| "UI_ANIMATION: invalid epoch")?;
            let generation =
                u64::try_from(generation).map_err(|_| "UI_ANIMATION: invalid generation")?;
            if !session.animation_presented(epoch, generation, now)? {
                return Ok(-1);
            }
            match session.view_animation_wakeup(now)? {
                crate::animation::Wakeup::None => Ok(-1),
                crate::animation::Wakeup::NextFrame => Ok(0),
                crate::animation::Wakeup::After(delay) => {
                    i64::try_from(delay.as_nanos().div_ceil(1_000_000))
                        .map_err(|_| "UI_ANIMATION: wakeup exceeds clock range".into())
                }
            }
        })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_ui_GpuView_nativeSnapshot(
    mut env: JNIEnv,
    _: JClass,
    page: jlong,
) -> jstring {
    guarded(&mut env, |env| {
        with_page(page, |_, snapshot| {
            env.new_string(&*snapshot)
                .map(|s| s.into_raw())
                .map_err(|e| e.to_string())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_ui_GpuView_nativeInput(
    mut env: JNIEnv,
    _: JClass,
    page: jlong,
    request: JString,
) -> jstring {
    guarded(&mut env, |env| {
        let length = env
            .call_method(&request, "length", "()I", &[])
            .and_then(|v| v.i())
            .map_err(|e| e.to_string())?;
        if length < 0 || length as usize > crate::input_protocol::MAX_REQUEST_BYTES {
            return Err("UI_INPUT: request budget exceeded".into());
        }
        // JavaStr's lossy CESU-8 fallback would silently replace unpaired surrogates.
        let chars = env
            .call_method(&request, "toCharArray", "()[C", &[])
            .and_then(|v| v.l())
            .map_err(|e| e.to_string())?;
        let chars = JCharArray::from(chars);
        let mut units = vec![0; length as usize];
        env.get_char_array_region(&chars, 0, &mut units)
            .map_err(|e| e.to_string())?;
        let request = String::from_utf16(&units).map_err(|_| "UI_INPUT: invalid Unicode")?;
        let reply = with_page(page, |session, _| session.input_request(&request))?;
        env.new_string(reply)
            .map(|s| s.into_raw())
            .map_err(|e| e.to_string())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_ui_GpuView_nativeClick(
    mut env: JNIEnv,
    _: JClass,
    page: jlong,
    generation: jlong,
    handle: jlong,
) -> jboolean {
    guarded(&mut env, |_| {
        with_page(page, |session, _| {
            session
                .click(generation as u64, handle as u64)
                .map(u8::from)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_ui_GpuView_nativePlot(
    mut env: JNIEnv,
    _: JClass,
    page: jlong,
    generation: jlong,
    identity: JString,
    mode: jint,
    x: jfloat,
    y: jfloat,
    index: jint,
) -> jboolean {
    guarded(&mut env, |env| {
        let identity: String = env.get_string(&identity).map_err(|e| e.to_string())?.into();
        let selection = match mode {
            0 => crate::PlotSelection::Point([x, y]),
            1 => crate::PlotSelection::Next,
            2 => crate::PlotSelection::Index(index as i64),
            _ => return Err("UI_PLOT: invalid selection mode".into()),
        };
        with_page(page, |session, _| {
            session
                .select_plot(generation as u64, &identity, selection)
                .map(u8::from)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_ui_GpuView_nativeNumber(
    mut env: JNIEnv,
    _: JClass,
    page: jlong,
    generation: jlong,
    handle: jlong,
    value: jni::sys::jdouble,
) -> jboolean {
    guarded(&mut env, |_| {
        with_page(page, |session, _| {
            session
                .number_event(generation as u64, handle as u64, value)
                .map(u8::from)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_ui_GpuView_nativeRelease(
    _: JNIEnv,
    _: JClass,
    page: jlong,
) {
    WORKER.with(|worker| {
        if let Some(worker) = worker.borrow_mut().as_mut()
            && let Some(page) = worker.pages.remove(&page)
        {
            worker.app.discard(page.id);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_ui_GpuView_nativeImage(
    mut env: JNIEnv,
    _: JClass,
    page: jlong,
    queue: jlong,
    token: jlong,
    bytes: JByteArray,
) -> jboolean {
    guarded(&mut env, |env| {
        if image_requests::current_source(queue, token)?.is_none() {
            return Ok(0);
        }
        if env.get_array_length(&bytes).map_err(|e| e.to_string())? as usize
            > crate::image::MAX_ENCODED_BYTES
        {
            return Err("UI_IMAGE: encoded size limit".into());
        }
        let bytes = env.convert_byte_array(&bytes).map_err(|e| e.to_string())?;
        let bitmap = crate::image::Bitmap::decode(&bytes)?;
        image_requests::bind_current(queue, token, |source| {
            with_page(page, |session, _| session.bind_image(source, bitmap))
        })
        .map(u8::from)
    })
}

extern "system" fn image_status(
    mut env: JNIEnv,
    _: JClass,
    page: jlong,
    queue: jlong,
    token: jlong,
    retry: JString,
    success: jboolean,
) {
    guarded(&mut env, |env| {
        let retry_key: String = env.get_string(&retry).map_err(|e| e.to_string())?.into();
        image_requests::bind_current(queue, token, |source| {
            with_page(page, |session, _| {
                session.image_completed(
                    crate::ImageSource {
                        source: source.into(),
                        retry_key,
                    },
                    success != 0,
                );
                Ok(true)
            })
        })
        .map(|_| ())
    })
}

extern "system" fn image_events(
    mut env: JNIEnv,
    _: JClass,
    page: jlong,
    generation: jlong,
) -> jboolean {
    guarded(&mut env, |_| {
        with_page(page, |session, _| {
            session
                .dispatch_image_events(generation as u64)
                .map(u8::from)
        })
    })
}
