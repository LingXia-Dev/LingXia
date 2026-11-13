//! `closeAfter`: a manual popover closes itself after its time, counted only while shown
//! and not held.
use super::*;

const FRAME: Duration = Duration::from_micros(16_667);

/// A page with a 1 s toast logging `onToggle`; `extra` adds props to the toast, `around`
/// adds elements to the page.
fn toast_page(
    platform: &Platform,
    worker: &Arc<ViewWorker>,
    extra: &str,
    around: &str,
) -> ViewHost<Fake> {
    let page = source(&format!(
        "function render(){{const shown=useLocal('shown',true);const ms=useLocal('ms',1000);const log=useLocal('log','');const keep=useLocal('keep',false);const modal=useLocal('modal',false);return h('page',{{}},h('button',{{'data-testid':'again',onClick:()=>shown.set(true)}},'Again'),h('button',{{'data-testid':'longer',onClick:()=>ms.set(2000)}},'Longer'),h('button',{{'data-testid':'pin',onClick:()=>keep.set(true)}},'Pin'),h('button',{{'data-testid':'cover',onClick:()=>modal.set(true)}},'Cover'),h('text',{{'data-testid':'log'}},log.value||'-'),{around}h('dialog',{{open:modal.value,onClose(){{modal.set(false)}},'aria-label':'Over'}},h('text',{{}},'Modal')),h('view',{{id:'toast',popover:'manual',open:shown.value,closeAfter:ms.value,onToggle:o=>{{log.set(log.value+(o?'T':'F'));if(!keep.value)shown.set(o)}},'data-testid':'toast',style:{{width:120,height:40}}{extra}}},h('button',{{'data-testid':'undo',onClick:()=>{{}}}},'Undo')))}}"
    ));
    let host = platform.host(worker, &page, false);
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

fn shown(host: &ViewHost<Fake>) -> bool {
    query(host, "toast")["exists"].as_bool().unwrap()
}

fn log(host: &ViewHost<Fake>) -> String {
    query(host, "log")["text"].as_str().unwrap().to_owned()
}

fn click(platform: &Platform, host: &ViewHost<Fake>, id: &str) {
    host.automation_request(&serde_json::json!({"op": "act", "css": format!("[data-testid=\"{id}\"]"), "action": {"kind": "click"}}))
        .unwrap();
    platform.settle(&[host]);
}

/// The toast's centre in device pixels.
fn centre(host: &ViewHost<Fake>) -> [f32; 2] {
    let node = query(host, "toast");
    let r = |key: &str| node["rect"][key].as_f64().unwrap() as f32;
    [
        (r("left") + r("width") / 2.0) * 2.0,
        (r("top") + r("height") / 2.0) * 2.0,
    ]
}

fn pointer(host: &ViewHost<Fake>, phase: PointerPhase, [x, y]: [f32; 2]) {
    host.pointer(Pointer {
        phase,
        x,
        y,
        slop: 8.0,
        velocity: [0.0; 2],
        min_fling: 50.0,
    });
}

/// Frames until the toast closes, at most `limit`.
fn frames_to_close(platform: &Platform, host: &ViewHost<Fake>, limit: u32) -> u32 {
    for frame in 1..=limit {
        run_frames(platform, host, 1);
        if !shown(host) {
            return frame;
        }
    }
    panic!("the toast stayed open for {limit} frames");
}

fn frames(ms: u64) -> u32 {
    (Duration::from_millis(ms).as_micros() as f64 / FRAME.as_micros() as f64).ceil() as u32
}

#[test]
fn a_toast_closes_on_its_time_with_one_toggle_and_not_a_frame_early() {
    let worker = ViewWorker::spawn("test-autoclose").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = toast_page(&platform, &worker, "", "");
    assert!(shown(&host));
    let before = log(&host);
    assert_eq!(frames_to_close(&platform, &host, 200), frames(1000));
    assert_eq!(log(&host), format!("{}F", before.trim_start_matches('-')));
    run_frames(&platform, &host, 120);
    assert!(
        log(&host).ends_with('F') && !log(&host).ends_with("FF"),
        "one close only"
    );
}

#[test]
fn a_held_press_pauses_the_count_and_release_resumes_the_remainder() {
    let worker = ViewWorker::spawn("test-autoclose-press").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = toast_page(&platform, &worker, "", "");
    run_frames(&platform, &host, frames(300));
    let at = centre(&host);
    pointer(&host, PointerPhase::Down, at);
    run_frames(&platform, &host, frames(2000));
    assert!(shown(&host), "a finger on the toast holds it");
    pointer(&host, PointerPhase::Up, at);
    let left = frames_to_close(&platform, &host, 200);
    assert!(
        (frames(700) - 1..=frames(700) + 1).contains(&left),
        "{left}"
    );
}

#[test]
fn a_resting_pointer_and_keyboard_focus_each_pause_the_count() {
    let worker = ViewWorker::spawn("test-autoclose-hover").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = toast_page(&platform, &worker, "", "");
    host.hover(Some(centre(&host)));
    run_frames(&platform, &host, frames(2000));
    assert!(shown(&host), "a pointer resting on the toast holds it");
    host.hover(None);
    run_frames(&platform, &host, frames(500));
    for _ in 0..10 {
        if query(&host, "undo")["focused"] == true {
            break;
        }
        host.key(crate::interaction::view::Key::Tab, false, false);
        platform.settle(&[&host]);
    }
    assert_eq!(
        query(&host, "undo")["focused"],
        true,
        "Tab reaches the toast's button"
    );
    run_frames(&platform, &host, frames(2000));
    assert!(shown(&host), "focus inside the toast holds it");
    host.key(crate::interaction::view::Key::Tab, true, false);
    platform.settle(&[&host]);
    let left = frames_to_close(&platform, &host, 200);
    assert!(
        (frames(500) - 1..=frames(500) + 1).contains(&left),
        "{left}"
    );
}

#[test]
fn a_new_time_or_a_reopening_restarts_the_count() {
    let worker = ViewWorker::spawn("test-autoclose-restart").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = toast_page(&platform, &worker, "", "");
    run_frames(&platform, &host, frames(800));
    click(&platform, &host, "longer");
    let left = frames_to_close(&platform, &host, 300);
    assert!(
        (frames(2000) - 1..=frames(2000) + 1).contains(&left),
        "{left}"
    );
    click(&platform, &host, "again");
    assert!(shown(&host));
    let left = frames_to_close(&platform, &host, 300);
    assert!(
        (frames(2000) - 1..=frames(2000) + 1).contains(&left),
        "{left}"
    );
}

#[test]
fn a_hidden_page_resumes_the_remainder_and_a_parked_one_drops_its_count() {
    let worker = ViewWorker::spawn("test-autoclose-hidden").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = toast_page(&platform, &worker, "", "");
    run_frames(&platform, &host, frames(700));
    host.set_visible(false);
    run_frames(&platform, &host, frames(5000));
    host.set_visible(true);
    platform.settle(&[&host]);
    assert!(shown(&host), "a hidden page's toast waits");
    let left = frames_to_close(&platform, &host, 200);
    assert!(
        (frames(300) - 1..=frames(300) + 1).contains(&left),
        "{left}"
    );

    click(&platform, &host, "again");
    run_frames(&platform, &host, frames(700));
    host.park();
    run_frames(&platform, &host, frames(3000));
    host.reload();
    platform.settle(&[&host]);
    assert!(shown(&host));
    let left = frames_to_close(&platform, &host, 200);
    assert!(
        (frames(1000) - 1..=frames(1000) + 1).contains(&left),
        "a reloaded toast counts afresh: {left}"
    );
}

#[test]
fn two_toasts_hidden_mid_count_close_apart_when_shown_again() {
    let worker = ViewWorker::spawn("test-autoclose-pair").unwrap();
    let platform = Platform::new(&[&worker]);
    let page = source(
        "function render(){const one=useLocal('one',true);const two=useLocal('two',false);const toast=(id,open)=>h('view',{id,popover:'manual',open:open.value,onToggle:o=>open.set(o),closeAfter:1000,'data-testid':id,style:{width:120,height:40}},h('text',{},id));return h('page',{},h('button',{'data-testid':'two',onClick:()=>two.set(true)},'Two'),toast('first',one),toast('later',two))}",
    );
    let host = platform.host(&worker, &page, false);
    host.set_visible(true);
    platform.settle(&[&host]);
    run_frames(&platform, &host, frames(400));
    click(&platform, &host, "two");
    run_frames(&platform, &host, frames(300));
    host.set_visible(false);
    run_frames(&platform, &host, frames(4000));
    host.set_visible(true);
    platform.settle(&[&host]);
    let exists = |id: &str| query(&host, id)["exists"].as_bool().unwrap();
    assert!(exists("first") && exists("later"));
    let mut first = None;
    for frame in 1..=120 {
        run_frames(&platform, &host, 1);
        if first.is_none() && !exists("first") {
            first = Some(frame);
            assert!(exists("later"), "the later toast keeps its own remainder");
        }
        if !exists("later") {
            let first = first.expect("the first toast closes first");
            let gap = frame - first;
            assert!(
                (frames(400) - 1..=frames(400) + 1).contains(&gap),
                "{first} {frame}"
            );
            return;
        }
    }
    panic!("the later toast never closed");
}

#[test]
fn a_controlled_toast_hears_one_close_even_when_it_stays_open() {
    let worker = ViewWorker::spawn("test-autoclose-controlled").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = toast_page(&platform, &worker, "", "");
    click(&platform, &host, "pin");
    run_frames(&platform, &host, frames(3000));
    assert!(shown(&host), "the page kept it open");
    assert_eq!(log(&host).matches('F').count(), 1, "{}", log(&host));
}

#[test]
fn a_toast_under_a_modal_dialog_keeps_counting() {
    let worker = ViewWorker::spawn("test-autoclose-modal").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = toast_page(&platform, &worker, "", "");
    click(&platform, &host, "cover");
    let left = frames_to_close(&platform, &host, 200);
    assert!(left <= frames(1000) + 1, "{left}");
}
