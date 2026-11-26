#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use crate::i18n::js_error;
use crate::i18n::{js_internal_error, js_invalid_parameter_error, js_state_error};
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use lingxia_platform::traits::ui::{ToastIcon, ToastOptions, ToastPosition, UserFeedback};
use lxapp::LxApp;
use lxapp::dialogs::ToastShown;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use rong::JSContextService;
use rong::{FromJSObject, JSContext, JSFunc, JSObject, JSResult, function::Optional};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

// The generation guard has to be scoped exactly like the presenter it guards.
// Desktop presents inside the calling lxapp's own WebView, so one counter per
// JS context *is* one lxapp's toast. The mobile hosts share a single
// process-wide overlay, so a toast from any lxapp replaces what is on screen
// and must invalidate every older handle — a per-context counter there would
// let a retained handle dismiss another lxapp's newer toast.
#[cfg(any(target_os = "macos", target_os = "windows"))]
#[derive(Default)]
struct ToastState(Arc<AtomicU64>);
#[cfg(any(target_os = "macos", target_os = "windows"))]
impl JSContextService for ToastState {}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn toast_generation(ctx: &JSContext) -> Arc<AtomicU64> {
    if ctx.get_service::<ToastState>().is_none() {
        ctx.set_service(ToastState::default());
    }
    ctx.get_service::<ToastState>()
        .expect("toast state inserted")
        .0
        .clone()
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn toast_generation(_ctx: &JSContext) -> Arc<AtomicU64> {
    static GENERATION: std::sync::LazyLock<Arc<AtomicU64>> =
        std::sync::LazyLock::new(|| Arc::new(AtomicU64::new(0)));
    GENERATION.clone()
}

/// `lx.ui.toast` options; the message is the first argument.
#[derive(FromJSObject, Default)]
#[ts_skip]
struct JSToastOptions {
    tone: Option<String>,
    loading: Option<bool>,
    #[js_name = "durationMs"]
    duration: Option<f64>,
    position: Option<String>,
    signal: Option<JSObject>,
}

/// A toast as the hosts take it: their `icon` vocabulary carries tone and loading.
struct ToastRequest {
    title: String,
    tone: &'static str,
    loading: bool,
    icon: &'static str,
    duration_ms: f64,
    position: &'static str,
}

impl ToastRequest {
    fn parse(message: String, options: JSToastOptions) -> JSResult<Self> {
        let (tone, tone_icon) = match options.tone.as_deref() {
            None | Some("default") => ("default", "none"),
            Some("success") => ("success", "success"),
            Some("danger") => ("danger", "error"),
            Some(other) => {
                return Err(js_invalid_parameter_error(format!(
                    "toast tone must be 'default', 'success' or 'danger', got '{other}'"
                )));
            }
        };
        let position = match options.position.as_deref() {
            None | Some("center") => "center",
            Some("top") => "top",
            Some("bottom") => "bottom",
            Some(other) => {
                return Err(js_invalid_parameter_error(format!(
                    "toast position must be 'top', 'center' or 'bottom', got '{other}'"
                )));
            }
        };
        let loading = options.loading.unwrap_or(false);
        Ok(Self {
            title: message,
            tone,
            loading,
            icon: if loading { "loading" } else { tone_icon },
            duration_ms: options.duration.unwrap_or(1500.0),
            position,
        })
    }
}

impl ToastRequest {
    fn into_feedback(self) -> lxapp::feedback::FeedbackToast {
        use lxapp::feedback::{FeedbackPosition, FeedbackTone};
        lxapp::feedback::FeedbackToast {
            title: self.title,
            tone: match self.tone {
                "success" => FeedbackTone::Success,
                "danger" => FeedbackTone::Danger,
                _ => FeedbackTone::Neutral,
            },
            loading: self.loading,
            position: match self.position {
                "top" => FeedbackPosition::Top,
                "bottom" => FeedbackPosition::Bottom,
                _ => FeedbackPosition::Center,
            },
            duration: (self.duration_ms.is_finite() && self.duration_ms > 0.0).then(|| {
                std::time::Duration::from_secs_f64(self.duration_ms.min(600_000.0) / 1000.0)
            }),
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
impl From<ToastRequest> for ToastOptions {
    fn from(request: ToastRequest) -> Self {
        ToastOptions {
            title: request.title,
            icon: match request.icon {
                "success" => ToastIcon::Success,
                "error" => ToastIcon::Error,
                "loading" => ToastIcon::Loading,
                _ => ToastIcon::None,
            },
            image: None,
            // Natives take seconds.
            duration: request.duration_ms / 1000.0,
            mask: false,
            position: match request.position {
                "top" => ToastPosition::Top,
                "bottom" => ToastPosition::Bottom,
                _ => ToastPosition::Center,
            },
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ToastPhase {
    Pending,
    Shown,
    Dismissed,
}

/// Show a short message over the page. Returns at once; a newer toast replaces it.
///
/// @example
/// ```ts
/// lx.ui.toast('Saved', { tone: 'success' });
/// const busy = lx.ui.toast('Uploading…', { loading: true, durationMs: 60_000 });
/// busy.dismiss();
/// ```
/// @throws {E_INVALID_STATE} The app is not open; `reason` is `'appClosed'`.
fn show_toast(
    ctx: JSContext,
    message: String,
    options: Optional<JSToastOptions>,
) -> JSResult<JSObject> {
    let mut options = options.0.unwrap_or_default();
    let signal = options.signal.take();
    crate::task_object::validate_abort_signal(signal.as_ref())?;
    let request = ToastRequest::parse(message, options)?;
    let lxapp = LxApp::from_ctx(&ctx)?;
    if !lxapp.is_opened() {
        return Err(js_state_error(
            "appClosed",
            "LxApp is closed; toast suppressed",
        ));
    }
    let state = toast_generation(&ctx);
    // Invalidate older handles before dispatch so they cannot hide a pending replacement.
    let previous = state.fetch_add(1, Ordering::SeqCst);
    let generation = previous + 1;
    let phase = Rc::new(Cell::new(ToastPhase::Pending));

    let present_ctx = ctx.clone();
    let present_state = state.clone();
    let present_phase = phase.clone();
    ctx.spawn_task(async move {
        // Dismissed, or replaced by a newer toast, before it could show.
        if present_state.load(Ordering::SeqCst) != generation {
            return;
        }
        let shown = ToastShown {
            message: request.title.clone(),
            tone: request.tone.to_string(),
            loading: request.loading,
            duration_ms: request.duration_ms,
        };
        if let Err(error) = present_toast(&present_ctx, request).await {
            let _ = present_state.compare_exchange(
                generation,
                previous,
                Ordering::SeqCst,
                Ordering::SeqCst,
            );
            log::warn!("toast presentation failed: {error:?}");
            return;
        }
        // Observed, never intercepted: the toast was presented as usual.
        if let Some(hook) = lxapp::dialogs::dialog_hook()
            && let Ok(lxapp) = LxApp::from_ctx(&present_ctx)
        {
            hook.toast(&lxapp.appid, &shown);
        }
        if present_phase.get() == ToastPhase::Dismissed {
            // Dismissed while presenting; a newer toast since then owns the overlay.
            if present_state.load(Ordering::SeqCst) == generation + 1 {
                let _ = hide_toast(&present_ctx).await;
            }
        } else {
            present_phase.set(ToastPhase::Shown);
        }
    });

    let handle = JSObject::new(&ctx);
    let dismiss = JSFunc::new(&ctx, move |ctx: JSContext| {
        let dismissed = state
            .compare_exchange(
                generation,
                generation + 1,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_ok();
        if !dismissed {
            return Ok::<(), rong::RongJSError>(());
        }
        let was = phase.replace(ToastPhase::Dismissed);
        if was == ToastPhase::Shown {
            let hide_ctx = ctx.clone();
            ctx.spawn_task(async move {
                let _ = hide_toast(&hide_ctx).await;
            });
        }
        Ok(())
    })?;
    handle.set("dismiss", dismiss.clone())?;
    crate::task_object::on_abort(&ctx, signal, dismiss)?;
    Ok(handle)
}

/// Resolves after the host accepts presentation, not after the toast expires.
async fn present_toast(ctx: &JSContext, request: ToastRequest) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(ctx)?;

    // Do not show UI if app is not opened
    if !lxapp.is_opened() {
        return Err(js_state_error(
            "appClosed",
            "LxApp is closed; toast suppressed",
        ));
    }
    if lxapp::feedback::LAYER {
        return super::feedback_layer()?
            .toast(&lxapp.appid, request.into_feedback())
            .map_err(|e| js_internal_error(format!("toast failed: {e}")));
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let params = serde_json::json!({
            "title": request.title,
            "icon": request.icon,
            "image": null,
            "duration": request.duration_ms,
            "mask": false,
            "position": request.position,
        });

        // The toast belongs to the app, not the page drawing it: a page that leaves (a toast
        // then `lx.nav.back()`) hands it to the page that takes its place.
        on_current_page(
            || {
                lxapp
                    .current_page()
                    .ok()
                    .map(|page| page.instance_id_string())
            },
            |id| {
                let lxapp = lxapp.clone();
                let params = params.clone();
                async move {
                    let page = lxapp.current_page()?;
                    if page.instance_id_string() != id {
                        return Err(lingxia_error::Error::internal("the page changed"));
                    }
                    page.call_view_with::<_, ()>("ui.showToast", &params).await
                }
            },
            || tokio::time::sleep(std::time::Duration::from_millis(20)),
        )
        .await
        .map_err(|e| js_internal_error(format!("Page toast failed: {}", e)))
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let toast_options: ToastOptions = request.into();
        lxapp.runtime.show_toast(toast_options).map_err(js_error)?;

        Ok(())
    }
}

/// Runs `call` on the current page; when it fails and the page is then replaced within about a
/// second, runs it once more on each page that takes its place (at most three). A failure on a
/// page that stays current is the answer.
#[cfg_attr(
    not(any(target_os = "macos", target_os = "windows", test)),
    allow(dead_code)
)]
async fn on_current_page<E, C, F, S, W>(
    current: C,
    call: impl Fn(String) -> F,
    wait: S,
) -> Result<(), E>
where
    E: From<lingxia_error::Error>,
    C: Fn() -> Option<String>,
    F: std::future::Future<Output = Result<(), E>>,
    S: Fn() -> W,
    W: std::future::Future<Output = ()>,
{
    const WAIT_STEPS: usize = 50;
    let no_page = || E::from(lingxia_error::Error::internal("No current page"));
    let mut page = current().ok_or_else(no_page)?;
    let mut hops = 0;
    loop {
        let error = match call(page.clone()).await {
            Ok(()) => return Ok(()),
            Err(error) => error,
        };
        if hops == 3 {
            return Err(error);
        }
        let mut next = None;
        for _ in 0..WAIT_STEPS {
            match current() {
                Some(id) if id != page => {
                    next = Some(id);
                    break;
                }
                _ => wait().await,
            }
        }
        let Some(next) = next else { return Err(error) };
        page = next;
        hops += 1;
    }
}

/// Hides whichever toast is showing.
async fn hide_toast(ctx: &JSContext) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(ctx)?;
    if !lxapp.is_opened() {
        return Err(js_state_error(
            "appClosed",
            "LxApp is closed; toast dismissal suppressed",
        ));
    }
    if lxapp::feedback::LAYER {
        super::feedback_layer()?.hide_toast(&lxapp.appid);
        return Ok(());
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let _: () = lxapp
            .call_view("ui.hideToast")
            .await
            .map_err(|e| js_internal_error(format!("WebView hideToast failed: {}", e)))?;

        Ok(())
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        lxapp.runtime.hide_toast().map_err(js_error)?;

        Ok(())
    }
}

/// Initialize toast functions
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace UiApi = super::ui_namespace(ctx)?;
        fn toast(
            ts_params = "message: string, options?: ToastOptions",
            ts_return = "ToastHandle"
        ) = show_toast;
    }
}

#[cfg(test)]
mod tests {
    use super::on_current_page;
    use std::cell::RefCell;

