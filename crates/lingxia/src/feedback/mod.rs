//! The window feedback layer: `lx.ui.toast` and its dialogs, one GPU View page per window on the
//! platform's window overlay surface. It opens with the first item and closes after the last.
// Each platform installs it as it gains the overlay surface.
#![allow(dead_code)]
#[cfg(target_os = "android")]
use lingxia_view_android::lingxia_view;
#[cfg(any(target_os = "ios", target_os = "macos"))]
use lingxia_view_apple::lingxia_view;
#[cfg(target_env = "ohos")]
use lingxia_view_harmony::lingxia_view;
#[cfg(windows)]
use lingxia_view_windows::lingxia_view;

mod model;
#[cfg(test)]
mod tests;
mod words;

use lingxia_view::platform::{OverlayInput, OverlaySurface, WindowId, WindowOverlays};
use lingxia_view::session::logic::{BridgeError, ToLogic, ToView};
use lingxia_view::session::state::StateWrite;
use lxapp::feedback::{FeedbackAnswer, FeedbackDialog, FeedbackLayer, FeedbackToast};
use model::{Choice, WindowFeedback};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;
use tokio::sync::oneshot;
use words::Words;

/// The built feedback page (`crates/lingxia/feedback`, `node crates/lingxia/feedback/build.mjs`).
static PAGE: &str = include_str!("page.js");
/// Longer than the page's exit transition, so the last item leaves before the surface does.
const CLOSE_AFTER: Duration = Duration::from_millis(400);

/// The platform hands over its overlays once, at startup; Logic reaches the layer through lxapp.
/// The platform keeps the layer to report closed windows.
pub(crate) fn install(overlays: Arc<dyn WindowOverlays>) -> Arc<Feedback> {
    let feedback = Feedback::new(overlays);
    lxapp::feedback::register_feedback_layer(Box::new(Installed(feedback.clone())));
    feedback
}

struct Installed(Arc<Feedback>);

impl FeedbackLayer for Installed {
    fn toast(&self, appid: &str, toast: FeedbackToast) -> Result<(), String> {
        self.0.toast(appid, toast)
    }
    fn hide_toast(&self, appid: &str) {
        self.0.hide_toast(appid)
    }
    fn dialog(
        &self,
        appid: &str,
        language: &str,
        dialog: FeedbackDialog,
    ) -> Result<oneshot::Receiver<FeedbackAnswer>, String> {
        self.0.dialog(appid, language, dialog)
    }
    fn dismiss(&self, appid: &str) {
        self.0.dismiss(appid)
    }
    fn forget(&self, appid: &str) {
        self.0.forget(appid)
    }
    fn contrast_changed(&self) {
        self.0.contrast_changed()
    }
    #[cfg(feature = "automation")]
    fn automation(
        &self,
        appid: &str,
    ) -> Option<Arc<dyn lingxia_automation::gpu::GpuPageAutomation>> {
        self.0.automation(appid)
    }
}

#[cfg(feature = "automation")]
impl crate::gpu_page::automation::Automated for dyn OverlaySurface {
    fn automation(&self, request: serde_json::Value, reply: crate::gpu_page::AutomationReply) {
        OverlaySurface::automation(self, request, reply)
    }
}

pub(crate) struct Feedback {
    overlays: Arc<dyn WindowOverlays>,
    /// An lxapp's bundled `theme.json`.
    themes: fn(&str) -> Option<String>,
    windows: Mutex<HashMap<WindowId, Layer>>,
    me: Weak<Feedback>,
}

#[derive(Default)]
struct Layer {
    model: WindowFeedback,
    surface: Option<Arc<dyn OverlaySurface>>,
    revision: u64,
    /// Bumped on every change, so a close scheduled before it does nothing.
    generation: u64,
    /// The lxapp whose accent the page has, and the theme push's revision.
    accent: Option<String>,
    theme_revision: u64,
}

/// Surface calls collected under the lock and made after it.
enum Call {
    State(u64, serde_json::Value),
    Input(OverlayInput),
    Focus(bool),
    Reply(String, Result<serde_json::Value, BridgeError>),
}

impl Feedback {
    pub(crate) fn new(overlays: Arc<dyn WindowOverlays>) -> Arc<Self> {
        Self::with_themes(overlays, |appid| lxapp::try_get(appid)?.read_theme_pack())
    }

