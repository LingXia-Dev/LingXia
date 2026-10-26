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

#[test]
fn component_locals_survive_a_swap_whatever_order_components_first_rendered_in() {
    // Two tab components; the page shows Paint first, then Inputs, whose local is set.
    let module = |label: &str| {
        super::source(&format!(
            "function Paint(){{const p=useLocal('p','paint');return h('text',{{}},p.value)}}
             function Inputs(){{const plan=useLocal('plan','initial');return h('button',{{testId:'plan',onClick(){{plan.set('yearly')}}}},'{label} '+plan.value)}}
             function render(){{const tab=useLocal('tab','paint');return h('column',{{}},h('button',{{testId:'tab',onClick(){{tab.set('inputs')}}}},'tab'),tab.value==='paint'?h(Paint,{{}}):h(Inputs,{{}}))}}"
        )) + "\nenableHotSwap();"
    };
    let mut page = ViewSession::new(&module("Plan")).unwrap();
    let press = |page: &mut ViewSession, id: &str| {
        let display = page.layout(200, 200, 1.0).unwrap();
        let handle = display
            .nodes
            .iter()
            .find(|n| n.test_id == id)
            .unwrap()
            .on_click
            .unwrap();
        assert!(page.click(page.generation(), handle).unwrap());
    };
    press(&mut page, "tab");
    press(&mut page, "plan");
    let shown = |page: &mut ViewSession| {
        let display = page.layout(200, 200, 1.0).unwrap();
        display
            .nodes
            .iter()
            .find(|n| n.test_id == "plan")
            .unwrap()
            .text
            .clone()
    };
    assert_eq!(shown(&mut page), "Plan yearly");
    // The new isolate meets Inputs before Paint; the local must still be found.
    page.swap_module(&module("Billing")).unwrap();
    assert_eq!(shown(&mut page), "Billing yearly");
}
