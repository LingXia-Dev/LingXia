use super::*;
use lingxia_view::platform::{OverlayLogic, Task};
use lxapp::feedback::{
    FeedbackAnswer as Answer, FeedbackDialog as DialogRequest, FeedbackPosition as Position,
    FeedbackToast as ToastRequest, FeedbackTone as Tone,
};
use serde_json::{Value, json};
use std::time::Instant;
use tokio::sync::oneshot::error::TryRecvError;

const MAIN: WindowId = WindowId(1);
const SIDE: WindowId = WindowId(2);

#[derive(Default)]
struct Surface {
    states: Mutex<Vec<(u64, Value)>>,
    inputs: Mutex<Vec<OverlayInput>>,
    focus: Mutex<Vec<bool>>,
    replies: Mutex<Vec<(String, bool)>>,
    environment: Mutex<Vec<Value>>,
}

/// Dropping the boxed surface is the platform closing it.
struct Handle(Arc<Surface>, Arc<Mutex<u32>>);

impl Drop for Handle {
    fn drop(&mut self) {
        *self.1.lock().unwrap() += 1;
    }
}

impl OverlaySurface for Handle {
    fn write_state(&self, write: StateWrite) {
        let StateWrite::Snapshot { rev, state } = write else {
            panic!("the layer writes snapshots")
        };
        self.0.states.lock().unwrap().push((rev, state));
    }
    fn send(&self, message: ToView) {
        let ToView::Reply { id, outcome } = message else {
            panic!("the layer only replies")
        };
        self.0.replies.lock().unwrap().push((id, outcome.is_ok()));
    }
    fn set_input(&self, input: OverlayInput) {
        self.0.inputs.lock().unwrap().push(input);
    }
    fn set_focus(&self, focused: bool) {
        self.0.focus.lock().unwrap().push(focused);
    }
    fn update_environment(&self, update: serde_json::Value) {
        self.0.environment.lock().unwrap().push(update);
    }
    fn automation(
        &self,
        request: serde_json::Value,
        reply: lingxia_view::platform::OverlayAutomationReply,
    ) {
        reply(Ok(serde_json::json!({ "asked": request })));
    }
}

#[derive(Default)]
struct Overlays {
    /// appid → window; others show in the main window.
    placement: Mutex<HashMap<String, WindowId>>,
    opened: Mutex<Vec<(WindowId, Arc<Surface>, OverlayLogic)>>,
    closed: Arc<Mutex<u32>>,
    timers: Mutex<Vec<(Duration, Task)>>,
}

impl WindowOverlays for Overlays {
    fn window_of(&self, appid: Option<&str>) -> Option<WindowId> {
        let placement = self.placement.lock().unwrap();
        Some(
            appid
                .and_then(|appid| placement.get(appid).copied())
                .unwrap_or(MAIN),
        )
    }
    fn open(
        &self,
        window: WindowId,
        module: &'static str,
        logic: OverlayLogic,
    ) -> Result<Box<dyn OverlaySurface>, String> {
        assert!(std::ptr::eq(module, PAGE));
        let surface = Arc::new(Surface::default());
        self.opened
            .lock()
            .unwrap()
            .push((window, surface.clone(), logic));
        Ok(Box::new(Handle(surface, self.closed.clone())))
    }
    fn after(&self, delay: Duration, task: Task) {
        self.timers.lock().unwrap().push((delay, task));
    }
}

struct Fixture {
    overlays: Arc<Overlays>,
    feedback: Arc<Feedback>,
}

