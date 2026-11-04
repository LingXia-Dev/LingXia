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
mod surface;
mod sys;
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
    fn message(&self, token: i64, connection: u64, message: &str);
    /// Restart the Logic connection after a session failure; false when it cannot.
    fn restart(&self, token: i64, connection: u64) -> bool;
    fn recovery(&self, token: i64, failure: Option<&Failure>);
    fn bootstrap_overdue(&self, token: i64, overdue: bool);
    /// Show the page in place of the outgoing one; see `HostEvents::revealed`.
    fn revealed(&self, token: i64, hold_surface: bool);
    /// Load authorized image bytes; `done` runs once unless the handle is dropped first.
    fn load_image(
        &self,
        token: i64,
        source: String,
        done: Box<dyn FnOnce(Result<Vec<u8>, String>) + Send>,
    ) -> Box<dyn Send>;
    /// Draw press (`ring` false) or focus-ring feedback, or clear it when `shape` is empty:
    /// the node's rounded rect then its clip masks, virtual pixels; `color` is ARGB.
    fn feedback(&self, token: i64, ring: bool, shape: &[f32], color: u32, width: f32);
    fn haptic(&self, token: i64);
    /// A top-layer dialog opened or closed; while open the page covers the host's bars.
    fn top_layer(&self, token: i64, modal: bool);
    fn copy_text(&self, text: &str);
    /// Read the clipboard; `done` runs once, on any thread.
    fn paste(&self, done: Box<dyn FnOnce(Option<String>) + Send>);
    /// Show or move the text menu (frame-contract `TextMenu` JSON), or close it (empty).
    fn text_menu(&self, token: i64, menu: &str);
    fn host_chrome(&self, token: i64, chrome: &HostChrome);
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
        tokens: HashMap<usize, i64>,
    }

    thread_local! {
        static PAGES: RefCell<Registry> = RefCell::new(Registry::default());
    }

    /// Runs `work` on page `token`'s host, never while the registry is borrowed.
    pub(crate) fn with<T>(token: i64, work: impl FnOnce(&Host) -> T) -> Option<T> {
        let host = PAGES.with(|pages| pages.borrow().hosts.get(&token).cloned())?;
        Some(work(&host))
    }

    pub(crate) fn with_node(token: i64, work: impl FnOnce(&xcomponent::Node)) {
        PAGES.with(|pages| {
            if let Some(node) = pages.borrow().nodes.get(&token) {
                work(node);
            }
        });
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

    /// Drops the page's node (and its ArkUI resources), then its host.
    pub(crate) fn remove(token: i64) -> Option<Host> {
        let (host, node) = PAGES.with(|pages| {
            let mut pages = pages.borrow_mut();
            (pages.hosts.remove(&token), pages.nodes.remove(&token))
        });
        drop(node);
        host
    }

    pub(crate) fn bind_node(node: sys::ArkUI_NodeHandle, token: i64) {
        PAGES.with(|pages| pages.borrow_mut().tokens.insert(node as usize, token));
    }

    pub(crate) fn unbind_node(node: sys::ArkUI_NodeHandle) {
        PAGES.with(|pages| pages.borrow_mut().tokens.remove(&(node as usize)));
    }

    pub(crate) fn node_token(node: sys::ArkUI_NodeHandle) -> Option<i64> {
        PAGES.with(|pages| pages.borrow().tokens.get(&(node as usize)).copied())
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
        let worker = ViewWorker::shared(worker)?;
        let host = ViewHost::new(HarmonyAdapter::new(token, shell), worker, logic);
        let _ = host.adapter().host.set(host.downgrade());
        pages::insert(token, host);
        Ok(Self { token })
    }

    pub fn token(&self) -> i64 {
        self.token
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
        let node = xcomponent::Node::mount(self.token, handle)?;
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
                host.request_frame();
            }
        });
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
        lingxia_view::renderer::surfaces::screenshot_png(id)
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        let Some(host) = pages::remove(self.token) else {
            return;
        };
        host.dispose();
        let surface = host
            .adapter()
            .gpu
            .swap(0, std::sync::atomic::Ordering::AcqRel);
        if surface != 0 {
            lingxia_view::renderer::surfaces::detach(surface);
        }
    }
}
