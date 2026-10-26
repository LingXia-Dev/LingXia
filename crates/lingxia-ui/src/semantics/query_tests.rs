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

#[test]
fn press_submits_editors_and_drives_keyboard_focus() {
    let mut i = interaction(PAGE);
    let page = PageTicket::new(1);
    let effects = i
        .automate(
            "[data-testid=\"amount\"]",
            None,
            AutomationAct::Press("Enter".into()),
            false,
            page,
        )
        .unwrap();
    assert!(effects.contains(&Effect::Dispatch));
    assert!(
        matches!(&i.take_dispatch()[..], [Dispatch::SubmitEditor { identity, .. }] if !identity.is_empty())
    );
    let effects = i
        .automate(
            "[data-testid=\"pay\"]",
            None,
            AutomationAct::Press("Enter".into()),
            false,
            page,
        )
        .unwrap();
    assert!(effects.contains(&Effect::Dispatch));
    assert!(matches!(i.take_dispatch()[..], [Dispatch::Click { .. }]));
    assert!(
        i.automate("", None, AutomationAct::Press("F13".into()), false, page)
            .unwrap_err()
            .starts_with("E_UNSUPPORTED")
    );
}

#[test]
fn the_tree_nests_named_nodes_in_reading_order_with_their_states() {
    let i = interaction(
        "h('column',{},h('text',{role:'heading',headingLevel:1},'Bills'),h('column',{role:'list',accessibilityLabel:'Due'},h('text',{testId:'w',role:'listitem',accessibilityState:{selected:true}},'Water')),h('button',{testId:'pay',disabled:true},'Pay'))",
    );
    let tree = i.automation_tree().unwrap();
    let nodes = tree["nodes"].as_array().unwrap();
    let roles: Vec<_> = nodes.iter().map(|n| n["role"].as_str().unwrap()).collect();
    assert_eq!(roles, ["heading", "list", "button"]);
    assert_eq!(nodes[0]["level"], 1);
    assert_eq!(nodes[1]["name"], "Due");
    let item = &nodes[1]["children"][0];
    assert_eq!(
        (
            item["role"].as_str(),
            item["testId"].as_str(),
            item["selected"].as_bool()
        ),
        (Some("listitem"), Some("w"), Some(true))
    );
    assert_eq!(
        (nodes[2]["testId"].as_str(), nodes[2]["disabled"].as_bool()),
        (Some("pay"), Some(true))
    );
}

#[test]
fn offscreen_selects_reveal_and_a_disabled_option_fails_at_once() {
    for layout in [
        "h('page',{},h('scroll',{style:{flexGrow:1}},h('column',{style:{padding:16,gap:12}},h('column',{style:{height:600}}),SELECT)))",
        "h('page',{},h('scroll',{style:{flexGrow:1}},h('column',{style:{padding:16,gap:12}},h('column',{style:{height:600}}),h('row',{style:{gap:8}},SELECT))))",
    ] {
        let mut i = interaction(&layout.replace("SELECT", "h('select',{testId:'plan',accessibilityLabel:'Plan',placeholder:'Choose',options:[{value:'monthly',label:'Monthly'}],value:'',onChange(){},style:{paddingLeft:12,paddingRight:12,paddingTop:8,paddingBottom:8,borderWidth:1,borderRadius:8}})"));
        let page = PageTicket::new(1);
        let revealed = i.automate(
            "[data-testid=\"plan\"]",
            None,
            AutomationAct::ScrollIntoView,
            false,
            page,
        );
        assert!(
            revealed.is_ok_and(|effects| effects.contains(&Effect::Redraw)),
            "{layout}"
        );
    }
    let mut i = interaction(
        "h('scroll',{style:{height:100}},h('column',{style:{height:400}}),h('select',{testId:'plan',accessibilityLabel:'Plan',options:[{value:'monthly',label:'Monthly'},{value:'yearly',label:'Yearly',disabled:true}],value:'monthly',onChange(){}}))",
    );
    let page = PageTicket::new(1);
    let revealed = i
        .automate(
            "[data-testid=\"plan\"]",
            None,
            AutomationAct::ScrollIntoView,
            false,
            page,
        )
        .unwrap();
    assert!(revealed.contains(&Effect::Redraw));
    let disabled = i
        .automate(
            "[data-testid=\"plan\"]",
            None,
            AutomationAct::SelectOption("yearly".into()),
            false,
            page,
        )
        .unwrap_err();
    assert!(
        !disabled.starts_with("Element not interactable"),
        "{disabled}"
    );
    assert!(
        disabled.starts_with("option \"yearly\"") && disabled.ends_with("is disabled"),
        "{disabled}"
    );
}

#[test]
fn a_double_click_selects_the_word_in_text_and_editors() {
    let mut i = interaction(
        "h('column',{},h('text',{testId:'copy',style:{userSelect:'text'}},'select this word'),h('input',{testId:'name',value:'Ada Lovelace',onInput(){}}),h('button',{testId:'go',onClick(){}},'Go'))",
    );
    let page = PageTicket::new(1);
    let text = i
        .automate(
            "[data-testid=\"copy\"]",
            None,
            AutomationAct::DoubleClick,
            false,
            page,
        )
        .unwrap();
    assert!(text.contains(&Effect::Dispatch));
    assert!(matches!(
        &i.take_dispatch()[..],
        [Dispatch::SelectText { .. }]
    ));
    let editor = i
        .automate(
            "[data-testid=\"name\"]",
            None,
            AutomationAct::DoubleClick,
            false,
            page,
        )
        .unwrap();
    assert!(editor.contains(&Effect::Dispatch));
    assert!(matches!(
        i.take_dispatch()[..],
        [Dispatch::Editor { word: true, .. }]
    ));
    let button = i
        .automate(
            "[data-testid=\"go\"]",
            None,
            AutomationAct::DoubleClick,
            false,
            page,
        )
        .unwrap();
    assert!(button.contains(&Effect::Dispatch));
}
