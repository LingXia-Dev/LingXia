//! Moving editor focus: `enterKeyHint` next/previous and a binding's `focus()`.
use crate::ViewSession;

const FORM: &str = "function render(){const a=useLocal('a',''),b=useLocal('b',''),c=useLocal('c',''),sent=useLocal('sent','');return h('column',{},h('input',{key:'a',bind:a,autoFocus:true,enterKeyHint:'next',onSubmit:()=>sent.set('a')}),h('input',{key:'off',value:'',disabled:true}),h('input',{key:'b',bind:b,enterKeyHint:'previous'}),h('input',{key:'c',bind:c,enterKeyHint:'next',onSubmit:()=>{sent.set('c');a.focus()}}),h('text',{},'['+sent.value+']'))}";

fn focused(frame: &crate::DisplayList) -> Vec<usize> {
    frame
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.input.as_ref().is_some_and(|i| i.edit_token.is_some()))
        .map(|(index, _)| index)
        .collect()
}

#[test]
fn next_and_previous_walk_enabled_editors_in_reading_order_and_the_last_submits() {
    let mut page = ViewSession::new(&super::source(FORM)).unwrap();
    let frame = page.layout(300, 400, 1.0).unwrap();
    let id = |i: usize| frame.nodes[i].identity.clone();
    let (a, b, c) = (id(1), id(3), id(4));
    let token = frame.nodes[1].input.as_ref().unwrap().edit_token.unwrap();
    // `next` skips the disabled editor and does not submit.
    let reply = page.edit_input(token, &a, &[], true).unwrap().unwrap();
    assert_eq!(reply.moved_to.as_deref(), Some(b.as_str()));
    let frame = page.prepare_native(300, 400, 1.0, false, None, Ok).unwrap();
    assert_eq!(focused(&frame), [3]);
    assert_eq!(frame.nodes[5].text, "[]");
    assert!(page.input_state(reply.token, &b).is_some());
    // `previous` goes back.
    let reply = page
        .edit_input(reply.token, &b, &[], true)
        .unwrap()
        .unwrap();
    assert_eq!(reply.moved_to.as_deref(), Some(a.as_str()));
    // The last editor in its direction submits, and that callback may focus by binding.
    let token = page
        .focus_input(page.generation(), &c)
        .unwrap()
        .unwrap()
        .token;
    let reply = page.edit_input(token, &c, &[], true).unwrap().unwrap();
    assert!(reply.moved_to.is_none() && reply.needs_view_render);
    let frame = page.layout(300, 400, 1.0).unwrap();
    assert_eq!(frame.nodes[5].text, "[c]");
    assert_eq!(focused(&frame), [1]);
    let json: serde_json::Value = serde_json::to_value(&reply).unwrap();
    assert!(json.get("moved_to").is_none());
}

#[test]
fn submitting_an_unfocused_editor_runs_its_input_method_action() {
    let mut page = ViewSession::new(&super::source(FORM)).unwrap();
    let frame = page.layout(300, 400, 1.0).unwrap();
    let c = frame.nodes[4].identity.clone();
    assert!(page.submit_editor(page.generation(), &c).unwrap());
    let frame = page.layout(300, 400, 1.0).unwrap();
    assert_eq!(frame.nodes[5].text, "[c]");
}
