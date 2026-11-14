//! `fill` on a date or time editor: the core applies the value as the View's picker would, on
//! every platform, with no input method edit.
use super::*;

fn page(platform: &Platform, worker: &Arc<ViewWorker>) -> ViewHost<Fake> {
    let host = platform.host(
        worker,
        &source(
            "function render(){const due=useLocal('due','');const at=useLocal('at','');const log=useLocal('log','');return h('view',{},h('input',{'data-testid':'due',type:'date',bind:due}),h('input',{'data-testid':'at',type:'time',bind:at}),h('text',{'data-testid':'log'},'['+due.value+']'))}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    host
}

fn read(host: &ViewHost<Fake>, id: &str, key: &str) -> serde_json::Value {
    host.automation_request(
        &serde_json::json!({"op": "query", "css": format!("[data-testid=\"{id}\"]")}),
    )
    .unwrap()[key]
        .clone()
}

/// Fill `id` with `text`; the call's answer once the platform settles.
fn fill(
    platform: &Platform,
    host: &ViewHost<Fake>,
    id: &str,
    text: &str,
) -> Result<serde_json::Value, String> {
    let call = host.automation_request(&serde_json::json!({
        "op": "act", "css": format!("[data-testid=\"{id}\"]"),
        "action": {"kind": "fill", "text": text},
    }))?;
    platform.settle(&[host]);
    host.request_frame();
    platform.settle(&[host]);
    host.automation_request(&serde_json::json!({"op": "result", "call": call["call"]}))
}

#[test]
fn fill_sets_a_date_or_time_editor_as_its_picker_would() {
    let worker = ViewWorker::spawn("test-picker-fill").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(&platform, &worker);
    fill(&platform, &host, "due", "2026-11-13").unwrap();
    fill(&platform, &host, "at", "09:30").unwrap();
    assert_eq!(read(&host, "due", "value"), "2026-11-13");
    assert_eq!(read(&host, "at", "value"), "09:30");
    // Logic heard it through the binding.
    assert_eq!(read(&host, "log", "text"), "[2026-11-13]");
    // Nothing went to the platform's input method, which a picker editor does not have.
    assert!(host.adapter().log.lock().unwrap().edits.is_empty());
    // Empty clears it, as the picker's clear does.
    fill(&platform, &host, "due", "").unwrap();
    assert_eq!(read(&host, "due", "value"), "");
}

#[test]
fn a_value_the_picker_could_not_choose_fails_the_fill_and_changes_nothing() {
    let worker = ViewWorker::spawn("test-picker-fill-bad").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(&platform, &worker);
    let failed = fill(&platform, &host, "due", "13/11/2026").unwrap_err();
    assert!(
        failed.contains("takes an ISO date (YYYY-MM-DD)"),
        "{failed}"
    );
    let failed = fill(&platform, &host, "at", "9:30").unwrap_err();
    assert!(failed.contains("takes an ISO time"), "{failed}");
    assert_eq!(read(&host, "due", "value"), "");
    // The editor still takes a good value afterwards.
    fill(&platform, &host, "due", "2026-01-02").unwrap();
    assert_eq!(read(&host, "due", "value"), "2026-01-02");
}

#[test]
fn a_value_past_the_pickers_bounds_fails_the_fill_naming_the_bound() {
    let worker = ViewWorker::spawn("test-picker-fill-bounds").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){const day=useLocal('day','');const at=useLocal('at','');return h('view',{},h('input',{'data-testid':'back',type:'date',min:'2026-12-18',max:'2027-01-31',bind:day}),h('input',{'data-testid':'quiet',type:'time',min:'18:00',max:'23:45',bind:at}))}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    for (id, text, message) in [
        (
            "back",
            "2026-12-10",
            "E_EDIT_FAILED: 'back' takes a date from 2026-12-18",
        ),
        (
            "back",
            "2027-02-01",
            "E_EDIT_FAILED: 'back' takes a date up to 2027-01-31",
        ),
        (
            "quiet",
            "17:59",
            "E_EDIT_FAILED: 'quiet' takes a time from 18:00",
        ),
        (
            "quiet",
            "23:46",
            "E_EDIT_FAILED: 'quiet' takes a time up to 23:45",
        ),
    ] {
        let failed = fill(&platform, &host, id, text).unwrap_err();
        assert!(failed.contains(message), "{text}: {failed}");
        assert_eq!(read(&host, id, "value"), "", "{text} changed nothing");
    }
    // The bounds themselves are choices.
    fill(&platform, &host, "back", "2026-12-18").unwrap();
    fill(&platform, &host, "quiet", "23:45").unwrap();
    assert_eq!(read(&host, "back", "value"), "2026-12-18");
    assert_eq!(read(&host, "quiet", "value"), "23:45");
}
