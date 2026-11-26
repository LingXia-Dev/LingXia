use lxapp::LxApp;
use rong::{JSContext, JSFunc, JSObject, JSResult};
use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

mod action_sheet;
pub(crate) use action_sheet::action_sheet;
mod modal;
mod more_actions;
mod navbar;
mod page;
mod page_chrome_patch;
mod pull_to_refresh;
pub(crate) mod router;
mod shell;
mod tabbar;
mod toast;
mod tray;

/// Pause between dismiss attempts while an aborted dialog's call settles.
const DISMISS_RETRY: Duration = Duration::from_millis(100);
/// Dismiss attempts before an aborted dialog stops waiting for its call.
const DISMISS_ATTEMPTS: u32 = 20;

/// When the page a dialog or picker shows on ends; `None` off any page.
pub(crate) type OwnerEnd = std::pin::Pin<Box<dyn Future<Output = ()>>>;

/// The end of the current page's entry, which owns what it shows.
pub(crate) fn owner_end(lxapp: &LxApp) -> Option<OwnerEnd> {
    lxapp
        .current_page()
        .ok()
        .map(|page| Box::pin(page.unloaded()) as OwnerEnd)
}

/// Run a dialog until it settles, `signal` aborts, or its page unloads. An abort
/// dismisses it and rejects `E_ABORT`; the page unloading dismisses it and the call
/// settles as the person dismissing it would (`false`, `canceled`). Dismissing
/// repeats until the call settles, because the end can land before the dialog is
/// on screen. An already-aborted signal rejects before `work` starts.
async fn dismissable<T, D, F>(
    ctx: &JSContext,
    signal: Option<JSObject>,
    owner: Option<OwnerEnd>,
    work: impl Future<Output = JSResult<T>>,
    mut dismiss: D,
) -> JSResult<T>
where
    D: FnMut() -> F,
    F: Future<Output = ()>,
{
    use futures::future::{Either, select};
    let mut listener = None;
    let mut aborted = None;
    if let Some(signal) = signal {
        crate::task_object::validate_abort_signal(Some(&signal))?;
        if signal.get::<_, bool>("aborted")? {
            return Err(crate::task_object::abort_error());
        }
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let tx = Rc::new(RefCell::new(Some(tx)));
        let on_abort = JSFunc::new(ctx, move || {
            if let Some(tx) = tx.borrow_mut().take() {
                let _ = tx.send(());
            }
        })?;
        let options = JSObject::new(ctx);
        options.set("once", true)?;
        let add: JSFunc = signal.get("addEventListener")?;
        add.call::<_, ()>(Some(signal.clone()), ("abort", on_abort.clone(), options))?;
        listener = Some((signal, on_abort));
        aborted = Some(rx);
    }
    let abort = async move {
        match aborted {
            Some(rx) => {
                let _ = rx.await;
            }
            None => std::future::pending().await,
        }
    };
    let unloaded = async move {
        match owner {
            Some(end) => end.await,
            None => std::future::pending().await,
        }
    };
    let outcome = {
        futures::pin_mut!(work, abort, unloaded);
        match select(work.as_mut(), select(abort, unloaded)).await {
            Either::Left((done, _)) => done,
            Either::Right((ended, _)) => {
                let by_abort = matches!(ended, Either::Left(_));
                let mut settled = None;
                for _ in 0..DISMISS_ATTEMPTS {
                    dismiss().await;
                    let pause = tokio::time::sleep(DISMISS_RETRY);
                    futures::pin_mut!(pause);
                    if let Either::Left((done, _)) = select(work.as_mut(), pause).await {
                        settled = Some(done);
                        break;
                    }
                }
                match settled {
                    Some(done) if !by_abort => done,
                    Some(_) => Err(crate::task_object::abort_error()),
                    None => {
                        log::warn!("a dismissed dialog did not close; its call is abandoned");
                        Err(crate::task_object::abort_error())
                    }
                }
            }
        }
    };
    if let Some((signal, on_abort)) = listener {
        let remove: JSFunc = signal.get("removeEventListener")?;
        let _ = remove.call::<_, ()>(Some(signal), ("abort", on_abort));
    }
    outcome
}

