//! A block link: a card that navigates, holding a button that does not.
use super::*;
use crate::interaction::view::Key;

fn page(platform: &Platform, worker: &Arc<ViewWorker>) -> ViewHost<Fake> {
    let host = platform.host(
        worker,
        &source(
            "function render(){const log=useLocal('log','');return h('page',{},h('link',{'data-testid':'card','aria-label':'Open card',onClick(){log.set(log.value+'card ')},style:{padding:12,gap:8}},h('text',{'data-testid':'title'},'Title'),h('button',{'data-testid':'like',onClick(){log.set(log.value+'like ')}},'Like')),h('text',{'data-testid':'log'},log.value||'-'))}",
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

fn click(platform: &Platform, host: &ViewHost<Fake>, id: &str) {
    host.automation_request(&serde_json::json!({"op": "act", "css": format!("[data-testid=\"{id}\"]"), "action": {"kind": "click"}}))
        .unwrap();
    platform.settle(&[host]);
    run_frames(platform, host, 2);
}

fn key(platform: &Platform, host: &ViewHost<Fake>, key: Key) -> bool {
    let taken = host.key(key, false, false);
    platform.settle(&[host]);
    run_frames(platform, host, 2);
    taken
}

fn log(host: &ViewHost<Fake>) -> String {
    query(host, "log")["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn the_innermost_control_takes_the_press_and_the_card_takes_the_rest() {
    let worker = ViewWorker::spawn("test-block-link").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(&platform, &worker);
    assert_eq!(query(&host, "card")["role"], "link");
    click(&platform, &host, "like");
    assert_eq!(log(&host), "like ");
    click(&platform, &host, "title");
    assert_eq!(log(&host), "like card ");
}

#[test]
fn enter_opens_a_block_link_and_space_leaves_it_to_the_platform() {
    let worker = ViewWorker::spawn("test-block-link").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(&platform, &worker);
    key(&platform, &host, Key::Tab);
    assert_eq!(query(&host, "card")["focused"], true);
    assert!(
        !key(&platform, &host, Key::Space),
        "Space on a link is not consumed"
    );
    assert_eq!(log(&host), "-");
    assert!(key(&platform, &host, Key::Activate));
    assert_eq!(log(&host), "card ");
    key(&platform, &host, Key::Tab);
    assert_eq!(query(&host, "like")["focused"], true);
    assert!(key(&platform, &host, Key::Space), "Space presses a button");
    assert_eq!(log(&host), "card like ");
}
