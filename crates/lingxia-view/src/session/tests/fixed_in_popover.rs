//! A fixed box inside an open popover belongs to that popover's layer: above its backdrop.
use super::*;

const PAGE: &str = "function render(){const open=useLocal('open',false),picked=useLocal('picked','-');return h('page',{},h('button',{'data-testid':'show',onClick(){open.set(true)},style:{width:40,height:20}},'Show'),h('view',{id:'menu',popover:'auto',open:open.value,onToggle(next){open.set(next)},'data-testid':'menu',style:{width:60,height:40}},h('button',{'data-testid':'pinned',onClick(){picked.set('pinned')},style:{position:'fixed',left:10,top:120,width:60,height:30}},'Pin')),h('text',{'data-testid':'picked'},picked.value))}";

fn found(host: &ViewHost<Fake>, id: &str) -> serde_json::Value {
    host.automation_request(
        &serde_json::json!({"op": "query", "css": format!("[data-testid=\"{id}\"]")}),
    )
    .unwrap()
}

#[test]
fn a_press_on_a_fixed_box_inside_a_popover_reaches_it_and_not_the_backdrop() {
    let worker = ViewWorker::spawn("test-fixed-in-popover").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(&worker, &source(PAGE), false);
    host.set_visible(true);
    platform.settle(&[&host]);
    host.automation_request(
        &serde_json::json!({"op":"act","css":"[data-testid=\"show\"]","action":{"kind":"click"}}),
    )
    .unwrap();
    platform.settle(&[&host]);
    run_frames(&platform, &host, 4);
    // Fixed still means the viewport: where its insets put it, outside the 60 x 40 panel.
    let rect = &found(&host, "pinned")["rect"];
    assert_eq!(
        ["left", "top", "width", "height"].map(|key| rect[key].as_f64().unwrap()),
        [10.0, 120.0, 60.0, 30.0]
    );
    // A finger on it, in device pixels at the fake surface's scale of 2.
    for phase in [PointerPhase::Down, PointerPhase::Up] {
        host.pointer(Pointer {
            phase,
            x: 40.0 * 2.0,
            y: 135.0 * 2.0,
            slop: 8.0,
            velocity: [0.0; 2],
            min_fling: 50.0,
        });
        platform.settle(&[&host]);
        run_frames(&platform, &host, 2);
    }
    assert_eq!(found(&host, "picked")["text"], "pinned");
    assert_eq!(
        found(&host, "menu")["exists"],
        true,
        "the backdrop did not take the press and close it"
    );
}
