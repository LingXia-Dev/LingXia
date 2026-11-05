//! `drag` elements end to end: authored points resolve against the element, an offset moves its
//! subtree, and a settle reaches the bound snap index.
use crate::ViewSession;
use crate::scene::node::DragSnapshot;
use std::collections::HashMap;

fn page(dir: &str) -> ViewSession {
    let module = super::source(&format!(
        "function render(){{const at=useLocal('at',0);return h('page',{{dir:'{dir}'}},h('column',{{'data-testid':'panel',drag:'x',dragSnapPoints:[0,'50%',-30],bind:at,'aria-label':'Panel',style:{{width:120,height:40}}}},h('text',{{'data-testid':'label'}},'at '+at.value)))}}"
    ));
    ViewSession::new(&module).unwrap()
}

#[test]
fn points_resolve_against_the_element_and_mirror_in_rtl() {
    for (dir, sign) in [("ltr", 1.0), ("rtl", -1.0)] {
        let mut session = page(dir);
        let display = session.layout(200, 100, 1.0).unwrap();
        let panel = display.nodes.iter().find(|n| n.test_id == "panel").unwrap();
        let drag = panel.drag.as_ref().unwrap();
        assert_eq!(drag.points, [0.0, 60.0 * sign, -30.0 * sign], "{dir}");
        assert_eq!((drag.index, drag.handle > 0), (0, true));
    }
}

#[test]
fn an_offset_moves_the_whole_subtree_and_a_settle_sets_the_bound_index() {
    let mut session = page("ltr");
    let display = session.layout(200, 100, 1.0).unwrap();
    let panel = display.nodes.iter().find(|n| n.test_id == "panel").unwrap();
    let (identity, handle) = (panel.identity.clone(), panel.drag.as_ref().unwrap().handle);
    let label_x = display
        .nodes
        .iter()
        .find(|n| n.test_id == "label")
        .unwrap()
        .bounds[0];
    let offsets = HashMap::from([(DragSnapshot::key(&identity), 60.0)]);
    let moved = session
        .prepare_native(200, 100, 1.0, false, Some(&offsets), Ok)
        .unwrap();
    let label = moved.nodes.iter().find(|n| n.test_id == "label").unwrap();
    assert_eq!(label.bounds[0], label_x + 60.0);
    let generation = session.generation();
    assert!(
        session
            .text_event(
                generation,
                handle,
                r#"{"phase":"settle","index":1,"velocity":0}"#
            )
            .unwrap()
    );
    let display = session.layout(200, 100, 1.0).unwrap();
    let panel = display.nodes.iter().find(|n| n.test_id == "panel").unwrap();
    assert_eq!(panel.drag.as_ref().unwrap().index, 1);
    assert_eq!(
        display
            .nodes
            .iter()
            .find(|n| n.test_id == "label")
            .unwrap()
            .text,
        "at 1"
    );
}

#[test]
fn a_release_the_start_render_overtook_still_reaches_the_element() {
    let module = super::source(
        "function render(){const last=useLocal('last','none');return h('page',{},h('view',{'data-testid':'card',drag:'x',dragSnapPoints:[0,50],'aria-label':'Card',onDragStart(){last.set('dragging')},onDragEnd(e){last.set('released at '+e.index)},style:{width:40,height:40}}),h('text',{'data-testid':'last'},last.value))}",
    );
    let mut session = ViewSession::new(&module).unwrap();
    let display = session.layout(200, 100, 1.0).unwrap();
    let card = display.nodes.iter().find(|n| n.test_id == "card").unwrap();
    let (identity, handle) = (card.identity.clone(), card.drag.as_ref().unwrap().handle);
    let pressed = session.generation();
    let start = r#"{"phase":"start","index":0,"velocity":0}"#;
    assert!(
        session
            .drag_event(pressed, handle, &identity, start)
            .unwrap()
    );
    let text = |session: &mut ViewSession| {
        let display = session.layout(200, 100, 1.0).unwrap();
        display
            .nodes
            .iter()
            .find(|n| n.test_id == "last")
            .unwrap()
            .text
            .clone()
    };
    assert_eq!(text(&mut session), "dragging");
    // The release was read against the frame the press saw.
    let end = r#"{"phase":"end","index":1,"velocity":900}"#;
    assert!(!session.text_event(pressed, handle, end).unwrap());
    assert!(session.drag_event(pressed, handle, &identity, end).unwrap());
    assert_eq!(text(&mut session), "released at 1");
}