    fn revoked() -> lingxia_error::Error {
        lingxia_error::Error::internal("Document session revoked")
    }

    /// Runs the helper over a page stack that moves to the next page after `moves_after` reads.
    fn run(
        pages: &[&str],
        moves_after: usize,
        fails_on: &[&str],
    ) -> (Result<(), lingxia_error::Error>, Vec<String>) {
        let reads = RefCell::new(0usize);
        let calls = RefCell::new(Vec::new());
        let current = || {
            let n = *reads.borrow();
            *reads.borrow_mut() += 1;
            pages
                .get((n / moves_after.max(1)).min(pages.len().saturating_sub(1)))
                .map(|p| p.to_string())
        };
        let result = futures::executor::block_on(on_current_page(
            current,
            |id: String| {
                calls.borrow_mut().push(id.clone());
                let fail = fails_on.contains(&id.as_str());
                async move { if fail { Err(revoked()) } else { Ok(()) } }
            },
            || async {},
        ));
        (result, calls.into_inner())
    }

    #[test]
    fn a_toast_on_a_page_that_leaves_shows_on_the_page_that_takes_its_place() {
        let (result, calls) = run(&["detail", "list"], 3, &["detail"]);
        assert!(result.is_ok());
        assert_eq!(calls, ["detail", "list"]);
    }

