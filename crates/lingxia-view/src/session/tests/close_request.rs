//! A menu item that keeps its menu open, and a popover that hears a close request and may refuse it.
use super::*;
use crate::interaction::view::Key;

fn page(platform: &Platform, worker: &Arc<ViewWorker>) -> ViewHost<Fake> {
    let host = platform.host(
        worker,
        &source(
            "function render(){const log=useLocal('log','');return h('page',{},h('button',{popoverTarget:'menu','data-testid':'more'},'More'),h('text',{'data-testid':'log'},log.value||'-'),h('view',{id:'menu',popover:'auto',role:'menu','aria-label':'View','data-testid':'menu',style:{width:80},onCloseRequest(){const asked=log.value.includes('ask');log.set(log.value+'ask ');return asked}},h('button',{role:'menuitem',keepOpen:true,'data-testid':'zoom',onClick(){log.set(log.value+'zoom ')}},'Zoom in'),h('button',{role:'menuitem','data-testid':'reset'},'Reset')))}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    host
}

fn query(host: &ViewHost<Fake>, id: &str) -> serde_json::Value {
    host.automation_request(
        &serde_json::json!({"op": "query", "css": format!("[data-testid=\"{id}\"]")}),
    )
    .unwrap()
}

fn exists(host: &ViewHost<Fake>, id: &str) -> bool {
    query(host, id)["exists"].as_bool().unwrap()
}

fn focused(host: &ViewHost<Fake>, id: &str) -> bool {
    query(host, id)["focused"].as_bool().unwrap_or(false)
}

fn key(platform: &Platform, host: &ViewHost<Fake>, key: Key) -> bool {
    let taken = host.key(key, false, false);
    platform.settle(&[host]);
    run_frames(platform, host, 2);
    taken
}

fn open_menu(platform: &Platform, host: &ViewHost<Fake>) {
    key(platform, host, Key::Tab);
    key(platform, host, Key::Activate);
    assert!(focused(host, "zoom"));
}

#[test]
fn a_keep_open_item_acts_and_leaves_the_menu_and_focus_where_they_are() {
    let worker = ViewWorker::spawn("test-keep-open").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(&platform, &worker);
    open_menu(&platform, &host);
    key(&platform, &host, Key::Activate);
    key(&platform, &host, Key::Activate);
    assert_eq!(query(&host, "log")["text"], "zoom zoom ");
    assert!(exists(&host, "menu"));
    assert!(focused(&host, "zoom"), "focus stays on the item");
    key(&platform, &host, Key::Down);
    key(&platform, &host, Key::Activate);
    assert!(!exists(&host, "menu"), "a plain item still closes it");
    assert!(focused(&host, "more"));
}

#[test]
fn escape_asks_the_popover_first_and_a_refusal_keeps_it_and_its_focus() {
    let worker = ViewWorker::spawn("test-close-request").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(&platform, &worker);
    open_menu(&platform, &host);
    assert!(
        key(&platform, &host, Key::Escape),
        "a refused request still takes the key"
    );
    assert_eq!(query(&host, "log")["text"], "ask ");
    assert!(exists(&host, "menu"));
    assert!(focused(&host, "zoom"));
    assert!(key(&platform, &host, Key::Escape));
    assert!(!exists(&host, "menu"), "the second request is let through");
    assert!(focused(&host, "more"));
}
