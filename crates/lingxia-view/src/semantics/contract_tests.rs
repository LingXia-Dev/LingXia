//! The role contract reports what a page left out, and nothing on a page that states it all.
use crate::interaction::frame::PresentedFrame;
use crate::semantics::{
    contract::{self, Rule},
    ids::SemanticIds,
    projection::Projection,
};

fn frame(render: &str) -> PresentedFrame {
    let source = crate::session::tests::source(&format!("function render(){{return {render}}}"));
    let mut page = crate::ViewSession::new(&source).unwrap();
    let display = page.layout(300, 800, 1.0).unwrap();
    PresentedFrame::new(
        crate::session::lifecycle::CommitTicket::new(1),
        1.0,
        display.nodes,
    )
}

/// Each violation's rule and the test id of the node it names.
fn violations(render: &str) -> Vec<(Rule, String)> {
    let frame = frame(render);
    let mut ids = SemanticIds::default();
    ids.update(frame.nodes.iter().map(|n| n.identity.as_ref()));
    let projection = Projection {
        frame: &frame,
        ids: &ids,
        keyboard_focus: None,
        inert: false,
        drags: None,
        obscured: [0.0; 4],
        touch_target: 0.0,
    };
    contract::check(&projection)
        .into_iter()
        .map(|v| (v.rule, v.test_id))
        .collect()
}

fn has(found: &[(Rule, String)], rule: Rule, test_id: &str) -> bool {
    found.iter().any(|(r, id)| *r == rule && id == test_id)
}

#[test]
fn a_page_that_states_every_role_contract_passes() {
    let found = violations(
        "h('page',{},h('view',{role:'tablist','aria-label':'Sections'},h('button',{role:'tab','aria-selected':false,onClick(){}},'Overview'),h('button',{role:'tab','aria-selected':true,onClick(){}},'Details')),h('view',{role:'tabpanel','aria-label':'Details'},h('text',{},'Body')),h('view',{role:'radiogroup','aria-label':'Size'},h('button',{role:'radio','aria-checked':false,onClick(){}},'S'),h('button',{role:'radio','aria-checked':true,onClick(){}},'M')),h('view',{role:'toolbar','aria-label':'Format'},h('button',{onClick(){}},'Bold'),h('button',{onClick(){}},'Italic')),h('view',{role:'tree','aria-label':'Files'},h('button',{role:'treeitem','aria-level':1,'aria-expanded':true,onClick(){}},'src'),h('button',{role:'treeitem','aria-level':2,onClick(){}},'a.rs')),h('view',{id:'view-menu',popover:true,open:true,role:'menu','aria-label':'View'},h('button',{role:'menuitemcheckbox','aria-checked':true,onClick(){}},'Wrap'),h('button',{role:'menuitem',onClick(){}},'Zoom')))",
    );
    assert_eq!(found, vec![]);
}

#[test]
fn unnamed_controls_and_sets_are_reported() {
    let found = violations(
        "h('view',{},h('button',{'data-testid':'icon',onClick(){}}),h('view',{role:'radiogroup','data-testid':'group'},h('button',{role:'radio','aria-checked':true,onClick(){}},'Daily')))",
    );
    assert!(has(&found, Rule::Name, "icon"), "{found:?}");
    assert!(has(&found, Rule::Name, "group"), "{found:?}");
}

#[test]
fn an_item_outside_its_composite_is_reported() {
    let found = violations(
        "h('view',{},h('button',{'data-testid':'stray',role:'tab','aria-selected':true,onClick(){}},'Week'),h('view',{role:'menu','aria-label':'More'},h('button',{'data-testid':'opt',role:'option',onClick(){}},'Pear')))",
    );
    assert!(has(&found, Rule::Owner, "stray"), "{found:?}");
    assert!(has(&found, Rule::Owner, "opt"), "{found:?}");
}

#[test]
fn missing_states_and_choices_are_reported() {
    let found = violations(
        "h('view',{},h('view',{role:'tablist','data-testid':'none'},h('button',{'data-testid':'bare',role:'tab',onClick(){}},'Day'),h('button',{role:'tab','aria-selected':false,onClick(){}},'Week')),h('view',{role:'tablist','data-testid':'two'},h('button',{role:'tab','aria-selected':true,onClick(){}},'Month'),h('button',{role:'tab','aria-selected':true,onClick(){}},'Year')),h('view',{role:'radiogroup','aria-label':'Plan','data-testid':'radios'},h('button',{role:'radio','aria-checked':true,onClick(){}},'Free'),h('button',{role:'radio','aria-checked':true,onClick(){}},'Pro')))",
    );
    assert!(has(&found, Rule::State, "bare"), "{found:?}");
    assert!(has(&found, Rule::Chosen, "none"), "{found:?}");
    assert!(has(&found, Rule::Chosen, "two"), "{found:?}");
    assert!(has(&found, Rule::Chosen, "radios"), "{found:?}");
}

