//! `side-*:` styles follow the side of its anchor a popover ended on, in the frame that placed
//! it, for the popover and everything inside it.
use crate::ViewSession;
use crate::protocol::{Record, field};
use std::collections::HashMap;

/// The classes these pages use, as the compiler emits them.
const CLASSES: &str = "configureClasses([\
{name:'side-top:bg-red',when:{state:'sideTop'},style:{backgroundColor:'#ff0000'}},\
{name:'side-bottom:bg-blue',when:{state:'sideBottom'},style:{backgroundColor:'#0000ff'}},\
{name:'side-end:bg-green',when:{state:'sideEnd'},style:{backgroundColor:'#00ff00'}},\
{name:'side-bottom:opacity-0',when:{state:'sideBottom'},style:{opacity:0}},\
{name:'side-top:opacity-0',when:{state:'sideTop'},style:{opacity:0}}\
]);__lingxiaPublish(render);";

fn session(body: &str) -> ViewSession {
    let module = super::source(&format!("function render(){{{body}}}"))
        .replace("__lingxiaPublish(render);", CLASSES);
    ViewSession::new(&module).unwrap()
}

/// The painted record with `testId`.
fn painted(page: &ViewSession, id: &str) -> Record {
    page.paint_records
        .as_ref()
        .unwrap()
        .iter()
        .find(|record| record.test_id == id)
        .unwrap()
        .clone()
}

fn background(page: &ViewSession, id: &str) -> Option<String> {
    painted(page, id)
        .string(field::BACKGROUND)
        .map(str::to_owned)
}

fn opacity(page: &ViewSession, id: &str) -> f32 {
    painted(page, id).number(field::OPACITY).unwrap_or(1.0)
}

/// A popover asking to sit above an anchor `top` points down the page, with an up and a down
/// arrow inside it; each arrow hides on the side where it would point away.
fn tip(top: u32, area: &str) -> String {
    format!(
        "return h('page',{{}},h('view',{{style:{{height:{top}}}}}),h('view',{{id:'a',style:{{marginLeft:100,width:40,height:20}}}}),h('view',{{id:'tip',popover:'manual',open:true,anchor:'a','data-testid':'tip',className:'side-top:bg-red side-bottom:bg-blue side-end:bg-green',style:{{positionArea:'{area}',width:80,height:60}}}},h('view',{{'data-testid':'down',className:'side-bottom:opacity-0',style:{{width:8,height:8}}}}),h('view',{{'data-testid':'up',className:'side-top:opacity-0',style:{{width:8,height:8}}}})))"
    )
}

#[test]
fn a_popover_and_its_arrows_take_the_side_it_ended_on_in_the_frame_that_placed_it() {
    // Room above: it stays on top, the down arrow shows.
    let mut page = session(&tip(200, "top"));
    page.layout(300, 400, 1.0).unwrap();
    assert_eq!(background(&page, "tip").as_deref(), Some("#ff0000"));
    assert_eq!((opacity(&page, "down"), opacity(&page, "up")), (1.0, 0.0));
    // No room above: flipped below in the same frame, the up arrow shows instead.
    let mut page = session(&tip(10, "top"));
    let display = page.layout(300, 400, 1.0).unwrap();
    let [tip_box, anchor] = ["tip", "down"].map(|id| {
        display
            .nodes
            .iter()
            .find(|n| n.test_id == id)
            .unwrap()
            .bounds
    });
    assert!(tip_box[1] > 30.0 && anchor[1] > 30.0, "below: {tip_box:?}");
    assert_eq!(background(&page, "tip").as_deref(), Some("#0000ff"));
    assert_eq!((opacity(&page, "down"), opacity(&page, "up")), (0.0, 1.0));
}

#[test]
fn a_popover_flipped_off_its_inline_start_takes_side_end() {
    // Asked for the inline start of an anchor 100 pt in; 80 + 4 does not fit to its left.
    let mut page = session(&tip(100, "inline-start").replace("marginLeft:100", "marginLeft:40"));
    page.layout(300, 400, 1.0).unwrap();
    assert_eq!(background(&page, "tip").as_deref(), Some("#00ff00"));
}

#[test]
fn a_scroll_that_flips_a_popover_restyles_it_without_the_view() {
    let mut page = session(
        "return h('page',{},h('scroll',{'data-testid':'s',style:{height:300}},h('view',{style:{height:200}}),h('view',{id:'a',style:{marginLeft:100,width:40,height:20}}),h('view',{style:{height:600}})),h('view',{id:'tip',popover:'manual',open:true,anchor:'a','data-testid':'tip',className:'side-top:bg-red side-bottom:bg-blue',style:{positionArea:'top',width:80,height:60}}))",
    );
    let display = page.layout(300, 400, 1.0).unwrap();
    assert_eq!(background(&page, "tip").as_deref(), Some("#ff0000"));
    let scroll = display
        .nodes
        .iter()
        .find(|n| n.test_id == "s")
        .unwrap()
        .identity
        .to_string();
    // The anchor scrolls up to 20 pt from the top: no room above, so the popover flips.
    let offsets = HashMap::from([(scroll, 180.0)]);
    page.prepare_native(300, 400, 1.0, false, Some(&offsets), Ok)
        .unwrap();
    assert_eq!(background(&page, "tip").as_deref(), Some("#0000ff"));
}
