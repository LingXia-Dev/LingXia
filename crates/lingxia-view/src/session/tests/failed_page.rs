//! A panic while a frame is prepared drops that frame; in a row it stalls the page, which then
//! answers automation with the failure and tries again only when something changes.
use super::*;

const PAGE: &str = "function render(){const n=useLocal('n',0);return h('button',{'data-testid':'go',style:{width:100,height:40},onClick(){n.set(n.value+1)}},'n'+n.value)}";

fn query(host: &ViewHost<Fake>) -> Result<serde_json::Value, String> {
    host.automation_request(&serde_json::json!({"op": "query", "css": "[data-testid=\"go\"]"}))
}

fn frame(host: &ViewHost<Fake>) -> serde_json::Value {
    host.automation_request(&serde_json::json!({"op": "frame"}))
        .unwrap()
}

fn shown(host: &ViewHost<Fake>) -> (usize, usize) {
    let log = host.adapter().log.lock().unwrap();
    (
        log.recovery.iter().filter(|r| r.is_some()).count(),
        log.presented.len(),
    )
}

fn opened(name: &str) -> (Arc<ViewWorker>, Platform, ViewHost<Fake>) {
    let worker = ViewWorker::spawn(name).unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(&worker, &source(PAGE), false);
    host.set_visible(true);
    platform.settle(&[&host]);
    assert_eq!(last(&host), ["n0"]);
    (worker, platform, host)
}

#[test]
fn one_panic_drops_its_frame_and_the_next_one_presents() {
    let (_worker, platform, host) = opened("test-panic-dropped");
    let (_, before) = shown(&host);
    host.panic_preparing(2);
    host.request_frame();
    platform.settle(&[&host]);
    assert_eq!(host.panics_left(), 0);
    assert!(
        host.failure().is_none(),
        "two dropped frames are no failure"
    );
    let (recoveries, presented) = shown(&host);
    assert_eq!(recoveries, 0, "no recovery chrome");
    assert!(
        presented > before,
        "the frame after the dropped ones presents"
    );
    assert_eq!(frame(&host)["stalled"], false);
    assert!(frame(&host)["failure"].is_null());
    assert_eq!(query(&host).unwrap()["exists"], true);
    // A presented frame ends the run: two more are dropped, not counted as the third.
    host.panic_preparing(2);
    host.request_frame();
    platform.settle(&[&host]);
    assert!(host.failure().is_none());
}

#[test]
fn three_panics_in_a_row_stall_the_page_and_automation_hears_it_at_once() {
    let (_worker, platform, host) = opened("test-panic-stall");
    host.panic_preparing(10);
    host.request_frame();
    platform.settle(&[&host]);
    assert_eq!(host.panics_left(), 7, "rendering stops at the third");
    let failure = host.failure().unwrap();
    assert!(failure.retained && failure.error.starts_with("UI_RENDER: native panic"));
    assert_eq!(shown(&host).0, 1);
    for request in [
        serde_json::json!({"op": "query", "css": "[data-testid=\"go\"]"}),
        serde_json::json!({"op": "act", "css": "[data-testid=\"go\"]", "action": {"kind": "click"}}),
        serde_json::json!({"op": "tree"}),
    ] {
        let error = host.automation_request(&request).unwrap_err();
        assert!(
            error.starts_with("E_PAGE_FAILED: the page stopped rendering: UI_RENDER"),
            "{request}: {error}"
        );
    }
    let metrics = frame(&host);
    assert_eq!(metrics["stalled"], true);
    assert_eq!(metrics["failure"], "UI_RENDER: native panic");
    // Nothing renders on its own.
    for _ in 0..4 {
        host.request_frame();
        platform.settle(&[&host]);
    }
    assert_eq!(host.panics_left(), 7);
    // A tap is a change: one attempt, which stalls again at once while the fault lasts.
    tap(&host, 20.0, 20.0);
    platform.settle(&[&host]);
    assert_eq!(host.panics_left(), 6, "one attempt per change");
    assert!(query(&host).is_err());
    host.environment_changed();
    platform.settle(&[&host]);
    assert_eq!(
        host.panics_left(),
        6,
        "an unchanged environment is no change"
    );
    host.automation_request(&serde_json::json!({"op": "environment", "theme": "dark"}))
        .unwrap();
    platform.settle(&[&host]);
    assert_eq!(host.panics_left(), 5, "an appearance change tries once");
    // With the fault gone the next change renders, and the page is whole again.
    host.panic_preparing(0);
    tap(&host, 20.0, 20.0);
    platform.settle(&[&host]);
    assert!(host.failure().is_none());
    assert_eq!(frame(&host)["stalled"], false);
    assert_eq!(query(&host).unwrap()["exists"], true);
    assert_eq!(
        host.adapter().log.lock().unwrap().recovery.last(),
        Some(&None),
        "recovery chrome is taken down"
    );
}

#[test]
fn a_panic_while_input_runs_fails_the_page_and_releases_the_input() {
    let (_worker, platform, host) = opened("test-panic-input");
    host.panic_input(1);
    tap(&host, 20.0, 20.0);
    platform.settle(&[&host]);
    let failure = host.failure().expect("the page fails");
    assert!(
        failure.error.starts_with("UI_RENDER: native panic"),
        "{}",
        failure.error
    );
    assert_eq!(
        host.input_held(),
        (0, false),
        "nothing waits on the input that panicked"
    );
}
