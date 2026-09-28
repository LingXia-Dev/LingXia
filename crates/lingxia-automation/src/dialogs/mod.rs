//! `DialogDriver` — the dialogs an lxapp's Logic opens during one spec.
//!
//! The test runner watches the app under test for each spec. While it does,
//! toasts are recorded as they are presented (the host still draws them),
//! and so are modals and `lx.showActionSheet`: drawn, with how the user
//! closed them. Queued answers apply once; later dialogs are drawn unless
//! the spec explicitly selects strict mode, where a missing answer fails
//! immediately. A watch belongs to its spec attempt; an unwatched app
//! presents every dialog as usual.

use crate::auto_err;
use crate::resolve::{js_object_to_json, json_to_js, upgrade_authorized};
use lxapp::LxApp;
use lxapp::dialogs::{ActionSheetShown, DialogDecision, DialogHook, ModalShown, ToastShown};
use rong::{HostError, IntoJSObject, JSContext, JSObject, JSResult, JSValue, js_class, js_method};
use serde_json::{Value, json};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use tokio::sync::watch;

/// Records one watch keeps of each kind; the oldest go first.
const MAX_RECORDS: usize = 200;
/// Answers one watch may hold queued.
const MAX_QUEUED: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SheetAnswer {
    Index(usize),
    Cancel,
}

impl SheetAnswer {
    fn to_json(self) -> Value {
        match self {
            Self::Index(index) => json!({ "index": index }),
            Self::Cancel => json!({ "cancel": true }),
        }
    }
}

struct Watch {
    run_id: String,
    /// The spec attempt that opened it; `None` for the whole run.
    attempt: Option<u64>,
    toasts: VecDeque<Value>,
    /// With the id a drawn one reports its close by.
    modals: VecDeque<(u64, Value)>,
    sheets: VecDeque<(u64, Value)>,
    modal_answers: VecDeque<bool>,
    sheet_answers: VecDeque<SheetAnswer>,
    /// Only explicit strict mode refuses an unqueued dialog.
    strict_modals: bool,
    strict_sheets: bool,
    next_id: u64,
    /// The first dialog that found no answer. Dropping the sender (the watch
    /// ended) wakes a waiting `unanswered()` with nothing.
    unanswered: watch::Sender<Option<String>>,
}

impl Watch {
    fn new(run_id: String, attempt: Option<u64>) -> Self {
        Self {
            run_id,
            attempt,
            toasts: VecDeque::new(),
            modals: VecDeque::new(),
            sheets: VecDeque::new(),
            modal_answers: VecDeque::new(),
            sheet_answers: VecDeque::new(),
            strict_modals: false,
            strict_sheets: false,
            next_id: 0,
            unanswered: watch::channel(None).0,
        }
    }

    fn fail(&self, message: &str) {
        self.unanswered.send_if_modified(|first| {
            if first.is_some() {
                return false;
            }
            *first = Some(message.to_string());
            true
        });
    }
}

fn push_record<T>(records: &mut VecDeque<T>, record: T) {
    if records.len() == MAX_RECORDS {
        records.pop_front();
    }
    records.push_back(record);
}

/// Record a dialog; a drawn one gets `drawn: true` and the id its close
/// reports by.
fn record_dialog(
    records: &mut VecDeque<(u64, Value)>,
    next_id: &mut u64,
    mut record: Value,
    drawn: bool,
) -> u64 {
    *next_id += 1;
    if drawn {
        record["drawn"] = json!(true);
    }
    push_record(records, (*next_id, record));
    *next_id
}

/// Put how a drawn dialog closed on its record.
fn close_dialog(
    appid: &str,
    id: u64,
    pick: impl FnOnce(&mut Watch) -> &mut VecDeque<(u64, Value)>,
    answer: Value,
) {
    with_watches(|watches| {
        if let Some(watch) = watches.get_mut(appid)
            && let Some((_, record)) = pick(watch).iter_mut().find(|(seen, _)| *seen == id)
        {
            record["answer"] = answer;
        }
    });
}

