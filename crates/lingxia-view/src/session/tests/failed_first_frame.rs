//! Automation on a page whose first render failed hears the reason at once.
use super::*;

fn query(host: &ViewHost<Fake>) -> Result<serde_json::Value, String> {
    host.automation_request(&serde_json::json!({"op": "query", "css": "[data-testid=\"go\"]"}))
}

#[test]
fn a_page_that_failed_before_its_first_frame_answers_automation_with_the_reason() {
    let worker = ViewWorker::spawn("test-failed-first-frame").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render(){throw Error('no such price')}"),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let failure = host.failure().unwrap();
    assert!(!failure.retained);
    for request in [
        serde_json::json!({"op": "query", "css": "[data-testid=\"go\"]"}),
        serde_json::json!({"op": "act", "css": "[data-testid=\"go\"]", "action": {"kind": "click"}}),
        serde_json::json!({"op": "tree"}),
    ] {
        let error = host.automation_request(&request).unwrap_err();
        assert!(
            error.starts_with("E_PAGE_FAILED:") && error.contains("no such price"),
            "{request}: {error}"
        );
    }
    // The page loads again and answers as any other.
    host.load(source(
        "function render(){const n=useLocal('n',0);if(n.value===1)throw Error('later');return h('button',{'data-testid':'go',onClick(){n.set(1)}},'Go')}",
    ));
    platform.settle(&[&host]);
    assert!(host.failure().is_none());
    assert_eq!(query(&host).unwrap()["exists"], true);
    // A failure after a frame keeps that frame, and automation keeps reading it.
    host.automation_request(
        &serde_json::json!({"op": "act", "css": "[data-testid=\"go\"]", "action": {"kind": "click"}}),
    )
    .unwrap();
    platform.settle(&[&host]);
    assert!(host.failure().unwrap().retained);
    assert_eq!(query(&host).unwrap()["exists"], true);
}
