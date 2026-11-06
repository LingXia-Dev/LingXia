//! NAPI marshalling for GPU pages. Rust owns each page's `ViewHost` and XComponent
//! node; ArkTS owns the `GpuPage` component (a `ContentSlot` plus chrome) by token.
use crate::gpu_page::{self, AutomationReply, Creation, NativeFactory, NativePage, Token};
use lingxia_view_harmony::lingxia_view::platform::{DeviceFacts, HostChrome};
use lingxia_view_harmony::lingxia_view::session::host::Failure;
use lingxia_view_harmony::lingxia_view::session::lifecycle::ConnectionTicket;
use lingxia_view_harmony::{Page, Shell};
use lxapp::{GpuViewGeneration, LxAppError, PageInstance};
use napi_derive_ohos::napi;
use napi_ohos::bindgen_prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

thread_local! {
    /// Main thread only: every page call is posted there.
    static PAGES: RefCell<HashMap<Token, Page>> = RefCell::new(HashMap::new());
}

fn with_page<T>(token: Token, work: impl FnOnce(&Page) -> T) -> Option<T> {
    PAGES.with(|pages| pages.borrow().get(&token).map(work))
}

fn on_page(token: Token, work: impl FnOnce(&Page) + Send + 'static) {
    lingxia_view_harmony::post_main(Box::new(move || {
        with_page(token, work);
    }));
}

/// `GpuPages.event(token, kind, payload)` on the ArkTS side.
fn arkts(token: Token, kind: &str, payload: &str) {
    let token = token.to_string();
    if let Err(error) = lingxia_webview::platform::harmony::tsfn::call_arkts(
        "gpuPageEvent",
        &[&token, kind, payload],
    ) {
        log::warn!(target: "lingxia_view", "GPU page {token} {kind}: {error}");
    }
}

/// Automation touches for GPU pages, by webtag; WebView pages have no driver here.
struct Gestures;

impl lingxia_platform::harmony::GestureDriver for Gestures {
    fn perform(
        &self,
        app_id: &str,
        webtag: &str,
        points: &[lingxia_platform::traits::gesture::GesturePoint],
        end: lingxia_platform::traits::gesture::GestureEnd,
        finger: Option<u64>,
    ) -> std::result::Result<(u32, Option<u64>), String> {
        use lingxia_platform::traits::gesture::GestureEnd;
        use lingxia_view_harmony::TouchEnd;
        let token = match finger {
            Some(finger) => finger as i64,
            None => gpu_token(app_id, webtag)?,
        };
        let points: Vec<[f64; 3]> = points.iter().map(|p| [p.x, p.y, p.at_ms]).collect();
        let end = match end {
            GestureEnd::Up => TouchEnd::Up,
            GestureEnd::Cancel => TouchEnd::Cancel,
            GestureEnd::Hold => TouchEnd::Hold,
        };
        let events = lingxia_view_harmony::inject_touch(token, &points, finger.is_some(), end)?;
        Ok((events, (end == TouchEnd::Hold).then_some(token as u64)))
    }

    fn viewport(&self, app_id: &str, webtag: &str) -> std::result::Result<[f64; 4], String> {
        lingxia_view_harmony::page_viewport(gpu_token(app_id, webtag)?)
            .ok_or_else(|| "page is not active".into())
    }
}

fn gpu_token(app_id: &str, webtag: &str) -> std::result::Result<i64, String> {
    let page = lxapp::find_page_by_webtag(app_id, webtag)
        .ok_or_else(|| format!("no page {webtag} in {app_id}"))?;
    match token_of(&page) {
        0 => Err("gestures reach GPU pages only on HarmonyOS".into()),
        token => Ok(token),
    }
}

/// Register the factory and the main-thread dispatcher; `lingxia_init` calls it once.
pub(super) fn install(env: &Env, data_dir: &str) {
    // SAFETY: `lingxia_init` runs on the main thread with its live env.
    if let Err(error) = unsafe { lingxia_view_harmony::install_main_thread(env.raw()) } {
        log::error!(target: "lingxia_view", "GPU pages unavailable: {error}");
        return;
    }
    lingxia_view_harmony::lingxia_view::renderer::sentinel::install(Box::new(
        lingxia_view_harmony::lingxia_view::renderer::sentinel::FileStore(
            std::path::Path::new(data_dir).join("lingxia-gpu-sentinel"),
        ),
    ));
    lingxia_platform::harmony::set_gesture_driver(Arc::new(Gestures));
    if lxapp::register_gpu_page_view_factory(Arc::new(gpu_page::Binding(Arc::new(Factory))))
        .is_err()
    {
        log::warn!("GPU page factory already registered");
    }
}

struct Bridge;

