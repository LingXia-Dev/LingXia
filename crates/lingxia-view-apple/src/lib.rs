//! The iOS adapter for the GPU View host: one [`Page`] per native page view.
//! Every [`Page`] method runs on the main thread.
#![cfg(target_vendor = "apple")]
use lingxia_view::platform::DeviceFacts;
use lingxia_view::session::host::{Failure, ViewHost, ViewWorker};
use std::sync::Arc;

mod adapter;
#[cfg(all(test, feature = "gpu"))]
mod attach_tests;
mod dispatch;
mod fonts;
mod outlines;
#[cfg(feature = "gpu")]
use lingxia_view::renderer::render_thread as render;
#[cfg(feature = "gpu")]
mod surface;

pub use adapter::AppleAdapter;
pub use lingxia_view;

/// Runs `task` on the main thread, exactly once.
pub fn post_main(task: lingxia_view::platform::Task) {
    dispatch::post_after(std::time::Duration::ZERO, task);
}

thread_local! {
    /// The main-thread turn that presented a frame in its layer transaction, numbered.
    static PRESENT_TURN: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
}

/// Main thread: this turn presented a frame in its layer transaction, so native views moved
/// now (not posted) show on screen with that frame.
pub fn presenting_with_transaction() -> bool {
    present_turn().is_some()
}

/// The number of this main-thread presenting turn, for the web view sync log.
pub fn present_turn() -> Option<u64> {
    PRESENT_TURN.with(std::cell::Cell::get)
}

