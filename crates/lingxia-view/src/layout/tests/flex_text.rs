//! Name-and-figure rows: a wrapping name beside a figure on one baseline, inside a box that is
//! sized before it is laid out. The name keeps its cell's width on every frame, a re-sort included.

/// Width read by `onLayout`: a box of auto width inside 180pt, sized horizontally.
const READ: &str = "h('view',{style:{width:180}},h('view',{onLayout(){}},";
/// Grown in a 180pt row: the row sizes the column for its automatic minimum, which sizes each
/// name row's height at min-content width.
const GROWN: &str =
    "h('view',{style:{flexDirection:'row',width:180}},h('view',{style:{flexGrow:1,flexShrink:1}},";

/// Two keyed rows inside `outer`; `sort` reverses them, `poke` re-renders only its own label.
/// `basis` is the name cell's extra style.
fn page(outer: &str, basis: &str) -> crate::ViewSession {
    crate::ViewSession::new(&crate::session::tests::source(&format!(
        "function render(){{const reversed=useLocal('reversed',false);const pokes=useLocal('pokes',0);
        const rows=[['a','Alexandra Papadopoulou-Vandersteen','1,642.5'],['b','Ben Okafor','918.0']];
        return {outer}
            h('button',{{'data-testid':'sort',onClick(){{reversed.set(!reversed.value)}}}},'Sort'),
            h('button',{{'data-testid':'poke',onClick(){{pokes.set(pokes.value+1)}}}},'Poke '+pokes.value),
            (reversed.value?[rows[1],rows[0]]:rows).map(([key,name,figure])=>h('view',{{key,style:{{paddingLeft:16,paddingRight:16}}}},
                h('view',{{style:{{flexDirection:'row',alignItems:'baseline',columnGap:12}}}},
                    h('view',{{'data-testid':'cell-'+key,style:{{flexGrow:1,flexShrink:1,minWidth:0{basis}}}}},
                        h('text',{{'data-testid':'name-'+key,style:{{fontSize:17,overflowWrap:'anywhere'}}}},name)),
                    h('view',{{'data-testid':'figure-'+key,style:{{flexShrink:0}}}},h('text',{{style:{{fontSize:17,whiteSpace:'nowrap'}}}},figure)))))))}}"
    )))
    .unwrap()
}

fn bounds(display: &crate::DisplayList, id: &str) -> [f32; 4] {
    display
        .nodes
        .iter()
        .find(|node| node.test_id == id)
        .unwrap_or_else(|| panic!("{id}"))
        .bounds
}

/// Each name fills its cell, the cell takes the row's room beside the figure, and the long name
/// wraps to more lines than the short one.
fn assert_rows(display: &crate::DisplayList, frame: &str) {
    for key in ["a", "b"] {
        let (cell, name) = (
            bounds(display, &format!("cell-{key}")),
            bounds(display, &format!("name-{key}")),
        );
        let figure = bounds(display, &format!("figure-{key}"));
        assert!(
            cell[2] > 60.0,
            "{frame}: cell {key} keeps its room: {cell:?}"
        );
        assert!(
            cell[0] + cell[2] <= figure[0] - 11.5,
            "{frame}: cell {key} {cell:?} ends a gap before its figure {figure:?}"
        );
        assert!(
            (name[2] - cell[2]).abs() < 0.5,
            "{frame}: name {key} {name:?} fills cell {cell:?}"
        );
    }
    let (long, short) = (bounds(display, "name-a"), bounds(display, "name-b"));
    assert!(
        long[3] > short[3] * 1.5,
        "{frame}: the long name wraps: {long:?} vs {short:?}"
    );
}

fn click(page: &mut crate::ViewSession, display: &crate::DisplayList, id: &str) {
    let handle = display
        .nodes
        .iter()
        .find(|node| node.test_id == id)
        .and_then(|node| node.on_click)
        .unwrap();
    page.click(page.generation(), handle).unwrap();
}

#[test]
fn a_wrapping_name_on_a_baseline_row_keeps_its_cell_width_after_a_re_render() {
    let mut page = page(READ, "");
    let mut display = page.layout(360, 800, 2.0).unwrap();
    assert_rows(&display, "first frame");
    for frame in ["poked once", "poked twice"] {
        click(&mut page, &display, "poke");
        display = page.layout(360, 800, 2.0).unwrap();
        assert_rows(&display, frame);
    }
}

#[test]
fn a_zero_basis_name_cell_keeps_its_width_when_its_keyed_row_moves() {
    let mut page = page(READ, ",flexBasis:0");
    let mut display = page.layout(360, 800, 2.0).unwrap();
    assert_rows(&display, "first frame");
    for frame in ["sorted", "sorted back"] {
        click(&mut page, &display, "sort");
        display = page.layout(360, 800, 2.0).unwrap();
        assert_rows(&display, frame);
    }
}

#[test]
fn a_wrapping_name_keeps_its_cell_width_after_its_column_is_sized_in_a_row() {
    let mut page = page(GROWN, "");
    let mut display = page.layout(360, 800, 2.0).unwrap();
    assert_rows(&display, "first frame");
    for frame in ["poked once", "poked twice"] {
        click(&mut page, &display, "poke");
        display = page.layout(360, 800, 2.0).unwrap();
        assert_rows(&display, frame);
    }
}