#[test]
fn the_release_spring_is_the_elements_transition_timing() {
    let module = super::source(
        "function render(){return h('page',{},h('view',{'data-testid':'a',drag:'y','aria-label':'A',style:{height:10,transition:{properties:['transform'],durationMs:500,delayMs:0,easing:{spring:0.25}}}}),h('view',{'data-testid':'b',drag:'y','aria-label':'B',style:{height:10}}))}",
    );
    let mut session = ViewSession::new(&module).unwrap();
    let display = session.layout(100, 100, 1.0).unwrap();
    let timing = |id: &str| {
        let drag = display
            .nodes
            .iter()
            .find(|n| n.test_id == id)
            .unwrap()
            .drag
            .clone()
            .unwrap();
        (drag.duration.as_millis(), drag.bounce)
    };
    assert_eq!(timing("a"), (500, 0.25));
    assert_eq!(timing("b"), (300, 0.0));
}

#[test]
fn a_dragged_dialog_knows_which_points_hide_it_and_its_scrim_follows_how_much_shows() {
    let module = super::source(
        "function render(){return h('page',{},h('dialog',{open:true,onClose(){},closedby:'any','aria-label':'Sheet','data-testid':'sheet',drag:'y',dragSnapPoints:[0,'50%','100%'],style:{marginTop:'auto',marginBottom:0,height:100}},h('text',{},'Hi')))}",
    );
    let mut session = ViewSession::new(&module).unwrap();
    let display = session.layout(100, 200, 1.0).unwrap();
    let panel = display.nodes.iter().find(|n| n.test_id == "sheet").unwrap();
    assert_eq!(panel.drag.as_ref().unwrap().hides, [false, false, true]);
    let retained = &display.retained;
    let scrim = retained
        .nodes
        .iter()
        .position(|n| n.drag_scrim)
        .expect("the backdrop fades");
    let half = HashMap::from([(DragSnapshot::key(&panel.identity), 50.0)]);
    let share = |offsets: &HashMap<String, f32>| {
        retained.scrim_share(scrim, [0.0; 2], offsets, [0.0, 0.0, 100.0, 200.0])
    };
    assert_eq!(share(&HashMap::new()), 1.0);
    assert!((share(&half) - 0.5).abs() < 1e-6);
}

#[test]
fn a_pager_snaps_on_the_pages_inside_its_content_row() {
    let module = super::source(
        "function render(){const at=useLocal('at',0);return h('page',{},h('scroll',{'data-testid':'pager',bind:at,style:{width:100,height:50,overflowX:'auto',scrollSnapType:'x',scrollSnapStop:'always'}},h('row',{},['a','b','c'].map(k=>h('column',{key:k,style:{width:100,height:50,flexShrink:0,scrollSnapAlign:'start'}})))))}",
    );
    let mut session = ViewSession::new(&module).unwrap();
    let display = session.layout(100, 50, 1.0).unwrap();
    let scroll = display
        .nodes
        .iter()
        .find(|n| n.test_id == "pager")
        .unwrap()
        .scroll
        .clone()
        .unwrap();
    assert_eq!(scroll.snap, [0.0, 100.0, 200.0]);
    assert!(scroll.snap_stop && scroll.on_snap.is_some());
    assert_eq!(scroll.snap_index, Some(0));
}
