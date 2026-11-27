use crate::i18n::{js_internal_error, js_invalid_parameter_error, js_state_error};
use lxapp::LxApp;
use lxapp::dialogs::ToastShown;
use rong::{
    FromJSObject, JSContext, JSContextService, JSFunc, JSObject, JSResult, JSValue,
    function::Optional,
};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

// The window layer hides one lxapp's toast, so the generation guard is one counter per JS
// context: one lxapp's.
#[derive(Default)]
struct ToastState(Arc<AtomicU64>);
impl JSContextService for ToastState {}

fn toast_generation(ctx: &JSContext) -> Arc<AtomicU64> {
    if ctx.get_service::<ToastState>().is_none() {
        ctx.set_service(ToastState::default());
    }
    ctx.get_service::<ToastState>()
        .expect("toast state inserted")
        .0
        .clone()
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

/// A parsed `lx.ui.toast` call.
struct ToastRequest {
    title: String,
    tone: &'static str,
    loading: bool,
    duration_ms: f64,
    position: &'static str,
}

impl ToastRequest {
    fn parse(message: String, options: JSToastOptions) -> JSResult<Self> {
        let tone = match options.tone.as_deref() {
            None | Some("default") => "default",
            Some("success") => "success",
            Some("danger") => "danger",
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
        Ok(Self {
            title: message,
            tone,
            loading: options.loading.unwrap_or(false),
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
fn show_toast(ctx: JSContext, message: JSValue, options: Optional<JSValue>) -> JSResult<JSObject> {
    // Read by hand: an error's `fallback` belongs to the error form's type, not `ToastOptions`.
    let raw = options.0.filter(|value| !value.is_undefined());
    let mut options = match &raw {
        Some(value) => value
            .clone()
            .to_rust::<JSToastOptions>()
            .map_err(|error| error.at_argument(2))?,
        None => JSToastOptions::default(),
    };
    let signal = options.signal.take();
    crate::task_object::validate_abort_signal(signal.as_ref())?;
    let message = if message.is_string() {
        message.to_rust::<String>()?
    } else if message.is_undefined() {
        return Err(js_invalid_parameter_error("toast needs a message or an error").at_argument(1));
    } else {
        // A failure reads as `errorText` words, `danger` unless told otherwise; a cancel shows nothing.
        let fallback = raw
            .and_then(|value| value.into_object())
            .map(|object| object.get_opt::<_, String>("fallback"))
            .transpose()
            .map_err(|error| error.at_argument(2))?
            .flatten();
        match crate::lxapp::error_words(&ctx, message, fallback)? {
            Some(words) => {
                options.tone.get_or_insert_with(|| "danger".into());
                words
            }
            None => return inert_handle(&ctx),
        }
    };
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

/// The handle for a toast that was never shown.
fn inert_handle(ctx: &JSContext) -> JSResult<JSObject> {
    let handle = JSObject::new(ctx);
    handle.set(
        "dismiss",
        JSFunc::new(ctx, || Ok::<(), rong::RongJSError>(()))?,
    )?;
    Ok(handle)
}

/// Resolves after the layer accepts presentation, not after the toast expires.
async fn present_toast(ctx: &JSContext, request: ToastRequest) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(ctx)?;

    // Do not show UI if app is not opened
    if !lxapp.is_opened() {
        return Err(js_state_error(
            "appClosed",
            "LxApp is closed; toast suppressed",
        ));
    }
    super::feedback_layer()?
        .toast(&lxapp.appid, request.into_feedback())
        .map_err(|e| js_internal_error(format!("toast failed: {e}")))
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
    super::feedback_layer()?.hide_toast(&lxapp.appid);
    Ok(())
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
mod error_form_tests {
    use rong::{JSEngine, JSObject, JSValue, Source, function::Optional};

    fn value(ctx: &rong::JSContext, js: &str) -> JSValue {
        ctx.eval(Source::from_bytes(js)).unwrap()
    }

    #[test]
    fn a_cancel_shows_nothing_and_hands_back_a_handle_that_does_nothing() {
        let runtime = <rong::RongJS as JSEngine>::runtime();
        let ctx = runtime.context();
        let cancel = value(
            &ctx,
            "Object.assign(new Error('stop'), { code: 'E_ABORT' })",
        );
        // No app is open: a toast that tried to show would refuse with `appClosed`.
        let handle: JSObject = super::show_toast(ctx.clone(), cancel, Optional(None)).unwrap();
        let dismiss: rong::JSFunc = handle.get("dismiss").unwrap();
        let _: () = dismiss.call(None, ()).unwrap();
    }

    #[test]
    fn a_toast_with_no_message_is_refused() {
        let runtime = <rong::RongJS as JSEngine>::runtime();
        let ctx = runtime.context();
        let missing = value(&ctx, "undefined");
        let error = super::show_toast(ctx.clone(), missing, Optional(None))
            .err()
            .unwrap();
        assert_eq!(error.code(), Some(rong::error::E_INVALID_ARG));
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
