//! A popover that names its anchor element sits against it, however it was opened.
use super::*;

fn rect(host: &ViewHost<Fake>, id: &str) -> Option<[f64; 4]> {
    let found = host
        .automation_request(
            &serde_json::json!({"op": "query", "css": format!("[data-testid=\"{id}\"]")}),
        )
        .unwrap();
    let rect = found.get("rect")?;
    Some(["left", "top", "width", "height"].map(|key| rect[key].as_f64().unwrap()))
}

fn click(platform: &Platform, host: &ViewHost<Fake>, id: &str) {
    host.automation_request(&serde_json::json!({"op":"act","css":format!("[data-testid=\"{id}\"]"),"action":{"kind":"click"}}))
        .unwrap();
    platform.settle(&[host]);
    run_frames(platform, host, 4);
}

#[test]
fn a_popover_opened_from_state_sits_under_the_element_it_names_and_goes_with_it() {
    let worker = ViewWorker::spawn("test-popover-anchor").unwrap();
    let platform = Platform::new(&[&worker]);
    // A field box holding an editor, 30 pt down and 10 pt in; the list opens from state.
    let host = platform.host(
        &worker,
        &source("function render(){const q=useLocal('q',''),open=useLocal('open',false),shown=useLocal('shown',true),picked=useLocal('picked','-');return h('page',{},h('view',{style:{height:30}}),shown.value&&h('view',{key:'field',id:'field','data-testid':'field',style:{marginLeft:10,width:70,height:24}},h('input',{bind:q,'aria-label':'Search','data-testid':'q'})),h('button',{'data-testid':'show',onClick(){open.set(true)}},'Show'),h('view',{id:'list',popover:'manual',anchor:'field',open:open.value,onToggle(next){open.set(next)},'data-testid':'list',style:{width:60,height:40}},h('button',{'data-testid':'option',onClick(){picked.set('one')}},'One'),h('button',{'data-testid':'gone',onClick(){shown.set(false)}},'Remove')),h('text',{'data-testid':'picked'},picked.value))}"),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    assert!(
        rect(&host, "list").is_none(),
        "closed until the page opens it"
    );
    click(&platform, &host, "show");
    let (field, list) = (rect(&host, "field").unwrap(), rect(&host, "list").unwrap());
    assert_eq!(
        list[0], field[0],
        "its start edge on the anchor's: {list:?} {field:?}"
    );
    assert_eq!(
        list[1],
        field[1] + field[3] + 4.0,
        "just under it: {list:?} {field:?}"
    );
    // A choice inside it clicks; nothing closes it but the page.
    click(&platform, &host, "option");
    assert!(rect(&host, "list").is_some());
    // The anchor leaves the page: the popover goes with it and the page is told.
    click(&platform, &host, "gone");
    assert!(rect(&host, "field").is_none());
    assert!(
        rect(&host, "list").is_none(),
        "an anchored popover closes with its anchor"
    );
}