impl Shell for Bridge {
    fn loaded(&self, token: i64) {
        arkts(token, "loaded", "");
    }
    fn presented(&self, token: i64, connection: u64) {
        gpu_page::presented(token, connection);
    }
    fn message(&self, token: i64, connection: u64, message: &str) {
        gpu_page::message(token, connection, message);
    }
    fn restart(&self, token: i64, connection: u64) -> bool {
        gpu_page::restart(token, connection)
    }
    fn recovery(&self, token: i64, failure: Option<&Failure>) {
        use lingxia_view_harmony::lingxia_view::session::host::{RecoveryAction, RecoveryMessage};
        // Message and action as indices into the shell's strings, as Android's panel reads them.
        let payload = failure
            .map(|f| {
                let message = match f.message {
                    RecoveryMessage::UpdateRequired => 0,
                    RecoveryMessage::SessionFailed => 1,
                    RecoveryMessage::UpdateFailed => 2,
                    RecoveryMessage::DisplayFailed => 3,
                };
                let action = match f.action {
                    Some(RecoveryAction::Retry) => 0,
                    Some(RecoveryAction::Reload) => 1,
                    None => -1,
                };
                serde_json::json!({"error": f.error, "retained": f.retained, "message": message, "action": action})
                    .to_string()
            })
            .unwrap_or_default();
        arkts(token, "recovery", &payload);
    }
    fn bootstrap_overdue(&self, token: i64, overdue: bool) {
        arkts(
            token,
            "bootstrapOverdue",
            if overdue { "true" } else { "false" },
        );
    }
    fn revealed(&self, token: i64, hold_surface: bool) {
        arkts(
            token,
            "revealed",
            if hold_surface { "true" } else { "false" },
        );
    }
    fn load_image(
        &self,
        token: i64,
        source: String,
        done: Box<dyn FnOnce(std::result::Result<Vec<u8>, String>) + Send>,
    ) -> Box<dyn Send> {
        Box::new(gpu_page::load_image(token, source, done))
    }
    fn feedback(&self, token: i64, ring: bool, shape: &[f32], color: u32, width: f32) {
        let payload =
            serde_json::json!({"ring": ring, "shape": shape, "color": color, "width": width});
        arkts(token, "feedback", &payload.to_string());
    }
    fn haptic(&self, _: i64) {
        let _ = crate::device::vibrate(false);
    }
    fn top_layer(&self, token: i64, modal: bool) {
        arkts(token, "topLayer", if modal { "true" } else { "false" });
    }
    fn copy_text(&self, text: &str) {
        arkts(0, "copy", text);
    }
    fn paste(&self, done: Box<dyn FnOnce(Option<String>) + Send>) {
        log::warn!(target: "lingxia_view", "UI_ACCESSIBILITY: paste needs the clipboard bridge");
        done(None);
    }
    fn text_menu(&self, token: i64, menu: &str) {
        arkts(token, "textMenu", menu);
    }
    fn host_chrome(&self, token: i64, chrome: &HostChrome) {
        let argb = |[r, g, b, a]: [u8; 4]| u32::from_be_bytes([a, r, g, b]);
        let payload = serde_json::json!({
            "canvas": argb(chrome.canvas), "bar": argb(chrome.bar), "barLine": argb(chrome.bar_line),
            "content": argb(chrome.content), "contentMuted": argb(chrome.content_muted),
            "accent": argb(chrome.accent), "dark": chrome.dark,
        });
        arkts(token, "hostChrome", &payload.to_string());
    }
}

struct HarmonyPage {
    token: Token,
}

