//! A finger press reaches a small control through the platform's minimum touch target; a
//! mouse, automation and the accessibility tree keep to its real box.
use super::*;

const PAGE: &str = "function render(){const log=useLocal('log','');const open=useLocal('open',false);const v=useLocal('v',0);const add=s=>log.set(log.value+s);const gap=n=>h('view',{style:{height:n}});return h('page',{},gap(30),h('button',{'data-testid':'b',onClick:()=>add('b'),style:{width:20,height:20,marginLeft:40}}),gap(30),h('button',{'data-testid':'opt',onClick:()=>add('o'),style:{width:20,height:20,marginLeft:40,touchTarget:'none'}}),gap(30),h('slider',{'data-testid':'s',bind:v,max:100,'aria-label':'Level',style:{width:80,height:16,minHeight:0,marginLeft:10}}),gap(20),h('button',{'data-testid':'show',onClick:()=>open.set(true)},'Show'),h('text',{'data-testid':'log'},log.value||'-'),h('view',{id:'pop',popover:'auto',open:open.value,onToggle:o=>open.set(o),'data-testid':'pop',style:{width:60,height:30}},h('text',{},'Pop')))}";

fn page(platform: &Platform, worker: &Arc<ViewWorker>, os: &str, pointer: &str) -> ViewHost<Fake> {
    let host = platform.host(worker, &source(PAGE), false);
    if let Some(environment) = host.adapter().environment.lock().unwrap().as_mut() {
        environment.value["os"] = os.into();
        environment.value["pointer"] = pointer.into();
    }
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

/// `id`'s logical rect: left, top, width, height.
fn rect(host: &ViewHost<Fake>, id: &str) -> [f32; 4] {
    let node = query(host, id);
    let r = |key: &str| node["rect"][key].as_f64().unwrap() as f32;
    [r("left"), r("top"), r("width"), r("height")]
}

fn log(host: &ViewHost<Fake>) -> String {
    query(host, "log")["text"].as_str().unwrap().to_owned()
}

fn at(phase: PointerPhase, [x, y]: [f32; 2]) -> Pointer {
    Pointer {
        phase,
        x: x * 2.0,
        y: y * 2.0,
        slop: 8.0,
        velocity: [0.0; 2],
        min_fling: 50.0,
    }
}

fn tap_at(platform: &Platform, host: &ViewHost<Fake>, point: [f32; 2]) {
    host.pointer(at(PointerPhase::Down, point));
    host.pointer(at(PointerPhase::Up, point));
    platform.settle(&[host]);
}

/// `by` logical px above `id`'s top edge, at its horizontal centre.
fn above(host: &ViewHost<Fake>, id: &str, by: f32) -> [f32; 2] {
    let [x, y, w, _] = rect(host, id);
    [x + w / 2.0, y - by]
}

#[test]
fn a_finger_presses_a_small_button_from_its_band_and_a_mouse_does_not() {
    let worker = ViewWorker::spawn("test-touch-target").unwrap();
    let platform = Platform::new(&[&worker]);
    let phone = page(&platform, &worker, "ios", "coarse");
    assert_eq!(rect(&phone, "b")[3], 20.0, "the reported box stays real");
    tap_at(&platform, &phone, above(&phone, "b", 10.0));
    assert_eq!(
        log(&phone),
        "b",
        "iOS: 44 pt reaches 12 pt past a 20 pt box"
    );
    tap_at(&platform, &phone, above(&phone, "b", 13.0));
    assert_eq!(log(&phone), "b", "past the band");
    tap_at(&platform, &phone, above(&phone, "opt", 4.0));
    assert_eq!(log(&phone), "b", "touch-target-none keeps its box");

    let android = page(&platform, &worker, "android", "coarse");
    tap_at(&platform, &android, above(&android, "b", 13.0));
    assert_eq!(log(&android), "b", "Android: 48 dp reaches 14 dp past it");

    let mouse = page(&platform, &worker, "ios", "fine");
    tap_at(&platform, &mouse, above(&mouse, "b", 4.0));
    assert_eq!(log(&mouse), "-");
}

#[test]
fn a_slider_drag_starting_in_its_band_moves_it() {
    let worker = ViewWorker::spawn("test-touch-target-slider").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(&platform, &worker, "ios", "coarse");
    let [x, y, w, h] = rect(&host, "s");
    let start = [x + 10.0, y - 10.0];
    host.pointer(at(PointerPhase::Down, start));
    run_frames(&platform, &host, 4);
    host.pointer(at(PointerPhase::Move, [x + w / 2.0, y - 10.0]));
    run_frames(&platform, &host, 4);
    host.pointer(at(PointerPhase::Move, [x + w * 0.75, y - 10.0]));
    run_frames(&platform, &host, 4);
    host.pointer(at(PointerPhase::Up, [x + w * 0.75, y - 10.0]));
    run_frames(&platform, &host, 4);
    platform.settle(&[&host]);
    let value: f64 = query(&host, "s")["value"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!(value > 50.0, "{value}");
    assert_eq!(rect(&host, "s")[3], h);
}

#[test]
fn a_press_in_a_band_outside_an_open_popover_dismisses_it_and_presses_nothing() {
    let worker = ViewWorker::spawn("test-touch-target-dismiss").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(&platform, &worker, "ios", "coarse");
    let point = above(&host, "b", 6.0);
    host.automation_request(&serde_json::json!({"op": "act", "css": "[data-testid=\"show\"]", "action": {"kind": "click"}}))
        .unwrap();
    platform.settle(&[&host]);
    assert_eq!(query(&host, "pop")["exists"], true);
    tap_at(&platform, &host, point);
    assert_eq!(query(&host, "pop")["exists"], false, "a light dismiss");
    assert_eq!(log(&host), "-", "the button under it heard nothing");
}

#[test]
fn a_press_on_a_dialog_panel_stays_in_it_and_one_outside_dismisses_it() {
    let worker = ViewWorker::spawn("test-touch-target-dialog").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(&worker, &source("function render(){const open=useLocal('open',true);const hit=useLocal('hit','');return h('page',{},h('text',{'data-testid':'state'},(open.value?'open':'shut')+hit.value),h('dialog',{open:open.value,closedby:'any',onClose:()=>open.set(false),'aria-label':'D',style:{width:80,height:80}},h('text',{'data-testid':'body'},'Body'),h('button',{'data-testid':'x',onClick:()=>hit.set('x'),style:{width:20,height:20,marginLeft:50}})))}"), false);
    if let Some(environment) = host.adapter().environment.lock().unwrap().as_mut() {
        environment.value["os"] = "ios".into();
    }
    host.set_visible(true);
    platform.settle(&[&host]);
    let state = |host: &ViewHost<Fake>| query(host, "state")["text"].as_str().unwrap().to_owned();
    let [x, y, _, h] = rect(&host, "body");
    tap_at(&platform, &host, [x + 2.0, y + h / 2.0]);
    assert_eq!(
        state(&host),
        "open",
        "the panel's own text is not the backdrop"
    );
    tap_at(&platform, &host, [x + 2.0, y + h + 40.0]);
    assert_eq!(state(&host), "open", "nor is its empty area");
    let [bx, by, _, bh] = rect(&host, "x");
    tap_at(&platform, &host, [bx - 8.0, by + bh / 2.0]);
    assert_eq!(
        state(&host),
        "openx",
        "the close button's band inside the panel"
    );
    tap_at(&platform, &host, [2.0, 190.0]);
    assert_eq!(state(&host), "shutx", "outside the panel: a light dismiss");
}
