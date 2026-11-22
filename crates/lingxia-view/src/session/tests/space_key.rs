//! Space presses controls as before, but stays text in an editor and leaves a scroll alone.
use super::*;
use crate::interaction::view::Key;

const PAGE: &str = "function render(){const c=useLocal('c',false),s=useLocal('s',false),q=useLocal('q',''),notes=useLocal('notes',''),log=useLocal('log','');const say=t=>()=>log.set(log.value+t+' ');return h('page',{},h('checkbox',{'data-testid':'c','aria-label':'Opt in',value:c.value,onChange(v){c.set(v);say('c:'+v)()}}),h('switch',{'data-testid':'s','aria-label':'Wi-Fi',value:s.value,onChange(v){s.set(v);say('s:'+v)()}}),h('button',{'data-testid':'b',onClick:say('press')},'Press'),h('scroll',{'data-testid':'sc',style:{height:60}},h('button',{'data-testid':'inner',onClick:say('inner')},'Inner'),h('view',{style:{height:400}})),h('input',{'data-testid':'q',bind:q,'aria-label':'Search',onSubmit:say('submit')}),h('textarea',{'data-testid':'notes',bind:notes,'aria-label':'Notes'}),h('text',{'data-testid':'log'},log.value||'-'))}";

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

    fn query(&self, id: &str) -> serde_json::Value {
        self.host
            .automation_request(
                &serde_json::json!({"op": "query", "css": format!("[data-testid=\"{id}\"]")}),
            )
            .unwrap()
    }

    fn log(&self) -> String {
        self.query("log")["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    }

    /// Presses `key`, then lets a second of frames pass so a switch's thumb settles.
    fn key(&self, key: Key) -> bool {
        let taken = self.host.key(key, false, false);
        run_frames(&self.platform, &self.host, 60);
        taken
    }

    fn ask(&self, request: serde_json::Value) -> serde_json::Value {
        let (tx, rx) = channel();
        self.host.input(request.to_string(), move |reply| {
            let _ = tx.send(reply);
        });
        self.platform.settle(&[&self.host]);
        let reply = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        serde_json::from_str(&reply.unwrap_or_else(|| panic!("refused: {request}"))).unwrap()
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
}

#[test]
fn space_toggles_a_checkbox_and_a_switch_and_presses_a_button() {
    let bench = Bench::new("test-space-controls");
    bench.key(Key::Tab);
    assert_eq!(bench.query("c")["focused"], true);
    assert!(bench.key(Key::Space));
    bench.key(Key::Tab);
    assert_eq!(bench.query("s")["focused"], true);
    assert!(bench.key(Key::Space));
    bench.key(Key::Tab);
    assert!(bench.key(Key::Space));
    assert_eq!(bench.log(), "c:true s:true press ");
}

#[test]
fn space_in_a_scroll_presses_the_focused_control_and_does_not_scroll() {
    let bench = Bench::new("test-space-scroll");
    for _ in 0..4 {
        bench.key(Key::Tab);
    }
    assert_eq!(bench.query("inner")["focused"], true);
    let before = bench.query("inner")["bounds"].clone();
    assert!(bench.key(Key::Space));
    assert_eq!(bench.log(), "inner ");
    assert_eq!(
        bench.query("inner")["bounds"],
        before,
        "Space did not page the scroll"
    );
    assert!(bench.key(Key::Activate));
    assert_eq!(bench.log(), "inner inner ");
    assert_eq!(bench.query("inner")["bounds"], before);
}

#[test]
fn space_in_an_input_or_textarea_is_text_not_a_press_or_a_submit() {
    let bench = Bench::new("test-space-editors");
    for id in ["q", "notes"] {
        let (identity, token) = bench.focus(id);
        assert!(
            !bench.key(Key::Space),
            "{id}: the page leaves Space to the editor"
        );
        assert!(
            !bench.host.editor_key(Key::Space),
            "{id}: no editor map takes Space"
        );
        bench.ask(
            serde_json::json!({"op":"edit","identity":identity,"token":token,"submit":false,
            "edits":[{"op":"replace","text":"a b","cursor":3,"composing":false}]}),
        );
        bench.settle();
        assert_eq!(bench.query(id)["value"], "a b", "{id}");
    }
    assert_eq!(bench.log(), "-", "nothing submitted or pressed");
}
