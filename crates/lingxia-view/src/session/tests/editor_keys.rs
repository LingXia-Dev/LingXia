//! `onKeyDown` on an editor: the keys its map lists go to the page while the caret stays.
use super::*;
use crate::interaction::view::Key;

const PAGE: &str = "function render(){const q=useLocal('q',''),open=useLocal('open',true),log=useLocal('log','');const say=s=>()=>log.set(log.value+s+' ');return h('page',{},h('input',{'data-testid':'q',bind:q,'aria-label':'Search',onSubmit:say('submit'),onKeyDown:{ArrowDown:say('down'),Enter:say('enter'),PageDown:say('page')}}),h('input',{'data-testid':'plain',value:'','aria-label':'Plain'}),h('view',{id:'list',popover:'auto',open:open.value,onToggle(next){open.set(next)},'data-testid':'list'},h('text',{},'List')),h('text',{'data-testid':'log'},log.value||'-'))}";

struct Bench {
    platform: Platform,
    host: ViewHost<Fake>,
}

impl Bench {
    fn new(name: &'static str) -> Self {
        let worker = ViewWorker::spawn(name).unwrap();
        let platform = Platform::new(&[&worker]);
        let host = platform.host(&worker, &source(PAGE), false);
        host.set_visible(true);
        platform.settle(&[&host]);
        Self { platform, host }
    }

    fn settle(&self) {
        self.platform.settle(&[&self.host]);
        self.host.request_frame();
        self.platform.settle(&[&self.host]);
        run_frames(&self.platform, &self.host, 2);
    }

    fn ask(&self, request: serde_json::Value) -> serde_json::Value {
        let (tx, rx) = channel();
        self.host.input(request.to_string(), move |reply| {
            let _ = tx.send(reply);
        });
        self.platform.settle(&[&self.host]);
        serde_json::from_str(&rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap()).unwrap()
    }

    /// Focuses the editor `id` as a platform editor does; its identity and token.
    fn focus(&self, id: &str) -> (serde_json::Value, serde_json::Value) {
        let snapshot: serde_json::Value =
            serde_json::from_str(self.host.snapshot().unwrap().json()).unwrap();
        let identity = snapshot["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["test_id"] == id)
            .unwrap()["identity"]
            .clone();
        let token = self.ask(serde_json::json!({"op":"focus","identity":identity,"generation":snapshot["generation"]}))["token"].clone();
        self.settle();
        (identity, token)
    }

    fn key(&self, key: Key) -> bool {
        let taken = self.host.editor_key(key);
        self.settle();
        taken
    }

    fn text(&self, id: &str) -> String {
        self.host
            .automation_request(
                &serde_json::json!({"op": "query", "css": format!("[data-testid=\"{id}\"]")}),
            )
            .unwrap()["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    }

    fn list_open(&self) -> bool {
        self.host
            .automation_request(
                &serde_json::json!({"op": "query", "css": "[data-testid=\"list\"]", "all": true}),
            )
            .unwrap()["count"]
            == 1
    }
}

#[test]
fn a_focused_editor_gives_the_page_the_keys_its_map_lists_and_keeps_the_rest() {
    let bench = Bench::new("test-editor-keys");
    assert!(!bench.key(Key::Down), "no editor has focus");
    bench.focus("q");
    assert!(bench.key(Key::Down));
    assert!(bench.key(Key::PageDown));
    assert!(
        bench.key(Key::Activate),
        "a listed Enter is the page's, not a submit"
    );
    assert_eq!(bench.text("log"), "down page enter ");
    assert!(!bench.key(Key::Up), "not listed: the editor keeps it");
    assert!(!bench.key(Key::Left));
    assert_eq!(bench.text("log"), "down page enter ");
    // Escape is not in the map: it closes the topmost overlay, then is the editor's again.
    assert!(bench.list_open());
    assert!(bench.key(Key::Escape));
    assert!(!bench.list_open(), "the popover closed");
    assert!(!bench.key(Key::Escape), "nothing left to close");
}

#[test]
fn a_composing_input_method_keeps_every_key_and_an_editor_without_a_map_keeps_them_all() {
    let bench = Bench::new("test-editor-keys-ime");
    let (identity, token) = bench.focus("q");
    bench.ask(
        serde_json::json!({"op":"edit","identity":identity,"token":token,"submit":false,
        "edits":[{"op":"replace","text":"ni","cursor":1,"composing":true}]}),
    );
    bench.settle();
    assert!(
        !bench.key(Key::Down),
        "the candidate window owns the arrows"
    );
    assert!(!bench.key(Key::Activate));
    bench.ask(
        serde_json::json!({"op":"edit","identity":identity,"token":token,"submit":false,
        "edits":[{"op":"finish_composition"}]}),
    );
    bench.settle();
    assert!(
        bench.key(Key::Down),
        "composition over: the page takes it again"
    );
    assert_eq!(bench.text("log"), "down ");
    bench.focus("plain");
    assert!(!bench.key(Key::Down));
    assert!(!bench.key(Key::Activate));
}

#[test]
fn automation_presses_a_listed_key_on_an_editor_and_refuses_one_that_is_not() {
    let bench = Bench::new("test-editor-keys-automation");
    let press = |key: &str| {
        let answer = bench.host.automation_request(&serde_json::json!({"op":"act","css":"[data-testid=\"q\"]","action":{"kind":"press","key":key}}));
        bench.settle();
        answer
    };
    press("ArrowDown").unwrap();
    press("Enter").unwrap();
    assert_eq!(bench.text("log"), "down enter ");
    assert!(press("Escape").is_ok(), "Escape closes the open popover");
    assert!(!bench.list_open());
    let refused = press("ArrowLeft").unwrap_err();
    assert!(refused.starts_with("E_UNSUPPORTED"), "{refused}");
}
