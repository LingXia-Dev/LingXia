//! Authored roles, heading levels, hidden subtrees and live regions.
use crate::interaction::frame::PresentedFrame;
use crate::semantics::{
    ids::SemanticIds,
    projection::{self, Event, Projection, Role, SemanticNode},
};

fn frame(render: &str) -> PresentedFrame {
    let source = crate::session::tests::source(&format!(
        "function render(){{const n=useLocal('n',0);return {render}}}"
    ));
    let mut page = crate::ViewSession::new(&source).unwrap();
    let display = page.layout(300, 400, 1.0).unwrap();
    PresentedFrame::new(
        crate::session::lifecycle::CommitTicket::new(1),
        1.0,
        display.nodes,
    )
}

fn semantic(frame: &PresentedFrame, test_id: &str) -> Option<SemanticNode> {
    let mut ids = SemanticIds::default();
    ids.update(frame.nodes.iter().map(|n| n.identity.as_str()));
    let index = frame.nodes.iter().position(|n| n.test_id == test_id)?;
    Projection {
        frame,
        ids: &ids,
        keyboard_focus: None,
        inert: false,
    }
    .node(ids.id(index)?)
}

#[test]
fn authored_roles_project_with_levels_positions_and_names() {
    let frame = frame(
        "h('column',{},h('text',{testId:'h',role:'heading',headingLevel:3},'Plans'),h('text',{testId:'h2',role:'heading'},'Default'),h('column',{testId:'list',role:'list',accessibilityLabel:'Bills'},h('row',{testId:'a',role:'listitem'},h('text',{},'Water')),h('row',{testId:'b',role:'listitem'},h('text',{},'Power'))),h('column',{testId:'d',role:'dialog',accessibilityLabel:'Confirm'},h('button',{testId:'ok',role:'heading',onClick(){}},'OK')))",
    );
    let heading = semantic(&frame, "h").unwrap();
    assert_eq!(
        (heading.role, heading.heading_level, heading.name.as_str()),
        (Role::Heading, Some(3), "Plans")
    );
    assert_eq!(semantic(&frame, "h2").unwrap().heading_level, Some(2));
    let list = semantic(&frame, "list").unwrap();
    assert_eq!(
        (list.role, list.name.as_str(), list.position),
        (Role::List, "Bills", Some((0, 2)))
    );
    assert_eq!(semantic(&frame, "b").unwrap().position, Some((1, 1)));
    assert_eq!(semantic(&frame, "b").unwrap().role, Role::ListItem);
    let dialog = semantic(&frame, "d").unwrap();
    assert_eq!(
        (dialog.role, dialog.name.as_str()),
        (Role::Dialog, "Confirm")
    );
    // Controls keep their own role.
    assert_eq!(semantic(&frame, "ok").unwrap().role, Role::Button);
}

#[test]
fn hidden_subtrees_leave_semantics_but_keep_paint() {
    let frame = frame(
        "h('column',{testId:'c',accessibilityHidden:true},h('text',{testId:'t'},'Decor'),h('button',{testId:'b',onClick(){}},'Ghost'))",
    );
    for id in ["c", "t", "b"] {
        assert!(semantic(&frame, id).is_none(), "{id}");
    }
    let button = frame.nodes.iter().find(|n| n.test_id == "b").unwrap();
    assert!(button.on_click.is_some() && button.bounds[2] > 0.0);
}

#[test]
fn live_regions_announce_changed_text_with_their_politeness() {
    let render = |text: &str| {
        frame(&format!(
            "h('column',{{}},h('column',{{testId:'s',role:'status'}},h('text',{{}},'{text}')),h('text',{{testId:'a',role:'alert'}},'Fix {text}'),h('text',{{testId:'q',accessibilityLiveRegion:'off',role:'alert'}},'{text}'))"
        ))
    };
    let (before, after) = (render("Saved"), render("Failed"));
    let mut ids = SemanticIds::default();
    ids.update(after.nodes.iter().map(|n| n.identity.as_str()));
    let announced: Vec<_> = projection::changes(Some(&before), &after, &ids)
        .into_iter()
        .filter_map(|event| match event {
            Event::Announce {
                text, assertive, ..
            } => Some((text, assertive)),
            _ => None,
        })
        .collect();
    assert_eq!(
        announced,
        [
            ("Failed".to_owned(), false),
            ("Fix Failed".to_owned(), true)
        ]
    );
    assert_eq!(
        semantic(&after, "s").unwrap().live_region.as_deref(),
        Some("polite")
    );
    assert_eq!(semantic(&after, "q").unwrap().live_region, None);
}

#[test]
fn heading_levels_need_a_heading_role_on_the_wire() {
    use crate::protocol::{decode, field, tag};
    use serde_json::json;
    let wire = |fields: serde_json::Value| {
        let mut wire = vec![
            json!(tag::TEXT),
            json!(-1),
            json!(""),
            json!(""),
            json!("x"),
        ];
        let fields = fields.as_array().unwrap().clone();
        wire.push(json!(fields.len() / 2));
        wire.extend(fields);
        decode(&wire)
    };
    assert!(wire(json!([field::ROLE, "heading", field::HEADING_LEVEL, 1])).is_ok());
    assert!(
        wire(json!([field::HEADING_LEVEL, 1]))
            .unwrap_err()
            .contains("role heading")
    );
    assert!(wire(json!([field::ROLE, "heading", field::HEADING_LEVEL, 7])).is_err());
    assert!(wire(json!([field::ROLE, "banner"])).is_err());
    assert!(wire(json!([field::ACCESSIBILITY_HIDDEN, 0])).is_err());
}

#[test]
fn projected_roles_match_the_schema_vocabulary() {
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../packages/lingxia-ui/contracts/schema.json"
    ))
    .unwrap();
    let roles: Vec<String> = [
        Role::Button,
        Role::CheckBox,
        Role::Switch,
        Role::RadioGroup,
        Role::RadioButton,
        Role::TextField,
        Role::ScrollView,
        Role::HorizontalScrollView,
        Role::Image,
        Role::Slider,
        Role::Text,
        Role::Link,
        Role::Heading,
        Role::List,
        Role::ListItem,
        Role::Dialog,
        Role::Alert,
        Role::Status,
    ]
    .iter()
    .map(|role| {
        serde_json::to_value(role)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned()
    })
    .collect();
    assert_eq!(serde_json::json!(roles), schema["semantics"]["roles"]);
}