/// The window feedback layer, which shows every toast, dialog and action sheet.
fn feedback_layer() -> JSResult<&'static dyn lxapp::feedback::FeedbackLayer> {
    lxapp::feedback::feedback_layer().ok_or_else(|| {
        crate::i18n::js_service_unavailable_error("the window feedback layer is not installed")
    })
}

/// Close the lxapp's modal and action sheet, shown or queued.
async fn dismiss_dialog(lxapp: Arc<LxApp>) {
    match feedback_layer() {
        Ok(layer) => layer.dismiss(&lxapp.appid),
        Err(error) => log::warn!("dismissing a dialog failed: {error:?}"),
    }
}

/// Feedback shown over the page: toasts, dialogs and action sheets.
fn ui_namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    match lx.get::<_, JSObject>("ui") {
        Ok(namespace) => Ok(namespace),
        Err(_) => {
            let namespace = JSObject::new(ctx);
            lx.set("ui", namespace.clone())?;
            Ok(namespace)
        }
    }
}

/// `tone: 'danger'` marks a destructive choice; the layer draws it in red.
fn destructive(tone: Option<&str>) -> JSResult<bool> {
    match tone {
        None | Some("default") => Ok(false),
        Some("danger") => Ok(true),
        Some(other) => Err(crate::i18n::js_invalid_parameter_error(format!(
            "tone must be 'default' or 'danger', got '{other}'"
        ))),
    }
}

rong::js_api! {
    fn register_ui_property(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        const ui: "UiApi" = ui_namespace(ctx)?;
    }
}

/// Only the page object's natives (navigation bar, env, refresh, video), for a context that
/// registers no other UI.
#[cfg(feature = "terminal")]
pub(crate) fn init_page(ctx: &JSContext) -> JSResult<()> {
    page::init(ctx)
}

/// Initialize UI module
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_ui_property(ctx)?;
    toast::init(ctx)?;
    modal::init(ctx)?;
    action_sheet::init(ctx)?;
    page::init(ctx)?;
    tabbar::init(ctx)?;
    router::init(ctx)?;
    more_actions::init(ctx)?;
    shell::init(ctx)?;
    tray::init(ctx)?;
    Ok(())
}

#[cfg(test)]
mod test_layer {
    use lxapp::feedback::{FeedbackAnswer, FeedbackDialog, FeedbackLayer, FeedbackToast};
    use std::sync::Mutex;
    use tokio::sync::oneshot;

    /// A window feedback layer that records each dialog and answers it with `answer`.
    pub(super) struct FakeLayer {
        pub(super) answer: FeedbackAnswer,
        pub(super) dialogs: Mutex<Vec<(String, String, FeedbackDialog)>>,
    }

    impl FakeLayer {
        pub(super) fn answering(answer: FeedbackAnswer) -> Self {
            Self {
                answer,
                dialogs: Mutex::default(),
            }
        }
    }

    impl FeedbackLayer for FakeLayer {
        fn toast(&self, _: &str, _: FeedbackToast) -> Result<(), String> {
            Ok(())
        }
        fn hide_toast(&self, _: &str) {}
        fn dialog(
            &self,
            appid: &str,
            language: &str,
            dialog: FeedbackDialog,
        ) -> Result<oneshot::Receiver<FeedbackAnswer>, String> {
            self.dialogs
                .lock()
                .unwrap()
                .push((appid.into(), language.into(), dialog));
            let (answer, settled) = oneshot::channel();
            answer.send(self.answer).unwrap();
            Ok(settled)
        }
        fn dismiss(&self, _: &str) {}
        fn forget(&self, _: &str) {}
    }
}

#[cfg(test)]
mod dismiss_tests {
    use super::dismissable;
    use crate::dismissal::USER_DISMISSED;
    use lingxia_error::Error;
    use rong::{JSContext, JSEngine, JSObject, JSResult, Source};
    use std::future::Future;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::sync::oneshot;

    type Answer = oneshot::Sender<Result<String, Error>>;

