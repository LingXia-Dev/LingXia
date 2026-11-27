use crate::i18n::{
    js_internal_error, js_invalid_parameter_error, js_service_unavailable_error, js_state_error,
};
use lxapp::LxApp;
use lxapp::dialogs::{DialogDecision, ModalShown};
use rong::{FromJSObject, JSContext, JSObject, JSResult, JSValue, RongJSError};
use std::sync::Arc;

/// `lx.ui.alert` / `lx.ui.confirm` options; a bare string is the title.
#[derive(FromJSObject)]
#[ts_skip]
struct JSDialogOptions {
    title: Option<String>,
    message: Option<String>,
    tone: Option<String>,
    #[js_name = "confirmText"]
    confirm_text: Option<String>,
    #[js_name = "cancelText"]
    cancel_text: Option<String>,
    signal: Option<JSObject>,
}

struct JSModalOptions {
    title: Option<String>,
    content: Option<String>,
    show_cancel: Option<bool>,
    cancel_text: Option<String>,
    confirm_text: Option<String>,
    /// The confirm button is a destructive choice.
    destructive: bool,
}

/// The modal to show, and the `signal` that dismisses it. `failure` titles it when the options
/// name no title.
fn dialog_options(
    value: JSValue,
    show_cancel: bool,
    failure: Option<String>,
) -> JSResult<(JSModalOptions, Option<JSObject>)> {
    let options = if value.is_string() {
        JSDialogOptions {
            title: Some(value.to_rust::<String>()?),
            message: None,
            tone: None,
            confirm_text: None,
            cancel_text: None,
            signal: None,
        }
    } else {
        value
            .to_rust::<JSDialogOptions>()
            .map_err(|error| error.at_argument(1))?
    };
    let title = options.title.or(failure).unwrap_or_default();
    // An empty title draws a dialog with no accessible name, which the View layer refuses.
    if title.trim().is_empty() {
        return Err(js_invalid_parameter_error(
            "a dialog needs a non-empty title",
        ));
    }
    crate::task_object::validate_abort_signal(options.signal.as_ref())?;
    let destructive = super::destructive(options.tone.as_deref())?;
    let modal = JSModalOptions {
        title: Some(title),
        content: options.message,
        show_cancel: Some(show_cancel),
        cancel_text: options.cancel_text.filter(|_| show_cancel),
        confirm_text: options.confirm_text,
        destructive,
    };
    Ok((modal, options.signal))
}

/// Resolves true when the user confirmed, false when they dismissed or
/// canceled. Rejects when presentation fails, and `E_ABORT` when `signal` aborts.
async fn show_modal(
    ctx: JSContext,
    (options, signal): (JSModalOptions, Option<JSObject>),
) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    // Do not show UI if app is not opened
    if !lxapp.is_opened() {
        return Err(js_state_error(
            "appClosed",
            "LxApp is closed; modal suppressed",
        ));
    }

    let work = async {
        match watched_modal(&lxapp.appid, &options) {
            DialogDecision::Present => present_modal(&lxapp, options).await,
            DialogDecision::Draw(id) => {
                let result = present_modal(&lxapp, options).await;
                if let Some(hook) = lxapp::dialogs::dialog_hook() {
                    hook.modal_closed(&lxapp.appid, id, result.as_ref().ok().copied());
                }
                result
            }
            DialogDecision::Answer(confirm) => Ok(confirm),
            DialogDecision::Refuse(message) => Err(js_service_unavailable_error(message)),
        }
    };
    let owner = super::owner_end(&lxapp);
    super::dismissable(&ctx, signal, owner, work, || {
        super::dismiss_dialog(lxapp.clone())
    })
    .await
}