    #[test]
    fn a_failure_on_a_page_that_stays_is_the_answer() {
        let (result, calls) = run(&["detail"], 1, &["detail"]);
        assert!(result.is_err());
        assert_eq!(calls, ["detail"]);
    }

    #[test]
    fn a_page_that_shows_it_is_called_once_and_no_page_is_an_error() {
        assert_eq!(run(&["home"], 1, &[]).1, ["home"]);
        assert!(run(&[], 1, &[]).0.is_err());
    }
}

#[cfg(test)]
mod layer_tests {
    use super::ToastRequest;
    use lxapp::feedback::{FeedbackPosition, FeedbackTone};
    use std::time::Duration;

    fn request(duration_ms: f64) -> ToastRequest {
        ToastRequest {
            title: "Saved".into(),
            tone: "success",
            loading: false,
            icon: "success",
            duration_ms,
            position: "bottom",
        }
    }

    #[test]
    fn a_toast_reaches_the_layer_with_its_tone_place_and_time() {
        let toast = request(1500.0).into_feedback();
        assert_eq!(toast.title, "Saved");
        assert_eq!(toast.tone, FeedbackTone::Success);
        assert_eq!(toast.position, FeedbackPosition::Bottom);
        assert_eq!(toast.duration, Some(Duration::from_millis(1500)));
    }

    #[test]
    fn a_toast_without_a_finite_time_stays_until_hidden() {
        for duration in [0.0, -1.0, f64::INFINITY, f64::NAN] {
            assert_eq!(request(duration).into_feedback().duration, None);
        }
    }
}
