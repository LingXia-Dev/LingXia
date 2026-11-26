//! Date and time fields paint and read their value in the user's clock, keep it ISO for the
//! page and automation, and are never narrower than the widest value their format can show.
use crate::ViewSession;
use crate::scene::paint::Paint;

fn environment(hour_cycle: &str) -> serde_json::Value {
    serde_json::json!({"appearance":"light","insets":{"top":0,"right":0,"bottom":0,"left":0},"keyboard":{"height":0},"os":"android","language":"en-US","canDock":false,"hourCycle":hour_cycle})
}

fn page(body: &str, hour_cycle: &str) -> ViewSession {
    let mut page = ViewSession::new(&super::source(&format!(
        "function render(){{return h('view',{{style:{{flexDirection:'row',alignItems:'flex-start'}}}},{body})}}"
    )))
    .unwrap();
    page.set_environment(environment(hour_cycle)).unwrap();
    page
}

fn glyphs(display: &crate::DisplayList) -> usize {
    display
        .commands
        .iter()
        .map(|paint| match paint {
            Paint::Text { glyphs, .. } => glyphs.len(),
            _ => 0,
        })
        .sum()
}

fn spoken(display: &crate::DisplayList, index: usize) -> String {
    let mut interaction = crate::interaction::view::ViewInteraction::default();
    interaction.presented(
        std::sync::Arc::new(crate::interaction::frame::PresentedFrame::new(
            crate::session::lifecycle::CommitTicket::new(1),
            1.0,
            display.nodes.clone(),
        )),
        0,
        &Default::default(),
        crate::session::lifecycle::PageTicket::new(1),
    );
    let semantics = interaction.semantics().unwrap();
    let id = semantics.ids.id(index).unwrap();
    semantics.node(id).unwrap().text.clone()
}

const FIELD: &str =
    "h('input',{'data-testid':'t',type:'time',value:'13:05',style:{width:40,padding:8}})";

#[test]
fn a_time_field_shows_the_users_clock_and_keeps_its_value_iso() {
    let mut field = page(FIELD, "h12");
    let display = field.prepare_native(400, 200, 1.0, true, None, Ok).unwrap();
    let index = display.nodes.iter().position(|n| n.tag == "input").unwrap();
    let node = &display.nodes[index];
    assert_eq!(
        node.input.as_ref().unwrap().value,
        "13:05",
        "the page and automation see ISO"
    );
    assert_eq!(
        spoken(&display, index),
        "1:05 PM",
        "the screen reader reads the clock"
    );
    let mut plain = page(
        "h('input',{value:'1:05 PM',style:{width:200,padding:8}})",
        "h12",
    );
    let shown = plain.prepare_native(400, 200, 1.0, true, None, Ok).unwrap();
    assert_eq!(
        glyphs(&display),
        glyphs(&shown),
        "paints the clock, not the ISO value"
    );
    assert_eq!(node.text, "1:05 PM", "text content is what the field shows");
    assert!(
        !serde_json::to_string(&display.nodes)
            .unwrap()
            .contains("00:00"),
        "the sizing hint never reaches the platform or automation"
    );
    assert!(!spoken(&display, index).contains("00:00"));

    // A 40 pt field grows to fit "12:00 PM"; the clock changing while open re-lowers it.
    let twelve = node.bounds[2];
    assert!(
        twelve > 40.0,
        "never narrower than its widest value: {twelve}"
    );
    field.set_environment(environment("h23")).unwrap();
    let display = field.prepare_native(400, 200, 1.0, true, None, Ok).unwrap();
    let node = &display.nodes[index];
    assert_eq!(spoken(&display, index), "13:05");
    assert!(
        node.bounds[2] > 40.0 && node.bounds[2] < twelve,
        "a 24-hour field needs less room: {} vs {twelve}",
        node.bounds[2]
    );
}

#[test]
fn an_authored_wider_minimum_wins_and_dates_fit_too() {
    let mut wide = page(
        "h('input',{type:'time',value:'13:05',style:{width:40,minWidth:200,padding:8}})",
        "h12",
    );
    let display = wide.prepare_native(400, 200, 1.0, true, None, Ok).unwrap();
    let node = display.nodes.iter().find(|n| n.tag == "input").unwrap();
    assert_eq!(node.bounds[2], 200.0);

    let mut date = page(
        "h('input',{type:'date',value:'2026-11-14',style:{width:20,padding:8}})",
        "h12",
    );
    let display = date.prepare_native(400, 200, 1.0, true, None, Ok).unwrap();
    let index = display.nodes.iter().position(|n| n.tag == "input").unwrap();
    assert!(
        display.nodes[index].bounds[2] > 60.0,
        "a date field fits a full date"
    );
    assert_eq!(
        display.nodes[index].input.as_ref().unwrap().value,
        "2026-11-14",
        "the page and automation see ISO"
    );
    assert_eq!(
        spoken(&display, index),
        "Nov 14, 2026",
        "the screen reader reads the date the field shows"
    );
}

#[test]
fn an_unbound_time_field_shows_the_users_pick_in_their_clock() {
    let mut field = page("h('input',{type:'time',style:{width:40,padding:8}})", "h12");
    let display = field.prepare_native(400, 200, 1.0, true, None, Ok).unwrap();
    let index = display.nodes.iter().position(|n| n.tag == "input").unwrap();
    let identity = display.nodes[index].identity.clone();
    assert!(
        field
            .choose_editor_value(&identity, "13:05".into())
            .unwrap()
    );
    let display = field.prepare_native(400, 200, 1.0, true, None, Ok).unwrap();
    let node = &display.nodes[index];
    assert_eq!(node.input.as_ref().unwrap().value, "13:05");
    assert_eq!(node.text, "1:05 PM", "unfocused, it shows the clock");
    assert_eq!(spoken(&display, index), "1:05 PM");
    // The clock changing re-formats the kept value.
    field.set_environment(environment("h23")).unwrap();
    let display = field.prepare_native(400, 200, 1.0, true, None, Ok).unwrap();
    assert_eq!(display.nodes[index].text, "13:05");
    assert_eq!(display.nodes[index].input.as_ref().unwrap().value, "13:05");
}
