use super::{guarded, with_page};
use jni::{
    JNIEnv,
    objects::{JClass, JObject, JString},
    sys::{jboolean, jfloat, jint, jlong},
};
use raw_window_handle::{
    AndroidDisplayHandle, AndroidNdkWindowHandle, DisplayHandle, HandleError, HasDisplayHandle,
    HasWindowHandle, WindowHandle,
};
use std::{
    collections::HashMap,
    ffi::c_void,
    ptr::NonNull,
    sync::{
        Arc, LazyLock, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

#[link(name = "android")]
unsafe extern "C" {
    fn ANativeWindow_fromSurface(env: *mut c_void, surface: *mut c_void) -> *mut c_void;
    fn ANativeWindow_release(window: *mut c_void);
}

struct NativeWindow(NonNull<c_void>);
// ANativeWindow is reference-counted by Android and supports use from render threads.
unsafe impl Send for NativeWindow {}
unsafe impl Sync for NativeWindow {}
impl Drop for NativeWindow {
    fn drop(&mut self) {
        unsafe {
            ANativeWindow_release(self.0.as_ptr());
        }
    }
}
impl HasWindowHandle for NativeWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        // The acquired native reference outlives every borrowed handle and the wgpu surface.
        Ok(unsafe { WindowHandle::borrow_raw(AndroidNdkWindowHandle::new(self.0).into()) })
    }
}
impl HasDisplayHandle for NativeWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(unsafe { DisplayHandle::borrow_raw(AndroidDisplayHandle::new().into()) })
    }
}