    fn with_themes(
        overlays: Arc<dyn WindowOverlays>,
        themes: fn(&str) -> Option<String>,
    ) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            overlays,
            themes,
            windows: Mutex::default(),
            me: me.clone(),
        })
    }

    fn window(&self, appid: &str) -> Result<WindowId, String> {
        self.overlays
            .window_of(Some(appid))
            .ok_or_else(|| format!("no window shows {appid}"))
    }

    /// Shows `request` over `appid`'s window, replacing the toast showing there.
    pub(crate) fn toast(&self, appid: &str, request: FeedbackToast) -> Result<(), String> {
        let window = self.window(appid)?;
        let duration = request.duration;
        let key = self.change(window, true, |model| Some(model.toast(appid, request)))?;
        if let (Some(key), Some(duration)) = (key, duration) {
            let me = self.me.clone();
            self.overlays.after(
                duration,
                Box::new(move || {
                    if let Some(me) = me.upgrade() {
                        let _ = me.change(window, false, |model| {
                            model.hide_toast(None, Some(key)).then_some(())
                        });
                    }
                }),
            );
        }
        Ok(())
    }

    pub(crate) fn hide_toast(&self, appid: &str) {
        if let Ok(window) = self.window(appid) {
            let _ = self.change(window, false, |model| {
                model.hide_toast(Some(appid), None).then_some(())
            });
        }
    }

    /// Queues `request` over `appid`'s window, its default words in `language`, the lxapp's page
    /// language. The receiver settles when it is answered, dismissed or its window goes.
    pub(crate) fn dialog(
        &self,
        appid: &str,
        language: &str,
        request: FeedbackDialog,
    ) -> Result<oneshot::Receiver<FeedbackAnswer>, String> {
        let window = self.window(appid)?;
        let words = Words::new(language);
        self.change(window, true, |model| {
            Some(model.dialog(appid, request, words))
        })?
        .ok_or_else(|| "feedback layer closed".to_string())
    }

    /// The layer over `appid`'s window for automation, while one is open.
    #[cfg(feature = "automation")]
    pub(crate) fn automation(
        &self,
        appid: &str,
    ) -> Option<Arc<dyn lingxia_automation::gpu::GpuPageAutomation>> {
        let window = self.overlays.window_of(Some(appid))?;
        let surface = self.windows.lock().unwrap().get(&window)?.surface.clone()?;
        Some(Arc::new(crate::gpu_page::automation::GpuAutomation {
            native: Arc::downgrade(&surface),
        }))
    }

    /// `lx.ui.dismissDialog`: `appid`'s dialogs, shown or queued, settle as dismissed.
    pub(crate) fn dismiss(&self, appid: &str) {
        self.each_window(|model| model.cancel(Some(appid)));
    }

    /// `appid` closed: its toast goes and its dialogs settle as dismissed.
    pub(crate) fn forget(&self, appid: &str) {
        self.each_window(|model| model.forget(appid));
    }

    /// The window closed, and its overlay with it: everything pending settles as dismissed.
    /// Every open layer takes the product's new contrast.
    pub(crate) fn contrast_changed(&self) {
        let surfaces: Vec<_> = (self.windows.lock().unwrap().values())
            .filter_map(|layer| layer.surface.clone())
            .collect();
        for surface in surfaces {
            surface.update_environment(contrast());
        }
    }

    pub(crate) fn window_closed(&self, window: WindowId) {
        let layer = self.windows.lock().unwrap().remove(&window);
        if let Some(mut layer) = layer {
            layer.model.cancel(None);
        }
    }

    fn each_window(&self, mut change: impl FnMut(&mut WindowFeedback) -> bool) {
        let windows: Vec<_> = self.windows.lock().unwrap().keys().copied().collect();
        for window in windows {
            let _ = self.change(window, false, |model| change(model).then_some(()));
        }
    }

    /// Applies `change` to `window`'s model and brings its surface up to date. `None` from
    /// `change` means nothing changed. Without `open`, a window with no layer is left alone.
    fn change<T>(
        &self,
        window: WindowId,
        open: bool,
        change: impl FnOnce(&mut WindowFeedback) -> Option<T>,
    ) -> Result<Option<T>, String> {
        let mut windows = self.windows.lock().unwrap();
        if !open && !windows.contains_key(&window) {
            return Ok(None);
        }
        let layer = windows.entry(window).or_default();
        if layer.surface.is_none() {
            // The platform calls `logic` from the View worker, never inside `open`.
            let me = self.me.clone();
            let logic = Arc::new(move |messages: Vec<ToLogic>| {
                if let Some(me) = me.upgrade() {
                    me.on_page_messages(window, messages);
                }
            });
            match self.overlays.open(window, PAGE, logic) {
                Ok(surface) => {
                    // The window gives the page its appearance; the product's contrast is ours.
                    surface.update_environment(contrast());
                    layer.surface = Some(Arc::from(surface));
                }
                Err(error) => {
                    windows.remove(&window);
                    return Err(error);
                }
            }
        }
        let Some(value) = change(&mut layer.model) else {
            return Ok(None);
        };
        let calls = self.sync(window, layer);
        // The item in front draws in its lxapp's accent; an empty layer keeps it for the exit.
        let accent = match layer.model.front_appid() {
            Some(appid) if layer.accent.as_deref() != Some(appid) => {
                layer.accent = Some(appid.to_string());
                layer.theme_revision += 1;
                Some((appid.to_string(), layer.theme_revision))
            }
            _ => None,
        };
        let surface = layer.surface.clone();
        drop(windows);
        // The theme file is read outside the lock, and pushed before the state that shows it.
        if let (Some((appid, revision)), Some(surface)) = (accent, &surface) {
            let pack = accent_pack((self.themes)(&appid).as_deref());
            surface.update_environment(serde_json::json!({
                "kind": "theme", "revision": revision, "pack": pack,
            }));
        }
        run(surface, calls);
        Ok(Some(value))
    }

    /// The page's data, input and focus after a change; an empty layer closes after its exit.
    fn sync(&self, window: WindowId, layer: &mut Layer) -> Vec<Call> {
        layer.revision += 1;
        layer.generation += 1;
        let modal = layer.model.modal();
        if layer.model.is_empty() {
            let generation = layer.generation;
            let me = self.me.clone();
            self.overlays.after(
                CLOSE_AFTER,
                Box::new(move || {
                    if let Some(me) = me.upgrade() {
                        me.close_if_idle(window, generation);
                    }
                }),
            );
        }
        vec![
            Call::State(layer.revision, layer.model.data()),
            Call::Input(if modal {
                OverlayInput::Block
            } else {
                OverlayInput::PassThrough
            }),
            Call::Focus(modal),
        ]
    }

    fn close_if_idle(&self, window: WindowId, generation: u64) {
        let mut windows = self.windows.lock().unwrap();
        let idle = windows
            .get(&window)
            .is_some_and(|layer| layer.generation == generation && layer.model.is_empty());
        if idle {
            // The surface drops after the lock: dropping it closes the overlay.
            let layer = windows.remove(&window);
            drop(windows);
            drop(layer);
        }
    }

    fn on_page_messages(&self, window: WindowId, messages: Vec<ToLogic>) {
        for message in messages {
            match message {
                // A fresh connection starts without state.
                ToLogic::Connected => {
                    let mut windows = self.windows.lock().unwrap();
                    let Some(layer) = windows.get_mut(&window) else {
                        continue;
                    };
                    layer.revision += 1;
                    let calls = vec![Call::State(layer.revision, layer.model.data())];
                    let surface = layer.surface.clone();
                    drop(windows);
                    run(surface, calls);
                }
                ToLogic::Call {
                    id, method, params, ..
                } => self.call(window, id, &method, params),
                _ => {}
            }
        }
    }

    fn call(&self, window: WindowId, id: String, method: &str, params: Option<serde_json::Value>) {
        #[derive(serde::Deserialize)]
        struct Answered {
            key: u64,
            choice: Choice,
        }
        let answered = (method == "answer")
            .then(|| serde_json::from_value::<Answered>(params?).ok())
            .flatten();
        let outcome = match &answered {
            Some(_) => Ok(serde_json::Value::Null),
            None => Err(BridgeError {
                code: "E_INVALID_ARG".into(),
                message: Some(format!("feedback page: invalid {method}")),
                reason: None,
                data: None,
            }),
        };
        let surface = self
            .windows
            .lock()
            .unwrap()
            .get(&window)
            .and_then(|layer| layer.surface.clone());
        run(surface, vec![Call::Reply(id, outcome)]);
        if let Some(Answered { key, choice }) = answered {
            let _ = self.change(window, false, |model| {
                model.answer(key, choice).then_some(())
            });
        }
    }
}

