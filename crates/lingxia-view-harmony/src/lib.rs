//! The HarmonyOS adapter for the GPU View host: one [`Page`] per native XComponent
//! node, mounted in an ArkTS `ContentSlot`. Rust owns the page's `ViewHost`; ArkTS
//! owns the slot and the chrome around it. Every [`Page`] method runs on the main thread.
#![cfg(target_env = "ohos")]
use lingxia_view::platform::{DeviceFacts, HostChrome};
use lingxia_view::session::host::{Failure, ViewHost, ViewWorker};
use std::sync::Arc;

mod accessibility;
mod adapter;
mod dispatch;
mod fonts;
mod ime;
use lingxia_view::renderer::render_thread as render;
mod surface;
mod sys;
mod video;
mod xcomponent;

pub use adapter::HarmonyAdapter;
pub use dispatch::{install as install_main_thread, post_after, post_main};
pub use lingxia_view;
pub use napi_sys_ohos;

/// The embedding host's half: ArkTS chrome around the page and the lxapp page binding.
/// Calls arrive on the main thread unless a method says otherwise.
pub trait Shell: Send + Sync + 'static {
    fn loaded(&self, token: i64);
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
    /// A shown page owed a frame this long; see `HostEvents::frame_gap`.
    fn frame_gap(&self, token: i64, owed_ms: u64) {
        let _ = (token, owed_ms);
    }
    /// Show the page in place of the outgoing one; see `HostEvents::revealed`.
    fn revealed(&self, token: i64, hold_surface: bool);
    /// Load authorized image bytes; `done` runs once unless the handle is dropped first.
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
    /// Draw press (`ring` false) or focus-ring feedback, or clear it when `shape` is empty:
    /// the node's rounded rect then its clip masks, virtual pixels; `color` is ARGB.
    fn feedback(&self, token: i64, ring: bool, shape: &[f32], color: u32, width: f32);
    fn haptic(&self, token: i64);
    /// The page's node took ArkUI focus. Call [`focus_by_key`] back when the focus system is in
    /// its keyboard state; a touch or a page switch focuses the node too and must show no ring.
    fn focus_gained(&self, token: i64);
    /// A top-layer dialog opened or closed; while open the page covers the host's bars.
    fn top_layer(&self, token: i64, modal: bool);
    /// The presented frame draws the host's bar snapshots up to `generation`; 0 when none.
    fn chrome_snapshot(&self, token: i64, generation: u64) {
        let _ = (token, generation);
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
    fn copy_text(&self, text: &str);
    /// Read the clipboard; `done` runs once, on any thread.
    fn paste(&self, done: Box<dyn FnOnce(Option<String>) + Send>);
    /// Show or move the text menu (frame-contract `TextMenu` JSON), or close it (empty).
    fn text_menu(&self, token: i64, menu: &str);
    /// Place the handles of a static-text selection (`{identity, color, ends}`), or remove
    /// them (empty).
    fn selection_handles(&self, token: i64, handles: &str);
    fn host_chrome(&self, token: i64, chrome: &HostChrome);
    /// Reconcile embedded `<webview>` mounts on the main thread.
    fn sync_webviews(&self, token: i64, mounts: &[lingxia_view::scene::webview::WebViewMount]) {
        let _ = (token, mounts);
    }
    /// Picture each of `web_ids` for the page to draw while they cannot show live; the PNGs
    /// come back through [`lingxia_view::session::host::ViewHost::webview_picture`].
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

/// Live pages and their nodes; main thread only, as are all ArkUI callbacks.
mod pages {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    pub(crate) type Host = ViewHost<HarmonyAdapter>;

    #[derive(Default)]
    struct Registry {
        hosts: HashMap<i64, Host>,
        nodes: HashMap<i64, xcomponent::Node>,
    }

    thread_local! {
        static PAGES: RefCell<Registry> = RefCell::new(Registry::default());
    }

    /// Runs `work` on page `token`'s host, never while the registry is borrowed.
    pub(crate) fn with<T>(token: i64, work: impl FnOnce(&Host) -> T) -> Option<T> {
        let host = PAGES.with(|pages| pages.borrow().hosts.get(&token).cloned())?;
        Some(work(&host))
    }

    pub(crate) fn provider(token: i64) -> Option<*mut sys::ArkUI_AccessibilityProvider> {
        PAGES.with(|pages| {
            let provider = pages.borrow().nodes.get(&token)?.provider;
            (!provider.is_null()).then_some(provider)
        })
    }

    pub(crate) fn insert(token: i64, host: Host) {
        PAGES.with(|pages| pages.borrow_mut().hosts.insert(token, host));
    }

    pub(crate) fn mounted(token: i64, node: xcomponent::Node) {
        PAGES.with(|pages| pages.borrow_mut().nodes.insert(token, node));
    }

    /// Takes the page's host and its node; the node owns the ArkUI surface.
    pub(crate) fn remove(token: i64) -> (Option<Host>, Option<xcomponent::Node>) {
        PAGES.with(|pages| {
            let mut pages = pages.borrow_mut();
            (pages.hosts.remove(&token), pages.nodes.remove(&token))
        })
    }

    /// Swaps a mounted page's XComponent between `SURFACE` and `TEXTURE` when `<webview>` mounts
    /// enter or leave the scene; a no-op before `mount` or when the node already matches.
    pub(crate) fn rebind_node(token: i64) {
        let Some((host, old_node)) = PAGES.with(|pages| {
            let mut pages = pages.borrow_mut();
            let host = pages.hosts.get(&token)?.clone();
            let want_texture = host.adapter().wants_texture();
            if pages.nodes.get(&token)?.texture == want_texture {
                return None;
            }
            Some((host, pages.nodes.remove(&token)?))
        }) else {
            return;
        };
        let content = old_node.content;
        let want_texture = host.adapter().wants_texture();
        host.adapter().surface_destroyed(&host);
        drop(old_node);
        match xcomponent::Node::mount(token, content, want_texture) {
            Ok(node) => mounted(token, node),
            Err(error) => log::warn!(target: "lingxia_view", "UI_SURFACE: rebind failed: {error}"),
        }
    }

    /// An accessibility Paste: the clipboard arrives later, so the action reports success now.
    pub(crate) fn paste(token: i64, id: u32) -> i32 {
        let Some(shell) = with(token, |host| host.adapter().shell.clone()) else {
            return sys::RESULT_FAILED;
        };
        shell.paste(Box::new(move |text| {
            let Some(text) = text else { return };
            dispatch::post_main(Box::new(move || {
                with(token, |host| {
                    host.accessibility(
                        id,
                        lingxia_view::interaction::view::AccessibilityAction::Paste(text),
                    )
                });
            }));
        }));
        sys::RESULT_SUCCESSFUL
    }
}

/// How an injected touch sequence ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchEnd {
    Up,
    Cancel,
    /// The finger stays down; a later sequence continues it.
    Hold,
}

