//! `PlatformAdapter` over the Swift page view. Frames present through Metal on the
//! view's layer, or rasterize on the CPU pool and reach the view as pixels.
use crate::Shell;
use lingxia_view::DisplayList;
use lingxia_view::interaction::view::{AccessibilityEffect, Dispatch, Effect};
use lingxia_view::platform::*;
use lingxia_view::renderer::pool::Mailbox;
use lingxia_view::session::host::{Failure, FrameSnapshot, FrameTiming, WeakViewHost};
use lingxia_view::session::lifecycle::{CommitTicket, ConnectionTicket, SurfaceTicket};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub struct AppleAdapter {
    pub(crate) token: i64,
    pub(crate) shell: Arc<dyn Shell>,
    started: Instant,
    pub(crate) host: OnceLock<WeakViewHost<AppleAdapter>>,
    /// The view's latest measurement; `None` until it has a size.
    pub(crate) facts: Mutex<Option<DeviceFacts>>,
    raster: OnceLock<Arc<Mailbox<(DisplayList, CommitTicket)>>>,
    /// The attached GPU presenter, or 0.
    #[cfg(feature = "gpu")]
    pub(crate) gpu: std::sync::atomic::AtomicU64,
    #[cfg(feature = "gpu")]
    pub(crate) target: Mutex<lingxia_view::renderer::surfaces::Target>,
    /// In-flight loads; dropping a handle cancels its transport.
    images: Mutex<HashMap<u64, Box<dyn Send>>>,
    /// The pressed and keyboard-focused nodes with feedback drawn, redrawn per frame.
    highlights: Mutex<[Option<usize>; 2]>,
    /// A dialog of the top layer is open: the view sits above the host's bars.
    modal: std::sync::atomic::AtomicBool,
}

impl AppleAdapter {
    pub(crate) fn new(token: i64, shell: Arc<dyn Shell>) -> Self {
        Self {
            token,
            shell,
            started: Instant::now(),
            host: OnceLock::new(),
            facts: Mutex::new(None),
            raster: OnceLock::new(),
            #[cfg(feature = "gpu")]
            gpu: std::sync::atomic::AtomicU64::new(0),
            #[cfg(feature = "gpu")]
            // CPU frames until the view attaches a Metal presenter.
            target: Mutex::new(lingxia_view::renderer::surfaces::Target::Unavailable),
            images: Mutex::new(HashMap::new()),
            highlights: Mutex::new([None; 2]),
            modal: Default::default(),
        }
    }

    /// Draw (or clear) press (`which` 0) or focus-ring (1) feedback for `node` of `frame`.
    fn highlight(&self, which: usize, node: Option<usize>, frame: Option<&FrameSnapshot>) {
        self.highlights.lock().unwrap_or_else(|p| p.into_inner())[which] = node;
        let shape = node
            .zip(frame)
            .and_then(|(index, frame)| {
                let node = serde_json::to_string(frame.frame.nodes.get(index)?).ok()?;
                lingxia_view::interaction::chrome::feedback_shape(
                    &lingxia_view::interaction::chrome::ChromeNode::parse(&node).ok()?,
                )
            })
            .unwrap_or_default();
        let style = frame.map(|f| f.feedback);
        let (color, width) = match (which, style) {
            (0, Some(style)) => (style.press_overlay, 0.0),
            (_, Some(style)) => (style.focus_ring, style.focus_ring_width),
            (_, None) => (0, 0.0),
        };
        let flat: Vec<f32> = shape.into_iter().flatten().collect();
        self.shell
            .feedback(self.token, which == 1, &flat, color, width);
    }

    fn snapshot(&self) -> Option<Arc<FrameSnapshot>> {
        self.host.get()?.upgrade()?.snapshot()
    }

    fn images(&self) -> std::sync::MutexGuard<'_, HashMap<u64, Box<dyn Send>>> {
        self.images.lock().unwrap_or_else(|p| p.into_inner())
    }
}

pub(crate) fn log(message: &str) {
    log::info!(target: "lingxia_view", "{message}");
}

