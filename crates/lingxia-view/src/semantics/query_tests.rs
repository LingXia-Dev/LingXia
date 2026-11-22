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

const PAGE: &str = "h('view',{},h('text',{'data-testid':'title',role:'heading',headingLevel:1},'Bills'),h('button',{'data-testid':'pay',onClick(){}},'Pay now'),h('button',{'data-testid':'off',disabled:true},'Later'),h('input',{'data-testid':'amount','aria-label':'Amount',value:'12'}),h('view',{'data-testid':'gone','aria-hidden':true},h('text',{},'x')),h('text',{},'Read ',h('link',{onClick(){}},'terms')))";

#[test]
fn typing_with_no_selector_reaches_the_editor_that_holds_focus() {
    let page = PageTicket::new(1);
    let mut idle = interaction(PAGE);
    let refused = idle.automate("", None, AutomationAct::Type("3".into()), false, page);
    assert!(refused.unwrap_err().ends_with("no editor has focus"));

    let source = crate::session::tests::source(&format!("function render(){{return {PAGE}}}"));
    let mut session = crate::ViewSession::new(&source).unwrap();
    let mut display = session.layout(300, 400, 1.0).unwrap();
    for node in &mut display.nodes {
        if let Some(input) = &mut node.input {
            input.edit_token = Some(1);
        }
    }
    let mut focused = ViewInteraction::default();
    focused.presented(
        Arc::new(PresentedFrame::new(
            CommitTicket::new(1),
            1.0,
            display.nodes,
        )),
        0,
        &Default::default(),
        page,
    );
    let effects = focused
        .automate("", None, AutomationAct::Type("34".into()), false, page)
        .unwrap();
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::EditText { text: Some(t), .. } if t == "1234"))
    );
}

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
fn a_test_id_clicks_an_aria_hidden_control_that_role_queries_never_find() {
    let mut i = interaction(
        "h('view',{},h('button',{'data-testid':'scrim','aria-hidden':true,'aria-label':'Close',onClick(){},style:{width:100,height:40}}))",
    );
    let page = PageTicket::new(1);
    assert_eq!(
        i.automation_query("[aria-label=\"Close\"]", None, false, None)
            .unwrap()["exists"],
        false
    );
    let effects = i
        .automate(
            "[data-testid=\"scrim\"]",
            None,
            AutomationAct::Click,
            false,
            page,
        )
        .unwrap();
    assert!(effects.contains(&Effect::Dispatch));
    assert!(matches!(i.take_dispatch()[..], [Dispatch::Click { .. }]));
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
        "h('text',{'data-testid':'para'},'Read ',h('span',{'data-testid':'em'},'bold'),' and ',h('link',{'data-testid':'terms',onClick(){}},'the ',h('span',{'data-testid':'inner'},'terms')))",
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
    // A run's rect is its own words, so a tap on its test id lands on them, not the paragraph.
    let rect = |id: &str| {
        let found = i
            .automation_query(&format!("[data-testid=\"{id}\"]"), None, false, None)
            .unwrap();
        let r = &found["rect"];
        [r["left"].as_f64().unwrap(), r["width"].as_f64().unwrap()]
    };
    let [para_left, para_width] = rect("para");
    for id in ["em", "terms"] {
        let [left, width] = rect(id);
        assert!(
            width > 0.0 && width < para_width / 2.0,
            "{id}: {width} of {para_width}"
        );
        assert!(
            left > para_left,
            "{id} starts after the paragraph's first word"
        );
    }
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
        "h('scroll',{style:{height:100}},h('view',{style:{height:400}}),h('button',{'data-testid':'far',onClick(){}},'Far'))",
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
fn fill_brings_an_off_screen_field_into_view_and_sets_its_text() {
    let mut i = interaction(
        "h('scroll',{style:{height:100}},h('view',{style:{height:400}}),h('input',{'data-testid':'far','aria-label':'Far',onInput(){}}),h('view',{style:{height:200}}))",
    );
    let page = PageTicket::new(1);
    let effects = i
        .automate(
            "[data-testid=\"far\"]",
            None,
            AutomationAct::Fill("Bo".into()),
            false,
            page,
        )
        .unwrap();
    assert!(effects.contains(&Effect::Redraw), "{effects:?}");
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::EditText { text: Some(t), .. } if t == "Bo")),
        "{effects:?}"
    );
    // Not flush on the edge: 12 of room below it, as a keyboard reveal keeps.
    let frame = i.frame().unwrap().clone();
    let far = frame.nodes.iter().find(|n| n.test_id == "far").unwrap();
    let scroll = far.scroll_parent.clone().unwrap();
    let end = far.bounds[1] + far.bounds[3];
    assert_eq!(i.scroll().offset(&scroll), end + 12.0 - 100.0);
}