#[cfg(feature = "gpu")]
pub(crate) fn mark_present_turn() -> u64 {
    static TURNS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    if let Some(turn) = present_turn() {
        return turn;
    }
    let turn = TURNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    PRESENT_TURN.with(|cell| cell.set(Some(turn)));
    post_main(Box::new(|| PRESENT_TURN.with(|cell| cell.set(None))));
    turn
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
    /// Draw press (`ring` false) or focus-ring feedback, or clear it when `shape` is empty.
    /// `shape` is [`chrome::feedback_shape`] flattened: the node's rounded rect, then the
    /// masks to clip it to, in logical pixels. `color` is ARGB; a ring strokes `width`.
    ///
    /// [`chrome::feedback_shape`]: lingxia_view::interaction::chrome::feedback_shape
    fn feedback(&self, token: i64, ring: bool, shape: &[f32], color: u32, width: f32);
    fn haptic(&self, token: i64);
    /// A top-layer dialog opened or closed: while one is open the view covers the host's bars,
    /// which take no input.
    fn top_layer(&self, token: i64, modal: bool);
    /// The presented frame draws the host's bar snapshots up to `generation` under its top
    /// layer; 0 when it draws none. See `ViewHost::set_chrome_snapshot`.
    fn chrome_snapshot(&self, token: i64, generation: u64) {
        let _ = (token, generation);
    }
    /// After [`Page::cover`], a frame reached the screen at the view's size: drop the cover.
    fn uncover(&self, token: i64) {
        let _ = token;
    }
    /// A full-screen video asks the window for its orientation, or gives it back (`None`).
    fn video_orientation(
        &self,
        token: i64,
        orientation: Option<lingxia_view::scene::video::Orientation>,
    );
    /// The View worker thread: a line page `token`'s `console` wrote.
    fn console(&self, token: i64, level: log::Level, message: &str) {
        let _ = token;
        log::log!(target: "lingxia_view::console", level, "{message}");
    }
    /// The View takes the gesture under the finger (`true`) or lets it go: while it holds it,
    /// enclosing scroll views and the back swipe stand down.
    fn claim(&self, token: i64, exclusive: bool);
    /// Put `text` on the system pasteboard.
    fn copy_text(&self, text: &str);
    /// Show or move the text menu (frame-contract `TextMenu` JSON), or close it (empty).
    fn text_menu(&self, token: i64, menu: &str);
    /// The colours the host paints around this page while it is shown.
    fn host_chrome(&self, token: i64, chrome: &lingxia_view::platform::HostChrome);
    /// A frame presented; `node` is the focused editor's frame node JSON, or empty.
    fn ime_presented(&self, token: i64, generation: u64, node: &str);
    /// Focus the editor `node` of frame `generation`, placing the caret at a logical
    /// `point` (selecting the word there when `word`) when one is given.
    fn ime_focus(
        &self,
        token: i64,
        generation: u64,
        node: &str,
        point: Option<[f32; 2]>,
        word: bool,
    );
    /// A non-editor activation: end text input.
    fn ime_activation(&self, token: i64);
    /// The page instance ended: drop the editor without blurring.
    fn ime_reset(&self, token: i64);
    /// [`InputMethod::edit_text`] on editor `node` of frame `generation`.
    ///
    /// [`InputMethod::edit_text`]: lingxia_view::platform::InputMethod::edit_text
    fn ime_edit(
        &self,
        token: i64,
        generation: u64,
        node: &str,
        text: Option<&str>,
        start: i32,
        end: i32,
    ) -> lingxia_view::platform::EditOutcome;
    /// An accessibility event for the OS: `kind` 0 focused, 1 focus cleared, 2 input focused,
    /// 5 value changed, 6 content changed, 7 announce `text` politely, 8 assertively.
    fn accessibility_event(&self, token: i64, kind: u8, id: u32, text: &str);
    /// A frame for `connection` reached the screen; 0 is a standalone page.
    fn presented(&self, token: i64, connection: u64);
    /// The View worker thread: what the View sends Logic for `connection`, in order.
    fn logic(
        &self,
        token: i64,
        connection: u64,
        messages: Vec<lingxia_view::session::logic::ToLogic>,
    );
    /// What became of Logic's typed state writes for `connection`.
    fn state(&self, token: i64, connection: u64, signal: lingxia_view::session::state::StateSignal);
    /// Restart the Logic connection after a session failure; false when it cannot.
    fn restart(&self, token: i64, connection: u64) -> bool;
    fn recovery(&self, token: i64, failure: Option<&Failure>);
    /// Load authorized image bytes for the page; `done` runs once unless the
    /// returned handle is dropped first, which cancels the load.
    fn load_image(
        &self,
        token: i64,
        source: String,
        done: Box<dyn FnOnce(Result<Vec<u8>, String>) + Send>,
    ) -> Box<dyn Send>;
    /// The URL the platform player opens for a page's `<video src>`; called on the View worker.
    fn video_url(&self, token: i64, source: &str) -> Result<String, String> {
        let _ = token;
        Ok(source.to_owned())
    }
    /// Reconcile embedded `<webview>` mounts on the main thread.
    fn sync_webviews(&self, token: i64, mounts: &[lingxia_view::scene::webview::WebViewMount]) {
        let _ = (token, mounts);
    }
    /// Picture `web_ids` at `scale` device pixels per point; each PNG goes back through
    /// `ViewHost::webview_picture`.
    fn picture_webviews(&self, token: i64, web_ids: &[String], scale: f32) {
        let _ = (token, web_ids, scale);
    }
    /// Exit web-initiated fullscreen on `web_id`.
    fn exit_webview_fullscreen(&self, token: i64, web_id: &str) {
        let _ = (token, web_id);
    }
    /// Tear down all embedded `<webview>` mounts for `token`.
    fn clear_webviews(&self, token: i64) {
        let _ = token;
    }
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
        lingxia_view::text::fonts::set_system_fonts(fonts::system_fonts);
        lingxia_view::text::outlines::set_glyph_outlines(Box::new(
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
            self.host.environment_changed();
        }
    }

    /// The pixels per point the view is shown at, when it differs from the measured scale
    /// (a simulated device on the Mac's screen); `None` draws at the measured scale. Layout,
    /// input and the environment keep the measured scale, and screenshots its pixels.
    pub fn raster(&self, scale: Option<f32>) {
        let scale = scale.filter(|s| s.is_finite() && *s > 0.0);
        let adapter = self.host.adapter();
        let old = std::mem::replace(
            &mut *adapter
                .raster_scale
                .lock()
                .unwrap_or_else(|p| p.into_inner()),
            scale,
        );
        if old != scale {
            self.host.redraw();
        }
    }
}

