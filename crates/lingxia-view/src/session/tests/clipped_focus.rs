//! A control inside a box that clips it away entirely takes no keyboard focus, in a scroll or not.
use super::*;
use crate::interaction::view::Key;

/// A scroll holding a button, a clipping box of `height` with a button inside, and a last button.
fn page(platform: &Platform, worker: &Arc<ViewWorker>, height: u32) -> ViewHost<Fake> {
    let host = platform.host(
        worker,
        &source(&format!(
            "function render(){{return h('scroll',{{style:{{height:120}}}},h('button',{{'data-testid':'first',style:{{height:30}}}},'First'),h('view',{{style:{{height:{height},overflow:'hidden'}}}},h('button',{{'data-testid':'inside',style:{{height:30,minHeight:30}}}},'Inside')),h('button',{{'data-testid':'last',style:{{height:30}}}},'Last'))}}"
        )),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    host
}

fn focused(host: &ViewHost<Fake>, id: &str) -> bool {
    host.automation_request(
        &serde_json::json!({"op": "query", "css": format!("[data-testid=\"{id}\"]")}),
    )
    .unwrap()["focused"]
        .as_bool()
        .unwrap_or(false)
}

fn tab(platform: &Platform, host: &ViewHost<Fake>) {
    host.key(Key::Tab, false, false);
    platform.settle(&[host]);
    run_frames(platform, host, 2);
}

#[test]
fn tab_passes_over_a_control_its_closed_box_clips_away() {
    let worker = ViewWorker::spawn("test-clipped-focus-closed").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(&platform, &worker, 0);
    tab(&platform, &host);
    assert!(focused(&host, "first"));
    tab(&platform, &host);
    assert!(!focused(&host, "inside"), "nothing of it shows");
    assert!(focused(&host, "last"));
}

#[test]
fn tab_reaches_the_control_once_its_box_shows_it() {
    let worker = ViewWorker::spawn("test-clipped-focus-open").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(&platform, &worker, 30);
    tab(&platform, &host);
    tab(&platform, &host);
    assert!(focused(&host, "inside"));
    tab(&platform, &host);
    assert!(focused(&host, "last"));
}
