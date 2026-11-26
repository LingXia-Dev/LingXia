//! Unbound controls hold their own value; automation and the accessibility tree read that value
//! as they read a bound one.
use super::*;

fn shown(platform: &Platform, worker: &Arc<ViewWorker>, render: &str) -> ViewHost<Fake> {
    let host = platform.host(worker, &source(render), false);
    host.set_visible(true);
    platform.settle(&[&host]);
    run_frames(platform, &host, 2);
    host
}

fn act(platform: &Platform, host: &ViewHost<Fake>, id: &str, action: serde_json::Value) {
    host.automation_request(
        &serde_json::json!({"op":"act","css":format!("[data-testid=\"{id}\"]"),"action":action}),
    )
    .unwrap();
    // A switch eases between states; a frozen clock would never let it settle.
    run_frames(platform, host, 20);
}

fn query(host: &ViewHost<Fake>, id: &str) -> serde_json::Value {
    host.automation_request(
        &serde_json::json!({"op":"query","css":format!("[data-testid=\"{id}\"]")}),
    )
    .unwrap()
}

#[test]
fn automation_reads_an_unbound_switch_and_slider_as_they_hold() {
    let worker = ViewWorker::spawn("test-uncontrolled-read").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = shown(
        &platform,
        &worker,
        "function render(){return h('page',{},h('switch',{'data-testid':'sw','aria-label':'Wifi'}),h('slider',{'data-testid':'s','aria-label':'Level',min:0,max:10}))}",
    );
    assert_eq!(query(&host, "sw")["checked"], false);
    act(&platform, &host, "sw", serde_json::json!({"kind":"click"}));
    assert_eq!(query(&host, "sw")["checked"], true);
    assert_eq!(query(&host, "s")["value"], "0");
    act(
        &platform,
        &host,
        "s",
        serde_json::json!({"kind":"press","key":"ArrowRight"}),
    );
    assert_eq!(query(&host, "s")["value"], "1");
}

#[test]
fn an_unbound_select_keeps_its_choice_and_reports_each_change_once() {
    let worker = ViewWorker::spawn("test-uncontrolled-select").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = shown(
        &platform,
        &worker,
        "function render(){const seen=useLocal('seen','');return h('page',{},h('select',{'data-testid':'pick','aria-label':'Fruit',placeholder:'Pick one',options:[{value:'a',label:'Apple'},{value:'b',label:'Banana'}],onChange:v=>seen.set(seen.value+v+',')}),h('select',{'data-testid':'free','aria-label':'Size',options:[{value:'s',label:'Small'},{value:'l',label:'Large'}]}),h('text',{'data-testid':'seen'},seen.value||'-'))}",
    );
    assert_eq!(query(&host, "pick")["text"], "Pick one");
    for _ in 0..2 {
        act(
            &platform,
            &host,
            "pick",
            serde_json::json!({"kind":"select","value":"b"}),
        );
    }
    // The callback's re-render kept the choice, and the repeat called nothing.
    assert_eq!(query(&host, "pick")["text"], "Banana");
    assert_eq!(query(&host, "pick")["value"], "b");
    assert_eq!(query(&host, "seen")["text"], "b,");
    act(
        &platform,
        &host,
        "free",
        serde_json::json!({"kind":"select","value":"l"}),
    );
    assert_eq!(query(&host, "free")["value"], "l");
    assert_eq!(query(&host, "pick")["value"], "b");
}

#[test]
fn a_page_parked_and_loaded_again_starts_with_unbound_controls_unset() {
    let worker = ViewWorker::spawn("test-uncontrolled-park").unwrap();
    let platform = Platform::new(&[&worker]);
    let page = "function render(){return h('page',{},h('switch',{'data-testid':'sw','aria-label':'Wifi'}))}";
    let host = shown(&platform, &worker, page);
    act(&platform, &host, "sw", serde_json::json!({"kind":"click"}));
    assert_eq!(query(&host, "sw")["checked"], true);
    host.set_visible(false);
    host.park();
    platform.settle(&[&host]);
    host.load(source(page));
    host.set_visible(true);
    platform.settle(&[&host]);
    run_frames(&platform, &host, 2);
    assert_eq!(
        query(&host, "sw")["checked"],
        false,
        "the parked instance's value does not carry over"
    );
}
