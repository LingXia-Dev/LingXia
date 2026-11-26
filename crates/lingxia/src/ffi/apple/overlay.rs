//! The Apple window overlay surface: a see-through GPU page whose Logic is Rust, over the window
//! that holds the caller's presentation. macOS hosts it in a borderless transparent child window
//! (the shell, or an lxapp's own window or float); iOS in a view over the scene's topmost LingXia
//! presentation, whose one window Swift reports as window 1. Every page call runs on the main
//! thread.
use super::live::{self, Bridge};
use crate::feedback::Feedback;
use lingxia_view_apple::Page;
use lingxia_view_apple::lingxia_view::platform::{
    OverlayAutomationReply, OverlayInput, OverlayLogic, OverlaySurface, Task, WindowId,
    WindowOverlays,
};
use lingxia_view_apple::lingxia_view::session::lifecycle::ConnectionTicket;
use lingxia_view_apple::lingxia_view::session::logic::ToView;
use lingxia_view_apple::lingxia_view::session::state::StateWrite;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

/// Overlays share one View worker, apart from every lxapp's.
const WORKER: &str = "lingxia:overlay";

/// Live overlays by page token, for the View worker's messages and closed windows.
static OVERLAYS: LazyLock<Mutex<HashMap<i64, (WindowId, OverlayLogic)>>> =
    LazyLock::new(Default::default);
/// Negative, so overlay tokens never meet lxapp page tokens.
static NEXT: AtomicI64 = AtomicI64::new(-1);
static FEEDBACK: OnceLock<Arc<Feedback>> = OnceLock::new();
static PRESENTATIONS: LazyLock<Mutex<Presentations>> = LazyLock::new(Default::default);

/// Where lxapps are presented, by window number, each with when the user last touched it.
#[derive(Default)]
struct Presentations {
    touches: u64,
    shell: Option<(u64, u64)>,
    apps: HashMap<String, (u64, u64)>,
}

impl Presentations {
    /// The window the user touched last of `appid`'s own and the shell; the shell otherwise.
    fn window_of(&self, appid: Option<&str>) -> Option<u64> {
        let own = appid.and_then(|appid| self.apps.get(appid)).copied();
        match (own, self.shell) {
            (Some(own), Some(shell)) => Some(if own.1 > shell.1 { own.0 } else { shell.0 }),
            (own, shell) => own.or(shell).map(|(window, _)| window),
        }
    }

    /// `appid`'s own window, or the shell for `None`; `touched` when the user brought it
    /// forward, otherwise it only fills a place nothing holds yet.
    fn presents(&mut self, appid: Option<&str>, window: u64, touched: bool) {
        let place = match appid {
            Some(appid) => self.apps.entry(appid.to_owned()).or_insert((window, 0)),
            None => self.shell.get_or_insert((window, 0)),
        };
        if touched {
            self.touches += 1;
            *place = (window, self.touches);
        } else if place.1 == 0 {
            place.0 = window;
        }
    }

    fn closed(&mut self, window: u64) {
        if self.shell.is_some_and(|(shell, _)| shell == window) {
            self.shell = None;
        }
        self.apps.retain(|_, (own, _)| *own != window);
    }
}

fn presentations() -> MutexGuard<'static, Presentations> {
    PRESENTATIONS.lock().unwrap_or_else(|p| p.into_inner())
}

fn overlays() -> MutexGuard<'static, HashMap<i64, (WindowId, OverlayLogic)>> {
    OVERLAYS.lock().unwrap_or_else(|p| p.into_inner())
}

/// Install the window feedback layer over Apple overlays; called once at startup.
pub(crate) fn install() {
    let _ = FEEDBACK.set(crate::feedback::install(Arc::new(AppleOverlays)));
}

/// View worker: overlay `token`'s Logic, or `None` when `token` is an lxapp page.
pub(super) fn logic_of(token: i64) -> Option<OverlayLogic> {
    overlays().get(&token).map(|(_, logic)| logic.clone())
}

/// Main thread: Swift reports where `appid` (the shell for `None`) is presented.
pub(super) fn presents(appid: Option<&str>, window: u64, touched: bool) {
    presentations().presents(appid, window, touched);
}

/// Main thread: `window` closed, or an overlay found no window by that number.
pub(super) fn window_closed(window: u64) {
    presentations().closed(window);
    if let Some(feedback) = FEEDBACK.get() {
        feedback.window_closed(WindowId(window));
    }
}

struct AppleOverlays;

impl WindowOverlays for AppleOverlays {
    fn window_of(&self, appid: Option<&str>) -> Option<WindowId> {
        presentations().window_of(appid).map(WindowId)
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
                    if !super::bridge::gpu_overlays_open(token, window.0) {
                        window_closed(window.0);
                    }
                }
                Err(error) => {
                    log::warn!("window overlay: {error}");
                    window_closed(window.0);
                }
            }
        }));
        Ok(Box::new(AppleSurface { token }))
    }

    fn after(&self, delay: Duration, task: Task) {
        lingxia_view_apple::post_main_after(delay, task);
    }
}

struct AppleSurface {
    token: i64,
}

/// Posted behind the page's creation, so calls made right after `open` reach it in order.
fn on_overlay(token: i64, work: impl FnOnce(&Page) + Send + 'static) {
    lingxia_view_apple::post_main(Box::new(move || live::with_page(token, work)));
}

impl OverlaySurface for AppleSurface {
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

    fn automation(&self, request: serde_json::Value, reply: OverlayAutomationReply) {
        live::automate(self.token, request, reply);
    }

    fn set_focus(&self, focused: bool) {
        let token = self.token;
        lingxia_view_apple::post_main(Box::new(move || {
            super::bridge::gpu_overlays_focus(token, focused)
        }));
    }

    fn update_environment(&self, update: serde_json::Value) {
        on_overlay(self.token, move |page| {
            if let Err(error) = page.host().update_environment(&update.to_string()) {
                log::warn!("window overlay environment: {error}");
            }
        });
    }
}

impl Drop for AppleSurface {
    fn drop(&mut self) {
        let token = self.token;
        overlays().remove(&token);
        lingxia_view_apple::post_main(Box::new(move || {
            drop(live::remove_page(token));
            super::bridge::gpu_overlays_close(token);
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::Presentations;

    #[test]
    fn an_lxapp_gets_the_window_the_user_touched_last_of_its_own_and_the_shell() {
        let mut places = Presentations::default();
        assert_eq!(places.window_of(Some("a")), None);
        // Shown without a touch (a dev session in the background) still places the shell.
        places.presents(None, 1, false);
        assert_eq!(places.window_of(Some("a")), Some(1));
        places.presents(Some("a"), 7, true);
        assert_eq!(places.window_of(Some("a")), Some(7));
        assert_eq!(places.window_of(Some("b")), Some(1));
        places.presents(None, 1, true);
        assert_eq!(places.window_of(Some("a")), Some(1));
        places.presents(Some("a"), 7, true);
        places.closed(7);
        assert_eq!(places.window_of(Some("a")), Some(1));
        places.closed(1);
        assert_eq!(places.window_of(None), None);
    }
}
