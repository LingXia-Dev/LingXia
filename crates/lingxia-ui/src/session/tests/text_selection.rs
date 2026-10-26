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
        "h('column',{},h('text',{testId:'p',style:{userSelect:'text',fontSize:16,width:300}},t.value),h('text',{testId:'q'},'plain'))",
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
        "h('column',{},h('text',{testId:'p',style:{userSelect:'all'}},t.value),h('button',{testId:'b',onClick:()=>t.set('changed')},'Change'))",
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
    let mut session = page("h('text',{testId:'p',style:{userSelect:'text'}},t.value)");
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
    ids.update(frame.nodes.iter().map(|n| n.identity.as_str()));
    let semantic = Projection {
        frame: &frame,
        ids: &ids,
        keyboard_focus: None,
        inert: false,
    }
    .node(ids.id(0).unwrap())
    .unwrap();
    assert!(
        semantic.actions.contains(&Action::Copy)
            && semantic.actions.contains(&Action::SetSelection)
    );
    assert_eq!(semantic.text_selection, Some([0, 5]));
}