/// Touch slop and the slowest fling of injected touches, as for real ones.
const INJECTED_SLOP_VP: f32 = 8.0;
const INJECTED_MIN_FLING_VP: f32 = 50.0;

/// Runs timed touches on page `token` as its node would take them: `[x, y, at_ms]` in
/// logical pixels, a down first unless `continued`. Blocking; never on the main thread.
/// Returns the events delivered.
pub fn inject_touch(
    token: i64,
    points: &[[f64; 3]],
    continued: bool,
    end: TouchEnd,
) -> Result<u32, String> {
    use lingxia_view::interaction::view::{Pointer, PointerPhase};
    let scale =
        dispatch::run_on_main(move || pages::with(token, |host| host.adapter().placement().0))
            .flatten()
            .ok_or("page is not active")?;
    let started = std::time::Instant::now();
    let mut tracker = xcomponent::Velocity::default();
    let mut events = 0;
    for (i, &[x, y, at]) in points.iter().enumerate() {
        let due = std::time::Duration::from_secs_f64(at.max(0.0) / 1000.0);
        if let Some(wait) = due.checked_sub(started.elapsed()) {
            std::thread::sleep(wait);
        }
        let last = i + 1 == points.len();
        let phase = match (i, last, end) {
            (0, _, _) if !continued => PointerPhase::Down,
            (_, true, TouchEnd::Up) => PointerPhase::Up,
            (_, true, TouchEnd::Cancel) => PointerPhase::Cancel,
            _ => PointerPhase::Move,
        };
        let (x, y) = (x as f32 * scale, y as f32 * scale);
        // Scheduled times, not delivery times: main-thread latency must not bend the authored velocity.
        tracker.add((at * 1e6) as i64, x, y);
        let velocity = if phase == PointerPhase::Up {
            tracker.estimate()
        } else {
            [0.0; 2]
        };
        let planned = started + due;
        dispatch::run_on_main(move || {
            pages::with(token, |host| {
                if phase == PointerPhase::Down {
                    host.adapter().begin_gesture();
                }
                // Planned, not delivered: a busy main thread counts against input latency.
                host.input_age(planned.elapsed().as_micros() as u64);
                host.pointer(Pointer {
                    phase,
                    x,
                    y,
                    slop: INJECTED_SLOP_VP * scale,
                    velocity,
                    min_fling: INJECTED_MIN_FLING_VP * scale,
                });
            })
        })
        .flatten()
        .ok_or("page is not active")?;
        events += 1;
    }
    Ok(events)
}

