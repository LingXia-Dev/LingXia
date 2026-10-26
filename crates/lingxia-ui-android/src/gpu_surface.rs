//! Vulkan presenters bound to Android `Surface`s. Prepare runs on the View
//! worker, present on the main thread; one registry shares the device context.
use lingxia_ui::renderer::gpu::Presenter;
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

pub(super) struct NativeWindow(NonNull<c_void>);
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

impl NativeWindow {
    /// # Safety
    /// `env` must be this thread's JNI environment and `surface` a live `android.view.Surface`.
    pub(super) unsafe fn from_surface(
        env: *mut jni::sys::JNIEnv,
        surface: jni::sys::jobject,
    ) -> Result<Self, String> {
        let window = unsafe { ANativeWindow_fromSurface(env.cast(), surface.cast()) };
        NonNull::new(window)
            .map(Self)
            .ok_or_else(|| "UI_SURFACE: missing native window".into())
    }
}

static NEXT: AtomicU64 = AtomicU64::new(1);
type Shared = Arc<Mutex<Presenter>>;
/// Held only to look up or insert; each presenter has its own lock, so one
/// page's encode or attach never blocks another page's present.
static SURFACES: LazyLock<Mutex<HashMap<u64, Shared>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn registry() -> Result<std::sync::MutexGuard<'static, HashMap<u64, Shared>>, String> {
    SURFACES
        .lock()
        .map_err(|_| "UI_SURFACE: poisoned presenters".into())
}

fn presenter(id: u64) -> Result<Option<Shared>, String> {
    Ok(registry()?.get(&id).cloned())
}

/// Drop what `id`'s presenter keeps only to speed up coming frames.
pub(super) fn release_transient(id: u64) {
    if let Ok(Some(presenter)) = presenter(id)
        && let Ok(mut presenter) = lock(&presenter)
    {
        presenter.release_transient();
    }
}

/// Memory pressure: every presenter drops its transient memory.
pub fn release_transient_all() {
    let Ok(presenters) = registry().map(|r| r.values().cloned().collect::<Vec<_>>()) else {
        return;
    };
    for presenter in presenters {
        if let Ok(mut presenter) = lock(&presenter) {
            presenter.release_transient();
        }
    }
}

fn lock(presenter: &Shared) -> Result<std::sync::MutexGuard<'_, Presenter>, String> {
    presenter
        .lock()
        .map_err(|_| "UI_SURFACE: poisoned presenter".into())
}

/// Bind `window` to the presenter `existing`, or to a new one; returns its id.
/// Runs on a dedicated attach thread: wgpu's GLES adapter makes its EGL context
/// current per call under its own lock, so the context is not tied to this thread.
pub(super) fn attach(
    window: NativeWindow,
    width: u32,
    height: u32,
    existing: u64,
) -> Result<u64, String> {
    if existing != 0
        && let Some(presenter) = presenter(existing)?
    {
        lock(&presenter)?.resume(Arc::new(window), width, height)?;
        return Ok(existing);
    }
    // The registry retains the device domain only while a presenter exists.
    let shared = registry()?.values().next().cloned();
    let context = match shared {
        Some(presenter) => lock(&presenter)?.context(),
        None => Default::default(),
    };
    let presenter = Presenter::with_fallback(context, Arc::new(window), width, height)?;
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    registry()?.insert(id, Arc::new(Mutex::new(presenter)));
    Ok(id)
}

/// The adapter record and metrics label of presenter `id`.
pub(super) fn describe(id: u64) -> Option<(String, &'static str)> {
    let presenter = presenter(id).ok()??;
    let presenter = lock(&presenter).ok()?;
    Some((presenter.adapter().to_owned(), presenter.backend()))
}

pub(super) fn suspend(id: u64) {
    if let Ok(Some(presenter)) = presenter(id)
        && let Ok(mut presenter) = lock(&presenter)
    {
        presenter.suspend();
    }
}

pub(super) fn detach(id: u64) {
    if let Ok(mut surfaces) = registry() {
        surfaces.remove(&id);
    }
}

/// Encode `display`; `None` while the presenter is suspended or detached.
pub(super) fn prepare(
    id: u64,
    display: &lingxia_ui::DisplayList,
    revision: u64,
) -> Result<Option<serde_json::Value>, String> {
    let Some(presenter) = presenter(id)? else {
        return Ok(None);
    };
    let mut presenter = lock(&presenter)?;
    if !presenter.prepare(
        display,
        u32::from(display.width),
        u32::from(display.height),
        revision,
    )? {
        return Ok(None);
    }
    let [scene, images, encode, submit] = presenter.preparation_timings_us();
    Ok(Some(serde_json::json!({
        "backend": presenter.backend(),
        "renderer": "sparse-strips",
        "loadedImages": display.loaded_image_count(),
        "prepare": {"sceneUs": scene, "imagesUs": images, "encodeUs": encode, "submitUs": submit},
        "passes": presenter.pass_timings_us(),
        "transientBytes": presenter.transient_bytes(),
        "sceneCacheHit": presenter.scene_cache_hit(),
        "gpuDevices": presenter.context().device_count()?,
        "gpuRenderers": presenter.context().renderer_count()?,
    })))
}

/// Returns the presenter's timings for diagnostics when the frame reached the screen.
pub(super) fn present(id: u64, revision: u64) -> Result<Option<[u64; 4]>, String> {
    let Some(presenter) = presenter(id)? else {
        return Ok(None);
    };
    let mut presenter = lock(&presenter)?;
    Ok(presenter
        .present(revision)?
        .then(|| presenter.presentation_timings_us()))
}
