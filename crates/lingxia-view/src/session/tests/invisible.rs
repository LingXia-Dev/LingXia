//! A fully transparent box keeps its layout, hit testing and semantics, but nothing of it
//! is drawn or composited: no layer, no backdrop blur, no focus ring. Its images stay decoded.
use super::*;

#[test]
fn a_transparent_subtree_costs_nothing_to_draw_but_stays_in_the_frame() {
    let page = |opacity: &str| {
        source(&format!(
            "function render(){{return h('view',{{}},h('view',{{'data-testid':'box',style:{{opacity:{opacity},backdropFilter:'blur(8px)',padding:4}}}},h('text',{{'data-testid':'t'}},'abc'),h('image',{{src:'a.png',alt:'',style:{{width:10,height:20}}}}),h('button',{{'data-testid':'b',onClick(){{}},style:{{width:40,height:20}}}},'go')))}}"
        ))
    };
    let mut shown = ViewSession::new(&page("1")).unwrap();
    let shown = shown.layout(100, 100, 1.0).unwrap().cost();
    assert!(
        shown.glyphs > 0 && shown.images == 1 && shown.layers >= 1,
        "{shown:?}"
    );
    let mut hidden = ViewSession::new(&page("0")).unwrap();
    let display = hidden.layout(100, 100, 1.0).unwrap();
    let cost = display.cost();
    assert_eq!(
        (cost.glyphs, cost.images, cost.layers),
        (0, 0, 0),
        "{cost:?}"
    );
    assert!(
        !display
            .commands
            .iter()
            .any(|paint| matches!(paint, crate::scene::paint::Paint::Backdrop { .. })),
        "a backdrop behind a transparent box never blurs"
    );
    assert!(
        display.image_sources().any(|src| src == "a.png"),
        "its image stays wanted and decoded"
    );
    let button = display.nodes.iter().find(|n| n.test_id == "b").unwrap();
    assert!(button.on_click.is_some() && button.visible_bounds[2] > 0.0);
    assert!(display.nodes.iter().any(|n| n.test_id == "t"));
}

#[test]
fn a_focused_control_inside_a_transparent_box_draws_no_ring() {
    let module = source(
        "function render(){return h('view',{style:{opacity:0}},h('button',{'data-testid':'b',onClick(){},style:{width:100,height:40}},'Go'))}",
    );
    let mut page = ViewSession::new(&module).unwrap();
    let display = page.layout(100, 40, 1.0).unwrap();
    let id = display
        .nodes
        .iter()
        .find(|n| n.test_id == "b")
        .unwrap()
        .identity
        .clone();
    assert!(page.set_control_focus(Some(&id)));
    let focused = page.prepare_native(100, 40, 1.0, false, None, Ok).unwrap();
    assert_eq!(focused.cost().paints, display.cost().paints);
    let pixels = focused.rasterize().unwrap();
    let bytes = pixels.pixels.data_as_u8_slice();
    assert!(
        bytes.chunks(4).all(|pixel| pixel == &bytes[..4]),
        "nothing of the ring shows"
    );
}
