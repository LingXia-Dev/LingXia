//! `onPointer`: an element hears a pointer over it in points of its own content box.
use super::*;

/// An 80 x 60 pad with 5 of padding, its content box at (15, 25), holding a 40 x 30 button;
/// `log` spells each event as `phase@x,y`.
fn pad(axis: &str, scroll: bool) -> String {
    let wrap = if scroll {
        "h('scroll',{style:{height:150}},"
    } else {
        "h('view',{},"
    };
    format!(
        "function render(){{const log=useLocal('log',''),hits=useLocal('hits',0);const on=e=>log.set(log.value+e.phase+'@'+Math.round(e.x)+','+Math.round(e.y)+' ');return h('page',{{}},{wrap}h('view',{{onPointer:on,pointerAxis:'{axis}','data-testid':'pad',style:{{marginTop:20,marginLeft:10,width:80,height:60,padding:5}}}},h('button',{{'data-testid':'in',onClick(){{hits.set(hits.value+1)}},style:{{width:40,height:30}}}},'B')),h('view',{{style:{{height:900}}}})),h('text',{{'data-testid':'log'}},log.value||'-'),h('text',{{'data-testid':'hits'}},String(hits.value)))}}"
    )
}

fn page(platform: &Platform, worker: &Arc<ViewWorker>, render: &str, fine: bool) -> ViewHost<Fake> {
    let host = platform.host(worker, &source(render), false);
    if let Some(environment) = host.adapter().environment.lock().unwrap().as_mut() {
        environment.value["pointer"] = if fine { "fine" } else { "coarse" }.into();
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

/// A pointer at a logical point; the fake display is 100 x 200 points at 2x.
fn send(host: &ViewHost<Fake>, phase: PointerPhase, x: f32, y: f32) {
    host.pointer(Pointer {
        phase,
        x: x * 2.0,
        y: y * 2.0,
        slop: 8.0,
        velocity: [0.0; 2],
        min_fling: 50.0,
    });
}

fn step(platform: &Platform, host: &ViewHost<Fake>, phase: PointerPhase, x: f32, y: f32) {
    send(host, phase, x, y);
    platform.settle(&[host]);
    run_frames(platform, host, 2);
}

#[test]
fn a_finger_reports_down_move_and_up_in_the_content_box_and_keeps_reporting_outside_it() {
    let worker = ViewWorker::spawn("test-pointer-events").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(&platform, &worker, &pad("both", false), false);
    step(&platform, &host, PointerPhase::Down, 45.0, 55.0);
    assert_eq!(
        text(&host, "log"),
        "down@30,30 ",
        "at once, from the content box's corner"
    );
    step(&platform, &host, PointerPhase::Move, 55.0, 60.0);
    step(&platform, &host, PointerPhase::Move, 5.0, 10.0);
    step(&platform, &host, PointerPhase::Up, 5.0, 10.0);
    assert_eq!(
        text(&host, "log"),
        "down@30,30 move@40,35 move@-10,-15 up@-10,-15 ",
        "raw, not clamped to the box"
    );
}

#[test]
fn a_button_inside_still_gets_its_click_and_the_element_hears_that_press() {
    let worker = ViewWorker::spawn("test-pointer-child").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(&platform, &worker, &pad("both", false), false);
    step(&platform, &host, PointerPhase::Down, 25.0, 35.0);
    step(&platform, &host, PointerPhase::Up, 25.0, 35.0);
    assert_eq!(text(&host, "hits"), "1", "the button clicked");
    assert_eq!(text(&host, "log"), "down@10,10 up@10,10 ");
    // Outside the pad nothing reports.
    step(&platform, &host, PointerPhase::Down, 95.0, 5.0);
    step(&platform, &host, PointerPhase::Up, 95.0, 5.0);
    assert_eq!(text(&host, "log"), "down@10,10 up@10,10 ");
}

#[test]
fn an_axis_the_element_keeps_stays_its_own_and_the_other_goes_to_the_scroll_with_a_cancel() {
    let worker = ViewWorker::spawn("test-pointer-axis").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(&platform, &worker, &pad("x", true), false);
    // Along x: the element's, wherever it goes next.
    step(&platform, &host, PointerPhase::Down, 45.0, 55.0);
    step(&platform, &host, PointerPhase::Move, 65.0, 56.0);
    step(&platform, &host, PointerPhase::Move, 68.0, 36.0);
    step(&platform, &host, PointerPhase::Up, 68.0, 36.0);
    assert_eq!(
        text(&host, "log"),
        "down@30,30 move@50,31 move@53,11 up@53,11 ",
        "once along x, later vertical movement scrolls nothing"
    );
    // Along y: the scroll takes it and the element hears cancel, then nothing.
    step(&platform, &host, PointerPhase::Down, 45.0, 55.0);
    step(&platform, &host, PointerPhase::Move, 46.0, 40.0);
    step(&platform, &host, PointerPhase::Move, 46.0, 30.0);
    step(&platform, &host, PointerPhase::Up, 46.0, 30.0);
    assert!(
        text(&host, "log").ends_with("up@53,11 down@30,30 cancel@31,15 "),
        "{}",
        text(&host, "log")
    );

    // `both` keeps a vertical drag too: the scroll under it stays put.
    let host = page(&platform, &worker, &pad("both", true), false);
    step(&platform, &host, PointerPhase::Down, 45.0, 55.0);
    step(&platform, &host, PointerPhase::Move, 46.0, 40.0);
    step(&platform, &host, PointerPhase::Up, 46.0, 40.0);
    assert_eq!(text(&host, "log"), "down@30,30 move@31,15 up@31,15 ");
}

#[test]
fn moves_faster_than_the_page_takes_them_arrive_as_the_latest_position() {
    let worker = ViewWorker::spawn("test-pointer-latest").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(&platform, &worker, &pad("both", false), false);
    step(&platform, &host, PointerPhase::Down, 45.0, 55.0);
    for x in [50.0, 55.0, 60.0, 65.0, 70.0] {
        send(&host, PointerPhase::Move, x, 55.0);
    }
    platform.settle(&[&host]);
    run_frames(&platform, &host, 2);
    assert_eq!(
        text(&host, "log"),
        "down@30,30 move@35,30 move@55,30 ",
        "the first, then the newest of those that waited"
    );
    // A release drops a waiting move: it carries the position itself.
    send(&host, PointerPhase::Move, 72.0, 55.0);
    send(&host, PointerPhase::Move, 74.0, 55.0);
    send(&host, PointerPhase::Up, 76.0, 55.0);
    platform.settle(&[&host]);
    run_frames(&platform, &host, 2);
    assert_eq!(
        text(&host, "log"),
        "down@30,30 move@35,30 move@55,30 move@57,30 up@61,30 "
    );
}

#[test]
fn a_mouse_with_no_button_down_hovers_and_leaves() {
    let worker = ViewWorker::spawn("test-pointer-hover").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(&platform, &worker, &pad("both", false), true);
    let hover = |point: Option<[f32; 2]>| {
        host.hover(point.map(|[x, y]| [x * 2.0, y * 2.0]));
        platform.settle(&[&host]);
        run_frames(&platform, &host, 2);
    };
    hover(Some([45.0, 55.0]));
    hover(Some([50.0, 55.0]));
    assert_eq!(text(&host, "log"), "hover@30,30 hover@35,30 ");
    hover(Some([95.0, 55.0]));
    assert_eq!(text(&host, "log"), "hover@30,30 hover@35,30 leave@80,30 ");
    hover(Some([45.0, 55.0]));
    hover(None);
    assert!(
        text(&host, "log").ends_with("hover@30,30 leave@30,30 "),
        "{}",
        text(&host, "log")
    );
    // A press is down and up from the mouse; no hover while the button is held.
    step(&platform, &host, PointerPhase::Down, 45.0, 55.0);
    hover(Some([50.0, 55.0]));
    step(&platform, &host, PointerPhase::Up, 50.0, 55.0);
    assert!(
        text(&host, "log").ends_with("leave@30,30 down@30,30 up@35,30 "),
        "{}",
        text(&host, "log")
    );
}