/// A key moved ArkUI focus to page `token`: keyboard focus walks into it. Main thread only.
pub fn focus_by_key(token: i64) {
    xcomponent::focus::by_key(token);
}

/// The localized phrases the accessibility provider speaks; ArkTS reads them from resources.
pub fn set_accessibility_strings(suggestions: String, level: String) {
    accessibility::set_strings(suggestions, level);
}

/// A drag an embedded `<webview>` yielded at its page's edge, which the page takes from where
/// the finger went down: `phase` 0 down, 1 move, 2 up, 3 cancel; page vp and vp/s. Main thread.
pub fn web_handoff(token: i64, phase: u8, point: [f32; 2], velocity: [f32; 2]) {
    use lingxia_view::interaction::view::{Pointer, PointerPhase};
    pages::with(token, |host| {
        let scale = host.adapter().placement().0;
        let phase = match phase {
            0 => PointerPhase::Down,
            1 => PointerPhase::Move,
            2 => PointerPhase::Up,
            _ => PointerPhase::Cancel,
        };
        if phase == PointerPhase::Down {
            host.adapter().begin_gesture();
            host.hover(None);
        }
        host.pointer(Pointer {
            phase,
            x: point[0] * scale,
            y: point[1] * scale,
            slop: xcomponent::SLOP_VP * scale,
            velocity: if phase == PointerPhase::Up {
                [velocity[0] * scale, velocity[1] * scale]
            } else {
                [0.0; 2]
            },
            min_fling: xcomponent::MIN_FLING_VP * scale,
        });
    });
}

/// Any thread: deliver an embedded `<webview>` event or readiness flag to `token`'s host.
pub fn webview_event(token: i64, web_id: String, event: lingxia_view::WebViewInbound) {
    dispatch::post_main(Box::new(move || {
        pages::with(token, |host| host.webview_event(web_id, event));
    }));
}

/// The page's visible area `[left, top, width, height]` in logical pixels.
pub fn page_viewport(token: i64) -> Option<[f64; 4]> {
    dispatch::run_on_main(move || {
        pages::with(token, |host| {
            let adapter = host.adapter();
            let (scale, _) = adapter.placement();
            let (w, h) = adapter.extent();
            [0.0, 0.0, f64::from(w / scale), f64::from(h / scale)]
        })
    })
    .flatten()
}

/// The newest frame page `token` showed, for a window screenshot: the window snapshot leaves
/// XComponent surfaces out. Read on the render thread; the caller encodes.
pub fn frame(token: i64) -> Option<(u32, u32, Vec<u8>)> {
    let id = dispatch::run_on_main(move || {
        pages::with(token, |host| {
            host.adapter()
                .gpu
                .load(std::sync::atomic::Ordering::Acquire)
        })
    })
    .flatten()
    .filter(|id| *id != 0)?;
    crate::render::run_now(move || lingxia_view::renderer::surfaces::readback(id))?
        .ok()
        .flatten()
}

