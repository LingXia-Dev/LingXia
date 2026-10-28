//! The iOS adapter for the GPU View host: one [`Page`] per native page view.
//! Every [`Page`] method runs on the main thread.
use lingxia_ui::platform::DeviceFacts;
use lingxia_ui::session::host::{Failure, ViewHost, ViewWorker};
use std::sync::Arc;

mod adapter;
mod dispatch;
mod fonts;
mod outlines;
#[cfg(feature = "gpu")]
mod surface;

pub use adapter::AppleAdapter;
pub use lingxia_ui;

/// Runs `task` on the main thread, exactly once.
pub fn post_main(task: lingxia_ui::platform::Task) {
    dispatch::post_after(std::time::Duration::ZERO, task);
}

/// The embedding host's half: the Swift view and the lxapp page binding.
pub trait Shell: Send + Sync + 'static {
    /// Show an opaque premultiplied RGBA8 frame; false when the view cannot take it now.
    fn present_pixels(&self, token: i64, pixels: &[u8], width: u32, height: u32) -> bool;
    fn loaded(&self, token: i64);
    /// Call [`ViewHost::vsync`] on the next display refresh; false without a display link.
    fn request_vsync(&self, token: i64) -> bool;
    /// Call [`ViewHost::long_press`] after the platform's long-press delay, unless disarmed.
    fn arm_long_press(&self, token: i64);
    fn cancel_long_press(&self, token: i64);
    /// Decay `velocity` (px/s) the platform's way, feeding each frame's distance to
    /// [`ViewHost::scroll_by`] for `region` until it stops or the region does not move.
    fn fling(&self, token: i64, region: &str, horizontal: bool, velocity: f32);
    fn stop_fling(&self, token: i64);
    /// An accessibility event for the OS: `kind` 0 focused, 1 focus cleared, 2 input focused,
    /// 5 value changed, 6 content changed, 7 announce `text` politely, 8 assertively.
    fn accessibility_event(&self, token: i64, kind: u8, id: u32, text: &str);
    /// A frame for `connection` reached the screen; 0 is a standalone page.
    fn presented(&self, token: i64, connection: u64);
    fn message(&self, token: i64, connection: u64, message: &str);
    /// Restart the Logic connection after a session failure; false when it cannot.
    fn restart(&self, token: i64, connection: u64) -> bool;
    fn recovery(&self, token: i64, failure: Option<&Failure>);
    fn bootstrap_overdue(&self, token: i64, overdue: bool);
    /// Load authorized image bytes for the page; `done` runs once unless the
    /// returned handle is dropped first, which cancels the load.
    fn load_image(
        &self,
        token: i64,
        source: String,
        done: Box<dyn FnOnce(Result<Vec<u8>, String>) + Send>,
    ) -> Box<dyn Send>;
}

pub struct Page {
    host: ViewHost<AppleAdapter>,
}

impl Page {
    /// Pages with the same `worker` key share one View isolate.
    pub fn new(
        token: i64,
        shell: Arc<dyn Shell>,
        worker: &str,
        logic: bool,
    ) -> Result<Self, String> {
        lingxia_ui::text::fonts::set_system_fonts(fonts::system_fonts);
        lingxia_ui::text::outlines::set_glyph_outlines(Box::new(
            outlines::CoreTextOutlines::default(),
        ));
        let worker = ViewWorker::shared(worker)?;
        let host = ViewHost::new(AppleAdapter::new(token, shell), worker, logic);
        let _ = host.adapter().host.set(host.downgrade());
        Ok(Self { host })
    }

    pub fn host(&self) -> &ViewHost<AppleAdapter> {
        &self.host
    }

    /// The view's measurement after layout or a trait change.
    pub fn measured(&self, facts: DeviceFacts) {
        let resized = {
            let mut current = self
                .host
                .adapter()
                .facts
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            if current.as_ref() == Some(&facts) {
                return;
            }
            let resized = current.as_ref().is_none_or(|old| {
                (old.width, old.height, old.scale) != (facts.width, facts.height, facts.scale)
            });
            *current = Some(facts);
            resized
        };
        if resized {
            self.host.surface_changed();
        } else {
            self.host.request_frame();
        }
    }
}

impl Page {
    /// Bind the view (an NSView or UIView backed by a CAMetalLayer) to a Metal
    /// presenter on the main thread; frames use the CPU until it is ready and
    /// whenever it fails.
    #[cfg(feature = "gpu")]
    pub fn attach(&self, view: usize, width: u32, height: u32) {
        use lingxia_ui::renderer::surfaces::{self, Target};
        use std::sync::atomic::Ordering;
        let adapter = self.host.adapter();
        let Some(view) = surface::AppleView::new(view) else {
            return;
        };
        let existing = adapter.gpu.load(Ordering::Acquire);
        let started = std::time::Instant::now();
        // AppKit and UIKit give up a view's layer only on the main thread.
        let attached = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            surfaces::attach(view, width, height, existing)
        }))
        .unwrap_or_else(|panic| {
            Err(panic
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "presenter setup panicked".into()))
        });
        let ms = started.elapsed().as_millis();
        let target = match attached {
            Ok(id) => {
                adapter.gpu.store(id, Ordering::Release);
                let (record, backend) = surfaces::describe(id).unwrap_or_default();
                adapter::log(&format!(
                    "backend={backend} surface={id} attachMs={ms} adapter={record}"
                ));
                Target::Ready
            }
            Err(error) => {
                adapter::log(&format!("GPU unavailable; using CPU renderer: {error}"));
                Target::Unavailable
            }
        };
        *adapter.target.lock().unwrap_or_else(|p| p.into_inner()) = target;
        self.host.surface_changed();
    }

    /// The newest GPU frame: width, height and straight-alpha RGBA rows.
    #[cfg(feature = "gpu")]
    pub fn readback(&self) -> Result<Option<(u32, u32, Vec<u8>)>, String> {
        let id = self
            .host
            .adapter()
            .gpu
            .load(std::sync::atomic::Ordering::Acquire);
        if id == 0 {
            return Ok(None);
        }
        lingxia_ui::renderer::surfaces::readback(id)
    }

    /// The newest GPU frame as a PNG.
    #[cfg(feature = "gpu")]
    pub fn screenshot_png(&self) -> Result<Option<Vec<u8>>, String> {
        let id = self
            .host
            .adapter()
            .gpu
            .load(std::sync::atomic::Ordering::Acquire);
        if id == 0 {
            return Ok(None);
        }
        lingxia_ui::renderer::surfaces::screenshot_png(id)
    }

    /// The view left the window or the app went to the background: nothing presents.
    #[cfg(feature = "gpu")]
    pub fn suspend(&self) {
        lingxia_ui::renderer::surfaces::suspend(
            self.host
                .adapter()
                .gpu
                .load(std::sync::atomic::Ordering::Acquire),
        );
        self.host.surface_lost();
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        self.host.dispose();
        #[cfg(feature = "gpu")]
        {
            let surface = self
                .host
                .adapter()
                .gpu
                .swap(0, std::sync::atomic::Ordering::AcqRel);
            if surface != 0 {
                lingxia_ui::renderer::surfaces::detach(surface);
            }
        }
    }
}
