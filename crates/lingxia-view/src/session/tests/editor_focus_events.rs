//! `onFocus` and `onBlur` on editors.
use super::*;

const PAGE: &str = "function render(){const a=useLocal('a',''),b=useLocal('b',''),log=useLocal('log','');const say=s=>()=>log.set(log.value+s+' ');return h('view',{},h('input',{'data-testid':'a',bind:a,'aria-label':'A',onFocus:say('a+'),onBlur:say('a-')}),h('textarea',{'data-testid':'b',bind:b,'aria-label':'B',onFocus:say('b+'),onBlur:say('b-')}),h('button',{'data-testid':'pick',onClick:say('click'),style:{width:80,height:40}},'Pick'),h('text',{'data-testid':'log'},log.value||'-'))}";

fn node<'a>(frame: &'a crate::DisplayList, id: &str) -> &'a crate::scene::node::NodeSnapshot {
    frame.nodes.iter().find(|node| node.test_id == id).unwrap()
}

#[test]
fn an_editor_hears_focus_and_blur_and_a_move_between_editors_blurs_the_first_before_it_focuses_the_next()
 {
    let mut page = ViewSession::new(&super::source(PAGE)).unwrap();
    let frame = page.layout(300, 400, 1.0).unwrap();
    let (a, b) = (
        node(&frame, "a").identity.clone(),
        node(&frame, "b").identity.clone(),
    );
    let log = |page: &mut ViewSession| {
        node(&page.layout(300, 400, 1.0).unwrap(), "log")
            .text
            .clone()
    };
    assert_eq!(log(&mut page), "-");
    page.focus_input(page.generation(), &a).unwrap().unwrap();
    assert_eq!(log(&mut page), "a+ ");
    // Focusing it again is no new focus.
    page.focus_input(page.generation(), &a).unwrap().unwrap();
    assert_eq!(log(&mut page), "a+ ");
    let focused = page.focus_input(page.generation(), &b).unwrap().unwrap();
    assert_eq!(log(&mut page), "a+ a- b+ ");
    // A press on a button is not an editor's business: it clicks, and the editor keeps focus.
    let click = node(&page.layout(300, 400, 1.0).unwrap(), "pick")
        .on_click
        .unwrap();
    assert!(page.click(page.generation(), click).unwrap());
    assert_eq!(log(&mut page), "a+ a- b+ click ");
    assert!(page.blur_input(focused.token, &b).unwrap());
    assert_eq!(log(&mut page), "a+ a- b+ click b- ");
}

#[test]
fn an_editor_without_the_handlers_and_a_disabled_one_report_nothing() {
    let mut page = ViewSession::new(&super::source(
        "function render(){const a=useLocal('a','');return h('view',{},h('input',{'data-testid':'a',bind:a,'aria-label':'A'}),h('input',{'data-testid':'off',disabled:true,value:'',onFocus(){throw Error('never')},'aria-label':'Off'}))}",
    ))
    .unwrap();
    let frame = page.layout(300, 400, 1.0).unwrap();
    assert_eq!(node(&frame, "a").on_focus, [None; 2]);
    assert_eq!(node(&frame, "off").on_focus, [None; 2]);
    let a = node(&frame, "a").identity.clone();
    let focused = page.focus_input(page.generation(), &a).unwrap().unwrap();
    assert!(page.blur_input(focused.token, &a).unwrap());
}
