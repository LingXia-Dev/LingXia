//! Automation's `blur` on an editor.
use super::*;

const PAGE: &str = "function render(){const a=useLocal('a',''),b=useLocal('b',''),log=useLocal('log','');const say=s=>()=>log.set(log.value+s+' ');return h('page',{},h('input',{'data-testid':'a',bind:a,'aria-label':'A',onFocus:say('a+'),onBlur:say('a-')}),h('input',{'data-testid':'b',bind:b,'aria-label':'B',onBlur:say('b-')}),h('button',{'data-testid':'go',onClick(){}},'Go'),h('text',{'data-testid':'log'},log.value||'-'))}";

#[test]
fn blur_takes_focus_from_the_editor_that_holds_it_and_leaves_any_other_match_alone() {
    let worker = ViewWorker::spawn("test-automation-blur").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(&worker, &source(PAGE), false);
    host.set_visible(true);
    platform.settle(&[&host]);
    let settle = || {
        platform.settle(&[&host]);
        host.request_frame();
        platform.settle(&[&host]);
        run_frames(&platform, &host, 2);
    };
    let query = |id: &str| {
        host.automation_request(
            &serde_json::json!({"op": "query", "css": format!("[data-testid=\"{id}\"]")}),
        )
        .unwrap()
    };
    let blur = |id: &str| {
        let answer = host.automation_request(&serde_json::json!({"op":"act","css":format!("[data-testid=\"{id}\"]"),"action":{"kind":"blur"}}));
        settle();
        answer
    };
    let snapshot: serde_json::Value =
        serde_json::from_str(host.snapshot().unwrap().json()).unwrap();
    let identity = snapshot["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["test_id"] == "a")
        .unwrap()["identity"]
        .clone();
    let (tx, rx) = channel();
    host.input(
        serde_json::json!({"op":"focus","identity":identity,"generation":snapshot["generation"]})
            .to_string(),
        move |reply| {
            let _ = tx.send(reply);
        },
    );
    platform.settle(&[&host]);
    rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
    settle();
    assert_eq!(query("log")["text"], "a+ ");
    assert_eq!(query("a")["focused"], true);

    // Another editor and a button do not hold focus: nothing changes.
    blur("b").unwrap();
    blur("go").unwrap();
    assert_eq!(query("log")["text"], "a+ ");
    assert_eq!(query("a")["focused"], true);

    blur("a").unwrap();
    assert_eq!(query("log")["text"], "a+ a- ");
    assert_eq!(query("a")["focused"], false);
    // Blurred already: no second blur.
    blur("a").unwrap();
    assert_eq!(query("log")["text"], "a+ a- ");
    assert!(
        blur("missing")
            .unwrap_err()
            .starts_with("Element not found")
    );
}