    /// A host whose dialog stays up until the user answers or Logic dismisses it.
    #[derive(Default, Clone)]
    struct FakeFeedback(Arc<Host>);

    impl std::ops::Deref for FakeFeedback {
        type Target = Host;
        fn deref(&self) -> &Host {
            &self.0
        }
    }

    #[derive(Default)]
    struct Host {
        on_screen: Mutex<Option<Answer>>,
        shown: AtomicUsize,
        dismissed: AtomicUsize,
        present_after: Mutex<Duration>,
    }

    impl Host {
        fn answer(&self, payload: &str) {
            let answer = self
                .on_screen
                .lock()
                .unwrap()
                .take()
                .expect("a dialog on screen");
            answer.send(Ok(payload.to_string())).unwrap();
        }

        fn on_screen(&self) -> bool {
            self.on_screen.lock().unwrap().is_some()
        }
    }

    impl FakeFeedback {
        /// A dialog that stays up until answered or dismissed.
        fn show_modal(&self) -> impl Future<Output = Result<String, Error>> + use<> {
            let this = self.clone();
            async move {
                let delay = *this.present_after.lock().unwrap();
                tokio::time::sleep(delay).await;
                let (answer, settled) = oneshot::channel();
                *this.on_screen.lock().unwrap() = Some(answer);
                this.shown.fetch_add(1, Ordering::SeqCst);
                settled.await.unwrap()
            }
        }

        fn dismiss_dialog(&self) {
            self.dismissed.fetch_add(1, Ordering::SeqCst);
            if let Some(answer) = self.on_screen.lock().unwrap().take() {
                let _ = answer.send(Err(Error::business(USER_DISMISSED, "dismissed")));
            }
        }
    }

    /// The confirm flow over `feedback`, settling as `modal.rs` reads the host.
    fn confirm(
        ctx: &JSContext,
        feedback: &FakeFeedback,
        signal: JSObject,
    ) -> impl Future<Output = JSResult<bool>> {
        owned_confirm(ctx, feedback, Some(signal), None)
    }

    /// `confirm`, optionally on a page whose end `owner` reports.
    fn owned_confirm(
        ctx: &JSContext,
        feedback: &FakeFeedback,
        signal: Option<JSObject>,
        owner: Option<super::OwnerEnd>,
    ) -> impl Future<Output = JSResult<bool>> {
        let shown = feedback.show_modal();
        let work = async move {
            match shown.await {
                Ok(payload) => Ok(payload.contains("true")),
                Err(error) if lingxia_platform::error::is_user_dismissed(&error) => Ok(false),
                Err(error) => Err(crate::i18n::js_internal_error(error.to_string())),
            }
        };
        let host = feedback.clone();
        let ctx = ctx.clone();
        async move {
            dismissable(&ctx, signal, owner, work, || {
                let host = host.clone();
                async move {
                    host.dismiss_dialog();
                }
            })
            .await
        }
    }

    fn signal(ctx: &JSContext, aborted: bool) -> JSObject {
        ctx.eval(Source::from_bytes(format!(
            "globalThis.signal = {{ aborted: {aborted}, listeners: [], \
             addEventListener(_, f) {{ this.listeners.push(f); }}, \
             removeEventListener(_, f) {{ this.listeners = this.listeners.filter(g => g !== f); }} }}"
        )))
        .unwrap()
    }

    fn abort(ctx: &JSContext) {
        let _: rong::JSValue = ctx
            .eval(Source::from_bytes(
                "signal.aborted = true; signal.listeners.slice().forEach(f => f());",
            ))
            .unwrap();
    }

    fn listeners(ctx: &JSContext) -> i32 {
        ctx.eval(Source::from_bytes("signal.listeners.length"))
            .unwrap()
    }

    fn run<T>(work: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(work)
    }