fn watches() -> &'static Mutex<HashMap<String, Watch>> {
    static WATCHES: OnceLock<Mutex<HashMap<String, Watch>>> = OnceLock::new();
    WATCHES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn with_watches<T>(f: impl FnOnce(&mut HashMap<String, Watch>) -> T) -> T {
    f(&mut watches().lock().unwrap_or_else(|err| err.into_inner()))
}

fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as f64)
        .unwrap_or(0.0)
}

/// End the watches attempt `attempt` of `run_id` opened. Returns how many.
pub(crate) fn reclaim_attempt(run_id: &str, attempt: u64) -> usize {
    with_watches(|watches| {
        let before = watches.len();
        watches.retain(|_, watch| !(watch.run_id == run_id && watch.attempt == Some(attempt)));
        before - watches.len()
    })
}

/// End every watch of a run. Called on every terminal transition of the run.
pub(crate) fn clear_run(run_id: &str) {
    with_watches(|watches| watches.retain(|_, watch| watch.run_id != run_id));
}

// ------------------------------ Logic side ------------------------------

struct Hook;

impl DialogHook for Hook {
    fn toast(&self, appid: &str, toast: &ToastShown) {
        with_watches(|watches| {
            if let Some(watch) = watches.get_mut(appid) {
                push_record(
                    &mut watch.toasts,
                    json!({
                        "title": toast.title,
                        "icon": toast.icon,
                        "duration": toast.duration_ms,
                        "at": now_ms(),
                    }),
                );
            }
        });
    }

    fn modal(&self, appid: &str, modal: &ModalShown) -> DialogDecision<bool> {
        with_watches(|watches| {
            let Some(watch) = watches.get_mut(appid) else {
                return DialogDecision::Present;
            };
            let answer = watch.modal_answers.pop_front();
            let mut record = json!({
                "title": modal.title,
                "content": modal.content,
                "answer": answer.map(|confirm| json!({ "confirm": confirm })),
            });
            if let Some(text) = &modal.confirm_text {
                record["confirmText"] = json!(text);
            }
            if let Some(text) = &modal.cancel_text {
                record["cancelText"] = json!(text);
            }
            let drawn = answer.is_none() && !watch.strict_modals;
            let id = record_dialog(&mut watch.modals, &mut watch.next_id, record, drawn);
            if drawn {
                return DialogDecision::Draw(id);
            }
            match answer {
                Some(confirm) => DialogDecision::Answer(confirm),
                None => {
                    let message = format!(
                        "a modal appeared with no answer queued: title {:?}, content {:?}; \
                         queue one before the action that opens it with \
                         t.app.dialogs.answerNextModal({{ confirm }})",
                        modal.title, modal.content
                    );
                    watch.fail(&message);
                    DialogDecision::Refuse(message)
                }
            }
        })
    }

    fn action_sheet(&self, appid: &str, sheet: &ActionSheetShown) -> DialogDecision<Option<usize>> {
        with_watches(|watches| {
            let Some(watch) = watches.get_mut(appid) else {
                return DialogDecision::Present;
            };
            let answer = watch.sheet_answers.pop_front();
            let drawn = answer.is_none() && !watch.strict_sheets;
            let id = record_dialog(
                &mut watch.sheets,
                &mut watch.next_id,
                json!({
                    "items": sheet.items,
                    "answer": answer.map(SheetAnswer::to_json),
                }),
                drawn,
            );
            if drawn {
                return DialogDecision::Draw(id);
            }
            let refused = match answer {
                Some(SheetAnswer::Cancel) => return DialogDecision::Answer(None),
                Some(SheetAnswer::Index(index)) if index < sheet.items.len() => {
                    return DialogDecision::Answer(Some(index));
                }
                Some(SheetAnswer::Index(index)) => format!(
                    "the queued action sheet answer {{ index: {index} }} is outside its {} items {:?}",
                    sheet.items.len(),
                    sheet.items
                ),
                None => format!(
                    "an action sheet appeared with no answer queued: items {:?}; queue one before \
                     the action that opens it with t.app.dialogs.answerNextActionSheet({{ index }} \
                     | {{ cancel: true }})",
                    sheet.items
                ),
            };
            watch.fail(&refused);
            DialogDecision::Refuse(refused)
        })
    }