pub enum Frame {
    /// Drawn on the render thread while the worker goes on; presented there too.
    #[cfg(feature = "gpu")]
    Gpu {
        surface: u64,
        revision: u64,
        drawn: Arc<lingxia_view::renderer::render_thread::Drawn>,
    },
    /// Rasterized off the View worker when presented.
    Cpu(DisplayList, CommitTicket),
}

impl AppleAdapter {
    #[cfg(feature = "gpu")]
    fn route(&self) -> lingxia_view::renderer::surfaces::Route {
        let target = *self.target.lock().unwrap_or_else(|p| p.into_inner());
        lingxia_view::renderer::surfaces::route(self.gpu.load(Ordering::Acquire), target)
    }
}

impl Presenter for AppleAdapter {
    type Frame = Frame;

    fn prepare(
        &self,
        display: DisplayList,
        _: SurfaceTicket,
        generation: CommitTicket,
    ) -> Result<Option<Frame>, String> {
        #[cfg(feature = "gpu")]
        {
            use lingxia_view::renderer::render_thread;
            use lingxia_view::renderer::surfaces::Route;
            match self.route() {
                Route::Wait => return Ok(None),
                Route::Gpu(surface) => {
                    return Ok(Some(Frame::Gpu {
                        surface,
                        revision: generation.get(),
                        drawn: render_thread::draw(
                            render_thread::SURFACES,
                            surface,
                            generation.get(),
                            display,
                        ),
                    }));
                }
                Route::Cpu => {}
            }
        }
        Ok(Some(Frame::Cpu(display, generation)))
    }

    fn metrics(&self, frame: &Frame) -> Option<serde_json::Value> {
        Some(match frame {
            #[cfg(feature = "gpu")]
            Frame::Gpu { .. } => {
                serde_json::json!({"renderer": "sparse-strips", "renderThread": true})
            }
            Frame::Cpu(..) => serde_json::json!({"backend": "vello-cpu"}),
        })
    }

    fn present(&self, frame: Frame) -> Result<bool, String> {
        match frame {
            #[cfg(feature = "gpu")]
            Frame::Gpu {
                surface,
                revision,
                drawn,
            } => {
                let Some(host) = self.host.get().cloned() else {
                    return Ok(false);
                };
                lingxia_view::renderer::render_thread::present(
                    surface,
                    revision,
                    drawn,
                    move |shown, gpu| {
                        let Some(host) = host.upgrade() else { return };
                        match shown {
                            Ok(shown) => {
                                host.deferred_drawn(CommitTicket::new(revision), shown, gpu)
                            }
                            Err(error) => {
                                host.deferred_presented(CommitTicket::new(revision), false);
                                host.fail(error);
                            }
                        }
                    },
                );
                Ok(true)
            }
            Frame::Cpu(display, generation) => {
                let Some(host) = self.host.get().cloned() else {
                    return Ok(false);
                };
                self.raster
                    .get_or_init(|| {
                        Mailbox::new(move |(display, generation)| {
                            rasterize(&host, display, generation)
                        })
                    })
                    .submit((display, generation));
                Ok(true)
            }
        }
    }

    fn deferred(&self, _: &Frame) -> bool {
        true
    }
}

/// Raster thread; the pixels go to the view on the main thread. Every frame is
/// reported once, shown or dropped; the host re-prepares a dropped one.
fn rasterize(host: &WeakViewHost<AppleAdapter>, display: DisplayList, generation: CommitTicket) {
    let Some(strong) = host.upgrade() else {
        return;
    };
    let frame = match display.rasterize() {
        Ok(frame) => frame,
        Err(error) => {
            strong.deferred_presented(generation, false);
            return strong.fail(error);
        }
    };
    drop(strong);
    let host = host.clone();
    crate::dispatch::post_after(
        Duration::ZERO,
        Box::new(move || {
            let Some(host) = host.upgrade() else {
                return;
            };
            let adapter = host.adapter();
            let (width, height) = (frame.pixels.width(), frame.pixels.height());
            let shown = adapter.shell.present_pixels(
                adapter.token,
                frame.pixels.data_as_u8_slice(),
                u32::from(width),
                u32::from(height),
            );
            host.deferred_presented(generation, shown);
        }),
    );
}