#[test]
fn fill_scrolls_a_field_whose_edge_is_cut_off() {
    // The field's bottom 2 sit below the scroll: visible, but not clear of the edge.
    let mut i = interaction(
        "h('scroll',{style:{height:100}},h('view',{style:{height:60}}),h('input',{'data-testid':'cut','aria-label':'Cut',style:{height:42},onInput(){}}),h('view',{style:{height:200}}))",
    );
    let effects = i
        .automate(
            "[data-testid=\"cut\"]",
            None,
            AutomationAct::Fill("x".into()),
            false,
            PageTicket::new(1),
        )
        .unwrap();
    assert!(effects.contains(&Effect::Redraw), "{effects:?}");
    let frame = i.frame().unwrap().clone();
    let cut = frame.nodes.iter().find(|n| n.test_id == "cut").unwrap();
    let scroll = cut.scroll_parent.clone().unwrap();
    assert_eq!(i.scroll().offset(&scroll), 60.0 + 42.0 + 12.0 - 100.0);
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

/// Three rows of three cells; the middle one takes no focus.
fn grid(on_page: &str) -> String {
    let cell = |id: &str, off: bool| {
        format!(
            "h('button',{{'data-testid':'{id}',role:'gridcell',disabled:{off},onClick(){{}}}},'{id}')"
        )
    };
    let row = |r: usize| {
        let cells: Vec<String> = (0..3)
            .map(|c| cell(&format!("c{r}{c}"), r == 1 && c == 1))
            .collect();
        format!(
            "h('view',{{role:'row',style:{{flexDirection:'row'}}}},{})",
            cells.join(",")
        )
    };
    format!(
        "h('view',{{}},h('button',{{'data-testid':'before',onClick(){{}}}},'Before'),h('view',{{role:'grid','aria-label':'Days'{on_page}}},{},{},{}))",
        row(0),
        row(1),
        row(2)
    )
}

#[test]
fn arrows_move_through_a_grid_by_row_and_column_past_cells_that_take_no_focus() {
    let mut i = interaction(&grid(""));
    let page = PageTicket::new(1);
    let focused = |i: &ViewInteraction| {
        (0..3)
            .flat_map(|r| (0..3).map(move |c| format!("c{r}{c}")))
            .find(|id| {
                i.automation_query(&format!("[data-testid=\"{id}\"]"), None, false, None)
                    .unwrap()["focused"]
                    == true
            })
            .unwrap_or_default()
    };
    let press = |i: &mut ViewInteraction, from: &str, key: &str| {
        let selector = if from.is_empty() {
            String::new()
        } else {
            format!("[data-testid=\"{from}\"]")
        };
        let _ = i.automate(
            &selector,
            None,
            AutomationAct::Press(key.into()),
            false,
            page,
        );
        focused(i)
    };
    assert_eq!(press(&mut i, "c00", "ArrowRight"), "c01");
    // Down from the top middle passes the cell that takes no focus.
    assert_eq!(press(&mut i, "", "ArrowDown"), "c21");
    assert_eq!(press(&mut i, "", "ArrowUp"), "c01");
    // Along the row too.
    assert_eq!(press(&mut i, "c12", "ArrowLeft"), "c10");
    assert_eq!(press(&mut i, "", "End"), "c12");
    assert_eq!(press(&mut i, "", "Home"), "c10");
    // Nothing beyond the edge: focus stays, in the row and in the column.
    assert_eq!(press(&mut i, "c22", "ArrowRight"), "c22");
    assert_eq!(press(&mut i, "", "ArrowDown"), "c22");
    // Without a page handler, PageUp and PageDown go to the column's ends.
    assert_eq!(press(&mut i, "", "PageUp"), "c02");
    assert_eq!(press(&mut i, "", "PageDown"), "c22");
    i.take_dispatch();

    // A grid that pages hears the key instead, and focus stays where it was.
    let mut paged = interaction(&grid(",onPage(){}"));
    assert_eq!(press(&mut paged, "c00", "PageDown"), "c00");
    assert!(matches!(
        paged.take_dispatch()[..],
        [Dispatch::GridPage { next: true, .. }]
    ));
}

#[test]
fn the_tree_nests_named_nodes_in_reading_order_with_their_states() {
    let i = interaction(
        "h('view',{},h('text',{role:'heading',headingLevel:1},'Bills'),h('view',{role:'list','aria-label':'Due'},h('text',{'data-testid':'w',role:'listitem','aria-selected':true},'Water')),h('button',{'data-testid':'pay',disabled:true},'Pay'))",
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
        "h('page',{},h('scroll',{style:{flexGrow:1}},h('view',{style:{padding:16,gap:12}},h('view',{style:{height:600}}),SELECT)))",
        "h('page',{},h('scroll',{style:{flexGrow:1}},h('view',{style:{padding:16,gap:12}},h('view',{style:{height:600}}),h('view',{style:{flexDirection:'row',gap:8}},SELECT))))",
    ] {
        let mut i = interaction(&layout.replace("SELECT", "h('select',{'data-testid':'plan','aria-label':'Plan',placeholder:'Choose',options:[{value:'monthly',label:'Monthly'}],value:'',onChange(){},style:{paddingLeft:12,paddingRight:12,paddingTop:8,paddingBottom:8,borderWidth:1,borderRadius:8}})"));
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
        "h('scroll',{style:{height:100}},h('view',{style:{height:400}}),h('select',{'data-testid':'plan','aria-label':'Plan',options:[{value:'monthly',label:'Monthly'},{value:'yearly',label:'Yearly',disabled:true}],value:'monthly',onChange(){}}))",
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
        "h('view',{},h('text',{'data-testid':'copy',style:{userSelect:'text'}},'select this word'),h('input',{'data-testid':'name',value:'Ada Lovelace',onInput(){}}),h('button',{'data-testid':'go',onClick(){}},'Go'))",
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

#[test]
fn a_container_reads_its_descendants_text_like_dom_text_content() {
    let i = interaction(
        "h('view',{'data-testid':'result'},h('view',{style:{flexDirection:'row'}},h('text',{},'Model'),h('text',{},'Pixel')),h('view',{},h('text',{},'OS ',h('span',{},'Android'))),h('input',{value:'not text','aria-label':'x'}))",
    );
    let result = i
        .automation_query("[data-testid=\"result\"]", None, false, None)
        .unwrap();
    assert_eq!(result["text"], "Model Pixel OS Android");
    // The semantic name of a leaf stays its own.
    let leaf = i.automation_query("span", None, true, None).unwrap();
    assert_eq!(leaf["items"][0]["text"], "Model");
}

#[test]
fn data_props_answer_attribute_selectors_and_reads_but_not_semantics() {
    let i = interaction(
        "h('view',{},h('button',{'data-testid':'mode','data-mode':'dark','data-selected':true,onClick(){}},'Dark'),h('button',{'data-testid':'light','data-mode':'light',onClick(){}},'Light'))",
    );
    let dark = i
        .automation_query("[data-mode=\"dark\"]", None, false, None)
        .unwrap();
    assert_eq!(dark["text"], "Dark");
    assert_eq!(
        dark["attributes"],
        serde_json::json!({"data-testid": "mode", "data-mode": "dark", "data-selected": ""})
    );
    assert_eq!(
        i.automation_query("[data-mode]", None, true, None).unwrap()["count"],
        2
    );
    assert_eq!(
        i.automation_query("[data-selected]", None, true, None)
            .unwrap()["count"],
        1
    );
    assert!(
        i.automation_query("[data-Mode]", None, false, None)
            .is_err()
    );
    let tree = i.automation_tree().unwrap().to_string();
    assert!(
        !tree.contains("data-mode") && !tree.contains("dark\""),
        "{tree}"
    );
}

#[test]
fn a_parked_page_answers_queries_from_its_last_frame_and_refuses_actions() {
    let mut i = interaction(PAGE);
    i.park();
    let pay = i
        .automation_query("[data-testid=\"pay\"]", None, false, None)
        .unwrap();
    assert_eq!(
        (pay["exists"].as_bool(), pay["text"].as_str()),
        (Some(true), Some("Pay now"))
    );
    assert!(i.automation_tree().is_ok());
    let error = i
        .automate(
            "[data-testid=\"pay\"]",
            None,
            AutomationAct::Click,
            false,
            PageTicket::new(1),
        )
        .unwrap_err();
    assert!(
        error.starts_with("E_PAGE_NOT_READY") && error.contains("parked"),
        "{error}"
    );
}

#[test]
fn unlabelled_editors_answer_placeholder_selectors() {
    let i = interaction(
        "h('view',{},h('input',{'data-testid':'email',value:'',placeholder:'Email'}),h('textarea',{'data-testid':'note',value:'',placeholder:'Note'}),h('input',{'data-testid':'bare',value:''}))",
    );
    let email = i
        .automation_query("input[placeholder=\"Email\"]", None, false, None)
        .unwrap();
    assert_eq!(email["placeholder"], "Email");
    assert_eq!(
        i.automation_query("[placeholder=\"Note\"]", None, false, None)
            .unwrap()["placeholder"],
        "Note"
    );
    assert_eq!(
        i.automation_query("[placeholder]", None, true, None)
            .unwrap()["count"],
        2
    );
}

#[test]
fn a_disclosure_button_announces_expanded() {
    let i = interaction(
        "h('view',{},h('button',{'data-testid':'lic','aria-expanded':false,onClick(){}},'Licenses'))",
    );
    let tree = i.automation_tree().unwrap().to_string();
    assert!(
        tree.contains("Licenses") && tree.contains("\"expanded\":false"),
        "{tree}"
    );
    let summary = i
        .automation_query("[data-testid=\"lic\"]", None, false, None)
        .unwrap();
    assert_eq!(summary["role"], "button");
    assert_eq!(summary["expanded"], false);
}

#[test]
fn a_match_reports_the_aria_states_a_screen_reader_announces() {
    let i = interaction(
        "h('view',{},h('checkbox',{'data-testid':'box',value:true,onChange(){},'aria-label':'Wi-Fi'}),h('button',{'data-testid':'radio',role:'radio','aria-checked':false,onClick(){}},'Daily'),h('button',{'data-testid':'tab',role:'tab','aria-selected':true,onClick(){}},'Week'),h('button',{'data-testid':'bold','aria-pressed':true,onClick(){}},'B'),h('button',{'data-testid':'plain',onClick(){}},'Go'),h('button',{'data-testid':'save','aria-busy':true,onClick(){}},'Save'))",
    );
    let states = |id: &str| {
        let m = i
            .automation_query(&format!("[data-testid=\"{id}\"]"), None, false, None)
            .unwrap();
        ["checked", "pressed", "selected", "expanded", "busy"].map(|k| m[k].as_bool())
    };
    assert_eq!(states("box"), [Some(true), None, None, None, None]);
    assert_eq!(states("radio"), [Some(false), None, None, None, None]);
    assert_eq!(states("tab"), [None, None, Some(true), None, None]);
    assert_eq!(states("bold"), [None, Some(true), None, None, None]);
    assert_eq!(states("plain"), [None; 5]);
    assert_eq!(states("save"), [None, None, None, None, Some(true)]);
}

#[test]
fn authored_aria_states_read_as_attributes_and_match_in_selectors() {
    let i = interaction(
        "h('view',{},h('button',{'data-testid':'bold','aria-pressed':true,'data-pressed':'no',onClick(){}},'B'),h('button',{'data-testid':'menu','aria-expanded':false,onClick(){}},'More'),h('button',{'data-testid':'tab',role:'tab','aria-selected':true,onClick(){}},'Week'),h('button',{'data-testid':'save','aria-busy':true,onClick(){}},'Save'),h('button',{'data-testid':'plain',onClick(){}},'Go'))",
    );
    let attribute = |id: &str, name: &str| {
        i.automation_query(&format!("[data-testid=\"{id}\"]"), None, false, None)
            .unwrap()["attributes"][name]
            .as_str()
            .map(str::to_owned)
    };
    assert_eq!(attribute("bold", "aria-pressed").as_deref(), Some("true"));
    assert_eq!(attribute("menu", "aria-expanded").as_deref(), Some("false"));
    assert_eq!(attribute("tab", "aria-selected").as_deref(), Some("true"));
    assert_eq!(attribute("save", "aria-busy").as_deref(), Some("true"));
    assert_eq!(attribute("plain", "aria-pressed"), None);
    let found = |selector: &str| {
        i.automation_query(selector, None, false, None).unwrap()["attributes"]["data-testid"]
            .as_str()
            .map(str::to_owned)
    };
    assert_eq!(found("[aria-pressed=\"true\"]").as_deref(), Some("bold"));
    assert_eq!(
        found("button[aria-expanded=\"false\"]").as_deref(),
        Some("menu")
    );
    assert_eq!(found("[aria-busy]").as_deref(), Some("save"));
    // An ARIA name never reads a data-* prop.
    assert_eq!(found("[aria-pressed=\"no\"]"), None);
}

#[test]
fn an_authored_label_reads_as_the_aria_label_attribute_and_text_alone_does_not() {
    let i = interaction(
        "h('view',{},h('button',{'data-testid':'play','aria-label':'Pause',onClick(){}},h('text',{},'||')),h('button',{'data-testid':'go',onClick(){}},'Go'))",
    );
    let label = |id: &str| {
        i.automation_query(&format!("[data-testid=\"{id}\"]"), None, false, None)
            .unwrap()["attributes"]["aria-label"]
            .as_str()
            .map(str::to_owned)
    };
    assert_eq!(label("play").as_deref(), Some("Pause"));
    // As in a DOM: a name computed from content is not an attribute.
    assert_eq!(label("go"), None);
}

#[test]
fn a_disabled_link_renders_takes_no_press_and_reads_unavailable() {
    let i = interaction(
        "h('view',{},h('text',{},'Read the ',h('link',{'data-testid':'off',disabled:true,onClick(){}},'terms'),' and the ',h('link',{'data-testid':'on',onClick(){}},'policy')))",
    );
    let read = |id: &str| {
        i.automation_query(&format!("[data-testid=\"{id}\"]"), None, false, None)
            .unwrap()
    };
    assert_eq!(read("off")["exists"], true);
    assert_eq!(read("off")["enabled"], false);
    assert_eq!(read("on")["enabled"], true);
}

#[test]
fn a_partly_checked_checkbox_reads_mixed_and_a_press_checks_it() {
    let mut i = interaction(
        "h('view',{},h('checkbox',{'data-testid':'all',value:false,indeterminate:true,onChange(){},'aria-label':'All'}),h('checkbox',{'data-testid':'one',value:true,onChange(){},'aria-label':'One'}),h('button',{'data-testid':'item',role:'menuitemcheckbox','aria-checked':'mixed',onClick(){}},'Columns'))",
    );
    let read = |i: &ViewInteraction, id: &str| {
        i.automation_query(&format!("[data-testid=\"{id}\"]"), None, false, None)
            .unwrap()
    };
    assert_eq!(read(&i, "all")["checked"], "mixed");
    assert_eq!(read(&i, "all")["attributes"]["aria-checked"], "mixed");
    assert_eq!(read(&i, "one")["checked"], true);
    assert_eq!(read(&i, "item")["checked"], "mixed");
    // The tree a screen reader walks says so too.
    let tree = i.automation_tree().unwrap().to_string();
    assert_eq!(tree.matches("\"mixed\":true").count(), 2, "{tree}");
    // A press is a press: the page hears a change and decides what the box becomes.
    i.automate(
        "[data-testid=\"all\"]",
        None,
        AutomationAct::Click,
        false,
        PageTicket::new(1),
    )
    .unwrap();
    assert_eq!(i.take_dispatch().len(), 1);
}

#[test]
fn any_drag_element_reads_its_point_as_its_value_whatever_its_role() {
    // Two pages: an open modal sheet takes the rest of its page out of the accessibility tree.
    let panel = interaction(
        "h('view',{},h('view',{'data-testid':'panel',drag:'x',dragSnapPoints:[0,40,80],bind:useLocal('at',1),'aria-label':'Panel',style:{width:120,height:40}}))",
    );
    let i = interaction(
        "h('view',{},h('dialog',{open:true,onClose(){},'aria-label':'Sheet','data-testid':'sheet',drag:'y',dragSnapPoints:[0,'50%','100%'],style:{marginTop:'auto',height:100}},h('text',{},'Hi')))",
    );
    let read = |i: &ViewInteraction, id: &str| {
        let m = i
            .automation_query(&format!("[data-testid=\"{id}\"]"), None, false, None)
            .unwrap();
        (
            m["role"].as_str().map(str::to_owned),
            m["value"].as_str().map(str::to_owned),
        )
    };
    assert_eq!(
        read(&panel, "panel"),
        (Some("slider".into()), Some("2".into()))
    );
    assert_eq!(read(&i, "sheet"), (Some("dialog".into()), Some("1".into())));
    // A screen reader reads the same point from the sheet's range.
    let p = i.semantics().unwrap();
    let index = p
        .frame
        .nodes
        .iter()
        .position(|n| n.test_id == "sheet")
        .unwrap();
    let range = p.node(p.ids.id(index).unwrap()).unwrap().range.unwrap();
    assert_eq!((range.min, range.max, range.value), (1.0, 3.0, 1.0));
}

#[test]
fn a_click_on_a_controls_label_lands_on_the_control() {
    let row = |disabled: bool| {
        format!(
            "h('view',{{}},h('checkbox',{{'data-testid':'box',value:false,onChange(){{}},disabled:{disabled},'aria-label':'Wi-Fi'}},h('view',{{style:{{flexDirection:'row'}}}},h('text',{{'data-testid':'label'}},'Wi-Fi'))),h('text',{{'data-testid':'loose'}},'Not a control'))"
        )
    };
    let page = PageTicket::new(1);
    let mut i = interaction(&row(false));
    let effects = i
        .automate(
            "[data-testid=\"label\"]",
            None,
            AutomationAct::Click,
            false,
            page,
        )
        .unwrap();
    assert!(effects.contains(&Effect::Dispatch), "{effects:?}");
    let refused = i
        .automate(
            "[data-testid=\"loose\"]",
            None,
            AutomationAct::Click,
            false,
            page,
        )
        .unwrap_err();
    assert!(refused.contains("not interactable"), "{refused}");
    let mut i = interaction(&row(true));
    let refused = i
        .automate(
            "[data-testid=\"label\"]",
            None,
            AutomationAct::Click,
            false,
            page,
        )
        .unwrap_err();
    assert!(refused.contains("not enabled"), "{refused}");
}

#[test]
fn a_match_under_a_bottom_bar_is_outside_the_viewport_until_revealed() {
    let mut i = interaction(
        "h('scroll',{style:{height:400,paddingBottom:60}},h('view',{style:{height:345}}),h('button',{'data-testid':'under',onClick(){}},'Under'))",
    );
    let under = |i: &ViewInteraction| {
        i.automation_query("[data-testid=\"under\"]", None, false, None)
            .unwrap()["inViewport"]
            .as_bool()
    };
    assert_eq!(under(&i), Some(true));
    i.set_obscured([[0.0, 0.0, 60.0, 0.0], [0.0; 4]]);
    assert_eq!(under(&i), Some(false));
    let page = PageTicket::new(1);
    let refused = i
        .automate(
            "[data-testid=\"under\"]",
            None,
            AutomationAct::Click,
            false,
            page,
        )
        .unwrap_err();
    assert!(refused.contains("outside the viewport"), "{refused}");
    let revealed = i
        .automate(
            "[data-testid=\"under\"]",
            None,
            AutomationAct::ScrollIntoView,
            false,
            page,
        )
        .unwrap();
    assert!(revealed.contains(&Effect::Redraw));
}

#[test]
fn a_reveal_in_an_unpadded_root_scroll_stops_above_the_bar() {
    let mut i = interaction(
        "h('scroll',{style:{height:400}},h('view',{style:{height:350}}),h('button',{'data-testid':'under',style:{height:30},onClick(){}},'Under'),h('view',{style:{height:200}}))",
    );
    i.set_obscured([[0.0, 0.0, 60.0, 0.0], [0.0; 4]]);
    let frame = i.frame().unwrap().clone();
    let node = frame.nodes.iter().find(|n| n.test_id == "under").unwrap();
    let offsets = i.scroll().reveal_offsets(&frame, node).unwrap();
    // The button ends at 380; above a 60 px bar its end sits at 340.
    assert_eq!(offsets.values().copied().collect::<Vec<_>>(), vec![40.0]);
}

#[test]
fn an_open_dialog_rises_above_host_bars_but_not_the_system_ones() {
    let mut i = interaction(
        "h('page',{},h('dialog',{open:true,'aria-label':'Sheet',style:{width:300,height:400,justifyContent:'flex-end'}},h('button',{'data-testid':'apply',style:{height:30},onClick(){}},'Apply')))",
    );
    let apply = |i: &ViewInteraction| {
        i.automation_query("[data-testid=\"apply\"]", None, false, None)
            .unwrap()["inViewport"]
            .as_bool()
    };
    i.set_obscured([[0.0, 0.0, 60.0, 0.0], [0.0; 4]]);
    assert_eq!(apply(&i), Some(true));
    i.set_obscured([[0.0, 0.0, 60.0, 0.0], [0.0, 0.0, 40.0, 0.0]]);
    assert_eq!(apply(&i), Some(false));
}

#[test]
fn a_match_out_of_the_accessibility_tree_can_still_be_revealed() {
    let mut i = interaction(
        "h('scroll',{style:{height:100}},h('view',{style:{height:400}}),h('view',{style:{flexDirection:'row'},'aria-hidden':true},h('button',{'data-testid':'dot',onClick(){}},'')))",
    );
    let page = PageTicket::new(1);
    let revealed = i
        .automate(
            "[data-testid=\"dot\"]",
            None,
            AutomationAct::ScrollIntoView,
            false,
            page,
        )
        .unwrap();
    assert!(revealed.contains(&Effect::Redraw));
}

#[test]
fn a_progressbar_reads_its_share_or_its_value_text_and_has_no_value_while_indeterminate() {
    let i = interaction(
        "h('view',{},h('view',{'data-testid':'up',role:'progressbar','aria-label':'Upload','aria-valuenow':42,style:{height:4}}),h('view',{'data-testid':'steps',role:'progressbar','aria-label':'Steps','aria-valuenow':2,'aria-valuemin':1,'aria-valuemax':5,'aria-valuetext':'Step 2 of 5',style:{height:4}}),h('view',{'data-testid':'spin',role:'progressbar','aria-label':'Loading',style:{height:4}}))",
    );
    let found = |id: &str| {
        i.automation_query(&format!("[data-testid=\"{id}\"]"), None, false, None)
            .unwrap()
    };
    assert_eq!(found("up")["role"], "progressbar");
    assert_eq!(found("up")["value"], "42");
    let tree = i.automation_tree().unwrap();
    assert_eq!(
        tree["nodes"][0]["value"],
        serde_json::json!({"max":100.0,"min":0.0,"now":42.0,"text":"42%"}),
        "{tree}"
    );
    assert_eq!(tree["nodes"][1]["value"]["text"], "Step 2 of 5", "{tree}");
    assert_eq!(found("spin")["role"], "progressbar");
    assert!(found("spin")["value"].is_null());
}

#[test]
fn the_visible_rect_is_the_box_less_scroll_clips_and_covered_edges() {
    let mut i = interaction(
        "h('scroll',{style:{height:400}},h('view',{style:{height:330}}),h('button',{'data-testid':'half',style:{height:40},onClick(){}},'Half'),h('view',{style:{height:300}}))",
    );
    i.set_obscured([[0.0, 0.0, 60.0, 0.0], [0.0; 4]]);
    let half = i
        .automation_query("[data-testid=\"half\"]", None, false, None)
        .unwrap();
    assert_eq!(half["inViewport"], true);
    assert_eq!(half["rect"]["height"], 40.0);
    assert_eq!(half["visibleRect"]["top"], 330.0);
    assert_eq!(half["visibleRect"]["height"], 10.0);
}

#[test]
fn a_descendant_chain_scopes_a_role_query_to_a_subtree() {
    let i = interaction(
        "h('view',{},h('view',{'data-testid':'bar',role:'toolbar'},h('button',{onClick(){}},'Save'),h('view',{},h('button',{onClick(){}},'Share'))),h('button',{onClick(){}},'Save'),h('view',{'data-testid':'empty'}))",
    );
    let all = |selector: &str| i.automation_query(selector, None, true, None).unwrap();
    let texts = |value: serde_json::Value| -> Vec<String> {
        value["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["text"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(texts(all("button")), ["Save", "Share", "Save"]);
    assert_eq!(
        texts(all(
            "[data-testid=\"bar\"] button, [data-testid=\"bar\"] [role=\"button\"]"
        )),
        ["Save", "Share"]
    );
    assert_eq!(texts(all("[role=\"toolbar\"] div button")), ["Share"]);
    assert_eq!(all("[data-testid=\"empty\"] button")["count"], 0);
    // The scope is an ancestor, never the node itself.
    assert_eq!(
        all("[data-testid=\"bar\"] [data-testid=\"bar\"]")["count"],
        0
    );
    assert!(
        i.automation_query("[data-testid=\"bar\"] > button", None, true, None)
            .unwrap_err()
            .starts_with("E_SELECTOR_UNSUPPORTED")
    );
}

#[test]
fn an_editor_reads_the_keyboard_it_asks_for_as_inputmode() {
    let i = interaction(
        "h('view',{},h('input',{'data-testid':'pin','aria-label':'PIN',inputMode:'numeric',value:''}),h('input',{'data-testid':'mail','aria-label':'Mail',type:'email',value:''}),h('input',{'data-testid':'name','aria-label':'Name',value:''}),h('input',{'data-testid':'day','aria-label':'Day',type:'date',value:''}))",
    );
    let mode = |id: &str| {
        i.automation_query(&format!("[data-testid=\"{id}\"]"), None, false, None)
            .unwrap()["attributes"]["inputmode"]
            .as_str()
            .map(str::to_owned)
    };
    assert_eq!(mode("pin").as_deref(), Some("numeric"));
    assert_eq!(mode("mail").as_deref(), Some("email"));
    assert_eq!(mode("name"), None, "plain text asks for nothing");
    assert_eq!(
        mode("day"),
        None,
        "a date field has a picker, not a keyboard"
    );
    // The tree dump carries the same key.
    let tree = i.automation_tree().unwrap().to_string();
    assert!(tree.contains(r#""inputmode":"numeric""#), "{tree}");
    assert_eq!(tree.matches(r#""inputmode""#).count(), 2, "{tree}");
}

#[test]
fn a_typed_key_the_keyboard_has_no_place_for_fails_at_once() {
    let mut i = interaction(
        "h('view',{},h('input',{'data-testid':'n','aria-label':'Count',inputMode:'numeric',value:'7'}),h('input',{'data-testid':'d','aria-label':'Price',inputMode:'decimal',value:'7.0'}))",
    );
    let page = PageTicket::new(1);
    let mut typed = |id: &str, text: &str| {
        i.automate(
            &format!("[data-testid=\"{id}\"]"),
            None,
            AutomationAct::Type(text.into()),
            false,
            page,
        )
    };
    let refused = typed("n", ".").unwrap_err();
    assert!(
        refused.starts_with("E_EDIT_REFUSED") && refused.contains("number field"),
        "{refused}"
    );
    // A decimal field takes one point.
    assert!(typed("d", ".").unwrap_err().starts_with("E_EDIT_REFUSED"));
    // A full-width digit is taken as its ASCII digit: the edit sends what the field will show.
    let effects = typed("n", "\u{FF18}").unwrap();
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::EditText { text: Some(t), .. } if t == "78")),
    );
}

#[test]
fn hidden_text_is_never_read_into_a_description_or_a_name() {
    let i = interaction(
        "h('view',{},h('input',{'data-testid':'field','aria-label':'Email','aria-describedby':'help',value:''}),h('view',{id:'help'},h('view',{'aria-hidden':true},h('text',{},'HIDDEN-A')),h('text',{},'Saved')),h('button',{'data-testid':'go',onClick(){}},h('text',{},h('span',{'aria-hidden':true},'HIDDEN-B'),'Send')))",
    );
    let tree = i.automation_tree().unwrap().to_string();
    assert!(tree.contains(r#""description":"Saved""#), "{tree}");
    assert!(!tree.contains("HIDDEN"), "{tree}");
}

#[test]
fn typing_inserts_at_a_focused_editors_selection_and_moves_the_caret_past_it() {
    let page = PageTicket::new(1);
    let typed = |selection: [u32; 2], text: &str| {
        let source = crate::session::tests::source(
            "function render(){return h('input',{'data-testid':'a','aria-label':'Amount',inputMode:'decimal',value:'2.5'})}",
        );
        let mut session = crate::ViewSession::new(&source).unwrap();
        let mut display = session.layout(300, 400, 1.0).unwrap();
        for node in &mut display.nodes {
            if let Some(input) = &mut node.input {
                input.edit_token = Some(1);
                input.edit = Some(crate::text::input::Snapshot {
                    text: "2.5".into(),
                    selection,
                    composition: None,
                    focused: true,
                    can_undo: false,
                    can_redo: false,
                });
            }
        }
        let mut focused = ViewInteraction::default();
        focused.presented(
            Arc::new(PresentedFrame::new(
                CommitTicket::new(1),
                1.0,
                display.nodes,
            )),
            0,
            &Default::default(),
            page,
        );
        focused
            .automate(
                "[data-testid=\"a\"]",
                None,
                AutomationAct::Type(text.into()),
                false,
                page,
            )
            .unwrap()
            .into_iter()
            .filter_map(|e| match e {
                Effect::EditText { text, start, .. } => Some((text, start)),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    // The caret between "." and "5", as a tap there leaves it.
    assert_eq!(typed([2, 2], "9"), [(Some("2.95".into()), -1), (None, 3)]);
    // A selection is replaced.
    assert_eq!(typed([0, 3], "7"), [(Some("7".into()), -1), (None, 1)]);
}

#[test]
fn automation_reads_no_break_spaces_as_written() {
    // Specs compare strings: "14 days" with U+00A0 must read back with it.
    let i = interaction(
        "h('view',{},h('text',{'data-testid':'up'},'Uptime 14\\u00a0days 6\\u202fhr'),h('button',{'data-testid':'go','aria-label':'Wait 14\\u00a0days',style:{height:40}},'go'))",
    );
    let up = i
        .automation_query("[data-testid=\"up\"]", None, false, None)
        .unwrap();
    assert_eq!(up["text"], "Uptime 14\u{a0}days 6\u{202f}hr");
    let go = i
        .automation_query("[data-testid=\"go\"]", None, false, None)
        .unwrap();
    assert_eq!(go["aria_label"], "Wait 14\u{a0}days");
}
