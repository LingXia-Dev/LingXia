//! A handler's throw or rejection fails the page under automation (or in a dev build); in a
//! shipped app it is logged once per failure and the page stays.
use super::*;

fn failed_by(handler: &str, name: &str) -> String {
    let page = format!(
        "function render(){{return h('button',{{'data-testid':'go',style:{{width:100,height:40}},onClick:{handler}}},'go')}}"
    );
    let worker = ViewWorker::spawn(name).unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(&worker, &source(&page), false);
    host.set_visible(true);
    platform.settle(&[&host]);
    host.automation_request(&serde_json::json!({"op": "act", "css": "[data-testid=\"go\"]", "action": {"kind": "click"}}))
        .unwrap();
    platform.settle(&[&host]);
    let error = host
        .automation_request(&serde_json::json!({"op": "query", "css": "[data-testid=\"go\"]"}))
        .unwrap_err();
    assert!(error.starts_with("E_PAGE_FAILED: "), "{error}");
    host.failure().expect("the page failed").error
}

#[test]
fn a_sync_throw_names_its_handler_and_element() {
    let error = failed_by("()=>{throw Error('no seats')}", "test-handler-throw");
    assert!(
        error.starts_with("UI_HANDLER: onClick of <button data-testid=\"go\"> threw: no seats"),
        "{error}"
    );
}

#[test]
fn a_rejection_fails_the_page_as_the_throw_would() {
    // Before and after its first await, as `actions.reserve({ replaces: undefined })` did.
    for (handler, name) in [
        (
            "async()=>{throw Error('no seats')}",
            "test-handler-reject-now",
        ),
        (
            "async()=>{await 0;throw Error('no seats')}",
            "test-handler-reject-later",
        ),
    ] {
        let error = failed_by(handler, name);
        assert!(
            error.starts_with(
                "UI_HANDLER: onClick of <button data-testid=\"go\"> rejected: no seats"
            ),
            "{handler}: {error}"
        );
    }
}

// A dev build fails the page for this too, so only a shipped build keeps it.
#[cfg(not(feature = "dev"))]
#[test]
fn without_automation_a_failed_handler_is_logged_and_the_page_stays() {
    // A sync throw discards its turn's writes; an async handler's writes before it failed stay.
    for (handler, name, verb, shown) in [
        (
            "()=>{throw Error('no seats')}",
            "test-handler-quiet-throw",
            "threw",
            "n0",
        ),
        (
            "async()=>{await 0;throw Error('no seats')}",
            "test-handler-quiet-reject",
            "rejected",
            "n2",
        ),
    ] {
        let page = format!(
            "function render(){{const n=useLocal('n',0);return h('button',{{'data-testid':'go',style:{{width:100,height:40}},onClick:()=>{{n.set(n.value+1);return ({handler})()}}}},'n'+n.value)}}"
        );
        let worker = ViewWorker::spawn(name).unwrap();
        let platform = Platform::new(&[&worker]);
        let host = platform.host(&worker, &source(&page), false);
        host.set_visible(true);
        platform.settle(&[&host]);
        for _ in 0..2 {
            tap(&host, 20.0, 20.0);
            platform.settle(&[&host]);
        }
        assert!(host.failure().is_none(), "{name}: the page stays");
        assert_eq!(last(&host), [shown], "{name}");
        let expected =
            format!("UI_HANDLER: onClick of <button data-testid=\"go\"> {verb}: no seats");
        let warnings = host.adapter().log.lock().unwrap().warnings.clone();
        let logged: Vec<_> = warnings
            .iter()
            .filter(|w| w.starts_with(&expected))
            .collect();
        assert_eq!(logged.len(), 2, "{name}: once per tap: {warnings:?}");
    }
}

#[cfg(feature = "dev")]
#[test]
fn in_a_dev_build_a_failed_handler_fails_the_page_without_automation() {
    let page = "function render(){return h('button',{'data-testid':'go',style:{width:100,height:40},onClick:()=>{throw Error('no seats')}},'go')}";
    let worker = ViewWorker::spawn("test-handler-dev-throw").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(&worker, &source(page), false);
    host.set_visible(true);
    platform.settle(&[&host]);
    tap(&host, 20.0, 20.0);
    platform.settle(&[&host]);
    let error = host.failure().expect("the page failed").error;
    assert!(
        error.starts_with("UI_HANDLER: onClick of <button data-testid=\"go\"> threw: no seats"),
        "{error}"
    );
}
