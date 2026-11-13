//! An open dialog or popover panel takes every press over it: one that hits none of its
//! controls does nothing, and never reaches its backdrop or the page beneath.
use super::*;

fn page(platform: &Platform, worker: &Arc<ViewWorker>, render: &str) -> ViewHost<Fake> {
    let host = platform.host(worker, &source(render), false);
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

fn text(host: &ViewHost<Fake>, id: &str) -> String {
    query(host, id)["text"].as_str().unwrap().to_owned()
}

/// `id`'s logical rect: left, top, width, height.
fn rect(host: &ViewHost<Fake>, id: &str) -> [f32; 4] {
    let node = query(host, id);
    let r = |key: &str| node["rect"][key].as_f64().unwrap() as f32;
    [r("left"), r("top"), r("width"), r("height")]
}

fn tap(platform: &Platform, host: &ViewHost<Fake>, [x, y]: [f32; 2]) {
    for phase in [PointerPhase::Down, PointerPhase::Up] {
        host.pointer(Pointer {
            phase,
            x: x * 2.0,
            y: y * 2.0,
            slop: 8.0,
            velocity: [0.0; 2],
            min_fling: 50.0,
        });
    }
    platform.settle(&[host]);
}

fn click(platform: &Platform, host: &ViewHost<Fake>, id: &str) {
    host.automation_request(&serde_json::json!({"op": "act", "css": format!("[data-testid=\"{id}\"]"), "action": {"kind": "click"}}))
        .unwrap();
    platform.settle(&[host]);
}

/// A point on `id`'s text, and one in the empty area of its panel `panel` below it.
fn on_text_and_padding(host: &ViewHost<Fake>, id: &str, panel: &str) -> ([f32; 2], [f32; 2]) {
    let [x, y, _, h] = rect(host, id);
    let [_, py, _, ph] = rect(host, panel);
    let below = y + h + (py + ph - y - h) / 2.0;
    assert!(
        below > y + h && below < py + ph,
        "the panel has room under its text"
    );
    ([x + 2.0, y + h / 2.0], [x + 2.0, below])
}

#[test]
fn a_tap_on_a_dialog_panel_text_or_padding_keeps_it_open_and_outside_closes_it() {
    let worker = ViewWorker::spawn("test-panel-press-dialog").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(
        &platform,
        &worker,
        "function render(){const open=useLocal('open',true);return h('page',{},h('text',{'data-testid':'state'},open.value?'open':'shut'),h('dialog',{open:open.value,closedby:'any',onClose:()=>open.set(false),'aria-label':'D','data-testid':'d',style:{width:80,height:80,padding:8}},h('text',{'data-testid':'body'},'Body')))}",
    );
    let (on_text, on_padding) = on_text_and_padding(&host, "body", "d");
    tap(&platform, &host, on_text);
    assert_eq!(text(&host, "state"), "open", "its text");
    tap(&platform, &host, on_padding);
    assert_eq!(text(&host, "state"), "open", "its empty area");
    tap(&platform, &host, [2.0, 196.0]);
    assert_eq!(text(&host, "state"), "shut", "outside: a light dismiss");
}

#[test]
fn a_tap_on_a_popover_panel_text_or_padding_keeps_it_open_and_outside_closes_it() {
    let worker = ViewWorker::spawn("test-panel-press-popover").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(
        &platform,
        &worker,
        "function render(){return h('page',{},h('button',{'data-testid':'open',popoverTarget:'pop'},'Open'),h('view',{id:'pop',popover:'auto','data-testid':'pop',style:{width:80,height:60,padding:8}},h('text',{'data-testid':'body'},'Body')))}",
    );
    click(&platform, &host, "open");
    assert!(exists(&host, "pop"));
    let (on_text, on_padding) = on_text_and_padding(&host, "body", "pop");
    tap(&platform, &host, on_text);
    assert!(exists(&host, "pop"), "its text");
    tap(&platform, &host, on_padding);
    assert!(exists(&host, "pop"), "its empty area");
    tap(&platform, &host, [2.0, 196.0]);
    assert!(!exists(&host, "pop"), "outside: a light dismiss");
}

#[test]
fn a_toast_takes_presses_only_over_its_own_box() {
    let worker = ViewWorker::spawn("test-panel-press-toast").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(
        &platform,
        &worker,
        "function render(){const log=useLocal('log','');return h('page',{},h('button',{'data-testid':'b',onClick:()=>log.set(log.value+'b'),style:{height:40}},'Tap'),h('text',{'data-testid':'log'},log.value||'-'),h('view',{id:'toast',popover:'manual',open:true,'data-testid':'toast',style:{width:60,height:30}},h('text',{},'Saved')))}",
    );
    let [bx, by, bw, bh] = rect(&host, "b");
    let [tx, ty, tw, th] = rect(&host, "toast");
    let clear = [bx + bw / 2.0, by + bh / 2.0];
    assert!(
        !(clear[0] >= tx && clear[0] < tx + tw && clear[1] >= ty && clear[1] < ty + th),
        "the toast leaves the button's centre clear"
    );
    tap(&platform, &host, clear);
    assert_eq!(
        text(&host, "log"),
        "b",
        "a page button beside a toast still presses"
    );
    assert!(exists(&host, "toast"));
}

#[test]
fn a_popover_inside_a_dialog_closes_first_and_the_dialog_panel_stays() {
    let worker = ViewWorker::spawn("test-panel-press-nested").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(
        &platform,
        &worker,
        "function render(){const open=useLocal('open',true);return h('page',{},h('text',{'data-testid':'state'},open.value?'open':'shut'),h('dialog',{open:open.value,closedby:'any',onClose:()=>open.set(false),'aria-label':'D','data-testid':'d',style:{width:90,height:120,padding:4}},h('button',{'data-testid':'more',popoverTarget:'menu'},'More'),h('view',{id:'menu',popover:'auto','data-testid':'menu',style:{width:50,height:30,padding:4}},h('text',{'data-testid':'item'},'Item')),h('text',{'data-testid':'body'},'Body')))}",
    );
    click(&platform, &host, "more");
    assert!(exists(&host, "menu"));
    let [ix, iy, _, ih] = rect(&host, "item");
    tap(&platform, &host, [ix + 2.0, iy + ih / 2.0]);
    assert!(exists(&host, "menu"), "the inner panel's text");
    // Inside the dialog's panel, outside the menu: the menu's light dismiss, not the dialog's.
    let [dx, dy, dw, dh] = rect(&host, "d");
    let [mx, my, mw, mh] = rect(&host, "menu");
    let inside = [dx + dw - 3.0, dy + dh - 3.0];
    assert!(!(inside[0] >= mx && inside[0] < mx + mw && inside[1] >= my && inside[1] < my + mh));
    tap(&platform, &host, inside);
    assert!(!exists(&host, "menu"), "the menu closes");
    assert_eq!(text(&host, "state"), "open", "the dialog stays");
    tap(&platform, &host, inside);
    assert_eq!(
        text(&host, "state"),
        "open",
        "now its own panel keeps the press"
    );
    tap(&platform, &host, [2.0, 196.0]);
    assert_eq!(text(&host, "state"), "shut");
}