impl Fixture {
    fn new() -> Self {
        let overlays = Arc::new(Overlays::default());
        let feedback = Feedback::new(overlays.clone());
        Self { overlays, feedback }
    }
    fn surface(&self, index: usize) -> Arc<Surface> {
        self.overlays.opened.lock().unwrap()[index].1.clone()
    }
    fn opened(&self) -> usize {
        self.overlays.opened.lock().unwrap().len()
    }
    fn closed(&self) -> u32 {
        *self.overlays.closed.lock().unwrap()
    }
    fn data(&self, index: usize) -> Value {
        self.surface(index)
            .states
            .lock()
            .unwrap()
            .last()
            .unwrap()
            .1
            .clone()
    }
    /// Runs the timers set so far whose delay is `delay`.
    fn fire(&self, delay: Duration) {
        let due: Vec<_> = {
            let mut timers = self.overlays.timers.lock().unwrap();
            let (due, kept) = std::mem::take(&mut *timers)
                .into_iter()
                .partition(|(at, _)| *at == delay);
            *timers = kept;
            due
        };
        for (_, task) in due {
            task();
        }
    }
    fn page(&self, index: usize, messages: Vec<ToLogic>) {
        let logic = self.overlays.opened.lock().unwrap()[index].2.clone();
        logic(messages);
    }
    fn answer(&self, index: usize, key: u64, choice: Value) {
        self.page(
            index,
            vec![ToLogic::Call {
                id: format!("call-{key}"),
                method: "answer".into(),
                params: Some(json!({ "key": key, "choice": choice })),
                sent_us: 0,
            }],
        );
    }
}

fn toast(title: &str, duration: Option<u64>) -> ToastRequest {
    ToastRequest {
        title: title.into(),
        tone: Tone::Success,
        loading: false,
        position: Position::Bottom,
        duration: duration.map(Duration::from_millis),
    }
}

fn confirm(title: &str) -> DialogRequest {
    DialogRequest::Confirm {
        title: title.into(),
        message: String::new(),
        confirm: None,
        cancel: None,
        destructive: false,
    }
}

fn alert(title: &str) -> DialogRequest {
    DialogRequest::Alert {
        title: title.into(),
        message: String::new(),
        confirm: None,
        destructive: false,
    }
}

/// Automation reaches the layer over the app's window while it is open, and nothing after.
#[cfg(feature = "automation")]
#[test]
fn automation_reaches_the_layer_over_the_apps_window_while_it_is_open() {
    let f = Fixture::new();
    assert!(f.feedback.automation("app.a").is_none());
    f.feedback
        .toast("app.a", toast("Saved", Some(1500)))
        .unwrap();
    let layer = f.feedback.automation("app.a").expect("the open layer");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let tree = runtime.block_on(layer.tree()).unwrap();
    assert_eq!(tree, json!({ "asked": { "op": "tree" } }));
    f.fire(Duration::from_millis(1500));
    f.fire(CLOSE_AFTER);
    assert!(f.feedback.automation("app.a").is_none());
    assert!(
        runtime.block_on(layer.tree()).is_err(),
        "the closed layer is not active"
    );
}

#[test]
fn a_toast_opens_the_layer_passing_input_through_and_closes_it_after_its_time() {
    let f = Fixture::new();
    f.feedback
        .toast("app.a", toast("Saved", Some(1500)))
        .unwrap();
    assert_eq!(f.opened(), 1);
    let surface = f.surface(0);
    assert_eq!(
        f.data(0),
        json!({"toast": {"key": 1, "title": "Saved", "tone": "success", "loading": false, "position": "bottom"}, "dialog": null})
    );
    assert_eq!(*surface.inputs.lock().unwrap(), [OverlayInput::PassThrough]);
    assert_eq!(*surface.focus.lock().unwrap(), [false]);

    f.fire(Duration::from_millis(1500));
    assert_eq!(f.data(0)["toast"], Value::Null);
    assert_eq!(f.closed(), 0, "the toast leaves before the surface does");
    f.fire(CLOSE_AFTER);
    assert_eq!(f.closed(), 1);

    f.feedback.toast("app.a", toast("Again", None)).unwrap();
    assert_eq!(f.opened(), 2, "the next item opens a fresh layer");
}

