//! `PlatformAdapter` over the Swift page view. Frames present through Metal on the
//! view's layer, or rasterize on the CPU pool and reach the view as pixels.
use crate::Shell;
use lingxia_ui::DisplayList;
use lingxia_ui::interaction::view::{AccessibilityEffect, Dispatch, Effect};
use lingxia_ui::platform::*;
use lingxia_ui::renderer::pool::Mailbox;
use lingxia_ui::session::host::{Failure, FrameSnapshot, FrameTiming, WeakViewHost};
use lingxia_ui::session::lifecycle::{CommitTicket, ConnectionTicket, SurfaceTicket};
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
    pub(crate) target: Mutex<lingxia_ui::renderer::surfaces::Target>,
    /// In-flight loads; dropping a handle cancels its transport.
    images: Mutex<HashMap<u64, Box<dyn Send>>>,
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
            target: Mutex::new(lingxia_ui::renderer::surfaces::Target::Unavailable),
            images: Mutex::new(HashMap::new()),
        }
    }

    fn images(&self) -> std::sync::MutexGuard<'_, HashMap<u64, Box<dyn Send>>> {
        self.images.lock().unwrap_or_else(|p| p.into_inner())
    }
}

pub(crate) fn log(message: &str) {
    log::info!(target: "lingxia_ui", "{message}");
}

pub enum Frame {
    #[cfg_attr(not(feature = "gpu"), allow(dead_code))]
    Gpu {
        surface: u64,
        revision: u64,
        metrics: serde_json::Value,
    },
    /// Rasterized off the View worker when presented.
    Cpu(DisplayList, CommitTicket),
}

impl AppleAdapter {
    #[cfg(feature = "gpu")]
    fn route(&self) -> lingxia_ui::renderer::surfaces::Route {
        let target = *self.target.lock().unwrap_or_else(|p| p.into_inner());
        lingxia_ui::renderer::surfaces::route(self.gpu.load(Ordering::Acquire), target)
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
            use lingxia_ui::renderer::surfaces::{self, Route};
            match self.route() {
                Route::Wait => return Ok(None),
                Route::Gpu(surface) => {
                    return Ok(surfaces::prepare(surface, &display, generation.get())?.map(
                        |metrics| Frame::Gpu {
                            surface,
                            revision: generation.get(),
                            metrics,
                        },
                    ));
                }
                Route::Cpu => {}
            }
        }
        Ok(Some(Frame::Cpu(display, generation)))
    }

    fn metrics(&self, frame: &Frame) -> Option<serde_json::Value> {
        Some(match frame {
            Frame::Gpu { metrics, .. } => metrics.clone(),
            Frame::Cpu(..) => serde_json::json!({"backend": "vello-cpu"}),
        })
    }

    fn present(&self, frame: Frame) -> Result<bool, String> {
        match frame {
            #[cfg(feature = "gpu")]
            Frame::Gpu {
                surface, revision, ..
            } => Ok(lingxia_ui::renderer::surfaces::present(surface, revision)?.is_some()),
            #[cfg(not(feature = "gpu"))]
            Frame::Gpu { .. } => Ok(false),
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

    fn deferred(&self, frame: &Frame) -> bool {
        matches!(frame, Frame::Cpu(..))
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
        use lingxia_ui::semantics::projection::Event;
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

impl InputMethod for AppleAdapter {
    fn reconcile(&self, _: &FrameSnapshot) {}
    fn focus(&self, _: &Dispatch, _: &FrameSnapshot) {}
    fn activation(&self) {}
    fn reset(&self) {}
    fn edit_text(&self, _: usize, _: Option<&str>, _: i32, _: i32) -> EditOutcome {
        EditOutcome::Failed
    }
}

impl Feedback for AppleAdapter {
    fn apply(&self, effects: &[Effect]) {
        for effect in effects {
            match effect {
                Effect::ArmLongPress => self.shell.arm_long_press(self.token),
                Effect::CancelLongPress => self.shell.cancel_long_press(self.token),
                Effect::Fling {
                    region,
                    horizontal,
                    velocity,
                } => self
                    .shell
                    .fling(self.token, region, *horizontal, *velocity),
                Effect::StopFling => self.shell.stop_fling(self.token),
                // Nothing on the page's Apple host can take a gesture from it.
                Effect::Claim { .. } => {}
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
        log(&format!(
            "present token={} generation={} size={}x{} workerMs={} nodes={}",
            self.token,
            frame.generation.get(),
            frame.width,
            frame.height,
            timing.worker_us / 1000,
            frame.frame.nodes.len(),
        ));
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
    fn warn(&self, message: &str) {
        log::warn!(target: "lingxia_ui", "{message}");
    }
    fn debug(&self, message: &str) {
        log::debug!(target: "lingxia_ui", "{message}");
    }
}

impl Clock for AppleAdapter {
    fn now(&self) -> Duration {
        self.started.elapsed()
    }
}
