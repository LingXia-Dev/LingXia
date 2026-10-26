//! `PlatformAdapter` over the Swift page view. Frames rasterize on the CPU pool
//! and reach the view as pixels; input, IME and accessibility arrive in later steps.
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
    raster: OnceLock<Arc<Mailbox<DisplayList>>>,
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
            images: Mutex::new(HashMap::new()),
        }
    }

    fn images(&self) -> std::sync::MutexGuard<'_, HashMap<u64, Box<dyn Send>>> {
        self.images.lock().unwrap_or_else(|p| p.into_inner())
    }
}

pub(crate) fn log(message: &str) {
    eprintln!("LingXia.UI: {message}");
}

impl Presenter for AppleAdapter {
    type Frame = DisplayList;

    fn prepare(
        &self,
        display: DisplayList,
        _: SurfaceTicket,
        _: CommitTicket,
    ) -> Result<Option<DisplayList>, String> {
        Ok(Some(display))
    }

    fn metrics(&self, _: &DisplayList) -> Option<serde_json::Value> {
        Some(serde_json::json!({"backend": "vello-cpu"}))
    }

    fn present(&self, display: DisplayList) -> Result<bool, String> {
        let Some(host) = self.host.get().cloned() else {
            return Ok(false);
        };
        self.raster
            .get_or_init(|| Mailbox::new(move |display| rasterize(&host, display)))
            .submit(display);
        Ok(true)
    }
}

/// Raster thread; the pixels go to the view on the main thread.
fn rasterize(host: &WeakViewHost<AppleAdapter>, display: DisplayList) {
    let Some(strong) = host.upgrade() else {
        return;
    };
    let frame = match display.rasterize() {
        Ok(frame) => frame,
        Err(error) => return strong.fail(error),
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
            if !shown {
                host.redraw();
            }
        }),
    );
}

impl FrameScheduler for AppleAdapter {
    fn request_vsync(&self) {
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
    fn announce(&self, _: &[AccessibilityEffect]) {}
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
    fn apply(&self, _: &[Effect]) {}
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
        log(message);
    }
}

impl Clock for AppleAdapter {
    fn now(&self) -> Duration {
        self.started.elapsed()
    }
}