/// One GPU page: its host, and once mounted, its XComponent node.
pub struct Page {
    token: i64,
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
        video::install();
        let worker = ViewWorker::shared(worker)?;
        let host = ViewHost::new(HarmonyAdapter::new(token, shell), worker, logic);
        let _ = host.adapter().host.set(host.downgrade());
        pages::insert(token, host);
        Ok(Self { token })
    }

    pub fn token(&self) -> i64 {
        self.token
    }

    /// A window overlay: frames are clear wherever the page draws nothing. Before `mount`.
    pub fn set_see_through(&self) {
        pages::with(self.token, |host| {
            host.adapter()
                .see_through
                .store(true, std::sync::atomic::Ordering::Relaxed)
        });
    }

    /// Runs `work` on this page's host.
    pub fn with_host<T>(&self, work: impl FnOnce(&ViewHost<HarmonyAdapter>) -> T) -> Option<T> {
        pages::with(self.token, work)
    }

    /// Creates the page's XComponent node in the ArkTS `NodeContent` passed as `content`.
    ///
    /// # Safety
    /// `env` must be the main thread's environment and `content` a `NodeContent` value in it.
    pub unsafe fn mount(
        &self,
        env: napi_sys_ohos::napi_env,
        content: napi_sys_ohos::napi_value,
    ) -> Result<(), String> {
        let mut handle = std::ptr::null_mut();
        // SAFETY: the caller's live env and value.
        let code = unsafe { sys::OH_ArkUI_GetNodeContentFromNapiValue(env, content, &mut handle) };
        if code != 0 || handle.is_null() {
            return Err(format!("UI_SURFACE: not a NodeContent ({code})"));
        }
        let texture =
            pages::with(self.token, |host| host.adapter().wants_texture()).unwrap_or(false);
        let node = xcomponent::Node::mount(self.token, handle, texture)?;
        pages::mounted(self.token, node);
        Ok(())
    }

    /// The page's measurement after layout, an avoid-area or a configuration change;
    /// `origin` is its top-left on screen in device pixels.
    pub fn measured(&self, facts: DeviceFacts, origin: [f32; 2]) {
        pages::with(self.token, |host| {
            let adapter = host.adapter();
            *adapter.origin.lock().unwrap_or_else(|p| p.into_inner()) = origin;
            let changed = {
                let mut current = adapter.facts.lock().unwrap_or_else(|p| p.into_inner());
                let changed = current.as_ref() != Some(&facts);
                *current = Some(facts);
                changed
            };
            if changed {
                host.environment_changed();
            }
        });
    }

    /// Whether a rightward drag from a screen point (device pixels) would scroll the page.
    pub fn scrolls_right_at(&self, screen: [f32; 2]) -> bool {
        pages::with(self.token, |host| {
            let (_, origin) = host.adapter().placement();
            let local = [screen[0] - origin[0], screen[1] - origin[1]];
            host.webview_at(local).is_some() || host.can_scroll_horizontally_at(local, -1)
        })
        .unwrap_or(false)
    }

    /// A selection handle dragged to a page point (vp): the selection end `which` follows it.
    pub fn move_text_selection(&self, which: u8, point: [f32; 2]) {
        use lingxia_view::interaction::chrome::{ChromeNode, drag_point};
        pages::with(self.token, |host| {
            let Some((identity, json)) = host.adapter().selection() else {
                return;
            };
            let Some(drag) = ChromeNode::parse(&json)
                .ok()
                .and_then(|node| drag_point(&node, f64::from(point[0]), f64::from(point[1])))
            else {
                return;
            };
            if drag.visible {
                host.move_text_selection(identity, which.min(1), drag.point.map(|v| v as f32));
            }
        });
    }

    /// A handle was grabbed or let go; the core hides the text menu meanwhile.
    pub fn text_handle_drag(&self, active: bool) {
        pages::with(self.token, |host| host.text_handle_drag(active));
    }

    /// Hidden but kept: the presenter drops its render targets; the surface keeps its frame.
    pub fn trim_surface(&self) {
        let id = pages::with(self.token, |host| {
            host.adapter()
                .gpu
                .load(std::sync::atomic::Ordering::Acquire)
        })
        .unwrap_or(0);
        if id != 0 {
            crate::render::run(move || lingxia_view::renderer::surfaces::trim(id));
        }
    }

    /// The newest GPU frame as a PNG, for automation screenshots.
    pub fn screenshot_png(&self) -> Result<Option<Vec<u8>>, String> {
        let id = pages::with(self.token, |host| {
            host.adapter()
                .gpu
                .load(std::sync::atomic::Ordering::Acquire)
        })
        .unwrap_or(0);
        if id == 0 {
            return Ok(None);
        }
        // GL work stays on the render thread.
        crate::render::run_now(move || lingxia_view::renderer::surfaces::screenshot_png(id))
            .unwrap_or(Ok(None))
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        let (host, node) = pages::remove(self.token);
        let Some(host) = host else {
            return;
        };
        // A late frame of a leaving page can reconnect its editor; the keyboard must not outlive it.
        ime::reset(self.token);
        host.dispose();
        let surfaces = host.adapter().release_gpu();
        if !surfaces.is_empty() {
            // On the thread that owns GL, ahead of frames still queued for it: once this returns
            // the platform may destroy the window, and a later swap would target a dead one.
            crate::render::run_now(move || {
                for surface in surfaces {
                    lingxia_view::renderer::surfaces::suspend(surface);
                    lingxia_view::renderer::surfaces::detach(surface);
                }
            });
        }
        // Dropping the node destroys its window, so it goes after the swapchain.
        drop(node);
    }
}
