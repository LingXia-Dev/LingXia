//! `<select>`: picker request, choice through the input protocol and arrow keys, combobox semantics.
use crate::ViewSession;
use crate::interaction::frame::PresentedFrame;
use crate::interaction::view::{Dispatch, Effect, Key, ViewInteraction};
use crate::scene::paint::Paint;
use crate::session::lifecycle::{CommitTicket, PageTicket};
use std::sync::Arc;

const PAGE: &str = "function render(){const plan=useLocal('plan','m');return h('column',{},h('select',{'data-testid':'s',bind:plan,'aria-label':'Plan',placeholder:'Choose',options:[{value:'m',label:'Monthly'},{value:'q',label:'Quarterly',disabled:true},{value:'y',label:'Yearly'}]}),h('text',{'data-testid':'t'},plan.value))}";

fn present(display: &crate::DisplayList) -> ViewInteraction {
    let mut interaction = ViewInteraction::default();
    interaction.presented(
        Arc::new(PresentedFrame::new(
            CommitTicket::new(1),
            1.0,
            display.nodes.clone(),
        )),
        0,
        &Default::default(),
        PageTicket::new(1),
    );
    interaction
}

fn shown(page: &mut ViewSession) -> String {
    let display = page.layout(300, 300, 1.0).unwrap();
    display
        .nodes
        .iter()
        .find(|n| n.test_id == "t")
        .unwrap()
        .text
        .clone()
}

#[test]
fn a_select_opens_its_picker_and_takes_choices_from_the_picker_and_arrows() {
    let mut page = ViewSession::new(&super::source(PAGE)).unwrap();
    let display = page.layout(300, 300, 1.0).unwrap();
    let index = display.nodes.iter().position(|n| n.test_id == "s").unwrap();
    let node = &display.nodes[index];
    assert_eq!(node.text, "Monthly");
    // Activation opens the View's picker instead of dispatching a click.
    let mut interaction = present(&display);
    let id = interaction.semantics().unwrap().ids.id(index).unwrap();
    let (handled, effects) = interaction.accessibility(
        id,
        crate::interaction::view::AccessibilityAction::Click,
        PageTicket::new(1),
    );
    assert!(handled && effects.contains(&Effect::Dispatch));
    assert!(matches!(
        interaction.take_dispatch().as_slice(),
        [crate::interaction::view::Dispatch::OpenSelect { .. }]
    ));
    let projected = interaction.semantics().unwrap().node(id).unwrap();
    assert_eq!(
        (
            projected.role,
            projected.name.as_str(),
            projected.text.as_str()
        ),
        (
            crate::semantics::projection::Role::ComboBox,
            "Plan",
            "Monthly"
        )
    );
    // The View's picker opens with the options and makes the choice; a disabled one is skipped.
    let (identity, generation) = (node.identity.clone(), page.generation());
    assert!(page.open_select(generation, &identity).unwrap());
    assert!(page.picker_key("ArrowDown").unwrap());
    assert!(page.picker_key("Enter").unwrap());
    assert_eq!(shown(&mut page), "y");
    // Arrows step over the disabled option without opening the picker.
    let display = page.layout(300, 300, 1.0).unwrap();
    let mut interaction = present(&display);
    interaction.key(Key::Tab, false, false, PageTicket::new(1));
    let (_, effects) = interaction.key(Key::Up, false, false, PageTicket::new(1));
    assert!(effects.contains(&Effect::Dispatch));
    let Dispatch::Select { index, .. } = interaction.take_dispatch().remove(0) else {
        panic!("expected a select dispatch")
    };
    assert_eq!(index, 0);
}

#[test]
fn the_chevron_sits_at_the_inline_end_and_mirrors_in_rtl() {
    let chevron_x = |dir: &str| {
        let source = super::source(&format!(
            "function render(){{return h('column',{{dir:'{dir}'}},h('select',{{'aria-label':'Plan',options:[{{value:'m',label:'Monthly'}}]}}))}}"
        ));
        let display = ViewSession::new(&source)
            .unwrap()
            .layout(300, 300, 1.0)
            .unwrap();
        display
            .commands
            .iter()
            .find_map(|paint| match paint {
                Paint::Stroke { points, .. } if points.len() == 3 => Some(points[1][0]),
                _ => None,
            })
            .unwrap()
    };
    let (ltr, rtl) = (chevron_x("ltr"), chevron_x("rtl"));
    assert!(ltr > 250.0 && rtl < 50.0, "{ltr} {rtl}");
}

#[test]
fn automation_chooses_an_option_by_value_or_label() {
    use crate::interaction::view::AutomationAct;
    let mut page = ViewSession::new(&super::source(PAGE)).unwrap();
    let display = page.layout(300, 300, 1.0).unwrap();
    let mut interaction = present(&display);
    let choose = |interaction: &mut ViewInteraction, choice: &str| {
        interaction.automate(
            "[data-testid=\"s\"]",
            None,
            AutomationAct::SelectOption(choice.into()),
            false,
            PageTicket::new(1),
        )
    };
    assert!(
        choose(&mut interaction, "Yearly")
            .unwrap()
            .contains(&Effect::Dispatch)
    );
    assert!(matches!(
        interaction.take_dispatch()[..],
        [Dispatch::Select { index: 2, .. }]
    ));
    assert!(
        choose(&mut interaction, "q")
            .unwrap_err()
            .ends_with("is disabled")
    );
    assert!(
        choose(&mut interaction, "weekly")
            .unwrap_err()
            .contains("has no option")
    );
}

/// Automation picks on the presented frame; a re-render (a tab switch) may commit before the
/// worker runs the choice. It finds its option again by value instead of being dropped.
#[test]
fn a_view_side_choice_on_a_superseded_frame_finds_its_option_by_value() {
    let page_source = "function render(){const plan=useLocal('plan','m'),tab=useLocal('tab',0);return h('column',{},h('button',{'data-testid':'tab',onClick:()=>tab.set(tab.value+1)},`tab ${tab.value}`),tab.value>0?h('text',{},'shifted'):null,h('select',{'data-testid':'s',bind:plan,'aria-label':'Plan',options:[{value:'m',label:'Monthly'},{value:'y',label:'Yearly'}]}),h('text',{'data-testid':'t'},plan.value))}";
    let mut page = ViewSession::new(&super::source(page_source)).unwrap();
    let display = page.layout(300, 300, 1.0).unwrap();
    let seen = page.generation();
    let find = |display: &crate::DisplayList, id: &str| {
        display
            .nodes
            .iter()
            .find(|n| n.test_id == id)
            .unwrap()
            .clone()
    };
    let select = find(&display, "s").identity;
    assert!(
        page.click(seen, find(&display, "tab").on_click.unwrap())
            .unwrap()
    );
    page.layout(300, 300, 1.0).unwrap();
    assert!(
        !page.presented_commit(seen),
        "the tab switch relaid the View out"
    );
    // The input protocol's strict path still refuses a stale commit.
    assert!(!page.select_option(seen, &select, 1).unwrap());
    assert!(page.choose_option(seen, &select, 1, "y").unwrap());
    assert_eq!(shown(&mut page), "y");
    page.take_clicks();
    assert!(!page.choose_option(seen, &select, 1, "gone").unwrap());
    // Recorded like a refused click, so the host warns and fails the automation call.
    let refused = page.take_clicks();
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0].outcome, Err("its option is gone"));
}