impl FrameScheduler for AppleAdapter {
    fn request_vsync(&self) {
        if self.shell.request_vsync(self.token) {
            return;
        }
        let Some(host) = self.host.get().cloned() else {
            return;
        };
        crate::dispatch::post_after(
            Duration::ZERO,
            Box::new(move || {
                if let Some(host) = host.upgrade() {
                    host.vsync();
                }
            }),
        );
    }
}

impl MainThread for AppleAdapter {
    fn post(&self, task: Task) {
        crate::dispatch::post_after(Duration::ZERO, task);
    }
    fn post_after(&self, delay: Duration, task: Task) {
        crate::dispatch::post_after(delay, task);
    }
}

impl EnvironmentSource for AppleAdapter {
    fn environment(&self) -> Option<FrameEnvironment> {
        self.facts
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()?
            .environment()
    }
}

impl ImageProvider for AppleAdapter {
    fn fetch(&self, id: u64, source: &str) {
        let Some(host) = self.host.get().cloned() else {
            return;
        };
        let finished = Arc::new(AtomicBool::new(false));
        let done = {
            let finished = finished.clone();
            Box::new(move |result: Result<Vec<u8>, String>| {
                finished.store(true, Ordering::Release);
                if let Some(host) = host.upgrade() {
                    host.adapter().images().remove(&id);
                    host.image_loaded(id, result);
                }
            })
        };
        let handle = self.shell.load_image(self.token, source.into(), done);
        // A loader may finish synchronously; then there is nothing left to cancel.
        if !finished.load(Ordering::Acquire) {
            self.images().insert(id, handle);
        }
    }
    fn cancel(&self, id: u64) {
        let handle = self.images().remove(&id);
        drop(handle);
    }
}

impl AccessibilitySink for AppleAdapter {
    fn announce(&self, events: &[AccessibilityEffect]) {
        use lingxia_view::semantics::projection::Event;
        for event in events {
            let (kind, id, text) = match event {
                AccessibilityEffect::Focused { id } => (0, *id, ""),
                AccessibilityEffect::FocusCleared { id } => (1, *id, ""),
                AccessibilityEffect::InputFocused { id } => (2, *id, ""),
                AccessibilityEffect::Change(Event::ValueChanged { id }) => (5, *id, ""),
                AccessibilityEffect::Change(Event::ContentChanged) => (6, u32::MAX, ""),
                AccessibilityEffect::Change(Event::Announce {
                    id,
                    text,
                    assertive,
                }) => (if *assertive { 8 } else { 7 }, *id, text.as_str()),
                // VoiceOver reads text and selection from the element itself; hovering is
                // the system's own exploration.
                _ => continue,
            };
            self.shell.accessibility_event(self.token, kind, id, text);
        }
    }
    // Apple plays no click sound for an activation.
    fn clicked(&self) {}
}

fn node_json(frame: &FrameSnapshot, index: usize) -> Option<String> {
    serde_json::to_string(frame.frame.nodes.get(index)?).ok()
}

impl InputMethod for AppleAdapter {
    fn reconcile(&self, frame: &FrameSnapshot) {
        let focused = frame.frame.nodes.iter().position(|node| {
            node.input
                .as_ref()
                .is_some_and(|input| input.enabled && input.edit_token.is_some_and(|t| t > 0))
        });
        let node = focused
            .and_then(|index| node_json(frame, index))
            .unwrap_or_default();
        self.shell
            .ime_presented(self.token, frame.generation.get(), &node);
    }
    fn focus(&self, editor: &Dispatch, frame: &FrameSnapshot) {
        let Dispatch::Editor {
            node,
            commit,
            point,
            word,
        } = editor
        else {
            return;
        };
        if let Some(json) = node_json(frame, *node) {
            self.shell
                .ime_focus(self.token, commit.get(), &json, *point, *word);
        }
    }
    fn activation(&self) {
        self.shell.ime_activation(self.token);
    }
    fn reset(&self) {
        self.shell.ime_reset(self.token);
    }
    fn edit_text(&self, node: usize, text: Option<&str>, start: i32, end: i32) -> EditOutcome {
        let Some((frame, json)) = self
            .snapshot()
            .and_then(|frame| node_json(&frame, node).map(|json| (frame, json)))
        else {
            return EditOutcome::Failed;
        };
        self.shell
            .ime_edit(self.token, frame.generation.get(), &json, text, start, end)
    }
}