static NEXT: AtomicU64 = AtomicU64::new(1);
static SURFACES: LazyLock<Mutex<HashMap<u64, crate::gpu::Presenter>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_ui_GpuView_nativeAttach(
    mut env: JNIEnv,
    _: JClass,
    surface: JObject,
    width: jint,
    height: jint,
    existing: jlong,
) -> jlong {
    guarded(&mut env, |env| {
        let window = unsafe {
            ANativeWindow_fromSurface(env.get_native_interface().cast(), surface.as_raw().cast())
        };
        let window = NativeWindow(NonNull::new(window).ok_or("UI_SURFACE: missing native window")?);
        let mut surfaces = SURFACES
            .lock()
            .map_err(|_| "UI_SURFACE: poisoned presenters")?;
        if existing != 0 {
            if let Some(presenter) = surfaces.get_mut(&(existing as u64)) {
                presenter.resume(Arc::new(window), width as u32, height as u32)?;
                return Ok(existing);
            }
        }
        // The registry retains the device domain only while a presenter exists.
        let context = surfaces
            .values()
            .next()
            .map(|page| page.context())
            .unwrap_or_default();
        let presenter = crate::gpu::Presenter::with_context(
            context,
            Arc::new(window),
            width as u32,
            height as u32,
        )?;
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        surfaces.insert(id, presenter);
        Ok(id as jlong)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_ui_GpuView_nativeSuspend(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
) {
    guarded(&mut env, |_| {
        if let Some(presenter) = SURFACES
            .lock()
            .map_err(|_| "UI_SURFACE: poisoned presenters")?
            .get_mut(&(id as u64))
        {
            presenter.suspend();
        }
        Ok(())
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_ui_GpuView_nativePrepareGpu(
    mut env: JNIEnv,
    _: JClass,
    page: jlong,
    id: jlong,
    width: jint,
    height: jint,
    scale: jfloat,
    rebuild: jboolean,
    caret_visible: jboolean,
    offsets: JString,
    epoch: jlong,
    time: jlong,
) {
    guarded(&mut env, |env| {
        let offsets: String = env.get_string(&offsets).map_err(|e| e.to_string())?.into();
        let offsets = serde_json::from_str(&offsets).map_err(|e| format!("UI_SCROLL: {e}"))?;
        with_page(page, |session, current_snapshot| {
            let now = super::animation_time(time)?;
            session.set_animation_surface(
                Some(u64::try_from(epoch).map_err(|_| "UI_ANIMATION: invalid epoch")?),
                now,
            )?;
            let generation = session.generation() + 1;
            let (mut snapshot, prepare, scene_cache_hit) = session.prepare_view_animated(
                u16::try_from(width).map_err(|_| "UI_VIEWPORT: width")?,
                u16::try_from(height).map_err(|_| "UI_VIEWPORT: height")?,
                scale,
                rebuild != 0,
                Some(&offsets),
                now,
                |mut display| {
                    if caret_visible == 0 { display.hide_caret(); }
                    let mut surfaces = SURFACES.lock().map_err(|_| "UI_SURFACE: poisoned presenters")?;
                    let presenter = surfaces.get_mut(&(id as u64)).ok_or("UI_SURFACE: detached")?;
                    if !presenter.prepare(&display, width as u32, height as u32, generation)? {
                        return Err("UI_SURFACE: suspended".into());
                    }
                    Ok((serde_json::json!({"generation":generation,"images":display.image_sources().collect::<std::collections::HashSet<_>>(),"loadedImages":display.loaded_image_count(),"nodes":display.nodes,"backend":"vello-vulkan","renderer":"sparse-strips","gpuDevices":presenter.context().device_count()?,"gpuRenderers":presenter.context().renderer_count()?}), presenter.preparation_timings_us(), presenter.scene_cache_hit()))
                },
            )?;
            snapshot["images"] =
                serde_json::to_value(session.image_requests()).map_err(|e| e.to_string())?;
            snapshot["metrics"] =
                serde_json::to_value(session.frame_metrics()).map_err(|e| e.to_string())?;
            snapshot["metrics"]["prepare"] = serde_json::json!({"sceneUs":prepare[0],"imagesUs":prepare[1],"encodeUs":prepare[2],"submitUs":prepare[3]});
            snapshot["metrics"]["sceneCacheHit"] = serde_json::json!(scene_cache_hit);
            snapshot["metrics"]["gpuDevices"] = snapshot["gpuDevices"].clone();
            snapshot["metrics"]["gpuRenderers"] = snapshot["gpuRenderers"].clone();
            *current_snapshot = snapshot.to_string();
            Ok(())
        })
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_ui_GpuView_nativePresentGpu(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    generation: jlong,
) -> jboolean {
    guarded(&mut env, |env| {
        let started = std::time::Instant::now();
        let mut surfaces = SURFACES
            .lock()
            .map_err(|_| "UI_SURFACE: poisoned presenters")?;
        let lock_us = started.elapsed().as_micros();
        let Some(surface) = surfaces.get_mut(&(id as u64)) else {
            return Ok(0);
        };
        let presented = surface.present(generation as u64)?;
        let [acquire, encode, submit, present] = surface.presentation_timings_us();
        drop(surfaces);
        if presented {
            // Diagnose host-thread stalls separately from the worker's render time.
            let message = format!(
                "GPU_PRESENT surface={id} generation={generation} lockUs={lock_us} acquireUs={acquire} encodeUs={encode} submitUs={submit} presentUs={present}"
            );
            let logged = (|| {
                let tag = env.new_string("LingXia.UI")?;
                let message = env.new_string(message)?;
                env.call_static_method(
                    "android/util/Log",
                    "i",
                    "(Ljava/lang/String;Ljava/lang/String;)I",
                    &[(&tag).into(), (&message).into()],
                )?;
                Ok::<_, jni::errors::Error>(())
            })();
            if logged.is_err() {
                let _ = env.exception_clear();
            }
        }
        Ok(u8::from(presented))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_ui_GpuView_nativeDetach(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
) {
    guarded(&mut env, |_| {
        SURFACES
            .lock()
            .map_err(|_| "UI_SURFACE: poisoned presenters")?
            .remove(&(id as u64));
        Ok(())
    });
}