impl Page {
    /// Bind the view's CAMetalLayer (an NSView or UIView backs it) to a Metal presenter on the
    /// render thread; frames wait for it, then use the CPU when it fails.
    #[cfg(feature = "gpu")]
    pub fn attach(&self, view: usize, width: u32, height: u32) {
        use lingxia_view::renderer::gpu::SurfaceSource;
        use lingxia_view::renderer::surfaces::{self, Target};
        use std::sync::atomic::Ordering;
        let adapter = self.host.adapter();
        // UIKit and AppKit give up a view's layer only on the main thread.
        let Some(layer) = surface::AppleLayer::of_view(view) else {
            adapter::log("GPU unavailable; using CPU renderer: the view has no CAMetalLayer");
            *adapter.target.lock().unwrap_or_else(|p| p.into_inner()) = Target::Unavailable;
            return self.host.surface_changed();
        };
        let existing = adapter.gpu.load(Ordering::Acquire);
        *adapter.target.lock().unwrap_or_else(|p| p.into_inner()) = Target::Waiting;
        let host = self.host.downgrade();
        let landing = adapter.landing.clone();
        let started = std::time::Instant::now();
        render::run(move || {
            let source = SurfaceSource::Layer(Arc::new(layer));
            let attached = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                surfaces::attach(source, width, height, existing)
            }))
            .unwrap_or_else(|panic| {
                Err(panic
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| panic.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "presenter setup panicked".into()))
            });
            if let Ok(id) = attached
                && !landing.bound(id)
            {
                return;
            }
            let ms = started.elapsed().as_millis();
            dispatch::post_after(
                std::time::Duration::ZERO,
                Box::new(move || {
                    // The page dropped meanwhile and detached it.
                    if let Ok(id) = attached
                        && !landing.take(id)
                    {
                        return;
                    }
                    let Some(host) = host.upgrade() else { return };
                    let adapter = host.adapter();
                    let target = match attached {
                        Ok(id) => {
                            // Two attaches in flight from one page: the newer presenter wins.
                            let old = adapter.gpu.swap(id, Ordering::AcqRel);
                            if old != 0 && old != id {
                                render::run(move || {
                                    surfaces::suspend(old);
                                    surfaces::detach(old);
                                });
                            }
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
                    host.surface_changed();
                }),
            );
        });
    }

    /// The newest GPU frame: width, height and straight-alpha RGBA rows.
    #[cfg(feature = "gpu")]
    pub fn readback(&self) -> Result<Option<(u32, u32, Vec<u8>)>, String> {
        let id = self.gpu();
        if id == 0 {
            return Ok(None);
        }
        render::run_now(move || lingxia_view::renderer::surfaces::readback(id))
            .ok_or("UI_SURFACE: render thread gone")?
    }

    /// The newest GPU frame as a PNG.
    #[cfg(feature = "gpu")]
    pub fn screenshot_png(&self) -> Result<Option<Vec<u8>>, String> {
        let id = self.gpu();
        if id == 0 {
            return Ok(None);
        }
        render::run_now(move || lingxia_view::renderer::surfaces::screenshot_png(id))
            .ok_or("UI_SURFACE: render thread gone")?
    }

    /// The view left the window or the app went to the background: nothing presents.
    #[cfg(feature = "gpu")]
    pub fn suspend(&self) {
        let id = self.gpu();
        render::run_now(move || lingxia_view::renderer::surfaces::suspend(id));
        self.host.surface_lost();
    }

    /// Hidden but kept, under the host's snapshot of its last frame: the presenter keeps its
    /// surface (parked) and caches, and drops what a redraw rebuilds.
    #[cfg(feature = "gpu")]
    pub fn trim(&self) {
        let id = self.gpu();
        if id != 0 {
            render::run_now(move || lingxia_view::renderer::surfaces::trim_covered(id));
        }
    }

    /// The view shows its snapshot of the last frame over the layer: see [`ViewHost::cover`].
    pub fn cover(&self) {
        self.host.cover();
    }

    #[cfg(feature = "gpu")]
    fn gpu(&self) -> u64 {
        self.host
            .adapter()
            .gpu
            .load(std::sync::atomic::Ordering::Acquire)
    }
}