    fn modal_closed(&self, appid: &str, id: u64, confirm: Option<bool>) {
        if let Some(confirm) = confirm {
            close_dialog(
                appid,
                id,
                |watch| &mut watch.modals,
                json!({ "confirm": confirm }),
            );
        }
    }

    fn action_sheet_closed(&self, appid: &str, id: u64, selection: Option<Option<usize>>) {
        let answer = match selection {
            Some(Some(index)) => SheetAnswer::Index(index),
            Some(None) => SheetAnswer::Cancel,
            None => return,
        };
        close_dialog(appid, id, |watch| &mut watch.sheets, answer.to_json());
    }
}

/// Answer the dialogs of watched apps. Once per process, before any Logic
/// context opens a dialog.
pub(crate) fn register_hook() {
    lxapp::dialogs::register_dialog_hook(Box::new(Hook));
}

// ----------------------------- driver side -----------------------------

/// The run a driver call belongs to, and the attempt it may act for.
#[derive(Clone)]
pub(crate) struct DialogRunScope {
    run_id: String,
    admit: Arc<dyn Fn() -> Result<Option<u64>, String> + Send + Sync>,
    live: Arc<dyn Fn() -> bool + Send + Sync>,
}

pub(crate) fn attach_run_scope(
    ctx: &JSContext,
    run_id: String,
    admit: impl Fn() -> Result<Option<u64>, String> + Send + Sync + 'static,
    live: impl Fn() -> bool + Send + Sync + 'static,
) {
    ctx.set_state(DialogRunScope {
        run_id,
        admit: Arc::new(admit),
        live: Arc::new(live),
    });
}

fn run_scope(ctx: &JSContext) -> JSResult<DialogRunScope> {
    let scope = ctx.get_state::<DialogRunScope>().cloned().ok_or_else(|| {
        auto_err("dialogs are watched only inside a host automation run (lxdev test)")
    })?;
    if !(scope.live)() {
        return Err(auto_err("this automation run has ended"));
    }
    Ok(scope)
}

fn not_watched(appid: &str) -> rong::RongJSError {
    auto_err(format!(
        "dialogs of {appid} are not watched: the test runner watches the app under test while \
         a spec runs"
    ))
}

#[derive(Debug, Clone, IntoJSObject)]
struct Unwatched {
    /// Modal answers queued that no modal used.
    #[js_name = "modalAnswers"]
    modal_answers: f64,
    /// Action sheet answers queued that no sheet used.
    #[js_name = "actionSheetAnswers"]
    action_sheet_answers: f64,
}

#[js_class(clone)]
pub(crate) struct JSDialogDriver {
    lxapp: Weak<LxApp>,
}

impl JSDialogDriver {
    /// Authorization is checked per call, so reading `.dialogs` never throws.
    pub(crate) fn new(lxapp: Weak<LxApp>) -> Self {
        Self { lxapp }
    }

    fn target(&self, ctx: &JSContext) -> JSResult<(Arc<LxApp>, DialogRunScope)> {
        let app = upgrade_authorized(ctx, &self.lxapp)?;
        let scope = run_scope(ctx)?;
        Ok((app, scope))
    }

    /// Act on this run's watch of the app.
    fn with_watch<T>(&self, ctx: &JSContext, f: impl FnOnce(&mut Watch) -> T) -> JSResult<T> {
        let (app, scope) = self.target(ctx)?;
        with_watches(|watches| match watches.get_mut(&app.appid) {
            Some(watch) if watch.run_id == scope.run_id => Ok(f(watch)),
            _ => Err(not_watched(&app.appid)),
        })
    }

    fn records(
        &self,
        ctx: &JSContext,
        pick: impl FnOnce(&Watch) -> Vec<Value>,
    ) -> JSResult<JSValue> {
        let records = self.with_watch(ctx, |watch| Value::Array(pick(watch)))?;
        json_to_js(ctx, &records)
    }
}

