//! An anchored popover never covers its anchor: it takes the roomier side, is capped to that
//! side's room and scrolls inside; a field with a keyboard up is revealed first.
use super::*;

fn node<'a>(frame: &'a crate::DisplayList, test_id: &str) -> &'a crate::scene::node::NodeSnapshot {
    frame
        .nodes
        .iter()
        .find(|node| node.test_id == test_id)
        .unwrap_or_else(|| panic!("no {test_id}"))
}

/// `count` options 40 pt tall in a popover anchored to `field`.
fn options(count: usize) -> String {
    let buttons: Vec<String> = (0..count)
        .map(|n| {
            format!(
                "h('button',{{id:'opt{n}','data-testid':'o{n}',style:{{height:40}}}},'Option {n}')"
            )
        })
        .collect();
    format!(
        "h('view',{{id:'list',popover:'manual',anchor:'field',open:true,'data-testid':'list',style:{{width:160}}}},{})",
        buttons.join(",")
    )
}

fn clear_of(list: [f32; 4], field: [f32; 4]) -> bool {
    list[1] + list[3] <= field[1] + 0.01 || list[1] >= field[1] + field[3] - 0.01
}

#[test]
fn a_long_list_under_a_field_near_the_top_is_capped_to_the_room_below_and_scrolls_inside() {
    let module = source(&format!(
        "function render(){{return h('page',{{}},h('view',{{style:{{height:100}}}}),h('view',{{id:'field','data-testid':'field',style:{{height:44}}}}),{})}}",
        options(20)
    ));
    let mut page = ViewSession::new(&module).unwrap();
    let frame = page.prepare_native(300, 500, 1.0, false, None, Ok).unwrap();
    let (field, list) = (node(&frame, "field").bounds, node(&frame, "list"));
    // 344 pt below the field (to 8 pt off the edge), 88 above: under it, capped to the 344
    // and then to the middle of the row that crosses it, so the cut row shows there is more.
    assert_eq!(field[1], 100.0);
    assert_eq!(
        list.bounds[1], 148.0,
        "4 pt under the field: {:?}",
        list.bounds
    );
    assert_eq!(list.bounds[3], 340.0, "capped, half of option 8 showing");
    assert!(clear_of(list.bounds, field));
    let scroll = list.scroll.as_ref().expect("a capped popover scrolls");
    assert_eq!(scroll.max_offset, 800.0 - 340.0);
    // Scrolled to its end inside the panel, the last option shows within it.
    let id = scroll.id.clone();
    let offsets = HashMap::from([(id, 460.0)]);
    let frame = page
        .prepare_native(300, 500, 1.0, false, Some(&offsets), Ok)
        .unwrap();
    let (list, last) = (node(&frame, "list").bounds, node(&frame, "o19").bounds);
    assert_eq!(last[1] + last[3], list[1] + list[3], "{last:?} {list:?}");
    assert_eq!(node(&frame, "field").bounds, field, "the page did not move");
}

#[test]
fn a_list_from_a_field_near_the_bottom_of_a_scrolled_page_opens_over_it_capped() {
    let module = source(&format!(
        "function render(){{return h('page',{{}},h('scroll',{{style:{{flexGrow:1}},'data-testid':'scroll'}},h('view',{{style:{{height:700}}}}),h('view',{{id:'field','data-testid':'field',style:{{height:44}}}}),h('view',{{style:{{height:300}}}})),{})}}",
        options(10)
    ));
    let mut page = ViewSession::new(&module).unwrap();
    let frame = page.prepare_native(300, 500, 1.0, false, None, Ok).unwrap();
    let id = node(&frame, "scroll").scroll.as_ref().unwrap().id.clone();
    let offsets = HashMap::from([(id, 400.0)]);
    let frame = page
        .prepare_native(300, 500, 1.0, false, Some(&offsets), Ok)
        .unwrap();
    let (field, list) = (node(&frame, "field").bounds, node(&frame, "list").bounds);
    // The field at 300: 144 pt below it, 288 above; the 400 pt list fits neither and takes above,
    // capped to the 288 and then to the middle of option 6, the last row wholly inside it.
    assert_eq!(field[1], 300.0);
    assert_eq!(list[3], 260.0, "capped to the room above: {list:?}");
    assert_eq!(list[1] + list[3], 296.0, "4 pt over the field: {list:?}");
    assert!(clear_of(list, field));
    // A short list fits below and keeps its own height.
    let module = source(&format!(
        "function render(){{return h('page',{{}},h('view',{{style:{{height:300}}}}),h('view',{{id:'field','data-testid':'field',style:{{height:44}}}}),{})}}",
        options(3)
    ));
    let mut page = ViewSession::new(&module).unwrap();
    let frame = page.prepare_native(300, 500, 1.0, false, None, Ok).unwrap();
    let list = node(&frame, "list");
    assert_eq!(list.bounds[1], 348.0);
    assert_eq!(list.bounds[3], 120.0);
    assert!(list.scroll.is_none(), "not capped, not a scroll");
}