#[test]
fn the_layer_takes_the_products_contrast_when_it_opens_and_when_it_changes() {
    let f = Fixture::new();
    f.feedback.toast("app.a", toast("Saved", None)).unwrap();
    let surface = f.surface(0);
    let pushed = |surface: &Surface| surface.environment.lock().unwrap().clone();
    let first = pushed(&surface);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0]["kind"], "contrast");
    f.feedback.contrast_changed();
    assert_eq!(pushed(&surface).len(), 2, "an open layer takes the change");
}

#[test]
fn a_new_toast_replaces_the_old_and_the_old_timer_leaves_it_alone() {
    let f = Fixture::new();
    f.feedback.toast("app.a", toast("1/2", Some(1000))).unwrap();
    f.feedback.toast("app.a", toast("2/2", Some(2000))).unwrap();
    assert_eq!(f.opened(), 1);
    f.fire(Duration::from_millis(1000));
    assert_eq!(f.data(0)["toast"]["title"], "2/2");
    f.feedback.hide_toast("app.b");
    assert_eq!(
        f.data(0)["toast"]["title"],
        "2/2",
        "another lxapp's hideToast"
    );
    f.feedback.hide_toast("app.a");
    assert_eq!(f.data(0)["toast"], Value::Null);
}

#[test]
fn dialogs_show_one_at_a_time_and_take_input_and_focus_while_shown() {
    let f = Fixture::new();
    let mut first = f
        .feedback
        .dialog("app.a", "en-US", confirm("Delete?"))
        .unwrap();
    let mut second = f.feedback.dialog("app.a", "en-US", alert("Done")).unwrap();
    let surface = f.surface(0);
    assert_eq!(f.data(0)["dialog"]["title"], "Delete?");
    assert_eq!(
        surface.inputs.lock().unwrap().last(),
        Some(&OverlayInput::Block)
    );
    assert_eq!(surface.focus.lock().unwrap().last(), Some(&true));

    f.answer(0, 2, json!("confirm"));
    assert_eq!(
        f.data(0)["dialog"]["key"],
        1,
        "only the dialog shown answers"
    );
    assert_eq!(first.try_recv(), Err(TryRecvError::Empty));

    f.answer(0, 1, json!("confirm"));
    assert_eq!(first.try_recv(), Ok(Answer::Confirmed));
    assert_eq!(f.data(0)["dialog"]["kind"], "alert");
    assert_eq!(f.data(0)["dialog"]["confirm"], "OK");

    f.answer(0, 2, json!("cancel"));
    assert_eq!(
        second.try_recv(),
        Ok(Answer::Confirmed),
        "an alert has one answer"
    );
    assert_eq!(
        surface.inputs.lock().unwrap().last(),
        Some(&OverlayInput::PassThrough)
    );
    assert_eq!(surface.focus.lock().unwrap().last(), Some(&false));
    assert_eq!(
        *surface.replies.lock().unwrap(),
        [
            (String::from("call-2"), true),
            (String::from("call-1"), true),
            (String::from("call-2"), true)
        ]
    );
    f.fire(CLOSE_AFTER);
    assert_eq!(f.closed(), 1);
}

#[test]
fn an_action_sheet_answers_the_item_chosen_or_cancelled() {
    let f = Fixture::new();
    let sheet = |items: &[&str]| DialogRequest::Sheet {
        items: items.iter().map(|item| item.to_string()).collect(),
        destructive: false,
    };
    let mut chosen = f
        .feedback
        .dialog("app.a", "en-US", sheet(&["Copy", "Share"]))
        .unwrap();
    let mut cancelled = f
        .feedback
        .dialog("app.a", "en-US", sheet(&["Copy"]))
        .unwrap();
    let mut outside = f
        .feedback
        .dialog("app.a", "en-US", sheet(&["Copy"]))
        .unwrap();
    assert_eq!(f.data(0)["dialog"]["label"], "Actions");
    f.answer(0, 1, json!(1));
    f.answer(0, 2, json!("cancel"));
    f.answer(0, 3, json!(5));
    assert_eq!(chosen.try_recv(), Ok(Answer::Chose(1)));
    assert_eq!(cancelled.try_recv(), Ok(Answer::Cancelled));
    assert_eq!(outside.try_recv(), Ok(Answer::Cancelled));
}

