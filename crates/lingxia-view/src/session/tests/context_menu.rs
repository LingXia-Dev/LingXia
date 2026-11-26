//! `onContextMenu`: a long press, a secondary click or automation asks for an element's menu.
use super::*;

/// A scroll holding a row (a view with a button inside) and a second row that only the outer
/// card covers; `log` spells each event as `who:pointer@x,y`.
const PAGE: &str = "function render(){const log=useLocal('log','');const say=who=>e=>log.set(log.value+who+':'+e.pointer+'@'+Math.round(e.x)+','+Math.round(e.y)+' ');return h('page',{},h('scroll',{style:{height:150},'data-testid':'scroll'},h('view',{onContextMenu:say('card'),'data-testid':'card',style:{width:90,height:300}},h('view',{onContextMenu:say('row'),'data-testid':'row',style:{width:80,height:40}},h('button',{'data-testid':'open',onClick(){log.set(log.value+'click ')},style:{width:40,height:30}},'Open')),h('text',{'data-testid':'plain',style:{height:40}},'Plain'))),h('text',{'data-testid':'log'},log.value||'-'))}";

fn page(platform: &Platform, worker: &Arc<ViewWorker>, fine: bool) -> ViewHost<Fake> {
    let host = platform.host(worker, &source(PAGE), false);
    if let Some(environment) = host.adapter().environment.lock().unwrap().as_mut() {
        environment.value.pointer = if fine { crate::protocol::env::Pointer::Fine } else { crate::protocol::env::Pointer::Coarse };
    }
    host.set_visible(true);
    platform.settle(&[&host]);
    host
}

fn text(host: &ViewHost<Fake>, id: &str) -> String {
    host.automation_request(
        &serde_json::json!({"op": "query", "css": format!("[data-testid=\"{id}\"]")}),
    )
    .unwrap()["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

fn step(platform: &Platform, host: &ViewHost<Fake>, phase: PointerPhase, x: f32, y: f32) {
    host.pointer(Pointer {
        phase,
        x: x * 2.0,
        y: y * 2.0,
        slop: 8.0,
        velocity: [0.0; 2],
        min_fling: 50.0,
    });
    platform.settle(&[host]);
    run_frames(platform, host, 2);
}

fn settle(platform: &Platform, host: &ViewHost<Fake>) {
    platform.settle(&[host]);
    run_frames(platform, host, 2);
}

#[test]
fn a_held_finger_asks_the_innermost_element_once_and_the_press_neither_clicks_nor_scrolls() {
    let worker = ViewWorker::spawn("test-context-menu-touch").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(&platform, &worker, false);
    // On the button inside the row: the row is the innermost element that asks.
    step(&platform, &host, PointerPhase::Down, 20.0, 15.0);
    host.long_press();
    settle(&platform, &host);
    assert_eq!(text(&host, "log"), "row:touch@20,15 ");
    // The finger moves on and lifts: nothing scrolls and the button is not clicked.
    step(&platform, &host, PointerPhase::Move, 20.0, 60.0);
    step(&platform, &host, PointerPhase::Up, 20.0, 60.0);
    assert_eq!(text(&host, "log"), "row:touch@20,15 ");
    // Below the row only the card asks.
    step(&platform, &host, PointerPhase::Down, 20.0, 100.0);
    host.long_press();
    settle(&platform, &host);
    step(&platform, &host, PointerPhase::Up, 20.0, 100.0);
    assert_eq!(text(&host, "log"), "row:touch@20,15 card:touch@20,100 ");
    // A tap is still a tap.
    step(&platform, &host, PointerPhase::Down, 20.0, 15.0);
    step(&platform, &host, PointerPhase::Up, 20.0, 15.0);
    assert_eq!(
        text(&host, "log"),
        "row:touch@20,15 card:touch@20,100 click "
    );
    // A finger that already moved is scrolling: the timer asks for nothing.
    step(&platform, &host, PointerPhase::Down, 20.0, 100.0);
    step(&platform, &host, PointerPhase::Move, 20.0, 70.0);
    host.long_press();
    settle(&platform, &host);
    step(&platform, &host, PointerPhase::Up, 20.0, 70.0);
    assert_eq!(
        text(&host, "log"),
        "row:touch@20,15 card:touch@20,100 click "
    );
}

#[test]
fn a_secondary_click_and_automation_ask_too_and_a_point_with_no_handler_asks_nobody() {
    let worker = ViewWorker::spawn("test-context-menu-mouse").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(&platform, &worker, true);
    // Device pixels: the fake display is 2x.
    assert!(host.context_menu([40.0, 30.0]));
    settle(&platform, &host);
    assert_eq!(text(&host, "log"), "row:mouse@20,15 ");
    assert!(!host.context_menu([190.0, 390.0]), "outside every handler");
    // A mouse held down is not a long press.
    step(&platform, &host, PointerPhase::Down, 20.0, 100.0);
    host.long_press();
    settle(&platform, &host);
    step(&platform, &host, PointerPhase::Up, 20.0, 100.0);
    assert_eq!(text(&host, "log"), "row:mouse@20,15 ");
    // Automation asks as the Menu key would: the element's centre, from the keyboard.
    let ask = |id: &str| {
        let answer = host.automation_request(&serde_json::json!({"op":"act","css":format!("[data-testid=\"{id}\"]"),"action":{"kind":"action","name":"contextmenu"}}));
        settle(&platform, &host);
        answer
    };
    ask("open").unwrap();
    assert_eq!(text(&host, "log"), "row:mouse@20,15 row:keyboard@20,15 ");
    assert!(ask("log").unwrap_err().contains("has no action"));
}
