//! Leaving a page inside the app closes what a tap outside would; dialogs and manual popovers stay.
use super::*;
use crate::interaction::view::Key;

const PAGE: &str = "function render(){const a=useLocal('a',''),log=useLocal('log','');return h('page',{},h('input',{'data-testid':'a',bind:a,'aria-label':'A',onBlur(){log.set(log.value+'a- ')}}),h('button',{popoverTarget:'menu','data-testid':'more'},'More'),h('text',{'data-testid':'log'},log.value||'-'),h('view',{id:'menu',popover:'auto','data-testid':'menu',style:{width:80}},h('text',{},'Menu')),h('view',{id:'pinned',popover:'manual',open:true,'data-testid':'pinned',style:{width:80}},h('text',{},'Pinned')))}";

#[test]
fn a_left_page_closes_its_light_dismiss_layers_and_blurs_its_editor_but_keeps_manual_ones() {
    let worker = ViewWorker::spawn("test-page-left").unwrap();
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

    // Open the menu from its button.
    host.key(Key::Tab, false, false);
    settle();
    host.key(Key::Tab, false, false);
    settle();
    host.key(Key::Activate, false, false);
    settle();
    assert_eq!(query("menu")["exists"], true);
    assert_eq!(query("pinned")["exists"], true);

    // Focus the editor; the menu stays open over it.
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
    assert_eq!(query("a")["focused"], true);

    host.page_left();
    settle();
    assert_eq!(
        query("menu")["exists"],
        false,
        "a light-dismiss popover closes"
    );
    assert_eq!(query("pinned")["exists"], true, "a manual popover stays");
    assert_eq!(query("a")["focused"], false);
    assert_eq!(query("log")["text"], "a- ", "the editor's own blur runs");
}