    #[test]
    fn its_page_unloading_closes_the_dialog_as_dismissed() {
        let runtime = <rong::RongJS as JSEngine>::runtime();
        let ctx = runtime.context();
        let feedback = FakeFeedback::default();
        let (unload, unloaded) = oneshot::channel::<()>();
        let owner: super::OwnerEnd = Box::pin(async move {
            let _ = unloaded.await;
        });
        let host = feedback.clone();
        let result = run(async move {
            let call = owned_confirm(&ctx, &feedback, None, Some(owner));
            let close = async move {
                while !host.on_screen() {
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                }
                unload.send(()).unwrap();
            };
            let (result, ()) = futures::join!(call, close);
            (result, feedback.dismissed.load(Ordering::SeqCst))
        });
        assert!(!result.0.unwrap(), "settles as dismissed, not E_ABORT");
        assert_eq!(result.1, 1);
    }

    #[test]
    fn a_dialog_off_any_page_waits_for_its_answer() {
        let runtime = <rong::RongJS as JSEngine>::runtime();
        let ctx = runtime.context();
        let feedback = FakeFeedback::default();
        let host = feedback.clone();
        let result = run(async move {
            let call = owned_confirm(&ctx, &feedback, None, None);
            let answer = async move {
                while !host.on_screen() {
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                }
                host.answer("{\"confirm\":true}");
            };
            let (result, ()) = futures::join!(call, answer);
            result
        });
        assert!(result.unwrap());
    }

    #[test]
    fn an_aborted_signal_rejects_before_anything_shows() {
        let runtime = <rong::RongJS as JSEngine>::runtime();
        let ctx = runtime.context();
        let feedback = FakeFeedback::default();
        let result = run(confirm(&ctx, &feedback, signal(&ctx, true)));
        assert!(result.unwrap_err().is_code(rong::error::E_ABORT));
        assert_eq!(feedback.shown.load(Ordering::SeqCst), 0);
        assert_eq!(listeners(&ctx), 0);
    }

    #[test]
    fn an_abort_while_shown_closes_the_dialog_and_rejects_abort() {
        let runtime = <rong::RongJS as JSEngine>::runtime();
        let ctx = runtime.context();
        let feedback = FakeFeedback::default();
        let call = confirm(&ctx, &feedback, signal(&ctx, false));
        let result = run(async {
            let (result, ()) = futures::join!(call, async {
                tokio::time::sleep(Duration::from_millis(20)).await;
                assert!(feedback.on_screen());
                abort(&ctx);
            });
            result
        });
        assert!(result.unwrap_err().is_code(rong::error::E_ABORT));
        assert!(!feedback.on_screen(), "no dialog is left");
        assert_eq!(feedback.dismissed.load(Ordering::SeqCst), 1);
        assert_eq!(listeners(&ctx), 0);
    }

    #[test]
    fn an_abort_before_the_dialog_appears_dismisses_it_once_it_does() {
        let runtime = <rong::RongJS as JSEngine>::runtime();
        let ctx = runtime.context();
        let feedback = FakeFeedback::default();
        *feedback.present_after.lock().unwrap() = Duration::from_millis(150);
        let call = confirm(&ctx, &feedback, signal(&ctx, false));
        let result = run(async {
            let (result, ()) = futures::join!(call, async {
                tokio::time::sleep(Duration::from_millis(10)).await;
                abort(&ctx);
            });
            result
        });
        assert!(result.unwrap_err().is_code(rong::error::E_ABORT));
        assert_eq!(feedback.shown.load(Ordering::SeqCst), 1);
        assert!(!feedback.on_screen(), "the late dialog is closed too");
        assert!(feedback.dismissed.load(Ordering::SeqCst) >= 2);
    }

    #[test]
    fn an_abort_after_the_answer_does_nothing() {
        let runtime = <rong::RongJS as JSEngine>::runtime();
        let ctx = runtime.context();
        let feedback = FakeFeedback::default();
        let call = confirm(&ctx, &feedback, signal(&ctx, false));
        let result = run(async {
            let (result, ()) = futures::join!(call, async {
                tokio::time::sleep(Duration::from_millis(20)).await;
                feedback.answer(r#"{"confirm":true}"#);
            });
            result
        });
        assert!(result.unwrap());
        assert_eq!(listeners(&ctx), 0);
        abort(&ctx);
        assert_eq!(feedback.dismissed.load(Ordering::SeqCst), 0);
    }
}
