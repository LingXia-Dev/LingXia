//! Static-text selection: long-press word, select-all, handles, copy, clearing.
use crate::ViewSession;

fn page(render: &str) -> ViewSession {
    ViewSession::new(&super::source(&format!(
        "function render(){{const t=useLocal('t','hello brave new world');return {render}}}"
    )))
    .unwrap()
}

fn node<'a>(display: &'a crate::DisplayList, id: &str) -> &'a crate::scene::node::NodeSnapshot {
    display.nodes.iter().find(|n| n.test_id == id).unwrap()
}

#[test]
fn long_press_selects_a_word_and_handles_extend_it() {
    let mut session = page(
        "h('view',{},h('text',{'data-testid':'p',style:{userSelect:'text',fontSize:16,width:300}},t.value),h('text',{'data-testid':'q'},'plain'))",
    );
    let display = session.layout(320, 200, 1.0).unwrap();
    let text = node(&display, "p");
    assert_eq!(text.selectable, Some("text"));
    assert_eq!(node(&display, "q").selectable, None);
    let identity = text.identity.clone();
    // A point inside "brave": about a third of the way along a 21-character line.
    let y = text.bounds[1] + text.bounds[3] / 2.0;
    let x = text.bounds[0] + 60.0;
    assert!(
        session
            .select_text(session.generation(), &identity, [x, y])
            .unwrap()
    );
    assert_eq!(session.selected_text().as_deref(), Some("brave"));
    let selected = session.layout(320, 200, 1.0).unwrap();
    let geometry = node(&selected, "p").text_selection.clone().unwrap();
    assert!(!geometry.selection.is_empty());
    assert_eq!(node(&selected, "p").selection_range, Some([6, 11]));
    // Dragging the focus handle to the end of the line extends the selection.
    let end = [text.bounds[0] + 290.0, y];
    assert!(session.move_text_selection(&identity, 1, end).unwrap());
    assert_eq!(session.selected_text().as_deref(), Some("brave new world"));
    assert!(session.clear_text_selection());
    assert_eq!(session.selected_text(), None);
    assert!(
        node(&session.layout(320, 200, 1.0).unwrap(), "p")
            .text_selection
            .is_none()
    );
}

#[test]
fn select_all_takes_the_whole_text_and_changes_drop_the_selection() {
    let mut session = page(
        "h('view',{},h('text',{'data-testid':'p',style:{userSelect:'all'}},t.value),h('button',{'data-testid':'b',onClick:()=>t.set('changed')},'Change'))",
    );
    let display = session.layout(320, 200, 1.0).unwrap();
    let text = node(&display, "p").clone();
    session
        .select_text(
            session.generation(),
            &text.identity,
            [text.bounds[0] + 1.0, text.bounds[1] + 1.0],
        )
        .unwrap();
    assert_eq!(
        session.selected_text().as_deref(),
        Some("hello brave new world")
    );
    let button = node(&session.layout(320, 200, 1.0).unwrap(), "b")
        .on_click
        .unwrap();
    session.click(session.generation(), button).unwrap();
    let changed = session.layout(320, 200, 1.0).unwrap();
    assert!(node(&changed, "p").text_selection.is_none());
    assert_eq!(session.selected_text(), None);
}

#[test]
fn selectable_text_projects_selection_actions() {
    use crate::interaction::frame::PresentedFrame;
    use crate::semantics::{
        ids::SemanticIds,
        projection::{Action, Projection},
    };
    let mut session = page("h('text',{'data-testid':'p',style:{userSelect:'text'}},t.value)");
    let display = session.layout(320, 200, 1.0).unwrap();
    let identity = display.nodes[0].identity.clone();
    session.set_text_selection(&identity, 0, 5).unwrap();
    assert_eq!(session.selected_text().as_deref(), Some("hello"));
    let display = session.layout(320, 200, 1.0).unwrap();
    let frame = PresentedFrame::new(
        crate::session::lifecycle::CommitTicket::new(1),
        1.0,
        display.nodes,
    );
    let mut ids = SemanticIds::default();
    ids.update(frame.nodes.iter().map(|n| n.identity.as_ref()));
    let semantic = Projection {
        frame: &frame,
        ids: &ids,
        keyboard_focus: None,
        inert: false,
        drags: None,
        obscured: [0.0; 4],
        touch_target: 0.0,
    }
    .node(ids.id(0).unwrap())
    .unwrap();
    assert!(
        semantic.actions.contains(&Action::Copy)
            && semantic.actions.contains(&Action::SetSelection)
    );
    assert_eq!(semantic.text_selection, Some([0, 5]));
}

