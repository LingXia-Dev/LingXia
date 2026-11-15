//! `reveal(id)`: page code scrolls an element of a `<scroll>` into view.
use super::*;

fn in_viewport(host: &ViewHost<Fake>, id: &str) -> bool {
    host.automation_request(
        &serde_json::json!({"op": "query", "css": format!("[data-testid=\"{id}\"]")}),
    )
    .unwrap()["inViewport"]
        .as_bool()
        .unwrap_or(false)
}

fn click(platform: &Platform, host: &ViewHost<Fake>, id: &str) {
    host.automation_request(&serde_json::json!({"op":"act","css":format!("[data-testid=\"{id}\"]"),"action":{"kind":"click"}}))
        .unwrap();
    platform.settle(&[host]);
    run_frames(platform, host, 4);
}

#[test]
fn a_handler_reveals_an_element_by_id_including_one_its_own_render_mounts() {
    let worker = ViewWorker::spawn("test-reveal").unwrap();
    let platform = Platform::new(&[&worker]);
    // Thirty 20 pt rows in an 80 pt scroll; `add` appends a row and reveals it in one handler.
    let host = platform.host(
        &worker,
        &source("function render(){const n=useLocal('n',30);return h('page',{},h('button',{'data-testid':'far',onClick(){reveal('row-25')}},'Far'),h('button',{'data-testid':'back',onClick(){reveal('row-0')}},'Back'),h('button',{'data-testid':'add',onClick(){const next=n.value;n.set(next+1);reveal('row-'+next)}},'Add'),h('button',{'data-testid':'none',onClick(){reveal('nowhere')}},'None'),h('scroll',{style:{height:80}},...Array.from({length:n.value},(_,i)=>h('text',{key:'r'+i,id:'row-'+i,'data-testid':'row-'+i,style:{height:20}},'Row '+i))))}"),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    assert!(in_viewport(&host, "row-0") && !in_viewport(&host, "row-25"));
    click(&platform, &host, "far");
    assert!(in_viewport(&host, "row-25"), "scrolled to it");
    assert!(!in_viewport(&host, "row-0"));
    click(&platform, &host, "none");
    assert!(
        in_viewport(&host, "row-25"),
        "an id nothing has moves nothing"
    );
    click(&platform, &host, "add");
    assert!(
        in_viewport(&host, "row-30"),
        "the row the same handler mounted"
    );
    click(&platform, &host, "back");
    assert!(in_viewport(&host, "row-0"));
}