#[test]
fn a_malformed_answer_is_refused_and_changes_nothing() {
    let f = Fixture::new();
    let mut shown = f.feedback.dialog("app.a", "en-US", confirm("Go?")).unwrap();
    let writes = f.surface(0).states.lock().unwrap().len();
    f.page(
        0,
        vec![ToLogic::Call {
            id: "bad".into(),
            method: "answer".into(),
            params: Some(json!({"key": 1})),
            sent_us: 0,
        }],
    );
    assert_eq!(
        *f.surface(0).replies.lock().unwrap(),
        [(String::from("bad"), false)]
    );
    assert_eq!(f.surface(0).states.lock().unwrap().len(), writes);
    assert_eq!(shown.try_recv(), Err(TryRecvError::Empty));
}

#[test]
fn default_words_follow_each_callers_page_language() {
    let f = Fixture::new();
    let _zh = f
        .feedback
        .dialog("app.zh", "zh-CN", confirm("删除?"))
        .unwrap();
    let _en = f
        .feedback
        .dialog("app.en", "en-US", confirm("Delete?"))
        .unwrap();
    assert_eq!(f.data(0)["dialog"]["cancel"], "取消");
    assert_eq!(f.data(0)["dialog"]["confirm"], "确定");
    f.answer(0, 1, json!("cancel"));
    assert_eq!(f.data(0)["dialog"]["cancel"], "Cancel");
    assert_eq!(f.data(0)["dialog"]["confirm"], "Confirm");
}

#[test]
fn dismiss_settles_only_the_callers_dialogs_shown_or_queued() {
    let f = Fixture::new();
    let mut a1 = f.feedback.dialog("app.a", "en-US", confirm("a1")).unwrap();
    let mut b1 = f.feedback.dialog("app.b", "en-US", confirm("b1")).unwrap();
    let mut a2 = f.feedback.dialog("app.a", "en-US", alert("a2")).unwrap();
    f.feedback.dismiss("app.a");
    assert_eq!(a1.try_recv(), Ok(Answer::Cancelled));
    assert_eq!(
        a2.try_recv(),
        Ok(Answer::Confirmed),
        "an alert has one answer"
    );
    assert_eq!(b1.try_recv(), Err(TryRecvError::Empty));
    assert_eq!(f.data(0)["dialog"]["title"], "b1");
}

#[test]
fn a_closed_lxapp_takes_its_toast_and_dialogs_with_it() {
    let f = Fixture::new();
    f.feedback.toast("app.a", toast("a", None)).unwrap();
    let mut a = f.feedback.dialog("app.a", "en-US", confirm("a")).unwrap();
    f.feedback.forget("app.a");
    assert_eq!(a.try_recv(), Ok(Answer::Cancelled));
    assert_eq!(f.data(0), json!({"toast": null, "dialog": null}));
    f.fire(CLOSE_AFTER);
    assert_eq!(f.closed(), 1);
}

#[test]
fn each_window_has_its_own_layer_and_a_closed_window_settles_its_own() {
    let f = Fixture::new();
    f.overlays
        .placement
        .lock()
        .unwrap()
        .insert("app.side".into(), SIDE);
    let mut main = f
        .feedback
        .dialog("app.main", "en-US", confirm("main"))
        .unwrap();
    let mut side = f
        .feedback
        .dialog("app.side", "en-US", confirm("side"))
        .unwrap();
    assert_eq!(f.opened(), 2);
    assert_eq!(f.overlays.opened.lock().unwrap()[1].0, SIDE);
    f.feedback.window_closed(SIDE);
    assert_eq!(side.try_recv(), Ok(Answer::Cancelled));
    assert_eq!(main.try_recv(), Err(TryRecvError::Empty));
    assert_eq!(f.closed(), 1);
}

