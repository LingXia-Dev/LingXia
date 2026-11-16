//! A tree's branches from the keyboard: the forward arrow opens one, then enters it; the back
//! arrow leaves it, then closes it.
use super::*;
use crate::interaction::view::Key;

/// "Fruit" holds "Apple" and starts closed; "Nuts" is a leaf after it.
fn page(platform: &Platform, worker: &Arc<ViewWorker>, dir: &str) -> ViewHost<Fake> {
    let host = platform.host(
        worker,
        &source(&format!(
            "function render(){{const open=useLocal('open',false);return h('page',{{dir:'{dir}'}},h('view',{{role:'tree','aria-label':'Food'}},h('button',{{role:'treeitem','aria-level':1,'aria-expanded':open.value,'data-testid':'fruit',onClick(){{open.set(!open.value)}}}},'Fruit'),open.value&&h('button',{{role:'treeitem','aria-level':2,'data-testid':'apple',onClick(){{}}}},'Apple'),h('button',{{role:'treeitem','aria-level':1,'data-testid':'nuts',onClick(){{}}}},'Nuts')))}}"
        )),
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

fn key(platform: &Platform, host: &ViewHost<Fake>, key: Key) {
    assert!(host.key(key, false, false), "{key:?} is taken");
    platform.settle(&[host]);
    run_frames(platform, host, 2);
}

#[test]
fn arrows_open_enter_leave_and_close_a_branch_and_swap_right_to_left() {
    for (dir, forward, back) in [
        ("ltr", Key::Right, Key::Left),
        ("rtl", Key::Left, Key::Right),
    ] {
        let worker = ViewWorker::spawn("test-tree-keys").unwrap();
        let platform = Platform::new(&[&worker]);
        let host = page(&platform, &worker, dir);
        key(&platform, &host, Key::Tab);
        assert!(focused(&host, "fruit"), "{dir}");
        key(&platform, &host, back);
        assert!(
            !exists(&host, "apple"),
            "{dir}: back on a closed branch opens nothing"
        );
        key(&platform, &host, forward);
        assert!(exists(&host, "apple"), "{dir}: forward opens the branch");
        assert!(focused(&host, "fruit"), "{dir}: and focus stays on it");
        key(&platform, &host, forward);
        assert!(focused(&host, "apple"), "{dir}: forward again enters it");
        key(&platform, &host, forward);
        assert!(focused(&host, "apple"), "{dir}: a leaf stays");
        key(&platform, &host, back);
        assert!(focused(&host, "fruit"), "{dir}: back goes to the parent");
        assert!(exists(&host, "apple"));
        key(&platform, &host, back);
        assert!(
            !exists(&host, "apple"),
            "{dir}: back on the open branch closes it"
        );
        assert!(focused(&host, "fruit"), "{dir}");
    }
}

#[test]
fn a_control_is_named_by_its_content_less_what_is_hidden_from_assistive_tech() {
    let worker = ViewWorker::spawn("test-tree-names").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){return h('page',{},h('view',{role:'tree','aria-label':'Files'},h('button',{role:'treeitem','aria-level':1,'aria-expanded':false,'data-testid':'docs',onClick(){}},h('text',{'aria-hidden':true},'>'),h('view',{'aria-hidden':true},h('text',{},'folder')),h('text',{},'Documents')),h('button',{role:'treeitem','aria-level':1,'data-testid':'notes','aria-label':'Notes, 3',onClick(){}},h('text',{},'Notes'),h('text',{},'3'))),h('button',{'data-testid':'save',onClick(){}},h('view',{style:{width:8,height:8}}),h('text',{},'Save')))}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let named = |css: &str| {
        host.automation_request(&serde_json::json!({"op": "query", "css": css}))
            .unwrap()["attributes"]["data-testid"]
            .as_str()
            .map(str::to_owned)
    };
    assert_eq!(
        named("button[aria-label=\"Save\"]").as_deref(),
        Some("save")
    );
    assert_eq!(
        named("[role=\"treeitem\"][aria-label=\"Documents\"]").as_deref(),
        Some("docs"),
        "its content names it"
    );
    assert_eq!(
        named("[role=\"treeitem\"][aria-label=\"Notes, 3\"]").as_deref(),
        Some("notes"),
        "a label of its own wins"
    );
}
