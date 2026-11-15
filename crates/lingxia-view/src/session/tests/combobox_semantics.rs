//! Editors that own a list or a number: the active option is spoken, a spinbutton steps through
//! its key map, and a combobox says what is chosen.
use super::*;
use crate::semantics::projection::Event;

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
    platform.settle(&[host]);
    host.request_frame();
    platform.settle(&[host]);
    run_frames(platform, host, 2);
}

fn announced(host: &ViewHost<Fake>) -> Vec<String> {
    host.adapter()
        .log
        .lock()
        .unwrap()
        .events
        .iter()
        .filter_map(|event| match event {
            AccessibilityEffect::Change(Event::Announce { text, .. }) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_option_an_editors_keys_point_at_is_spoken_as_it_changes() {
    let worker = ViewWorker::spawn("test-active-descendant").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = shown(
        &platform,
        &worker,
        "function render(){const at=useLocal('at',-1);const names=['Apple','Banana','Cherry'];return h('page',{},h('input',{'data-testid':'q',value:'',role:'combobox','aria-expanded':true,'aria-controls':'list','aria-autocomplete':'list','aria-label':'Fruit','aria-activedescendant':at.value<0?undefined:'opt-'+at.value,onKeyDown:{ArrowDown(){at.set(Math.min(2,at.value+1))},ArrowUp(){at.set(Math.max(0,at.value-1))}}}),h('view',{id:'list',role:'listbox','aria-label':'Fruits'},...names.map((name,i)=>h('button',{key:name,id:'opt-'+i,role:'option','aria-selected':i===at.value,onClick(){}},name))))}",
    );
    assert!(announced(&host).is_empty(), "nothing is active yet");
    for key in ["ArrowDown", "ArrowDown", "ArrowUp"] {
        act(
            &platform,
            &host,
            "q",
            serde_json::json!({"kind":"press","key":key}),
        );
    }
    assert_eq!(announced(&host), ["Apple", "Banana", "Apple"]);
}

#[test]
fn a_spinbutton_editor_reports_its_value_and_steps_through_its_key_map() {
    let worker = ViewWorker::spawn("test-spinbutton").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = shown(
        &platform,
        &worker,
        "function render(){const n=useLocal('n',5);return h('page',{},h('input',{'data-testid':'w',type:'number',inputMode:'decimal',value:String(n.value),role:'spinbutton','aria-label':'Weight','aria-valuenow':n.value,'aria-valuemin':0,'aria-valuemax':10,'aria-valuetext':n.value+' kg',onKeyDown:{ArrowUp(){n.set(Math.min(10,n.value+1))},ArrowDown(){n.set(Math.max(0,n.value-1))}}}),h('text',{'data-testid':'n'},String(n.value)))}",
    );
    let text = |host: &ViewHost<Fake>| {
        host.automation_request(&serde_json::json!({"op": "query", "css": "[data-testid=\"n\"]"}))
            .unwrap()["text"]
            .clone()
    };
    act(
        &platform,
        &host,
        "w",
        serde_json::json!({"kind":"action","name":"increment"}),
    );
    assert_eq!(text(&host), "6");
    act(
        &platform,
        &host,
        "w",
        serde_json::json!({"kind":"action","name":"decrement"}),
    );
    act(
        &platform,
        &host,
        "w",
        serde_json::json!({"kind":"action","name":"decrement"}),
    );
    assert_eq!(text(&host), "4");
    let value_changes = host
        .adapter()
        .log
        .lock()
        .unwrap()
        .events
        .iter()
        .filter(|event| {
            matches!(
                event,
                AccessibilityEffect::Change(Event::ValueChanged { .. })
            )
        })
        .count();
    assert_eq!(
        value_changes, 3,
        "each step is a value change for assistive tech"
    );
}

#[test]
fn a_combobox_button_reads_as_a_combobox_with_what_is_chosen() {
    let worker = ViewWorker::spawn("test-combobox-role").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = shown(
        &platform,
        &worker,
        "function render(){return h('page',{},h('button',{'data-testid':'c',role:'combobox','aria-expanded':false,'aria-controls':'list','aria-valuetext':'Kenya','aria-label':'Country',onClick(){}},'Country'))}",
    );
    let (role, value) = host.semantics(|projection, _| {
        let projection = projection.unwrap();
        let index = projection
            .frame
            .nodes
            .iter()
            .position(|node| node.test_id == "c")
            .unwrap();
        let node = projection.node(projection.ids.id(index).unwrap()).unwrap();
        (format!("{:?}", node.role), node.value.clone())
    });
    assert_eq!(role, "ComboBox");
    assert_eq!(value.as_deref(), Some("Kenya"));
}
