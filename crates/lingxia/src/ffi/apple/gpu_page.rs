//! Swift marshalling for GPU pages. Rust owns each page's `ViewHost`; Swift
//! owns its `GpuPageView` and reports measurements to it by token.
#[cfg(all(any(target_os = "ios", target_os = "macos"), feature = "apple-gpu"))]
use crate::gpu_page::{self, AutomationReply, Creation, NativeFactory, NativePage, Token};
#[cfg(all(any(target_os = "ios", target_os = "macos"), feature = "apple-gpu"))]
use lxapp::{GpuViewGeneration, LxAppError, PageInstance};

#[swift_bridge::bridge]
mod bridge {
    extern "Rust" {
        // The view's size, scale and surroundings after layout; device pixels.
        #[swift_bridge(swift_name = "gpuPageMeasured")]
        fn gpu_page_measured(
            token: i64,
            width: u32,
            height: u32,
            scale: f32,
            dark: bool,
            inset_top: f32,
            inset_right: f32,
            inset_bottom: f32,
            inset_left: f32,
            chrome_top: f32,
            chrome_bottom: f32,
            keyboard: f32,
            smallest_width: f32,
            locale: &str,
            reduce_motion: bool,
        );

        #[swift_bridge(swift_name = "gpuPageSetVisible")]
        fn gpu_page_set_visible(token: i64, visible: bool);
    }

    extern "Swift" {
        // Show premultiplied RGBA8 pixels, valid only for this call.
        #[swift_bridge(swift_name = "GpuPages.presentPixels")]
        fn gpu_pages_present_pixels(
            token: i64,
            pixels: usize,
            length: usize,
            width: u32,
            height: u32,
        ) -> bool;

        #[swift_bridge(swift_name = "GpuPages.dispose")]
        fn gpu_pages_dispose(token: i64);
    }
}

#[allow(clippy::too_many_arguments)]
fn gpu_page_measured(
    token: i64,
    width: u32,
    height: u32,
    scale: f32,
    dark: bool,
    inset_top: f32,
    inset_right: f32,
    inset_bottom: f32,
    inset_left: f32,
    chrome_top: f32,
    chrome_bottom: f32,
    keyboard: f32,
    smallest_width: f32,
    locale: &str,
    reduce_motion: bool,
) {
    #[cfg(all(any(target_os = "ios", target_os = "macos"), feature = "apple-gpu"))]
    {
        let facts = lingxia_ui_apple::lingxia_ui::platform::DeviceFacts {
            width,
            height,
            scale,
            os: if cfg!(target_os = "macos") { "macos" } else { "ios" }.into(),
            dark,
            system_insets: [inset_top, inset_right, inset_bottom, inset_left],
            chrome_insets: [chrome_top, 0.0, chrome_bottom, 0.0],
            keyboard,
            smallest_width,
            locale: locale.into(),
            fine_pointer: cfg!(target_os = "macos"),
            animation_scale: 1.0,
            reduce_motion,
            text_scale: None,
            runner: false,
        };
        with_page(token, |page| page.measured(facts));
    }
    #[cfg(not(all(any(target_os = "ios", target_os = "macos"), feature = "apple-gpu")))]
    let _ = (
        token,
        width,
        height,
        scale,
        dark,
        [inset_top, inset_right, inset_bottom, inset_left],
        [chrome_top, chrome_bottom, keyboard, smallest_width],
        locale,
        reduce_motion,
    );
}

fn gpu_page_set_visible(token: i64, visible: bool) {
    #[cfg(all(any(target_os = "ios", target_os = "macos"), feature = "apple-gpu"))]
    with_page(token, |page| page.host().set_visible(visible));
    #[cfg(not(all(any(target_os = "ios", target_os = "macos"), feature = "apple-gpu")))]
    let _ = (token, visible);
}

#[cfg(all(any(target_os = "ios", target_os = "macos"), feature = "apple-gpu"))]
pub(super) use live::{install, token_of};

#[cfg(all(any(target_os = "ios", target_os = "macos"), feature = "apple-gpu"))]
mod live {
    use super::*;
    use lingxia_ui_apple::lingxia_ui::session::lifecycle::ConnectionTicket;
    use lingxia_ui_apple::{Page, Shell};
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::sync::Arc;

    thread_local! {
        /// Main thread only: every page call is posted there.
        static PAGES: RefCell<HashMap<Token, Page>> = RefCell::new(HashMap::new());
    }

    pub(super) fn with_page(token: Token, work: impl FnOnce(&Page)) {
        PAGES.with(|pages| {
            if let Some(page) = pages.borrow().get(&token) {
                work(page);
            }
        });
    }

    fn on_page(token: Token, work: impl FnOnce(&Page) + Send + 'static) {
        lingxia_ui_apple::post_main(Box::new(move || with_page(token, work)));
    }

