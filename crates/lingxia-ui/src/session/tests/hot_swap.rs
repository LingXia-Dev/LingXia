//! Dev hot swap: a new module keeps local state; a broken one never replaces a working one.
use crate::ViewSession;

fn module(label: &str) -> String {
    super::source(&format!(
        "function render(){{const n=useLocal('n',0);return h('column',{{}},h('button',{{testId:'b',onClick(){{n.set(n.value+1)}}}},'{label} '+n.value),h('scroll',{{testId:'s',style:{{height:50}}}},h('column',{{style:{{height:300}}}})))}}"
    )) + "\nenableHotSwap();"
}

fn text(page: &mut ViewSession) -> String {
    let display = page.layout(200, 200, 1.0).unwrap();
    display
        .nodes
        .iter()
        .find(|n| n.test_id == "b")
        .unwrap()
        .text
        .clone()
}

#[test]
fn a_swapped_module_keeps_local_state_and_a_broken_one_rolls_back() {
    let mut page = ViewSession::new(&module("Count")).unwrap();
    let first = page.layout(200, 200, 1.0).unwrap();
    let handle = first
        .nodes
        .iter()
        .find(|n| n.test_id == "b")
        .unwrap()
        .on_click
        .unwrap();
    assert!(page.click(page.generation(), handle).unwrap());
    assert_eq!(text(&mut page), "Count 1");
    page.swap_module(&module("Total")).unwrap();
    assert_eq!(text(&mut page), "Total 1");
    // A module that throws while rendering leaves the running one in place.
    let broken =
        module("Total").replace("return h('column'", "throw Error('typo');return h('column'");
    let error = page.swap_module(&broken).unwrap_err();
    assert!(error.contains("typo"), "{error}");
    assert_eq!(text(&mut page), "Total 1");
    assert!(page.swap_module("not javascript (").is_err());
    assert_eq!(text(&mut page), "Total 1");
}