/// Selection handles of static text, as touch hosts draw and drag them.
impl Page {
    /// The presented node holding a static-text selection, with its chrome geometry.
    fn static_selection(&self) -> Option<(String, lingxia_view::interaction::chrome::ChromeNode)> {
        let frame = self.host.snapshot()?;
        let node = frame
            .frame
            .nodes
            .iter()
            .find(|n| n.input.is_none() && n.text_selection.is_some())?;
        let json = serde_json::to_string(node).ok()?;
        let chrome = lingxia_view::interaction::chrome::ChromeNode::parse(&json).ok()?;
        Some((node.identity.to_string(), chrome))
    }

    /// The handles' ARGB colour, then the visible ends in logical pixels:
    /// `[which, center x, line bottom, line middle]` per end, anchor (0) then caret (1).
    pub fn selection_handles(&self) -> (u32, Vec<f32>) {
        let Some((_, node)) = self.static_selection() else {
            return (0, Vec::new());
        };
        let color = self
            .host
            .snapshot()
            .map_or(0, |frame| frame.feedback.selection_handle);
        let ends = (0..2)
            .filter_map(|which| {
                let [x, bottom, middle] =
                    lingxia_view::interaction::chrome::endpoint(&node, which)?;
                Some([which as f32, x as f32, bottom as f32, middle as f32])
            })
            .flatten()
            .collect();
        (color, ends)
    }

    /// A touch at a logical point grabs a handle: `[which, dx, dy]`, the offset from the
    /// line it moves, or `None`.
    pub fn handle_hit(&self, x: f32, y: f32) -> Option<[f32; 3]> {
        let (_, node) = self.static_selection()?;
        let (which, [cx, _, middle]) =
            lingxia_view::interaction::chrome::handle_hit(&node, x.into(), y.into(), false)?;
        self.host.text_handle_drag(true);
        Some([which as f32, x - cx as f32, y - middle as f32])
    }

    /// A grabbed handle moved to a logical point (its grab offset removed); `None` ends
    /// the drag.
    pub fn handle_drag(&self, which: Option<u8>, x: f32, y: f32) {
        let Some(which) = which else {
            return self.host.text_handle_drag(false);
        };
        let Some((identity, node)) = self.static_selection() else {
            return self.host.text_handle_drag(false);
        };
        if let Some(point) =
            lingxia_view::interaction::chrome::drag_point(&node, x.into(), y.into())
            && point.visible
        {
            self.host.move_text_selection(
                identity,
                which,
                [point.point[0] as f32, point.point[1] as f32],
            );
        }
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        self.host.dispose();
        #[cfg(feature = "gpu")]
        {
            let adapter = self.host.adapter();
            let mut surfaces = adapter.landing.release();
            surfaces.push(adapter.gpu.swap(0, std::sync::atomic::Ordering::AcqRel));
            surfaces.retain(|surface| *surface != 0);
            if !surfaces.is_empty() {
                // Ahead of frames still queued for it: once this returns the layer may go away.
                render::run_now(move || {
                    for surface in surfaces {
                        lingxia_view::renderer::surfaces::suspend(surface);
                        lingxia_view::renderer::surfaces::detach(surface);
                    }
                });
            }
        }
    }
}