    /// The live GPU page token of `page`, or 0.
    pub(crate) fn token_of(page: &PageInstance) -> i64 {
        gpu_page::native_view(page)
            .and_then(|native| native.as_any().downcast_ref::<ApplePage>().map(|p| p.token))
            .unwrap_or(0)
    }

    struct Bridge;

    impl Shell for Bridge {
        fn present_pixels(&self, token: i64, pixels: &[u8], width: u32, height: u32) -> bool {
            super::bridge::gpu_pages_present_pixels(
                token,
                pixels.as_ptr() as usize,
                pixels.len(),
                width,
                height,
            )
        }
        fn loaded(&self, _: i64) {}
        fn presented(&self, token: i64, connection: u64) {
            gpu_page::presented(token, connection);
        }
        fn message(&self, token: i64, connection: u64, message: &str) {
            gpu_page::message(token, connection, message);
        }
        fn restart(&self, token: i64, connection: u64) -> bool {
            gpu_page::restart(token, connection)
        }
        fn recovery(
            &self,
            token: i64,
            failure: Option<&lingxia_ui_apple::lingxia_ui::session::host::Failure>,
        ) {
            if let Some(failure) = failure {
                log::warn!("GPU page {token} failed: {}", failure.error);
            }
        }
        fn bootstrap_overdue(&self, token: i64, overdue: bool) {
            if overdue {
                log::warn!("GPU page {token}: Logic bootstrap overdue");
            }
        }
        fn load_image(
            &self,
            token: i64,
            source: String,
            done: Box<dyn FnOnce(Result<Vec<u8>, String>) + Send>,
        ) -> Box<dyn Send> {
            Box::new(gpu_page::load_image(token, source, done))
        }
    }

    struct ApplePage {
        token: Token,
    }

    impl NativePage for ApplePage {
        fn connect(&self, generation: GpuViewGeneration, nonce: &str) -> Result<(), LxAppError> {
            let nonce = nonce.to_owned();
            on_page(self.token, move |page| {
                page.host()
                    .connect(ConnectionTicket::new(generation.get()), nonce)
            });
            Ok(())
        }
        fn receive(&self, generation: GpuViewGeneration, frame: &str) -> Result<(), LxAppError> {
            let frame = frame.to_owned();
            on_page(self.token, move |page| {
                page.host()
                    .receive(ConnectionTicket::new(generation.get()), frame)
            });
            Ok(())
        }
        fn request_leave(
            &self,
            generation: GpuViewGeneration,
            reason: &str,
        ) -> Result<(), LxAppError> {
            let reason = reason.to_owned();
            on_page(self.token, move |page| {
                page.host()
                    .request_leave(ConnectionTicket::new(generation.get()), reason)
            });
            Ok(())
        }
        fn update_environment(&self, update: &serde_json::Value) -> Result<(), LxAppError> {
            let update = update.to_string();
            on_page(self.token, move |page| {
                if let Err(error) = page.host().update_environment(&update) {
                    log::warn!("GPU environment update rejected: {error}");
                }
            });
            Ok(())
        }
        fn reload(&self, source: &str) -> Result<(), LxAppError> {
            let source = source.to_owned();
            on_page(self.token, move |page| page.host().load(source));
            Ok(())
        }
        fn park(&self) -> Result<(), LxAppError> {
            on_page(self.token, |page| page.host().park());
            Ok(())
        }
        fn retain_surface(&self, _: bool) -> Result<(), LxAppError> {
            Ok(())
        }
        fn automation(&self, _: serde_json::Value, reply: AutomationReply) {
            reply(Err("GPU automation is not available on iOS yet".into()));
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    struct Factory;

    impl NativeFactory for Factory {
        fn create(
            &self,
            token: Token,
            _: &PageInstance,
            request: Creation,
        ) -> Result<(), LxAppError> {
            lingxia_ui_apple::post_main(Box::new(move || {
                let created = Page::new(token, Arc::new(Bridge), &request.worker, request.logic)
                    .map(|page| {
                        page.host().load(request.source);
                        PAGES.with(|pages| pages.borrow_mut().insert(token, page));
                        Arc::new(ApplePage { token }) as Arc<dyn NativePage>
                    });
                gpu_page::created(token, created);
            }));
            Ok(())
        }

        fn dispose(&self, token: Token) {
            lingxia_ui_apple::post_main(Box::new(move || {
                let page = PAGES.with(|pages| pages.borrow_mut().remove(&token));
                drop(page);
                super::bridge::gpu_pages_dispose(token);
            }));
        }
    }

    /// Register the GPU page factory; called once at startup.
    pub(crate) fn install() {
        if lxapp::register_gpu_page_view_factory(Arc::new(gpu_page::Binding(Arc::new(Factory))))
            .is_err()
        {
            log::warn!("GPU page factory already registered");
        }
    }
}

#[cfg(all(any(target_os = "ios", target_os = "macos"), feature = "apple-gpu"))]
use live::with_page;
