//! GPU presenters bound to platform windows, shared by every adapter. Prepare
//! runs on the View worker, present on the main thread; one registry shares the
//! device context between pages.
use super::gpu::Presenter;
use image::ImageEncoder;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

static NEXT: AtomicU64 = AtomicU64::new(1);
type Shared = Arc<Mutex<Presenter>>;
/// Held only to look up or insert; each presenter has its own lock, so one
/// page's encode or attach never blocks another page's present.
static SURFACES: LazyLock<Mutex<HashMap<u64, Shared>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn registry() -> Result<MutexGuard<'static, HashMap<u64, Shared>>, String> {
    SURFACES
        .lock()
        .map_err(|_| "UI_SURFACE: poisoned presenters".into())
}

fn presenter(id: u64) -> Result<Option<Shared>, String> {
    Ok(registry()?.get(&id).cloned())
}

fn lock(presenter: &Shared) -> Result<MutexGuard<'_, Presenter>, String> {
    presenter
        .lock()
        .map_err(|_| "UI_SURFACE: poisoned presenter".into())
}

/// Where a page's GPU target stands; see [`route`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// No presenter yet, or an attach is in flight.
    Waiting,
    Ready,
    /// Attach failed; frames use the CPU reference renderer.
    Unavailable,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Route {
    Gpu(u64),
    Cpu,
    /// Not ready to take a frame; the host prepares again when the target is.
    Wait,
}

/// A page whose GPU attach is pending must not rasterize on the CPU: that work
/// shares the View worker and would hold up its own attach and other pages.
pub fn route(surface: u64, target: Target) -> Route {
    match (surface, target) {
        (0, Target::Unavailable) => Route::Cpu,
        (0, _) => Route::Wait,
        (surface, _) => Route::Gpu(surface),
    }
}

/// Bind `window` to the presenter `existing`, or to a new one; returns its id.
pub fn attach<W>(window: W, width: u32, height: u32, existing: u64) -> Result<u64, String>
where
    Arc<W>: Into<wgpu::SurfaceTarget<'static>>,
    W: Send + Sync + 'static,
{
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
pub fn describe(id: u64) -> Option<(String, &'static str)> {
    let presenter = presenter(id).ok()??;
    let presenter = lock(&presenter).ok()?;
    Some((presenter.adapter().to_owned(), presenter.backend()))
}

pub fn suspend(id: u64) {
    if let Ok(Some(presenter)) = presenter(id)
        && let Ok(mut presenter) = lock(&presenter)
    {
        presenter.suspend();
    }
}

pub fn detach(id: u64) {
    if let Ok(mut surfaces) = registry() {
        surfaces.remove(&id);
    }
}

/// Encode `display`; `None` while the presenter is suspended or detached.
pub fn prepare(
    id: u64,
    display: &crate::DisplayList,
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
    let (filtered, cached, cached_bytes) = presenter.backdrop_cache_stats();
    Ok(Some(serde_json::json!({
        "backdropCache": {"filtered": filtered, "cached": cached, "cachedBytes": cached_bytes},
        "composite": presenter.composite_stats(),
        "backend": presenter.backend(),
        "renderer": "sparse-strips",
        "loadedImages": display.loaded_image_count(),
        "prepare": {"sceneUs": scene, "imagesUs": images, "encodeUs": encode, "submitUs": submit},
        "passes": presenter.pass_timings_us(),
        "sceneCacheHit": presenter.scene_cache_hit(),
        "gpuDevices": presenter.context().device_count()?,
        "gpuRenderers": presenter.context().renderer_count()?,
    })))
}

/// The presenter's timings when the frame reached the screen, `None` when it did not.
pub fn present(id: u64, revision: u64) -> Result<Option<[u64; 4]>, String> {
    let Some(presenter) = presenter(id)? else {
        return Ok(None);
    };
    let mut presenter = lock(&presenter)?;
    Ok(presenter
        .present(revision)?
        .then(|| presenter.presentation_timings_us()))
}

/// The newest frame presenter `id` holds as a PNG, `None` when it holds none.
pub fn screenshot_png(id: u64) -> Result<Option<Vec<u8>>, String> {
    let Some((width, height, pixels)) = readback(id)? else {
        return Ok(None);
    };
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(&pixels, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|e| format!("UI_SURFACE: screenshot: {e}"))?;
    Ok(Some(png))
}

/// The newest frame presenter `id` holds: width, height and straight-alpha RGBA rows.
pub fn readback(id: u64) -> Result<Option<(u32, u32, Vec<u8>)>, String> {
    let Some(presenter) = presenter(id)? else {
        return Ok(None);
    };
    lock(&presenter)?.readback()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_gpu_targets_wait_instead_of_rasterizing() {
        assert_eq!(route(0, Target::Waiting), Route::Wait);
        assert_eq!(route(0, Target::Ready), Route::Wait);
        assert_eq!(
            route(7, Target::Waiting),
            Route::Gpu(7),
            "re-attach keeps its presenter"
        );
        assert_eq!(route(7, Target::Ready), Route::Gpu(7));
        assert_eq!(route(0, Target::Unavailable), Route::Cpu);
    }
}