#[test]
fn an_empty_choice_is_reported_but_an_empty_list_box_is_not() {
    let found = violations(
        "h('view',{},h('view',{role:'radiogroup','aria-label':'Plan','data-testid':'radios',style:{height:40}},h('text',{},'Loading')),h('view',{role:'listbox','aria-label':'Results','data-testid':'results',style:{height:40}},h('text',{},'No results')))",
    );
    assert!(has(&found, Rule::Items, "radios"), "{found:?}");
    assert!(!has(&found, Rule::Items, "results"), "{found:?}");
}

#[test]
fn twin_siblings_are_reported_but_repeated_rows_are_not() {
    let found = violations(
        "h('view',{},h('view',{style:{flexDirection:'row'}},h('button',{onClick(){}},'Delete'),h('button',{'data-testid':'twin',onClick(){}},'Delete')),h('view',{role:'list','aria-label':'Bills'},h('view',{role:'listitem',style:{flexDirection:'row'}},h('text',{},'Water'),h('button',{'data-testid':'row-a',onClick(){}},'Pay')),h('view',{role:'listitem',style:{flexDirection:'row'}},h('text',{},'Power'),h('button',{'data-testid':'row-b',onClick(){}},'Pay'))))",
    );
    assert!(has(&found, Rule::Distinct, "twin"), "{found:?}");
    assert!(
        found
            .iter()
            .filter(|(r, _)| *r == Rule::Distinct)
            .all(|(_, id)| id == "twin"),
        "rows each have their own Pay: {found:?}"
    );
}

#[test]
fn a_composite_with_an_extra_control_takes_two_tab_stops() {
    let found = violations(
        "h('view',{},h('view',{role:'tablist','aria-label':'Files','data-testid':'tabs'},h('button',{role:'tab','aria-selected':true,onClick(){}},'main.rs'),h('button',{onClick(){}},'Close')))",
    );
    assert!(has(&found, Rule::TabStops, "tabs"), "{found:?}");
}

#[test]
fn a_composite_nested_in_another_counts_toward_itself_alone() {
    let found = violations(
        "h('view',{},h('view',{role:'toolbar','aria-label':'Format','data-testid':'bar'},h('button',{onClick(){}},'Bold'),h('view',{role:'radiogroup','aria-label':'Align','data-testid':'align'},h('button',{role:'radio','aria-checked':true,onClick(){}},'Left'),h('button',{role:'radio','aria-checked':false,onClick(){}},'Right'))))",
    );
    assert!(!has(&found, Rule::TabStops, "bar"), "{found:?}");
    assert!(!has(&found, Rule::TabStops, "align"), "{found:?}");
}

#[test]
fn rule_codes_match_their_serialized_names() {
    for rule in [
        Rule::Name,
        Rule::Items,
        Rule::Owner,
        Rule::State,
        Rule::Chosen,
        Rule::Distinct,
        Rule::TabStops,
    ] {
        assert_eq!(serde_json::to_value(rule).unwrap(), rule.as_str());
    }
}

#[test]
fn a_grid_of_rows_without_cells_is_reported_once() {
    let found = violations(
        "h('view',{},h('view',{role:'grid','aria-label':'Orders','data-testid':'table'},h('view',{role:'row',style:{flexDirection:'row'}},h('text',{},'#1'),h('button',{onClick(){}},'Open')),h('view',{role:'row',style:{flexDirection:'row'}},h('text',{},'#2'),h('button',{onClick(){}},'Refund'))))",
    );
    assert!(has(&found, Rule::Items, "table"), "{found:?}");
    assert!(!has(&found, Rule::TabStops, "table"), "{found:?}");
}

#[test]
fn a_menu_inside_a_toolbar_keeps_its_stops_to_itself() {
    let found = violations(
        "h('view',{},h('view',{role:'toolbar','aria-label':'Edit','data-testid':'bar'},h('button',{onClick(){}},'Undo'),h('view',{id:'more',popover:true,open:true,role:'menu','aria-label':'More'},h('button',{role:'menuitem',onClick(){}},'Cut'),h('button',{role:'menuitem',onClick(){}},'Paste'))))",
    );
    assert!(!has(&found, Rule::TabStops, "bar"), "{found:?}");
}