#[test]
fn a_reconnected_page_gets_the_layer_again() {
    let f = Fixture::new();
    f.feedback.toast("app.a", toast("Saved", None)).unwrap();
    let before = f.surface(0).states.lock().unwrap().len();
    f.page(0, vec![ToLogic::Connected]);
    let surface = f.surface(0);
    let states = surface.states.lock().unwrap();
    assert_eq!(states.len(), before + 1);
    assert!(states[before].0 > states[before - 1].0, "a newer revision");
    assert_eq!(states[before].1, states[before - 1].1);
}

#[test]
fn a_layer_that_got_a_new_item_while_closing_stays_open() {
    let f = Fixture::new();
    f.feedback.toast("app.a", toast("one", Some(100))).unwrap();
    f.fire(Duration::from_millis(100));
    f.feedback.toast("app.a", toast("two", None)).unwrap();
    f.fire(CLOSE_AFTER);
    assert_eq!(f.closed(), 0);
    assert_eq!(f.data(0)["toast"]["title"], "two");
}

fn snapshot(state: Value) -> StateWrite {
    StateWrite::Snapshot { rev: 1, state }
}

/// The built page draws the layer's data and answers through `answer`.
#[test]
fn the_built_page_draws_a_dialog_and_answers_its_buttons() {
    use lingxia_view::ViewSession;
    let started = Instant::now();
    let mut page = ViewSession::new_with_logic(PAGE, true).unwrap();
    page.begin_bridge().unwrap();
    page.write_state(snapshot(json!({
        "toast": {"key": 1, "title": "Saved", "tone": "success", "loading": false, "position": "bottom"},
        "dialog": null,
    })))
    .unwrap();
    let frame = page.layout(390, 844, 3.0).unwrap();
    // A debug-build proxy for the first-toast budget; the phone number comes from the device.
    eprintln!(
        "feedback page: first toast frame in {:?}",
        started.elapsed()
    );
    let toast = frame.nodes.iter().find(|node| node.test_id == "lx-toast");
    assert!(toast.is_some(), "the toast is drawn");
    // Over a window, only the toast covers it: the corner shows the window beneath.
    let mut clear = frame.clone();
    clear.set_see_through();
    let pixels = clear.rasterize().unwrap().pixels;
    assert_eq!(
        pixels.data_as_u8_slice()[..4],
        [0, 0, 0, 0],
        "the corner is clear"
    );
    assert!(
        frame
            .nodes
            .iter()
            .any(|node| node.live_region.as_deref() == Some("polite"))
    );

    let mut page = ViewSession::new_with_logic(PAGE, true).unwrap();
    page.begin_bridge().unwrap();
    page.write_state(snapshot(json!({
        "toast": null,
        "dialog": {"key": 7, "kind": "confirm", "title": "删除?", "message": "", "confirm": "确定", "cancel": "取消", "destructive": false},
    })))
    .unwrap();
    let frame = page.layout(390, 844, 3.0).unwrap();
    let texts: Vec<_> = frame.nodes.iter().map(|node| node.text.as_str()).collect();
    assert!(
        texts.contains(&"取消") && texts.contains(&"确定"),
        "{texts:?}"
    );
    let confirm = frame
        .nodes
        .iter()
        .find(|node| node.test_id == "lx-modal-confirm")
        .and_then(|node| node.on_click)
        .expect("the confirm button is clickable");
    assert!(page.click(page.generation(), confirm).unwrap());
    let calls = page.take_action_requests().unwrap();
    let answer = calls.iter().find_map(|call| match call {
        ToLogic::Call { method, params, .. } if method == "answer" => params.clone(),
        _ => None,
    });
    assert_eq!(answer, Some(json!({"key": 7, "choice": "confirm"})));
}