impl NativePage for HarmonyPage {
    fn connect(
        &self,
        generation: GpuViewGeneration,
        nonce: &str,
    ) -> std::result::Result<(), LxAppError> {
        let nonce = nonce.to_owned();
        on_page(self.token, move |page| {
            page.with_host(|host| host.connect(ConnectionTicket::new(generation.get()), nonce));
        });
        Ok(())
    }
    fn receive(
        &self,
        generation: GpuViewGeneration,
        frame: &str,
    ) -> std::result::Result<(), LxAppError> {
        let frame = frame.to_owned();
        on_page(self.token, move |page| {
            page.with_host(|host| host.receive(ConnectionTicket::new(generation.get()), frame));
        });
        Ok(())
    }
    fn request_leave(
        &self,
        generation: GpuViewGeneration,
        reason: &str,
    ) -> std::result::Result<(), LxAppError> {
        let reason = reason.to_owned();
        on_page(self.token, move |page| {
            page.with_host(|host| {
                host.request_leave(ConnectionTicket::new(generation.get()), reason)
            });
        });
        Ok(())
    }
    fn update_environment(
        &self,
        update: &serde_json::Value,
    ) -> std::result::Result<(), LxAppError> {
        let update = update.to_string();
        on_page(self.token, move |page| {
            if let Some(Err(error)) = page.with_host(|host| host.update_environment(&update)) {
                log::warn!("GPU environment update rejected: {error}");
            }
        });
        Ok(())
    }
    fn reload(&self, source: &str) -> std::result::Result<(), LxAppError> {
        let source = source.to_owned();
        on_page(self.token, move |page| {
            page.with_host(|host| host.load(source));
        });
        Ok(())
    }
    #[cfg(feature = "devtool")]
    fn swap_module(&self, source: &str) -> std::result::Result<(), LxAppError> {
        let source = source.to_owned();
        on_page(self.token, move |page| {
            page.with_host(|host| host.swap_module(source));
        });
        Ok(())
    }
    fn park(&self) -> std::result::Result<(), LxAppError> {
        on_page(self.token, |page| {
            page.with_host(|host| host.park());
        });
        Ok(())
    }
    fn retain_surface(&self, retained: bool) -> std::result::Result<(), LxAppError> {
        arkts(
            self.token,
            "retainSurface",
            if retained { "true" } else { "false" },
        );
        Ok(())
    }
    fn automation(&self, request: serde_json::Value, reply: AutomationReply) {
        let token = self.token;
        lingxia_view_harmony::post_main(Box::new(move || {
            let answer = with_page(token, |page| {
                if request["op"] != "screenshot" {
                    return page
                        .with_host(|host| host.automation_request(&request))
                        .unwrap_or_else(|| Err("page is not active".into()));
                }
                use base64::Engine;
                match page.screenshot_png() {
                    Ok(Some(png)) => Ok(serde_json::json!({
                        "png": base64::engine::general_purpose::STANDARD.encode(png)
                    })),
                    Ok(None) => Err("E_PAGE_NOT_READY: no presented frame".into()),
                    Err(error) => Err(error),
                }
            });
            reply(answer.unwrap_or_else(|| Err("page is not active".into())));
        }));
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
        page: &PageInstance,
        request: Creation,
    ) -> std::result::Result<(), LxAppError> {
        let describe = serde_json::json!({
            "appId": page.appid(), "path": page.path(), "webTag": page.webtag().to_string(),
        })
        .to_string();
        lingxia_view_harmony::post_main(Box::new(move || {
            // ArkTS must know the token before the host's first events, sent during creation;
            // it builds the page's ContentSlot, then mounts it with `gpuPageMount`.
            arkts(token, "create", &describe);
            let created =
                Page::new(token, Arc::new(Bridge), &request.worker, request.logic).map(|page| {
                    page.with_host(|host| host.load(request.source));
                    PAGES.with(|pages| pages.borrow_mut().insert(token, page));
                    Arc::new(HarmonyPage { token }) as Arc<dyn NativePage>
                });
            if created.is_err() {
                arkts(token, "dispose", "");
            }
            gpu_page::created(token, created);
        }));
        Ok(())
    }

    fn dispose(&self, token: Token) {
        lingxia_view_harmony::post_main(Box::new(move || {
            let page = PAGES.with(|pages| pages.borrow_mut().remove(&token));
            drop(page);
            arkts(token, "dispose", "");
        }));
    }
}

/// The live GPU page token of `page`, or 0: the container hosts a `GpuPage` for it.
pub(super) fn token_of(page: &PageInstance) -> i64 {
    gpu_page::native_view(page)
        .and_then(|native| {
            native
                .as_any()
                .downcast_ref::<HarmonyPage>()
                .map(|p| p.token)
        })
        .unwrap_or(0)
}

/// Mount the page's XComponent node into the ArkTS `NodeContent` its `GpuPage` built.
#[napi]
pub fn gpu_page_mount(env: Env, token: i64, content: Unknown<'_>) -> bool {
    with_page(token, |page| {
        // SAFETY: NAPI calls arrive on the main thread with their live env and value.
        unsafe { page.mount(env.raw(), content.raw()) }
            .inspect_err(|error| log::warn!(target: "lingxia_view", "GPU page {token}: {error}"))
            .is_ok()
    })
    .unwrap_or(false)
}

