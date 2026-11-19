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
    ids.update(frame.nodes.iter().map(|n| n.identity.as_ref()));
    let index = frame.nodes.iter().position(|n| n.test_id == test_id)?;
    Projection {
        frame,
        ids: &ids,
        keyboard_focus: None,
        inert: false,
        drags: None,
        obscured: [0.0; 4],
        touch_target: 0.0,
    }
    .node(ids.id(index)?)
}

#[test]
fn authored_roles_project_with_levels_positions_and_names() {
    let frame = frame(
        "h('view',{},h('text',{'data-testid':'h',role:'heading',headingLevel:3},'Plans'),h('text',{'data-testid':'h2',role:'heading'},'Default'),h('view',{'data-testid':'list',role:'list','aria-label':'Bills'},h('view',{style:{flexDirection:'row'},'data-testid':'a',role:'listitem'},h('text',{},'Water')),h('view',{style:{flexDirection:'row'},'data-testid':'b',role:'listitem'},h('text',{},'Power'))),h('view',{'data-testid':'d',role:'menu','aria-label':'Confirm'},h('button',{'data-testid':'ok',role:'heading',onClick(){}},'OK')))",
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
    let menu = semantic(&frame, "d").unwrap();
    assert_eq!((menu.role, menu.name.as_str()), (Role::Menu, "Confirm"));
    // Controls keep their own role.
    assert_eq!(semantic(&frame, "ok").unwrap().role, Role::Button);
}

#[test]
fn hidden_subtrees_leave_semantics_but_keep_paint() {
    let frame = frame(
        "h('view',{'data-testid':'c','aria-hidden':true},h('text',{'data-testid':'t'},'Decor'),h('button',{'data-testid':'b',onClick(){}},'Ghost'))",
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
            "h('view',{{}},h('view',{{'data-testid':'s',role:'status'}},h('text',{{}},'{text}')),h('text',{{'data-testid':'a',role:'alert'}},'Fix {text}'),h('text',{{'data-testid':'q','aria-live':'off',role:'alert'}},'{text}'))"
        ))
    };
    let (before, after) = (render("Saved"), render("Failed"));
    let mut ids = SemanticIds::default();
    ids.update(after.nodes.iter().map(|n| n.identity.as_ref()));
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
fn an_alert_speaks_when_it_appears_and_a_status_only_when_it_changes() {
    let before = frame("h('view',{},h('text',{},'Form'))");
    let after = frame(
        "h('view',{},h('text',{},'Form'),h('text',{role:'alert'},'Card declined'),h('text',{role:'status'},'Saved'),h('text',{role:'alert','aria-live':'off'},'Quiet'))",
    );
    let mut ids = SemanticIds::default();
    ids.update(after.nodes.iter().map(|n| n.identity.as_ref()));
    let announced = |previous: Option<&PresentedFrame>| -> Vec<_> {
        projection::changes(previous, &after, &ids)
            .into_iter()
            .filter_map(|event| match event {
                Event::Announce {
                    text, assertive, ..
                } => Some((text, assertive)),
                _ => None,
            })
            .collect()
    };
    assert_eq!(
        announced(Some(&before)),
        [("Card declined".to_owned(), true)]
    );
    // A page that opens with an alert is read with the page, not announced over it.
    assert!(announced(None).is_empty());
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
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../../../contracts/view/schema.json")).unwrap();
    let roles: Vec<String> = [
        Role::Button,
        Role::CheckBox,
        Role::Switch,
        Role::RadioGroup,
        Role::RadioButton,
        Role::ComboBox,
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
        Role::Grid,
        Role::GridCell,
        Role::ListBox,
        Role::Option,
        Role::Menu,
        Role::MenuItem,
        Role::Video,
        Role::ProgressBar,
        Role::AlertDialog,
        Role::Group,
        Role::Tooltip,
        Role::TabList,
        Role::Tab,
        Role::TabPanel,
        Role::Toolbar,
        Role::Tree,
        Role::TreeItem,
        Role::Row,
        Role::MenuItemCheckBox,
        Role::MenuItemRadio,
        Role::WebView,
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

#[test]
fn a_select_speaks_its_chosen_label_else_its_placeholder() {
    let options = "options:[{value:'monthly',label:'Monthly'},{value:'yearly',label:'Yearly'}]";
    let frame = frame(&format!(
        "h('view',{{}},h('select',{{'data-testid':'chosen','aria-label':'Billing plan',placeholder:'Choose a plan',{options},value:'yearly',onChange(){{}}}}),h('select',{{'data-testid':'empty','aria-label':'Billing plan',placeholder:'Choose a plan',{options},value:'',onChange(){{}}}}))"
    ));
    let chosen = semantic(&frame, "chosen").unwrap();
    assert_eq!(
        (chosen.role, chosen.name.as_str(), chosen.value.as_deref()),
        (Role::ComboBox, "Billing plan", Some("Yearly"))
    );
    assert_eq!(
        semantic(&frame, "empty").unwrap().value.as_deref(),
        Some("Choose a plan")
    );
}

#[test]
fn an_open_dialog_is_modal_and_escapes_its_clipped_scrolled_parent() {
    let frame = frame(
        "h('page',{},h('scroll',{style:{height:80,overflow:'hidden'}},h('button',{'data-testid':'behind',onClick(){}},'Behind'),h('view',{style:{height:400}}),h('dialog',{open:true,'aria-label':'Confirm','data-testid':'d',style:{width:200,height:100}},h('button',{'data-testid':'ok',onClick(){}},'OK'))))",
    );
    assert!(
        semantic(&frame, "behind").is_none(),
        "the page behind is hidden"
    );
    let dialog = semantic(&frame, "d").unwrap();
    assert_eq!(
        (dialog.role, dialog.name.as_str()),
        (Role::Dialog, "Confirm")
    );
    assert!(semantic(&frame, "ok").is_some());
    // Laid out against the page, centred by its auto margins, outside the 80 px scroll.
    let node = frame.nodes.iter().find(|n| n.test_id == "d").unwrap();
    assert_eq!(node.bounds, [50.0, 150.0, 200.0, 100.0]);
}

#[test]
fn a_menu_and_its_items_project_their_roles() {
    let frame = frame(
        "h('page',{},h('button',{popoverTarget:'m'},'More'),h('view',{id:'m',popover:true,open:true,role:'menu','aria-label':'Actions','data-testid':'menu'},h('button',{role:'menuitem','data-testid':'rename',onClick(){}},'Rename')))",
    );
    let menu = semantic(&frame, "menu").unwrap();
    assert_eq!((menu.role, menu.name.as_str()), (Role::Menu, "Actions"));
    let item = semantic(&frame, "rename").unwrap();
    assert_eq!((item.role, item.name.as_str()), (Role::MenuItem, "Rename"));
}

#[test]
fn described_invalid_required_fields_and_group_tooltip_roles_project() {
    let frame = frame(
        "h('view',{'data-testid':'g',role:'group','aria-label':'Delivery'},h('input',{'data-testid':'zip','aria-label':'ZIP','aria-invalid':true,'aria-required':true,'aria-describedby':'zip-hint zip-error missing'}),h('text',{id:'zip-hint'},'Five digits'),h('view',{id:'zip-error'},h('text',{},'Not a ZIP'),h('text',{},'here')),h('text',{'data-testid':'tip',role:'tooltip'},'Where we deliver'))",
    );
    let group = semantic(&frame, "g").unwrap();
    assert_eq!((group.role, group.name.as_str()), (Role::Group, "Delivery"));
    let zip = semantic(&frame, "zip").unwrap();
    assert_eq!(
        zip.description.as_deref(),
        Some("Five digits Not a ZIP here")
    );
    assert_eq!((zip.invalid, zip.required), (Some(true), Some(true)));
    let tip = semantic(&frame, "tip").unwrap();
    assert_eq!(
        (tip.role, tip.name.as_str()),
        (Role::Tooltip, "Where we deliver")
    );
    assert_eq!(semantic(&frame, "g").unwrap().invalid, None);
}

#[test]
fn a_combobox_names_the_mounted_list_it_controls() {
    let frame = frame(
        "h('view',{},h('input',{'data-testid':'city','aria-label':'City',role:'combobox','aria-controls':'gone cities','aria-autocomplete':'list'}),h('view',{id:'cities',role:'listbox','aria-label':'Cities','data-testid':'list'},h('button',{role:'option',onClick(){}},'Oslo')))",
    );
    let mut ids = SemanticIds::default();
    ids.update(frame.nodes.iter().map(|n| n.identity.as_ref()));
    let list = frame
        .nodes
        .iter()
        .position(|n| n.test_id == "list")
        .unwrap();
    let city = semantic(&frame, "city").unwrap();
    assert_eq!(
        city.controls,
        [ids.id(list).unwrap()],
        "an unmounted id is dropped"
    );
    assert_eq!(city.autocomplete.as_deref(), Some("list"));
    assert_eq!(city.role, Role::ComboBox, "a typeahead reads as a combobox");
    assert!(city.text_field.is_some(), "and stays editable");
}

#[test]
fn an_alertdialog_is_modal_like_a_dialog() {
    let frame = frame(
        "h('page',{},h('button',{'data-testid':'behind',onClick(){}},'Behind'),h('dialog',{open:true,role:'alertdialog','aria-label':'Delete note?','data-testid':'d'},h('button',{'data-testid':'ok',onClick(){}},'Delete')))",
    );
    assert!(semantic(&frame, "behind").is_none());
    let dialog = semantic(&frame, "d").unwrap();
    assert_eq!(
        (dialog.role, dialog.name.as_str()),
        (Role::AlertDialog, "Delete note?")
    );
}

#[test]
fn composite_roles_project_with_positions_checked_states_and_tree_levels() {
    let frame = frame(
        "h('page',{},h('view',{role:'tablist','aria-label':'Sections','data-testid':'tabs'},h('button',{role:'tab','aria-selected':false,onClick(){},'data-testid':'tab-a'},'Overview'),h('button',{role:'tab','aria-selected':true,onClick(){},'data-testid':'tab-b'},'Details')),h('view',{role:'tabpanel','aria-label':'Details','data-testid':'panel'},h('text',{},'Body')),h('view',{role:'radiogroup','aria-label':'Size','data-testid':'size'},h('button',{role:'radio','aria-checked':false,onClick(){}},'S'),h('button',{role:'radio','aria-checked':true,onClick(){},'data-testid':'m'},'M')),h('view',{role:'toolbar','aria-label':'Format','data-testid':'bar'},h('button',{onClick(){}},'Bold')),h('view',{role:'tree','aria-label':'Files','data-testid':'tree'},h('button',{role:'treeitem','aria-level':1,'aria-expanded':true,onClick(){}},'src'),h('button',{role:'treeitem','aria-level':2,onClick(){}},'a.rs'),h('button',{role:'treeitem','aria-level':2,onClick(){},'data-testid':'b'},'b.rs'),h('button',{role:'treeitem','aria-level':1,onClick(){},'data-testid':'docs'},'docs')),h('view',{role:'menu','aria-label':'View'},h('button',{role:'menuitemcheckbox','aria-checked':true,onClick(){},'data-testid':'wrap'},'Wrap'),h('button',{role:'menuitemradio','aria-checked':false,onClick(){},'data-testid':'zoom'},'Zoom')))",
    );
    let tabs = semantic(&frame, "tabs").unwrap();
    assert_eq!(
        (tabs.role, tabs.name.as_str(), tabs.position),
        (Role::TabList, "Sections", Some((0, 2)))
    );
    let tab = semantic(&frame, "tab-b").unwrap();
    assert_eq!(
        (tab.role, tab.selected, tab.position),
        (Role::Tab, Some(true), Some((1, 1)))
    );
    assert_eq!(semantic(&frame, "panel").unwrap().role, Role::TabPanel);
    let size = semantic(&frame, "size").unwrap();
    assert_eq!((size.role, size.position), (Role::RadioGroup, Some((0, 2))));
    let m = semantic(&frame, "m").unwrap();
    assert_eq!(
        (m.role, m.checked, m.position),
        (Role::RadioButton, Some(true), Some((1, 1)))
    );
    let bar = semantic(&frame, "bar").unwrap();
    assert_eq!((bar.role, bar.position), (Role::Toolbar, None));
    let b = semantic(&frame, "b").unwrap();
    assert_eq!(
        (b.role, b.heading_level, b.position),
        (Role::TreeItem, Some(2), Some((1, 1)))
    );
    let docs = semantic(&frame, "docs").unwrap();
    assert_eq!(
        (docs.heading_level, docs.position),
        (Some(1), Some((1, 1))),
        "siblings count by level"
    );
    let wrap = semantic(&frame, "wrap").unwrap();
    assert_eq!(
        (wrap.role, wrap.checked),
        (Role::MenuItemCheckBox, Some(true))
    );
    assert_eq!(semantic(&frame, "zoom").unwrap().role, Role::MenuItemRadio);
}

#[test]
fn an_img_role_speaks_its_label_in_place_of_what_it_draws() {
    let frame = frame(
        "h('view',{},h('view',{'data-testid':'avatar',role:'img','aria-label':'Ada Lovelace',style:{width:40,height:40}},h('text',{'data-testid':'initials'},'AL')),h('text',{'data-testid':'stars',role:'img','aria-label':'4 of 5 stars'},'★★★★☆'))",
    );
    let avatar = semantic(&frame, "avatar").unwrap();
    assert_eq!(
        (avatar.role, avatar.name.as_str()),
        (Role::Image, "Ada Lovelace")
    );
    assert!(
        semantic(&frame, "initials").is_none(),
        "the initials are not spoken again"
    );
    let stars = semantic(&frame, "stars").unwrap();
    assert_eq!(
        (stars.role, stars.name.as_str()),
        (Role::Image, "4 of 5 stars")
    );
}