fn parse_modal_answer(answer: &Value) -> Result<bool, String> {
    match answer.get("confirm").and_then(Value::as_bool) {
        Some(confirm) if answer.as_object().is_some_and(|fields| fields.len() == 1) => Ok(confirm),
        _ => Err(format!(
            "answerNextModal takes {{ confirm: true | false }}, got {answer}"
        )),
    }
}

fn parse_sheet_answer(answer: &Value) -> Result<SheetAnswer, String> {
    let fields = answer.as_object().filter(|fields| fields.len() == 1);
    let invalid =
        || format!("answerNextActionSheet takes {{ index }} or {{ cancel: true }}, got {answer}");
    match fields.and_then(|fields| fields.iter().next()) {
        Some((key, value)) if key == "cancel" && value == &Value::Bool(true) => {
            Ok(SheetAnswer::Cancel)
        }
        Some((key, value)) if key == "index" => value
            .as_u64()
            .and_then(|index| usize::try_from(index).ok())
            .map(SheetAnswer::Index)
            .ok_or_else(invalid),
        _ => Err(invalid()),
    }
}

#[js_class(rename = "DialogDriver")]
impl JSDialogDriver {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(
            rong::error::E_ILLEGAL_CONSTRUCTOR,
            "Use lx.automation().lxapp().dialogs",
        )
        .into())
    }

    /// Start watching this app's dialogs for the open attempt, with nothing
    /// recorded or queued. A watch this run already had for the app ends.
    #[js_method]
    fn watch(&self, ctx: JSContext) -> JSResult<()> {
        let (app, scope) = self.target(&ctx)?;
        let attempt = (scope.admit)().map_err(auto_err)?;
        with_watches(|watches| {
            if let Some(other) = watches.get(&app.appid)
                && other.run_id != scope.run_id
            {
                return Err(auto_err(format!(
                    "dialogs of {} are watched by another automation run",
                    app.appid
                )));
            }
            watches.insert(app.appid.clone(), Watch::new(scope.run_id.clone(), attempt));
            Ok(())
        })
    }

    /// Stop watching; resolves the answers queued that no dialog used.
    #[js_method]
    fn unwatch(&self, ctx: JSContext) -> JSResult<Unwatched> {
        let (app, scope) = self.target(&ctx)?;
        with_watches(|watches| match watches.get(&app.appid) {
            Some(watch) if watch.run_id == scope.run_id => {
                let watch = watches.remove(&app.appid).expect("watch present");
                Ok(Unwatched {
                    modal_answers: watch.modal_answers.len() as f64,
                    action_sheet_answers: watch.sheet_answers.len() as f64,
                })
            }
            _ => Ok(Unwatched {
                modal_answers: 0.0,
                action_sheet_answers: 0.0,
            }),
        })
    }

    /// Resolves the first dialog that found no answer, or `null` once the
    /// watch ends without one.
    #[js_method]
    async fn unanswered(&self, ctx: JSContext) -> JSResult<Option<String>> {
        let mut receiver = self.with_watch(&ctx, |watch| watch.unanswered.subscribe())?;
        loop {
            if let Some(message) = receiver.borrow_and_update().clone() {
                return Ok(Some(message));
            }
            if receiver.changed().await.is_err() {
                return Ok(None);
            }
        }
    }

    /// Toasts presented since the watch started, oldest first:
    /// `{ title, icon, duration, at }`.
    #[js_method]
    fn toasts(&self, ctx: JSContext) -> JSResult<JSValue> {
        self.records(&ctx, |watch| watch.toasts.iter().cloned().collect())
    }

    /// Modals since the watch started, oldest first: `{ title, content,
    /// confirmText?, cancelText?, answer, drawn? }`; a drawn one's `answer`
    /// is the user's once it closed.
    #[js_method]
    fn modals(&self, ctx: JSContext) -> JSResult<JSValue> {
        self.records(&ctx, |watch| {
            watch
                .modals
                .iter()
                .map(|(_, record)| record.clone())
                .collect()
        })
    }

    /// `lx.showActionSheet` calls since the watch started: `{ items, answer }`.
    #[js_method(rename = "actionSheets")]
    fn action_sheets(&self, ctx: JSContext) -> JSResult<JSValue> {
        self.records(&ctx, |watch| {
            watch
                .sheets
                .iter()
                .map(|(_, record)| record.clone())
                .collect()
        })
    }

    /// Queue one modal answer; later unqueued modals are drawn by default.
    #[js_method(rename = "answerNextModal")]
    fn answer_next_modal(&self, ctx: JSContext, answer: JSObject) -> JSResult<()> {
        let confirm = parse_modal_answer(&js_object_to_json(&answer)?).map_err(auto_err)?;
        self.with_watch(&ctx, |watch| {
            if watch.modal_answers.len() >= MAX_QUEUED {
                return Err(auto_err(format!(
                    "{MAX_QUEUED} modal answers are already queued"
                )));
            }
            watch.modal_answers.push_back(confirm);
            Ok(())
        })?
    }

    /// Queue the answer of the next `lx.showActionSheet`: `{ index }` picks
    /// that item, `{ cancel: true }` dismisses it. Later unqueued sheets
    /// are drawn by default.
    #[js_method(rename = "answerNextActionSheet")]
    fn answer_next_action_sheet(&self, ctx: JSContext, answer: JSObject) -> JSResult<()> {
        let answer = parse_sheet_answer(&js_object_to_json(&answer)?).map_err(auto_err)?;
        self.with_watch(&ctx, |watch| {
            if watch.sheet_answers.len() >= MAX_QUEUED {
                return Err(auto_err(format!(
                    "{MAX_QUEUED} action sheet answers are already queued"
                )));
            }
            watch.sheet_answers.push_back(answer);
            Ok(())
        })?
    }

    /// Select whether an unqueued dialog is drawn or fails this watch.
    /// Returns the prior modes so a caller can restore a temporary scope.
    #[js_method(rename = "setAnswerMode")]
    fn set_answer_mode(&self, ctx: JSContext, mode: JSObject) -> JSResult<JSValue> {
        let mode = js_object_to_json(&mode)?;
        let fields = mode
            .as_object()
            .ok_or_else(|| auto_err("setAnswerMode takes an options object"))?;
        for (key, value) in fields {
            if !matches!(key.as_str(), "modals" | "actionSheets")
                || !matches!(value.as_str(), Some("draw" | "strict"))
            {
                return Err(auto_err(format!(
                    "setAnswerMode: {key} must be 'draw' or 'strict'"
                )));
            }
        }
        let previous = self.with_watch(&ctx, |watch| {
            let previous = json!({
                "modals": if watch.strict_modals { "strict" } else { "draw" },
                "actionSheets": if watch.strict_sheets { "strict" } else { "draw" },
            });
            if let Some(value) = fields.get("modals") {
                watch.strict_modals = value.as_str() == Some("strict");
            }
            if let Some(value) = fields.get("actionSheets") {
                watch.strict_sheets = value.as_str() == Some("strict");
            }
            previous
        })?;
        json_to_js(&ctx, &previous)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn watched(appid: &str, run_id: &str, attempt: u64) {
        with_watches(|watches| {
            watches.insert(
                appid.to_string(),
                Watch::new(run_id.to_string(), Some(attempt)),
            )
        });
    }

    fn modal(title: &str) -> ModalShown {
        ModalShown {
            title: title.to_string(),
            content: "Really?".to_string(),
            confirm_text: Some("Delete".to_string()),
            cancel_text: None,
            show_cancel: true,
        }
    }

    #[test]
    fn an_unwatched_app_presents_every_dialog() {
        let hook = Hook;
        assert_eq!(
            hook.modal("dialogs-unwatched", &modal("Delete?")),
            DialogDecision::Present
        );
        let sheet = ActionSheetShown {
            items: vec!["A".into()],
        };
        assert_eq!(
            hook.action_sheet("dialogs-unwatched", &sheet),
            DialogDecision::Present
        );
    }

    #[test]
    fn a_spec_that_queues_nothing_sees_dialogs_drawn_and_recorded() {
        let hook = Hook;
        watched("dialogs-drawn", "run-d", 1);
        let DialogDecision::Draw(id) = hook.modal("dialogs-drawn", &modal("Sign out?")) else {
            panic!("an unarmed watch draws the modal");
        };
        let sheet = ActionSheetShown {
            items: vec!["Edit".into(), "Delete".into()],
        };
        let DialogDecision::Draw(sheet_id) = hook.action_sheet("dialogs-drawn", &sheet) else {
            panic!("an unarmed watch draws the sheet");
        };
        let DialogDecision::Draw(failed) = hook.modal("dialogs-drawn", &modal("Broken")) else {
            panic!("drawn");
        };
        hook.modal_closed("dialogs-drawn", id, Some(false));
        hook.action_sheet_closed("dialogs-drawn", sheet_id, Some(Some(1)));
        hook.modal_closed("dialogs-drawn", failed, None);
        let (modals, sheets, unanswered) = with_watches(|watches| {
            let watch = &watches["dialogs-drawn"];
            (
                watch.modals.clone(),
                watch.sheets.clone(),
                watch.unanswered.borrow().clone(),
            )
        });
        assert_eq!(
            modals[0].1,
            json!({ "title": "Sign out?", "content": "Really?", "confirmText": "Delete", "drawn": true, "answer": { "confirm": false } })
        );
        assert_eq!(
            modals[1].1["answer"],
            Value::Null,
            "presentation failed: no answer"
        );
        assert_eq!(sheets[0].1["answer"], json!({ "index": 1 }));
        assert_eq!(unanswered, None, "drawn dialogs never fail the spec");
        clear_run("run-d");
    }

    #[test]
    fn queued_answer_is_one_shot_without_strict_mode() {
        let hook = Hook;
        watched("dialogs-once", "run-once", 1);
        with_watches(|watches| {
            let watch = watches.get_mut("dialogs-once").unwrap();
            watch.modal_answers.push_back(true);
            watch.sheet_answers.push_back(SheetAnswer::Cancel);
        });
        assert_eq!(
            hook.modal("dialogs-once", &modal("First")),
            DialogDecision::Answer(true)
        );
        assert!(matches!(
            hook.modal("dialogs-once", &modal("Second")),
            DialogDecision::Draw(_)
        ));
        let sheet = ActionSheetShown {
            items: vec!["A".into()],
        };
        assert_eq!(
            hook.action_sheet("dialogs-once", &sheet),
            DialogDecision::Answer(None)
        );
        assert!(matches!(
            hook.action_sheet("dialogs-once", &sheet),
            DialogDecision::Draw(_)
        ));
        clear_run("run-once");
    }

    #[test]
    fn queued_answers_answer_modals_in_order_and_a_missing_one_refuses() {
        let hook = Hook;
        watched("dialogs-modal", "run-m", 1);
        with_watches(|watches| {
            let watch = watches.get_mut("dialogs-modal").unwrap();
            watch.modal_answers.extend([true, false]);
            watch.strict_modals = true;
        });
        let mut receiver = with_watches(|watches| watches["dialogs-modal"].unanswered.subscribe());
        assert_eq!(
            hook.modal("dialogs-modal", &modal("One")),
            DialogDecision::Answer(true)
        );
        assert_eq!(
            hook.modal("dialogs-modal", &modal("Two")),
            DialogDecision::Answer(false)
        );
        let DialogDecision::Refuse(message) = hook.modal("dialogs-modal", &modal("Three")) else {
            panic!("an unanswered modal must refuse once answering");
        };
        assert!(
            message.contains("\"Three\"") && message.contains("\"Really?\""),
            "{message}"
        );
        assert_eq!(
            receiver.borrow_and_update().as_deref(),
            Some(message.as_str())
        );
        let modals = with_watches(|watches| watches["dialogs-modal"].modals.clone());
        assert_eq!(
            modals[0].1,
            json!({ "title": "One", "content": "Really?", "confirmText": "Delete", "answer": { "confirm": true } })
        );
        assert_eq!(modals[2].1["answer"], Value::Null);
        // The first unanswered dialog is the one reported.
        hook.modal("dialogs-modal", &modal("Four"));
        assert!(receiver.borrow().as_deref().unwrap().contains("Three"));
        // Answering modals leaves action sheets drawn.
        let sheet = ActionSheetShown {
            items: vec!["A".into()],
        };
        assert!(matches!(
            hook.action_sheet("dialogs-modal", &sheet),
            DialogDecision::Draw(_)
        ));
        assert_eq!(reclaim_attempt("run-m", 1), 1);
        assert_eq!(
            hook.modal("dialogs-modal", &modal("After")),
            DialogDecision::Present
        );
    }

    #[test]
    fn action_sheets_pick_cancel_or_refuse_an_index_out_of_range() {
        let hook = Hook;
        watched("dialogs-sheet", "run-s", 2);
        with_watches(|watches| {
            let watch = watches.get_mut("dialogs-sheet").unwrap();
            watch.sheet_answers.extend([
                SheetAnswer::Index(1),
                SheetAnswer::Cancel,
                SheetAnswer::Index(5),
            ]);
            watch.strict_sheets = true;
        });
        let sheet = ActionSheetShown {
            items: vec!["Edit".into(), "Delete".into()],
        };
        assert_eq!(
            hook.action_sheet("dialogs-sheet", &sheet),
            DialogDecision::Answer(Some(1))
        );
        assert_eq!(
            hook.action_sheet("dialogs-sheet", &sheet),
            DialogDecision::Answer(None)
        );
        let DialogDecision::Refuse(message) = hook.action_sheet("dialogs-sheet", &sheet) else {
            panic!("an index outside the items must refuse");
        };
        assert!(message.contains("index: 5"), "{message}");
        clear_run("run-s");
        assert_eq!(
            hook.action_sheet("dialogs-sheet", &sheet),
            DialogDecision::Present
        );
    }

    #[test]
    fn toasts_are_recorded_only_while_watched_and_bounded() {
        let hook = Hook;
        let toast = ToastShown {
            title: "Saved".into(),
            icon: "success".into(),
            duration_ms: 1500.0,
        };
        hook.toast("dialogs-toast", &toast);
        watched("dialogs-toast", "run-t", 3);
        for _ in 0..MAX_RECORDS + 5 {
            hook.toast("dialogs-toast", &toast);
        }
        let toasts = with_watches(|watches| watches["dialogs-toast"].toasts.clone());
        assert_eq!(toasts.len(), MAX_RECORDS);
        assert_eq!(toasts[0]["title"], "Saved");
        assert_eq!(toasts[0]["icon"], "success");
        assert_eq!(toasts[0]["duration"], 1500.0);
        assert!(toasts[0]["at"].as_f64().unwrap() > 0.0);
        assert_eq!(
            reclaim_attempt("run-t", 4),
            0,
            "another attempt's watch stays"
        );
        clear_run("run-t");
    }

    #[test]
    fn answers_parse_strictly() {
        assert_eq!(parse_modal_answer(&json!({ "confirm": false })), Ok(false));
        assert!(parse_modal_answer(&json!({ "confirm": "yes" })).is_err());
        assert!(parse_modal_answer(&json!({ "confirm": true, "x": 1 })).is_err());
        assert_eq!(
            parse_sheet_answer(&json!({ "index": 2 })),
            Ok(SheetAnswer::Index(2))
        );
        assert_eq!(
            parse_sheet_answer(&json!({ "cancel": true })),
            Ok(SheetAnswer::Cancel)
        );
        assert!(parse_sheet_answer(&json!({ "index": -1 })).is_err());
        assert!(parse_sheet_answer(&json!({ "cancel": false })).is_err());
        assert!(parse_sheet_answer(&json!({ "index": 1, "cancel": true })).is_err());
    }
}
