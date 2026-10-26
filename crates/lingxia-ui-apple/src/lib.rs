//! The iOS adapter for the GPU View host: one [`Page`] per native page view.
//! Every [`Page`] method runs on the main thread.
use lingxia_ui::platform::DeviceFacts;
use lingxia_ui::session::host::{Failure, ViewHost, ViewWorker};
use std::sync::Arc;

mod adapter;
mod dispatch;

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

impl Drop for Page {
    fn drop(&mut self) {
        self.host.dispose();
    }
}
