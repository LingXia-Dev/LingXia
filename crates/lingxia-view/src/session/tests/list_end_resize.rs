//! An end-anchored list through a resize: one resting at its end keeps showing its end, and one
//! the reader scrolled away from keeps its place.
use crate::ViewSession;
use std::collections::HashMap;

/// A list of 50 rows of 40 above a composer whose height the test sets.
fn conversation(anchor: &str) -> ViewSession {
    let module = super::source(&format!(
        "globalThis.composer=40;function render(){{return h('page',{{}},h(listTag,{{'data-testid':'list',count:50,anchor:{anchor},style:{{flexGrow:1,flexBasis:0}},row:i=>h('text',{{'data-testid':'row-'+i,style:{{height:40}}}},String(i))}}),h('view',{{style:{{height:globalThis.composer}}}}))}}"
    ));
    ViewSession::new(&module).unwrap()
}

const LAST_ROW: &str = "{index:49,align:'end',seq:50}";

/// The list's id, offset, the offset of its end, and its viewport height.
fn list(frame: &crate::DisplayList) -> (String, f32, f32, f32) {
    let node = frame.nodes.iter().find(|n| n.test_id == "list").unwrap();
    let scroll = node.scroll.as_ref().unwrap();
    (
        scroll.id.clone(),
        scroll.offset,
        scroll.max_offset,
        node.bounds[3],
    )
}

fn shows_last_row(frame: &crate::DisplayList) -> bool {
    let node = frame.nodes.iter().find(|n| n.test_id == "list").unwrap();
    frame.nodes.iter().any(|n| {
        n.test_id == "row-49"
            && (n.bounds[1] + n.bounds[3] - (node.bounds[1] + node.bounds[3])).abs() < 0.1
    })
}

#[test]
fn a_list_resting_at_its_end_stays_there_through_every_resize() {
    for anchor in [LAST_ROW, "'end'"] {
        let mut page = conversation(anchor);
        let first = page.layout(200, 400, 1.0).unwrap();
        assert!(shows_last_row(&first), "{anchor}: it opens at its end");
        // A keyboard takes part of the surface, then gives it back.
        for (width, height, what) in [
            (200, 250, "a shorter surface"),
            (200, 400, "the surface restored"),
            (400, 200, "a rotation"),
        ] {
            let frame = page.layout(width, height, 1.0).unwrap();
            let (_, offset, end, _) = list(&frame);
            assert!(
                (offset - end).abs() < 0.1,
                "{anchor}, {what}: {offset} / {end}"
            );
            assert!(shows_last_row(&frame), "{anchor}, {what}");
        }
        page.eval("globalThis.composer=130").unwrap();
        let grown = page.layout(400, 200, 1.0).unwrap();
        let (_, offset, end, height) = list(&grown);
        assert_eq!(height, 70.0);
        assert!(
            (offset - end).abs() < 0.1,
            "{anchor}, a taller composer: {offset} / {end}"
        );
        assert!(shows_last_row(&grown), "{anchor}, a taller composer");
    }
}

#[test]
fn a_list_the_reader_scrolled_away_from_keeps_its_place() {
    let mut page = conversation(LAST_ROW);
    let first = page.layout(200, 400, 1.0).unwrap();
    let (id, _, end, _) = list(&first);
    let away = end - 600.0;
    page.prepare_native(
        200,
        400,
        1.0,
        false,
        Some(&HashMap::from([(id.clone(), away)])),
        Ok,
    )
    .unwrap();
    let shorter = page.layout(200, 250, 1.0).unwrap();
    assert_eq!(list(&shorter).1, away, "a shorter surface leaves it alone");
    page.eval("globalThis.composer=130").unwrap();
    let grown = page.layout(200, 250, 1.0).unwrap();
    assert_eq!(list(&grown).1, away, "and so does a taller composer");
    // Back at the end by the reader's own scroll, it follows the next resize again.
    let (_, _, end, _) = list(&grown);
    page.prepare_native(200, 250, 1.0, false, Some(&HashMap::from([(id, end)])), Ok)
        .unwrap();
    let restored = page.layout(200, 400, 1.0).unwrap();
    let (_, offset, end, _) = list(&restored);
    assert!((offset - end).abs() < 0.1, "{offset} / {end}");
    assert!(shows_last_row(&restored));
}

#[test]
fn a_list_with_no_end_anchor_keeps_its_offset_when_it_shrinks() {
    let mut page = conversation("undefined");
    let first = page.layout(200, 400, 1.0).unwrap();
    let (id, _, end, _) = list(&first);
    page.prepare_native(200, 400, 1.0, false, Some(&HashMap::from([(id, end)])), Ok)
        .unwrap();
    let shorter = page.layout(200, 250, 1.0).unwrap();
    let (_, offset, shorter_end, _) = list(&shorter);
    assert_eq!(offset, end);
    assert!(shorter_end > end);
}