#[test]
fn with_the_keyboard_up_the_field_is_revealed_first_and_its_list_takes_what_is_left() {
    let module = source(&format!(
        "function render(){{return h('page',{{}},h('scroll',{{style:{{flexGrow:1}}}},h('view',{{style:{{height:300}}}}),h('input',{{id:'field','data-testid':'field',value:'ap',style:{{height:44}}}}),h('view',{{style:{{height:300}}}})),{})}}",
        options(6)
    ));
    let mut page = ViewSession::new(&module).unwrap();
    let frame = page.prepare_native(300, 500, 1.0, false, None, Ok).unwrap();
    let id = node(&frame, "field").identity.to_string();
    page.focus_input(page.generation(), &id).unwrap().unwrap();
    page.prepare_native(300, 500, 1.0, false, None, Ok).unwrap();
    let mut environment = page.environment.clone();
    environment["keyboard"]["height"] = serde_json::json!(250);
    page.set_environment(environment).unwrap();
    let frame = page.prepare_native(300, 500, 1.0, false, None, Ok).unwrap();
    let (field, list) = (node(&frame, "field").bounds, node(&frame, "list"));
    assert!(
        field[1] + field[3] <= 250.0,
        "the field clears the keyboard: {field:?}"
    );
    assert!(
        clear_of(list.bounds, field),
        "the list never covers the field or its caret: {:?} {field:?}",
        list.bounds
    );
    // No room under the field above the keyboard: over it, capped to the room there.
    assert_eq!(list.bounds[1] + list.bounds[3], field[1] - 4.0);
    assert!(
        list.bounds[1] >= 8.0 && list.bounds[3] < 240.0,
        "{:?}",
        list.bounds
    );
    assert!(
        list.scroll.is_some(),
        "the 240 pt list is capped and scrolls"
    );
}

#[test]
fn rounding_never_makes_a_list_that_nearly_fits_scroll() {
    // 120 pt below the field; a panel 0.8 pt taller fits, 2 pt taller is capped.
    let page_with = |height: f32| {
        source(&format!(
            "function render(){{return h('page',{{}},h('view',{{style:{{height:100}}}}),h('view',{{id:'field','data-testid':'field',style:{{height:44}}}}),h('view',{{id:'list',popover:'manual',anchor:'field',open:true,'data-testid':'list'}},h('view',{{style:{{height:{height}}}}})))}}"
        ))
    };
    let mut page = ViewSession::new(&page_with(120.8)).unwrap();
    let frame = page.prepare_native(300, 276, 1.0, false, None, Ok).unwrap();
    let list = node(&frame, "list");
    assert!(list.scroll.is_none(), "{:?}", list.bounds);
    assert_eq!(list.bounds[1], 148.0);
    let mut page = ViewSession::new(&page_with(122.0)).unwrap();
    let frame = page.prepare_native(300, 276, 1.0, false, None, Ok).unwrap();
    let list = node(&frame, "list");
    assert!(list.scroll.is_some());
    assert_eq!(
        list.bounds[3], 120.0,
        "one tall child is no row: capped to the room"
    );
    // Of no width, its content scrolls nothing; the cap still holds instead of undoing itself,
    // and a dev build names the popover whose height did not settle.
    assert_eq!(list.bounds[2], 0.0);
    let warnings = page.take_budget_warnings();
    assert_eq!(
        warnings
            .iter()
            .any(|w| w.starts_with("UI_POPOVER") && w.contains("#list")),
        page.loud,
        "{warnings:?}"
    );
}

#[test]
fn with_room_for_less_than_two_rows_the_page_scrolls_the_field_to_make_room() {
    // A 200 pt surface: 84 pt under the field, 48 over it; the page can scroll.
    let module = source(&format!(
        "function render(){{return h('page',{{}},h('scroll',{{style:{{flexGrow:1}},'data-testid':'scroll'}},h('view',{{style:{{height:60}}}}),h('view',{{id:'field','data-testid':'field',style:{{height:44}}}}),h('view',{{style:{{height:400}}}})),{})}}",
        options(10)
    ));
    let mut page = ViewSession::new(&module).unwrap();
    let frame = page.prepare_native(300, 200, 1.0, false, None, Ok).unwrap();
    let (field, list) = (node(&frame, "field").bounds, node(&frame, "list").bounds);
    assert!(field[1] < 60.0, "the page scrolled the field up: {field:?}");
    assert_eq!(
        list[1],
        field[1] + field[3] + 4.0,
        "under it: {list:?} {field:?}"
    );
    assert!(list[3] >= 96.0, "room for about two rows: {list:?}");
    assert!(clear_of(list, field));
}

#[test]
fn the_option_an_editors_keys_point_at_scrolls_into_view_in_a_capped_list() {
    let module = source(&format!(
        "function render(){{return h('page',{{}},h('view',{{style:{{height:100}}}}),h('input',{{id:'field','data-testid':'field',value:'',style:{{height:44}},'aria-activedescendant':'opt15'}}),{})}}",
        options(20)
    ));
    let mut page = ViewSession::new(&module).unwrap();
    let frame = page.prepare_native(300, 500, 1.0, false, None, Ok).unwrap();
    assert_eq!(
        node(&frame, "list").scroll.as_ref().unwrap().offset,
        0.0,
        "unfocused, the list rests at its start"
    );
    let id = node(&frame, "field").identity.to_string();
    page.focus_input(page.generation(), &id).unwrap().unwrap();
    let frame = page.prepare_native(300, 500, 1.0, false, None, Ok).unwrap();
    let (list, option) = (node(&frame, "list").bounds, node(&frame, "o15").bounds);
    assert!(
        option[1] >= list[1] && option[1] + option[3] <= list[1] + list[3] + 0.01,
        "option 15 inside the panel: {option:?} {list:?}"
    );
}
