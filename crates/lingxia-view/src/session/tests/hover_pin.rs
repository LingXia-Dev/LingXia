//! A test pins a hover: the element holds its `hover:` look and its hint as a resting pointer
//! would, whatever the real pointer does, until the pin lifts.
use super::*;

fn red(host: &ViewHost<Fake>) -> bool {
    let rects = host.adapter().log.lock().unwrap().rects.clone();
    rects.iter().any(|(color, _)| color[..3] == [255, 0, 0])
}

fn pin(host: &ViewHost<Fake>, id: &str, hover: bool) {
    host.automation_request(&serde_json::json!({"op": "pinState", "css": format!("[data-testid=\"{id}\"]"), "hover": hover}))
        .unwrap();
}

#[test]
fn a_pinned_node_keeps_its_hover_look_while_the_pointer_moves_elsewhere() {
    let worker = ViewWorker::spawn("test-hover-pin").unwrap();
    let platform = Platform::new(&[&worker]);
    let page = source(
        "function render(){return h('view',{},h('view',{'data-testid':'b',className:'cold hover:hot',style:{width:100,height:40}}),h('view',{'data-testid':'other',style:{width:100,height:40}}))}",
    )
    .replace(
        "__lingxiaPublish(render);",
        "configureClasses([{name:'cold',style:{backgroundColor:'#0000ff'}},{name:'hover:hot',when:{state:'hover'},style:{backgroundColor:'#ff0000'}}]);__lingxiaPublish(render);",
    );
    let host = platform.host(&worker, &page, false);
    host.set_visible(true);
    platform.settle(&[&host]);
    assert!(!red(&host));
    pin(&host, "b", true);
    run_frames(&platform, &host, 4);
    assert!(red(&host), "pinned");
    // Device pixels at the fake surface's scale of 2: over `other`.
    host.hover(Some([20.0, 120.0]));
    run_frames(&platform, &host, 4);
    assert!(red(&host), "a real move elsewhere does not clear the pin");
    pin(&host, "b", false);
    run_frames(&platform, &host, 4);
    assert!(!red(&host), "unpinned, the pointer's own hover holds");
    host.hover(Some([20.0, 20.0]));
    run_frames(&platform, &host, 4);
    assert!(red(&host), "the real pointer hovers again");
}

#[test]
fn a_pinned_interest_trigger_shows_its_hint_and_unpinning_hides_it_after_the_grace() {
    let worker = ViewWorker::spawn("test-hover-pin-hint").unwrap();
    let platform = Platform::new(&[&worker]);
    let page = source(
        "function render(){return h('page',{},h('button',{interestFor:'tip','aria-label':'About','data-testid':'about',style:{width:100,height:40}}),h('view',{id:'tip',popover:'hint',style:{width:60,height:30,backgroundColor:'#ff0000'}}))}",
    );
    let host = platform.host(&worker, &page, false);
    host.set_visible(true);
    platform.settle(&[&host]);
    pin(&host, "about", true);
    platform.advance(Duration::from_millis(500));
    platform.settle(&[&host]);
    run_frames(&platform, &host, 4);
    assert!(red(&host), "the hint shows after the usual rest");
    host.hover(None);
    host.window_focus(false);
    platform.advance(Duration::from_millis(500));
    platform.settle(&[&host]);
    run_frames(&platform, &host, 4);
    assert!(
        red(&host),
        "the pointer leaving does not hide a pinned hint"
    );
    pin(&host, "about", false);
    platform.advance(Duration::from_millis(250));
    platform.settle(&[&host]);
    run_frames(&platform, &host, 4);
    assert!(!red(&host), "unpinned, it hides after the grace");
}