fn run(surface: Option<Arc<dyn OverlaySurface>>, calls: Vec<Call>) {
    let Some(surface) = surface else {
        return;
    };
    for call in calls {
        match call {
            Call::State(rev, state) => surface.write_state(StateWrite::Snapshot { rev, state }),
            Call::Input(input) => surface.set_input(input),
            Call::Focus(focused) => surface.set_focus(focused),
            Call::Reply(id, outcome) => surface.send(ToView::Reply { id, outcome }),
        }
    }
}

/// The default theme with `app`'s accent family: feedback keeps the host's surfaces and words but
/// its buttons, focus and selection match the lxapp that asked.
fn accent_pack(app: Option<&str>) -> lingxia_theme::Pack {
    let mut pack = lingxia_theme::default_pack();
    let Some(app) = app.and_then(|app| serde_json::from_str::<lingxia_theme::Pack>(app).ok())
    else {
        return pack;
    };
    let family = |role: &str| role.contains("accent") || role == "focus" || role == "selection";
    let take = |colors: &lingxia_theme::Colors| -> lingxia_theme::Colors {
        (colors.iter())
            .filter(|(role, _)| family(role))
            .map(|(role, color)| (role.clone(), *color))
            .collect()
    };
    let (light, dark) = (take(&app.appearance.light), take(&app.appearance.dark));
    // Written accent roles without a seed are used as written; a default seed would fight them.
    if app.accent.is_some() || !light.is_empty() || !dark.is_empty() {
        pack.accent = app.accent;
    }
    pack.appearance.light.extend(light);
    pack.appearance.dark.extend(dark);
    let contrast = |colors: &Option<lingxia_theme::Colors>| {
        colors
            .as_ref()
            .map(take)
            .filter(|colors| !colors.is_empty())
    };
    pack.appearance.high_contrast_light = contrast(&app.appearance.high_contrast_light);
    pack.appearance.high_contrast_dark = contrast(&app.appearance.high_contrast_dark);
    pack
}

/// The product's contrast preference as a View environment push.
fn contrast() -> serde_json::Value {
    let update = lxapp::host_contrast_update();
    serde_json::json!({
        "kind": "contrast",
        "revision": update.revision,
        "more": update.preference == lxapp::ContrastPreference::More,
    })
}
