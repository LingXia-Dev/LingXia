//! The Harmony window overlay surface: a see-through GPU page, whose Logic is Rust, in a sub-window
//! of the main window (window 1), on a texture XComponent that blends with what is under it.
//! ArkTS makes only the sub-window and its `GpuPage`; Rust owns the page and loads its module.
//! Every page call runs on the main thread.
use super::{Bridge, PAGES, arkts, capture_gpu_only_png, with_page};
use crate::feedback::Feedback;
use lingxia_view_harmony::Page;
use lingxia_view_harmony::lingxia_view::interaction::view::Key;
use lingxia_view_harmony::lingxia_view::platform::{
    OverlayAutomationReply, OverlayInput, OverlayLogic, OverlaySurface, Task, WindowId,
    WindowOverlays,
};
use lingxia_view_harmony::lingxia_view::session::lifecycle::ConnectionTicket;
use lingxia_view_harmony::lingxia_view::session::logic::ToView;
use lingxia_view_harmony::lingxia_view::session::state::StateWrite;
use napi_derive_ohos::napi;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

/// Overlays share one View worker, apart from every lxapp's.
const WORKER: &str = "lingxia:overlay";
/// The main window; ArkTS keeps the sub-windows over it.
const WINDOW: WindowId = WindowId(1);
const CONNECTION: u64 = 1;

/// Live overlays' Logic by page token, for the View worker's messages.
static OVERLAYS: LazyLock<Mutex<HashMap<i64, OverlayLogic>>> = LazyLock::new(Default::default);
/// Negative, so overlay tokens never meet lxapp page tokens.
static NEXT: AtomicI64 = AtomicI64::new(-1);
static FEEDBACK: OnceLock<Arc<Feedback>> = OnceLock::new();

fn overlays() -> MutexGuard<'static, HashMap<i64, OverlayLogic>> {
    OVERLAYS.lock().unwrap_or_else(|p| p.into_inner())
}

/// Install the window feedback layer over Harmony overlays; called once at startup.
pub(super) fn install() {
    let _ = FEEDBACK.set(crate::feedback::install(Arc::new(HarmonyOverlays)));
}

/// View worker: overlay `token`'s Logic, or `None` when `token` is an lxapp page.
pub(super) fn logic_of(token: i64) -> Option<OverlayLogic> {
    overlays().get(&token).cloned()
}

/// Main thread: the main window can show no overlay; what it holds settles.
pub(super) fn window_closed(window: u64) {
    if let Some(feedback) = FEEDBACK.get() {
        feedback.window_closed(WindowId(window));
    }
}

struct HarmonyOverlays;

impl WindowOverlays for HarmonyOverlays {
    fn window_of(&self, _: Option<&str>) -> Option<WindowId> {
        Some(WINDOW)
    }

    fn open(
        &self,
        window: WindowId,
        module: &'static str,
        logic: OverlayLogic,
    ) -> Result<Box<dyn OverlaySurface>, String> {
        let token = NEXT.fetch_sub(1, Ordering::Relaxed);
        overlays().insert(token, logic);
        lingxia_view_harmony::post_main(Box::new(move || {
            // ArkTS must know the token before the host's first events, sent during creation.
            arkts(token, "overlayOpen", "");
            let created = Page::new(token, Arc::new(Bridge), WORKER, true).and_then(|page| {
                page.set_see_through();
                page.with_host(|host| {
                    host.load(module.to_owned());
                    host.connect(ConnectionTicket::new(CONNECTION));
                })
                .ok_or_else(|| "the overlay page has no host".to_string())?;
                PAGES.with(|pages| pages.borrow_mut().insert(token, page));
                Ok(())
            });
            if let Err(error) = created {
                log::warn!("window overlay: {error}");
                arkts(token, "overlayClose", "");
                window_closed(window.0);
            }
        }));
        Ok(Box::new(HarmonySurface { token }))
    }

    fn after(&self, delay: Duration, task: Task) {
        lingxia_view_harmony::post_after(delay, task);
    }
}

struct HarmonySurface {
    token: i64,
}

/// Posted behind the page's creation, so calls made right after `open` reach it in order.
fn on_overlay(token: i64, work: impl FnOnce(&Page) + Send + 'static) {
    lingxia_view_harmony::post_main(Box::new(move || {
        with_page(token, work);
    }));
}

impl OverlaySurface for HarmonySurface {
    fn write_state(&self, write: StateWrite) {
        on_overlay(self.token, move |page| {
            page.with_host(|host| host.write_state(ConnectionTicket::new(CONNECTION), write));
        });
    }

    fn send(&self, message: ToView) {
        on_overlay(self.token, move |page| {
            page.with_host(|host| host.deliver(ConnectionTicket::new(CONNECTION), message));
        });
    }

    fn set_input(&self, input: OverlayInput) {
        let token = self.token;
        let block = if input == OverlayInput::Block {
            "true"
        } else {
            "false"
        };
        lingxia_view_harmony::post_main(Box::new(move || arkts(token, "overlayInput", block)));
    }

    fn set_focus(&self, focused: bool) {
        let token = self.token;
        let focused = if focused { "true" } else { "false" };
        lingxia_view_harmony::post_main(Box::new(move || arkts(token, "overlayFocus", focused)));
    }

    fn automation(&self, request: serde_json::Value, reply: OverlayAutomationReply) {
        let token = self.token;
        lingxia_view_harmony::post_main(Box::new(move || {
            let answer = if request["op"] == "screenshot" {
                capture_gpu_only_png(token)
            } else {
                with_page(token, |page| {
                    page.with_host(|host| host.automation_request(&request))
                        .unwrap_or_else(|| Err("the window overlay is not active".into()))
                })
                .unwrap_or_else(|| Err("the window overlay is not active".into()))
            };
            reply(answer);
        }));
    }
}

impl Drop for HarmonySurface {
    fn drop(&mut self) {
        let token = self.token;
        overlays().remove(&token);
        lingxia_view_harmony::post_main(Box::new(move || {
            drop(PAGES.with(|pages| pages.borrow_mut().remove(&token)));
            arkts(token, "overlayClose", "");
        }));
    }
}

/// Back while an overlay's dialog holds focus: Escape for its page, which cancels the dialog.
#[napi]
pub fn gpu_overlay_escape(token: i64) -> bool {
    with_page(token, |page| {
        page.with_host(|host| host.key(Key::Escape, false, false))
            .unwrap_or(false)
    })
    .unwrap_or(false)
}