/// The page's surroundings after layout or a change: the same facts as Android's
/// `GpuPlatform.environment` JSON (`width`/`height` are the surface's, in device pixels),
/// plus the page's top-left on screen.
#[napi]
pub fn gpu_page_measured(token: i64, facts: String, origin_x: f64, origin_y: f64) {
    let Ok(facts) = serde_json::from_str::<serde_json::Value>(&facts) else {
        return log::warn!(target: "lingxia_view", "GPU page {token}: malformed facts");
    };
    let number = |key: &str| facts[key].as_f64().unwrap_or(0.0) as f32;
    let edges = |key: &str| {
        let mut edges = [0.0; 4];
        for (edge, value) in edges
            .iter_mut()
            .zip(facts[key].as_array().into_iter().flatten())
        {
            *edge = value.as_f64().unwrap_or(0.0) as f32;
        }
        edges
    };
    let facts = DeviceFacts {
        width: facts["width"].as_u64().unwrap_or(0) as u32,
        height: facts["height"].as_u64().unwrap_or(0) as u32,
        scale: number("scale"),
        os: "harmony".into(),
        dark: facts["dark"].as_bool().unwrap_or(false),
        system_insets: edges("system_insets"),
        chrome_insets: edges("chrome_insets"),
        keyboard: number("keyboard"),
        smallest_width: number("smallest_width"),
        locale: facts["locale"].as_str().unwrap_or_default().to_owned(),
        fine_pointer: facts["fine_pointer"].as_bool().unwrap_or(false),
        animation_scale: facts["animation_scale"].as_f64().map_or(1.0, |v| v as f32),
        reduce_motion: facts["reduce_motion"].as_bool().unwrap_or(false),
        high_contrast: facts["high_contrast"].as_bool().unwrap_or(false),
        text_scale: facts["text_scale"].as_array().map(|rows| {
            rows.iter()
                .filter_map(|row| Some([row[0].as_f64()? as f32, row[1].as_f64()? as f32]))
                .collect()
        }),
        hour_cycle: facts["hour_cycle"].as_str().map(str::to_owned),
        first_weekday: facts["first_weekday"]
            .as_u64()
            .and_then(|d| u8::try_from(d).ok()),
    };
    with_page(token, |page| {
        page.measured(facts, [origin_x as f32, origin_y as f32])
    });
}

#[napi]
pub fn gpu_page_set_visible(token: i64, visible: bool) {
    with_page(token, |page| {
        page.with_host(|host| host.set_visible(visible))
    });
}

#[napi]
pub fn gpu_page_window_visible(token: i64, visible: bool) {
    with_page(token, |page| {
        page.with_host(|host| host.window_visible(visible))
    });
}

#[napi]
pub fn gpu_page_window_focus(token: i64, focused: bool) {
    with_page(token, |page| {
        page.with_host(|host| host.window_focus(focused))
    });
}

#[napi]
pub fn gpu_page_reveal(token: i64, surface_holds_frame: bool) {
    with_page(token, |page| {
        page.with_host(|host| host.reveal(surface_holds_frame))
    });
}

/// The recovery panel's button: the action the last failure offered.
#[napi]
pub fn gpu_page_retry(token: i64) {
    with_page(token, |page| page.with_host(|host| host.retry()));
}

/// Back: true when the page closed its own top layer.
#[napi]
pub fn gpu_page_close_request(token: i64) -> bool {
    with_page(token, |page| page.with_host(|host| host.close_request()))
        .flatten()
        .unwrap_or(false)
}

/// Pull to refresh asks whether the page scrolls at a virtual-pixel point.
#[napi]
pub fn gpu_page_can_scroll_vertically(token: i64, x: f64, y: f64, direction: i32) -> bool {
    with_page(token, |page| {
        page.with_host(|host| host.can_scroll_vertically_at([x as f32, y as f32], direction))
    })
    .flatten()
    .unwrap_or(false)
}

/// The text menu's actions, in `GpuTextMenu.ACTIONS` order.
#[napi]
pub fn gpu_page_text_menu_action(token: i64, action: i32) -> bool {
    use lingxia_view_harmony::lingxia_view::session::host::MenuAction;
    let action = match action {
        0 => MenuAction::Cut,
        1 => MenuAction::Copy,
        2 => MenuAction::Paste,
        3 => MenuAction::SelectAll,
        4 => MenuAction::Undo,
        5 => MenuAction::Redo,
        _ => return false,
    };
    with_page(token, |page| {
        page.with_host(|host| host.text_menu_action(action))
    })
    .flatten()
    .unwrap_or(false)
}

#[napi]
pub fn gpu_page_text_menu_dismissed(token: i64) {
    with_page(token, |page| {
        page.with_host(|host| host.text_menu_dismissed())
    });
}

#[napi]
pub fn gpu_page_input_blocked(token: i64) -> bool {
    with_page(token, |page| page.with_host(|host| host.input_blocked()))
        .flatten()
        .unwrap_or(false)
}

/// The GPU token of a page instance, or 0 for a WebView page.
#[napi]
pub fn gpu_page_token(page_instance_id: String) -> i64 {
    lxapp::find_page_by_instance_id(&page_instance_id).map_or(0, |page| token_of(&page))
}