/// A test run watching the app records the modal, or answers it instead
/// of the user once its spec queued answers.
fn watched_modal(appid: &str, options: &JSModalOptions) -> DialogDecision<bool> {
    let Some(hook) = lxapp::dialogs::dialog_hook() else {
        return DialogDecision::Present;
    };
    let show_cancel = options.show_cancel.unwrap_or(true);
    hook.modal(
        appid,
        &ModalShown {
            title: options.title.clone().unwrap_or_default(),
            content: options.content.clone().unwrap_or_default(),
            confirm_text: options.confirm_text.clone(),
            cancel_text: options.cancel_text.clone().filter(|_| show_cancel),
            show_cancel,
        },
    )
}

/// Acknowledgement-only dialog. Resolves when the user dismisses it. With `error`, the failure's
/// words (what the View's `errorText` says) are the title unless `title` is given, and a cancel shows no dialog.
///
/// @example
/// ```ts
/// try {
///   await lx.clipboard.writeText('https://example.com');
/// } catch (e) {
///   await lx.ui.alert({ error: e, fallback: "Couldn't copy the link" });
/// }
/// ```
/// @throws {E_INVALID_ARG} A missing or empty `title`, without `error`.
/// @throws {E_ABORT} `signal` aborted; the dialog is closed.
/// @throws {E_INVALID_STATE} No page can show a dialog now; `reason` is `'appClosed'` when the app is not open.
/// @throws {E_INTERNAL} The dialog could not be shown.
async fn alert(ctx: JSContext, options: JSValue) -> JSResult<()> {
    let title = match failure_title(&ctx, &options)? {
        FailureTitle::Cancel => return Ok(()),
        FailureTitle::Words(words) => Some(words),
        FailureTitle::None => None,
    };
    show_modal(ctx, dialog_options(options, false, title)?).await?;
    Ok(())
}

enum FailureTitle {
    /// No `error`: the options name the title.
    None,
    /// `error`'s `errorText` words, the title unless one is given.
    Words(String),
    /// `error` is a cancel: no dialog.
    Cancel,
}

/// `error` and `fallback` are read by hand: they belong to `AlertOptions` alone, not `ConfirmOptions`.
fn failure_title(ctx: &JSContext, options: &JSValue) -> JSResult<FailureTitle> {
    let Some(object) = options.clone().into_object() else {
        return Ok(FailureTitle::None);
    };
    let Some(error) = object
        .get_opt::<_, JSValue>("error")
        .map_err(|e| e.at_argument(1))?
        .filter(|error| !error.is_undefined())
    else {
        return Ok(FailureTitle::None);
    };
    let fallback = object
        .get_opt::<_, String>("fallback")
        .map_err(|e| e.at_argument(1))?;
    Ok(match crate::lxapp::error_words(ctx, error, fallback)? {
        Some(words) => FailureTitle::Words(words),
        None => FailureTitle::Cancel,
    })
}

/// Ask a yes/no question. Dismissal resolves false; presentation failure rejects.
///
/// @throws {E_INVALID_ARG} A missing or empty `title`.
/// @throws {E_ABORT} `signal` aborted; the dialog is closed.
/// @throws {E_INVALID_STATE} No page can show a dialog now; `reason` is `'appClosed'` when the app is not open.
/// @throws {E_INTERNAL} The dialog could not be shown.
async fn confirm(ctx: JSContext, options: JSValue) -> JSResult<bool> {
    show_modal(ctx, dialog_options(options, true, None)?).await
}

/// Resolves true when the user confirmed.
async fn present_modal(lxapp: &Arc<LxApp>, options: JSModalOptions) -> Result<bool, RongJSError> {
    let language = lxapp.content_language(&lxapp::display_language());
    present_modal_layer(super::feedback_layer()?, &lxapp.appid, &language, options).await
}

