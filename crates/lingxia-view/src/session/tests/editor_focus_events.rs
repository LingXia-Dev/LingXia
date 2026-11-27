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

#[test]
fn a_read_only_editor_takes_focus_and_a_selection_but_no_edits_or_keyboard() {
    use crate::text::input::Edit;
    let mut page = ViewSession::new(&super::source(
        "function render(){const a=useLocal('a','kept');return h('view',{},h('input',{'data-testid':'a',bind:a,readOnly:true,'aria-label':'A'}),h('text',{'data-testid':'value'},a.value))}",
    ))
    .unwrap();
    let frame = page.layout(300, 400, 1.0).unwrap();
    let a = node(&frame, "a").identity.clone();
    assert!(
        node(&frame, "a")
            .input
            .as_ref()
            .unwrap()
            .attributes
            .read_only
    );
    let focused = page.focus_input(page.generation(), &a).unwrap().unwrap();
    let frame = page.layout(300, 400, 1.0).unwrap();
    let input = node(&frame, "a").input.as_ref().unwrap();
    assert!(input.edit_token.is_some(), "it takes focus");
    assert!(!input.raise_keyboard, "and raises no keyboard");
    page.edit_input(
        focused.token,
        &a,
        &[
            Edit::SelectAll,
            Edit::Replace {
                text: "typed".into(),
                cursor: 1,
                composing: false,
            },
        ],
        false,
    )
    .unwrap();
    let frame = page.layout(300, 400, 1.0).unwrap();
    assert_eq!(node(&frame, "value").text, "kept", "its text stays");
    let edit = node(&frame, "a")
        .input
        .as_ref()
        .unwrap()
        .edit
        .clone()
        .unwrap();
    assert_eq!(edit.text, "kept");
    assert_ne!(edit.selection[0], edit.selection[1], "the selection moved");
}

const CONTROLS: &str = "function render(){const a=useLocal('a',''),log=useLocal('log','');const say=s=>e=>log.set(log.value+s+(e.focusVisible?'v':'p')+(typeof e.timeStamp==='number'?'':'?')+' ');return h('view',{},h('button',{'data-testid':'save',onClick(){},onFocus:say('save+'),onBlur:say('save-'),style:{width:80,height:40}},'Save'),h('text',{},h('link',{'data-testid':'help',onClick(){},onFocus:say('help+'),onBlur:say('help-')},'Help')),h('button',{'data-testid':'off',disabled:true,onFocus(){throw Error('never')},style:{width:80,height:40}},'Off'),h('input',{'data-testid':'a',bind:a,'aria-label':'A',onFocus:say('a+'),onBlur:say('a-')}),h('text',{'data-testid':'log'},log.value||'-'))}";

#[test]
fn a_control_hears_focus_and_blur_with_whether_focus_shows() {
    let mut page = ViewSession::new(&super::source(CONTROLS)).unwrap();
    let frame = page.layout(300, 400, 1.0).unwrap();
    let (save, help, off) = (
        node(&frame, "save").identity.clone(),
        node(&frame, "help").identity.clone(),
        node(&frame, "off").identity.clone(),
    );
    assert!(node(&frame, "save").on_focus.iter().all(Option::is_some));
    assert_eq!(node(&frame, "off").on_focus, [None; 2], "a disabled control takes no focus");
    let log = |page: &mut ViewSession| {
        node(&page.layout(300, 400, 1.0).unwrap(), "log")
            .text
            .clone()
    };
    // From a key: focus shows; the blur reports how it showed.
    assert!(page.set_control_focus_visible(Some(&save), true).unwrap());
    assert!(page.focus_visible());
    assert_eq!(log(&mut page), "save+v ");
    // A pointer moves it on: the button blurs as it showed, the link focuses without a ring.
    assert!(page.set_control_focus_visible(Some(&help), false).unwrap());
    assert!(!page.focus_visible());
    assert_eq!(log(&mut page), "save+v save-v help+p ");
    // The same control turning visible is no new focus.
    assert!(page.set_control_focus_visible(Some(&help), true).unwrap());
    assert_eq!(log(&mut page), "save+v save-v help+p ");
    page.set_control_focus_visible(Some(&off), true).unwrap();
    assert!(page.set_control_focus_visible(None, false).unwrap());
    assert_eq!(log(&mut page), "save+v save-v help+p help-v ");
}

#[test]
fn an_editor_s_focus_always_shows() {
    let mut page = ViewSession::new(&super::source(CONTROLS)).unwrap();
    let frame = page.layout(300, 400, 1.0).unwrap();
    let a = node(&frame, "a").identity.clone();
    let focused = page.focus_input(page.generation(), &a).unwrap().unwrap();
    assert!(page.blur_input(focused.token, &a).unwrap());
    assert_eq!(
        node(&page.layout(300, 400, 1.0).unwrap(), "log").text,
        "a+v a-v "
    );
}