#[test]
fn long_press_between_words_selects_the_nearest_word() {
    let mut session = page(
        "h('view',{},h('text',{'data-testid':'p',style:{userSelect:'text',whiteSpace:'pre',fontFamily:'monospace',fontSize:16,alignSelf:'flex-start'}},'ab        cd'))",
    );
    let display = session.layout(320, 200, 1.0).unwrap();
    let text = node(&display, "p");
    let identity = text.identity.clone();
    let y = text.bounds[1] + text.bounds[3] / 2.0;
    let advance = text.bounds[2] / 12.0;
    for (column, word) in [(3.0, "ab"), (8.5, "cd")] {
        let x = text.bounds[0] + advance * column;
        assert!(
            session
                .select_text(session.generation(), &identity, [x, y])
                .unwrap()
        );
        assert_eq!(
            session.selected_text().as_deref(),
            Some(word),
            "column {column}"
        );
    }
}

/// Every selectable text realizes, however many: one scrolled into view without a re-render
/// selects a word.
#[test]
fn a_selectable_far_down_a_long_page_selects_after_a_scroll() {
    let mut session = ViewSession::new(&super::source(
        "function render(){return h('scroll',{style:{height:200}},...Array.from({length:200},(_,i)=>h('text',{'data-testid':'p'+i,style:{userSelect:'text',fontSize:16,height:20}},'word'+i+' tail')))}",
    ))
    .unwrap();
    let display = session.layout(320, 200, 1.0).unwrap();
    let scroll = display.nodes[0].scroll.as_ref().unwrap().id.clone();
    let offsets = std::collections::HashMap::from([(scroll, 3000.0)]);
    let scrolled = session
        .prepare_native(320, 200, 1.0, false, Some(&offsets), Ok)
        .unwrap();
    let text = node(&scrolled, "p150").clone();
    assert_eq!(text.selectable, Some("text"));
    let y = text.bounds[1] + text.bounds[3] / 2.0;
    assert!((0.0..200.0).contains(&y), "on screen: {:?}", text.bounds);
    assert!(
        session
            .select_text(
                session.generation(),
                &text.identity,
                [text.bounds[0] + 8.0, y]
            )
            .unwrap()
    );
    assert_eq!(session.selected_text().as_deref(), Some("word150"));
}

/// Realizing selectables copies no paragraph: 2,000 of them take one layout per distinct text,
/// and twice the paragraphs take about twice the build.
#[test]
fn selectables_share_their_layouts_and_scale_linearly() {
    let page = |count: usize| {
        ViewSession::new(&super::source(&format!(
            "function render(){{return h('scroll',{{}},...Array.from({{length:{count}}},(_,i)=>h('text',{{style:{{userSelect:'text',fontSize:14}}}},'paragraph '+(i%10)+' of a long reader page')))}}"
        )))
        .unwrap()
    };
    let build = |count: usize| {
        let mut session = page(count);
        let started = std::time::Instant::now();
        let display = session.layout(320, 640, 1.0).unwrap();
        (started.elapsed(), display)
    };
    let (half, _) = build(1000);
    let (full, display) = build(2000);
    let selectables: Vec<_> = display.retained.selectables.iter().flatten().collect();
    assert_eq!(selectables.len(), 2000, "no cap");
    let distinct: std::collections::HashSet<_> = selectables
        .iter()
        .map(|text| std::sync::Arc::as_ptr(&text.layout))
        .collect();
    assert!(
        distinct.len() <= 10,
        "{} layouts for 10 texts",
        distinct.len()
    );
    assert!(
        full.as_secs_f64() < half.as_secs_f64() * 4.0 + 0.05,
        "1000: {half:?}, 2000: {full:?}"
    );
}