impl Feedback for AppleAdapter {
    fn apply(&self, effects: &[Effect]) {
        for effect in effects {
            match effect {
                // A node with a pressed style repaints itself.
                Effect::Press {
                    node,
                    styled: false,
                    ..
                } => self.highlight(0, Some(*node), self.snapshot().as_deref()),
                Effect::Press { .. } | Effect::ClearPress => self.highlight(0, None, None),
                Effect::FocusRing { node } => {
                    self.highlight(1, Some(*node), self.snapshot().as_deref())
                }
                Effect::ClearFocusRing => self.highlight(1, None, None),
                Effect::Haptic => self.shell.haptic(self.token),
                Effect::ArmLongPress => self.shell.arm_long_press(self.token),
                Effect::CancelLongPress => self.shell.cancel_long_press(self.token),
                Effect::Fling {
                    region,
                    horizontal,
                    velocity,
                } => self.shell.fling(self.token, region, *horizontal, *velocity),
                Effect::StopFling => self.shell.stop_fling(self.token),
                Effect::Claim { exclusive } => self.shell.claim(self.token, *exclusive),
                _ => {}
            }
        }
    }
}

impl HostEvents for AppleAdapter {
    fn loaded(&self) {
        self.shell.loaded(self.token);
    }
    fn presented(&self, frame: &FrameSnapshot, timing: &FrameTiming) {
        // An exiting dialog still covers the bars until its exit ends.
        let open = frame.frame.nodes.iter().any(|node| {
            node.role.as_deref() == Some("dialog")
                && lingxia_view::interaction::frame::visible(node)
        });
        let modal = open
            || (self.modal.load(std::sync::atomic::Ordering::Relaxed) && frame.metrics.ghosts > 0);
        if self.modal.swap(modal, std::sync::atomic::Ordering::Relaxed) != modal {
            self.shell.top_layer(self.token, modal);
        }
        // Feedback follows its node as the page scrolls and changes.
        let highlights = *self.highlights.lock().unwrap_or_else(|p| p.into_inner());
        for (which, node) in highlights.into_iter().enumerate() {
            if node.is_some() {
                self.highlight(which, node, Some(frame));
            }
        }
        log::debug!(
            target: "lingxia_view",
            "present token={} generation={} size={}x{} workerMs={} nodes={}",
            self.token,
            frame.generation.get(),
            frame.width,
            frame.height,
            timing.worker_us / 1000,
            frame.frame.nodes.len(),
        );
        let connection = self
            .host
            .get()
            .and_then(WeakViewHost::upgrade)
            .map_or(0, |host| host.lifecycle().connection().get());
        self.shell.presented(self.token, connection);
    }
    fn message(&self, connection: ConnectionTicket, message: &str) {
        self.shell.message(self.token, connection.get(), message);
    }
    fn restart(&self, connection: ConnectionTicket) -> bool {
        self.shell.restart(self.token, connection.get())
    }
    fn recovery(&self, failure: Option<&Failure>) {
        if let Some(failure) = failure {
            log(&format!("recovery token={}: {}", self.token, failure.error));
        }
        self.shell.recovery(self.token, failure);
    }
    fn bootstrap_overdue(&self, overdue: bool) {
        self.shell.bootstrap_overdue(self.token, overdue);
    }
    fn copy_text(&self, text: &str) {
        self.shell.copy_text(text);
    }
    fn text_menu(&self, menu: Option<&lingxia_view::session::host::TextMenu>) {
        let json = menu
            .and_then(|menu| serde_json::to_string(menu).ok())
            .unwrap_or_default();
        self.shell.text_menu(self.token, &json);
    }
    fn host_chrome(&self, chrome: &lingxia_view::platform::HostChrome) {
        self.shell.host_chrome(self.token, chrome);
    }
    fn warn(&self, message: &str) {
        log::warn!(target: "lingxia_view", "{message}");
    }
    fn debug(&self, message: &str) {
        log::debug!(target: "lingxia_view", "{message}");
    }
}

impl Clock for AppleAdapter {
    fn now(&self) -> Duration {
        self.started.elapsed()
    }
}
