//! Test locators over GPU frames: selection, payload shape and actions.
use crate::interaction::frame::PresentedFrame;
use crate::interaction::view::{AutomationAct, Dispatch, Effect, ViewInteraction};
use crate::session::lifecycle::{CommitTicket, PageTicket};
use std::sync::Arc;

fn interaction(render: &str) -> ViewInteraction {
    let source = crate::session::tests::source(&format!("function render(){{return {render}}}"));
    let mut page = crate::ViewSession::new(&source).unwrap();
    let display = page.layout(300, 400, 1.0).unwrap();
    let mut interaction = ViewInteraction::default();
    interaction.presented(
        Arc::new(PresentedFrame::new(
            CommitTicket::new(1),
            1.0,
            display.nodes,
        )),
        0,
        &Default::default(),
        PageTicket::new(1),
    );
    interaction
}

const PAGE: &str = "h('column',{},h('text',{testId:'title',role:'heading',headingLevel:1},'Bills'),h('button',{testId:'pay',onClick(){}},'Pay now'),h('button',{testId:'off',disabled:true},'Later'),h('input',{testId:'amount',accessibilityLabel:'Amount',value:'12'}),h('column',{testId:'gone',accessibilityHidden:true},h('text',{},'x')),h('text',{},'Read ',h('link',{onPress(){}},'terms')))";

#[test]
fn locators_match_test_ids_roles_names_and_tags_in_reading_order() {
    let i = interaction(PAGE);
    let one = |selector: &str| i.automation_query(selector, None, false, None).unwrap();
    let pay = one("[data-testid=\"pay\"]");
    assert_eq!(
        (
            pay["tag"].as_str(),
            pay["role"].as_str(),
            pay["text"].as_str()
        ),
        (Some("button"), Some("button"), Some("Pay now"))
    );
    assert!(pay["rect"]["width"].as_f64().unwrap() > 0.0 && pay["inViewport"] == true);
    assert_eq!(pay["rect"]["viewport_width"], 300.0);
    let buttons = i
        .automation_query("button, [role=\"button\"]", None, true, None)
        .unwrap();
    assert_eq!(buttons["count"], 2);
    assert_eq!(buttons["items"][1]["enabled"], false);
    assert_eq!(one("h1, [role=\"heading\"]")["text"], "Bills");
    let amount = one("input, textarea, [role=\"textbox\"]");
    assert_eq!(
        (
            amount["aria_label"].as_str(),
            amount["value"].as_str(),
            amount["editable"].as_bool()
        ),
        (Some("Amount"), Some("12"), Some(true))
    );
    assert_eq!(one("a[href], [role=\"link\"]")["text"], "terms");
    // A test id reaches nodes hidden from accessibility; roles never do.
    assert_eq!(one("[data-testid=\"gone\"]")["exists"], true);
    assert_eq!(one("[aria-label=\"x\"]")["exists"], false);
    assert!(
        i.automation_query(".card", None, false, None)
            .unwrap_err()
            .starts_with("E_SELECTOR_UNSUPPORTED")
    );
}

#[test]
fn actions_take_the_assistive_input_paths_and_refuse_disabled_targets() {
    let mut i = interaction(PAGE);
    let page = PageTicket::new(1);
    let effects = i
        .automate(
            "[data-testid=\"pay\"]",
            None,
            AutomationAct::Click,
            false,
            page,
        )
        .unwrap();
    assert!(effects.contains(&Effect::Dispatch));
    assert!(matches!(i.take_dispatch()[..], [Dispatch::Click { .. }]));
    let effects = i
        .automate(
            "[data-testid=\"amount\"]",
            None,
            AutomationAct::Type("34".into()),
            false,
            page,
        )
        .unwrap();
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::EditText { text: Some(t), .. } if t == "1234"))
    );
    let effects = i
        .automate(
            "[data-testid=\"amount\"]",
            None,
            AutomationAct::Fill("9".into()),
            false,
            page,
        )
        .unwrap();
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::EditText { text: Some(t), .. } if t == "9"))
    );
    let refused = i.automate(
        "[data-testid=\"off\"]",
        None,
        AutomationAct::Click,
        false,
        page,
    );
    assert!(refused.unwrap_err().ends_with("is not enabled"));
    let not_editor = i.automate(
        "[data-testid=\"pay\"]",
        None,
        AutomationAct::Fill("x".into()),
        false,
        page,
    );
    assert!(not_editor.unwrap_err().ends_with("is not editable"));
    assert!(
        i.automate(
            "[data-testid=\"none\"]",
            None,
            AutomationAct::Click,
            false,
            page
        )
        .unwrap_err()
        .starts_with("Element not found:")
    );
}

#[test]
fn test_ids_reach_the_projection_for_spans_and_link_pieces() {
    let i = interaction(
        "h('text',{testId:'para'},'Read ',h('span',{testId:'em'},'bold'),' and ',h('link',{testId:'terms',onPress(){}},'the ',h('span',{testId:'inner'},'terms')))",
    );
    for id in ["para", "em", "terms", "inner"] {
        let found = i
            .automation_query(&format!("[data-testid=\"{id}\"]"), None, false, None)
            .unwrap();
        assert_eq!(found["exists"], true, "{id}");
    }
    let terms = i
        .automation_query("[data-testid=\"terms\"]", None, false, None)
        .unwrap();
    assert_eq!(
        (terms["tag"].as_str(), terms["role"].as_str()),
        (Some("a"), Some("link"))
    );
}

#[test]
fn reading_order_ids_and_names_are_stable_across_unchanged_frames() {
    let source = crate::session::tests::source(&format!("function render(){{return {PAGE}}}"));
    let mut page = crate::ViewSession::new(&source).unwrap();
    let mut interaction = ViewInteraction::default();
    let mut snapshots = Vec::new();
    for commit in 1..=3 {
        let display = page.layout(300, 400, 1.0).unwrap();
        interaction.presented(
            Arc::new(PresentedFrame::new(
                CommitTicket::new(commit),
                1.0,
                display.nodes,
            )),
            0,
            &Default::default(),
            PageTicket::new(1),
        );
        let projection = interaction.semantics().unwrap();
        snapshots.push(
            (0..projection.frame.nodes.len())
                .filter_map(|index| projection.ids.id(index))
                .filter_map(|id| projection.node(id))
                .map(|n| (n.id, n.index, n.name, n.test_id))
                .collect::<Vec<_>>(),
        );
    }
    assert!(!snapshots[0].is_empty());
    assert_eq!(snapshots[0], snapshots[1]);
    assert_eq!(snapshots[1], snapshots[2]);
}

#[test]
fn offscreen_matches_are_visible_but_must_be_revealed_before_input() {
    let mut i = interaction(
        "h('scroll',{style:{height:100}},h('column',{style:{height:400}}),h('button',{testId:'far',onClick(){}},'Far'))",
    );
    let far = i
        .automation_query("[data-testid=\"far\"]", None, false, None)
        .unwrap();
    assert_eq!(
        (
            far["renderer"].as_str(),
            far["visible"].as_bool(),
            far["inViewport"].as_bool()
        ),
        (Some("gpu"), Some(true), Some(false))
    );
    let page = PageTicket::new(1);
    let refused = i.automate(
        "[data-testid=\"far\"]",
        None,
        AutomationAct::Click,
        true,
        page,
    );
    assert!(refused.unwrap_err().ends_with("is outside the viewport"));
    let revealed = i
        .automate(
            "[data-testid=\"far\"]",
            None,
            AutomationAct::ScrollIntoView,
            false,
            page,
        )
        .unwrap();
    assert!(revealed.contains(&Effect::Redraw));
}
