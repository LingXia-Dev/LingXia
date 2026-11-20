//! `checked:` and `has-checked:` are state styles the core lays over the base whenever a control
//! reads checked, bound or not; `defaultChecked`/`defaultValue` seed an unbound control once.
use crate::ViewSession;
use crate::protocol::{Record, field};

/// The classes these pages use, as the compiler emits them.
const CLASSES: &str = "configureClasses([\
{name:'checked:bg-accent-solid',when:{checked:true},style:{backgroundColor:'#2563eb'}},\
{name:'has-checked:bg-accent-solid',when:{state:'hasChecked'},style:{backgroundColor:'#2563eb'}},\
{name:'disabled:bg-surface-muted',when:{state:'disabled'},style:{backgroundColor:'#e5e7eb'}},\
{name:'disabled:checked:bg-accent-muted',when:{state:'disabled',checked:true},style:{backgroundColor:'#93c5fd'}}\
]);__lingxiaPublish(render);";

fn session(body: &str) -> ViewSession {
    let module = super::source(&format!("function render(){{{body}}}"))
        .replace("__lingxiaPublish(render);", CLASSES);
    let mut page = ViewSession::new(&module).unwrap();
    page.layout(300, 400, 1.0).unwrap();
    page
}

/// The painted record of the `nth` element with tag `tag`.
fn painted(page: &ViewSession, tag: usize, nth: usize) -> Record {
    page.paint_records
        .as_ref()
        .unwrap()
        .iter()
        .filter(|record| record.tag == tag)
        .nth(nth)
        .unwrap()
        .clone()
}

fn background(record: &Record) -> Option<String> {
    record.string(field::BACKGROUND).map(str::to_owned)
}

fn click(page: &mut ViewSession, node: usize) {
    let frame = page.layout(300, 400, 1.0).unwrap();
    assert!(
        page.click(page.generation(), frame.nodes[node].on_click.unwrap())
            .unwrap()
    );
    page.layout(300, 400, 1.0).unwrap();
}

const SWITCH: usize = crate::protocol::tag::SWITCH;
const CHECKBOX: usize = crate::protocol::tag::CHECKBOX;
const VIEW: usize = crate::protocol::tag::COLUMN;

#[test]
fn an_unbound_switch_takes_its_checked_style_when_pressed_as_a_bound_one_does() {
    let class = "'checked:bg-accent-solid'";
    let bound = |on: bool| {
        let page = session(&format!(
            "return h('view',{{}},h('switch',{{'aria-label':'Wifi',className:{class},value:{on},onChange(){{}}}}))"
        ));
        background(&painted(&page, SWITCH, 0))
    };
    let (off, on) = (bound(false), bound(true));
    assert_ne!(off, on, "checked: changes the switch's paint");
    let mut page = session(&format!(
        "return h('view',{{}},h('switch',{{'aria-label':'Wifi',className:{class}}}))"
    ));
    assert_eq!(background(&painted(&page, SWITCH, 0)), off);
    click(&mut page, 1);
    assert_eq!(background(&painted(&page, SWITCH, 0)), on);
    assert_eq!(painted(&page, SWITCH, 0).number(field::CHECKED), Some(1.0));
    click(&mut page, 1);
    assert_eq!(background(&painted(&page, SWITCH, 0)), off);
}

#[test]
fn a_mixed_checkbox_is_not_checked_for_its_own_style_but_counts_for_has_checked() {
    let page = session(
        "return h('view',{className:'has-checked:bg-accent-solid'},h('checkbox',{'aria-label':'All',className:'checked:bg-accent-solid',value:true,indeterminate:true,onChange(){}}))",
    );
    let plain = session(
        "return h('view',{className:'has-checked:bg-accent-solid'},h('checkbox',{'aria-label':'All',className:'checked:bg-accent-solid',value:false,onChange(){}}))",
    );
    assert_eq!(
        background(&painted(&page, CHECKBOX, 0)),
        background(&painted(&plain, CHECKBOX, 0)),
        "mixed is not checked:"
    );
    assert_ne!(
        background(&painted(&page, VIEW, 0)),
        background(&painted(&plain, VIEW, 0)),
        "a checked value under mixed still counts for has-checked:, as CSS :checked"
    );
}

#[test]
fn has_checked_follows_an_unbound_box_and_only_an_answered_radio() {
    let mut page = session(
        "return h('view',{},h('view',{className:'has-checked:bg-accent-solid'},h('checkbox',{'aria-label':'Opt in'})),h('view',{className:'has-checked:bg-accent-solid'},h('radio',{'aria-label':'Pace',value:'b',onChange(){},options:[{value:'a',label:'A'},{value:'b',label:'B'}]})),h('view',{className:'has-checked:bg-accent-solid'},h('radio',{'aria-label':'Size',onChange(){},options:[{value:'s',label:'S'}]})))",
    );
    let row = |page: &ViewSession, n: usize| background(&painted(page, VIEW, n));
    let plain = row(&page, 1);
    assert_ne!(
        row(&page, 2),
        plain,
        "the answered group's row is has-checked"
    );
    assert_eq!(row(&page, 3), plain, "an unanswered group is not");
    click(&mut page, 2);
    assert_ne!(row(&page, 1), plain, "the box checked on its own");
}

#[test]
fn disabled_and_checked_combine_as_before() {
    let shown = |on: bool, disabled: bool| {
        let page = session(&format!(
            "return h('view',{{}},h('switch',{{'aria-label':'Wifi',className:'checked:bg-accent-solid disabled:bg-surface-muted disabled:checked:bg-accent-muted',value:{on},disabled:{disabled},onChange(){{}}}}))"
        ));
        background(&painted(&page, SWITCH, 0))
    };
    let disabled_on = shown(true, true);
    assert_ne!(disabled_on, shown(false, true));
    assert_ne!(disabled_on, shown(true, false));
}

#[test]
fn a_default_seeds_an_unbound_control_once() {
    let mut page = session(
        "const d=useLocal('d',true);return h('view',{},h('switch',{'aria-label':'Wifi',defaultChecked:d.value}),h('slider',{'aria-label':'Level',min:0,max:10,defaultValue:7}),h('select',{'aria-label':'Fruit',defaultValue:'b',options:[{value:'a',label:'Apple'},{value:'b',label:'Banana'}]}),h('button',{onClick:()=>d.set(false)},'Flip'))",
    );
    let frame = page.layout(300, 400, 1.0).unwrap();
    assert_eq!(frame.nodes[1].checked, Some(true));
    assert_eq!(frame.nodes[2].range.as_ref().unwrap().value, 7.0);
    let select = painted(&page, crate::protocol::tag::SELECT, 0);
    assert_eq!(select.string(field::INPUT_VALUE), Some("b"));
    assert_eq!(select.text, "Banana");
    // A later default does not reset what the control holds.
    let button = frame
        .nodes
        .iter()
        .position(|node| node.tag == "button")
        .unwrap();
    click(&mut page, button);
    let frame = page.layout(300, 400, 1.0).unwrap();
    assert_eq!(frame.nodes[1].checked, Some(true));
}