/// The window feedback layer: default words in `language`, the lxapp's page language.
async fn present_modal_layer(
    layer: &dyn lxapp::feedback::FeedbackLayer,
    appid: &str,
    language: &str,
    options: JSModalOptions,
) -> Result<bool, RongJSError> {
    use lxapp::feedback::{FeedbackAnswer, FeedbackDialog};
    let title = options.title.unwrap_or_default();
    let message = options.content.unwrap_or_default();
    let destructive = options.destructive;
    let dialog = if options.show_cancel.unwrap_or(true) {
        FeedbackDialog::Confirm {
            title,
            message,
            confirm: options.confirm_text,
            cancel: options.cancel_text,
            destructive,
        }
    } else {
        FeedbackDialog::Alert {
            title,
            message,
            confirm: options.confirm_text,
            destructive,
        }
    };
    let answer = layer
        .dialog(appid, language, dialog)
        .map_err(|e| js_internal_error(format!("modal failed: {e}")))?
        .await
        .map_err(|_| js_internal_error("the feedback layer dropped the modal"))?;
    Ok(answer == FeedbackAnswer::Confirmed)
}

/// Initialize modal functions
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace UiApi = super::ui_namespace(ctx)?;
        fn alert(ts_params = "options: string | AlertOptions", ts_return = "Promise<void>") = alert;
        fn confirm(
            ts_params = "options: string | ConfirmOptions",
            ts_return = "Promise<boolean>"
        ) = confirm;
    }
}

#[cfg(test)]
mod layer_tests {
    use super::{JSModalOptions, present_modal_layer};
    use crate::ui::test_layer::FakeLayer;
    use lxapp::feedback::{FeedbackAnswer, FeedbackDialog};

    fn options(show_cancel: bool, danger: bool) -> JSModalOptions {
        JSModalOptions {
            title: Some("Delete?".into()),
            content: Some("It cannot be undone.".into()),
            show_cancel: Some(show_cancel),
            cancel_text: None,
            confirm_text: Some("Delete".into()),
            destructive: danger,
        }
    }

    #[test]
    fn a_confirm_goes_to_the_layer_in_the_page_language_and_resolves_its_answer() {
        for (answer, confirmed) in [
            (FeedbackAnswer::Confirmed, true),
            (FeedbackAnswer::Cancelled, false),
        ] {
            let layer = FakeLayer::answering(answer);
            let result = futures::executor::block_on(present_modal_layer(
                &layer,
                "app",
                "zh-CN",
                options(true, true),
            ));
            assert_eq!(result.unwrap(), confirmed);
            assert_eq!(
                layer.dialogs.lock().unwrap().as_slice(),
                [(
                    "app".to_string(),
                    "zh-CN".to_string(),
                    FeedbackDialog::Confirm {
                        title: "Delete?".into(),
                        message: "It cannot be undone.".into(),
                        confirm: Some("Delete".into()),
                        cancel: None,
                        destructive: true,
                    }
                )]
            );
        }
    }

    #[test]
    fn a_modal_without_cancel_is_an_alert() {
        let layer = FakeLayer::answering(FeedbackAnswer::Confirmed);
        let result = futures::executor::block_on(present_modal_layer(
            &layer,
            "app",
            "en-US",
            options(false, false),
        ));
        assert!(result.unwrap());
        assert!(matches!(
            layer.dialogs.lock().unwrap()[0].2,
            FeedbackDialog::Alert {
                destructive: false,
                ..
            }
        ));
    }
}

#[cfg(test)]
mod failure_title_tests {
    use super::{FailureTitle, failure_title};
    use rong::{JSEngine, Source};

    fn title(js: &str) -> FailureTitle {
        let runtime = <rong::RongJS as JSEngine>::runtime();
        let ctx = runtime.context();
        let options = ctx.eval(Source::from_bytes(js)).unwrap();
        failure_title(&ctx, &options).unwrap()
    }

    #[test]
    fn an_alert_takes_its_title_from_a_failure_and_shows_nothing_for_a_cancel() {
        assert!(matches!(title("({ title: 'Saved' })"), FailureTitle::None));
        assert!(matches!(title("'Saved'"), FailureTitle::None));
        assert!(matches!(
            title("({ error: Object.assign(new Error('stop'), { code: 'E_ABORT' }) })"),
            FailureTitle::Cancel
        ));
        assert!(matches!(
            title("({ error: new Error('boom'), fallback: 'Not saved' })"),
            FailureTitle::Words(words) if words == "Not saved"
        ));
    }
}
