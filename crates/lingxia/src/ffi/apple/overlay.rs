//! The macOS window overlay surface: a borderless transparent child window over the shell window,
//! holding a see-through GPU page whose Logic is Rust. Every page call runs on the main thread.
use super::live::{self, Bridge};
use crate::feedback::Feedback;
use lingxia_view_apple::Page;
use lingxia_view_apple::lingxia_view::platform::{
    OverlayInput, OverlayLogic, OverlaySurface, Task, WindowId, WindowOverlays,
};
use lingxia_view_apple::lingxia_view::session::lifecycle::ConnectionTicket;
use lingxia_view_apple::lingxia_view::session::logic::ToView;
use lingxia_view_apple::lingxia_view::session::state::StateWrite;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

/// macOS shows every lxapp in its one shell window.
const SHELL: WindowId = WindowId(1);
/// Overlays share one View worker, apart from every lxapp's.
const WORKER: &str = "lingxia:overlay";

/// Live overlays by page token, for the View worker's messages and closed windows.
static OVERLAYS: LazyLock<Mutex<HashMap<i64, (WindowId, OverlayLogic)>>> =
    LazyLock::new(Default::default);
/// Negative, so overlay tokens never meet lxapp page tokens.
static NEXT: AtomicI64 = AtomicI64::new(-1);
static FEEDBACK: OnceLock<Arc<Feedback>> = OnceLock::new();

fn overlays() -> MutexGuard<'static, HashMap<i64, (WindowId, OverlayLogic)>> {
    OVERLAYS.lock().unwrap_or_else(|p| p.into_inner())
}

/// Install the window feedback layer over macOS overlays; called once at startup.
pub(crate) fn install() {
    let _ = FEEDBACK.set(crate::feedback::install(Arc::new(MacOverlays)));
}

/// View worker: overlay `token`'s Logic, or `None` when `token` is an lxapp page.
pub(super) fn logic_of(token: i64) -> Option<OverlayLogic> {
    overlays().get(&token).map(|(_, logic)| logic.clone())
}

/// Main thread: overlay `token`'s window closed, or there was none to put it over.
pub(super) fn gone(token: i64) {
    let window = overlays().get(&token).map(|(window, _)| *window);
    if let (Some(window), Some(feedback)) = (window, FEEDBACK.get()) {
        feedback.window_closed(window);
    }
}

struct MacOverlays;

impl WindowOverlays for MacOverlays {
    fn window_of(&self, _: Option<&str>) -> Option<WindowId> {
        Some(SHELL)
    }

    fn open(
        &self,
        window: WindowId,
        module: &'static str,
        logic: OverlayLogic,
    ) -> Result<Box<dyn OverlaySurface>, String> {
        let token = NEXT.fetch_sub(1, Ordering::Relaxed);
        overlays().insert(token, (window, logic));
        lingxia_view_apple::post_main(Box::new(move || {
            match Page::new(token, Arc::new(Bridge), WORKER, true) {
                Ok(page) => {
                    page.set_see_through();
                    page.host().load(module.to_owned());
                    page.host().connect(ConnectionTicket::new(1));
                    live::insert_page(token, page);
                    if !super::bridge::gpu_overlays_open(token) {
                        gone(token);
                    }
                }
                Err(error) => {
                    log::warn!("window overlay: {error}");
                    gone(token);
                }
            }
        }));
        Ok(Box::new(MacSurface { token }))
    }

    fn after(&self, delay: Duration, task: Task) {
        lingxia_view_apple::post_main_after(delay, task);
    }
}

struct MacSurface {
    token: i64,
}

/// Posted behind the page's creation, so calls made right after `open` reach it in order.
fn on_overlay(token: i64, work: impl FnOnce(&Page) + Send + 'static) {
    lingxia_view_apple::post_main(Box::new(move || live::with_page(token, work)));
}

impl OverlaySurface for MacSurface {
    fn write_state(&self, write: StateWrite) {
        on_overlay(self.token, move |page| {
            page.host().write_state(ConnectionTicket::new(1), write)
        });
    }

    fn send(&self, message: ToView) {
        on_overlay(self.token, move |page| {
            page.host().deliver(ConnectionTicket::new(1), message)
        });
    }

    fn set_input(&self, input: OverlayInput) {
        let token = self.token;
        lingxia_view_apple::post_main(Box::new(move || {
            super::bridge::gpu_overlays_input(token, input == OverlayInput::Block)
        }));
    }

    fn set_focus(&self, focused: bool) {
        let token = self.token;
        lingxia_view_apple::post_main(Box::new(move || {
            super::bridge::gpu_overlays_focus(token, focused)
        }));
    }
}

impl Drop for MacSurface {
    fn drop(&mut self) {
        let token = self.token;
        overlays().remove(&token);
        lingxia_view_apple::post_main(Box::new(move || {
            drop(live::remove_page(token));
            super::bridge::gpu_overlays_close(token);
        }));
    }
}
