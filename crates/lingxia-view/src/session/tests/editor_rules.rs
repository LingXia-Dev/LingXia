//! Native editor attributes: maxLength, bounds and step, date/time pickers.
use crate::ViewSession;
use crate::text::input::Edit;

fn replace(text: &str) -> Edit {
    Edit::Replace {
        text: text.into(),
        cursor: 1,
        composing: false,
    }
}

fn page(props: &str) -> (ViewSession, String, u64) {
    let module = super::source(&format!(
        "function render(){{const v=useLocal('v','');return h('column',{{}},h('input',{{key:'i',bind:v,autoFocus:true,{props}}}),h('text',{{}},'['+v.value+']'))}}"
    ));
    let mut page = ViewSession::new(&module).unwrap();
    let first = page.layout(300, 300, 1.0).unwrap();
    let id = first.nodes[1].identity.clone();
    let token = first.nodes[1].input.as_ref().unwrap().edit_token.unwrap();
    (page, id.to_string(), token)
}

fn shown(page: &mut ViewSession) -> String {
    page.layout(300, 300, 1.0).unwrap().nodes[2].text.clone()
}

#[test]
fn max_length_cuts_insertions_and_reports_the_kept_text() {
    let (mut page, id, token) =
        page("maxLength:4,autoComplete:'one-time-code',enterKeyHint:'done',spellCheck:false");
    let reply = page
        .edit_input(token, &id, &[replace("123456")], false)
        .unwrap()
        .unwrap();
    assert_eq!(reply.state.text, "1234");
    assert_eq!(shown(&mut page), "[1234]");
    let display = page.layout(300, 300, 1.0).unwrap();
    let input = display.nodes[1].input.as_ref().unwrap();
    assert_eq!(input.attributes.max_length, Some(4));
    assert_eq!(
        input.attributes.auto_complete.as_deref(),
        Some("one-time-code")
    );
    assert_eq!(input.attributes.enter_key_hint.as_deref(), Some("done"));
    assert_eq!(input.attributes.spell_check, Some(false));
    let json = serde_json::to_value(input).unwrap();
    assert_eq!(json["max_length"], 4);
}

#[test]
fn numbers_clamp_and_snap_when_the_editor_commits() {
    let (mut page, id, token) = page("type:'number',min:0,max:10,step:0.5");
    page.edit_input(token, &id, &[replace("12.2")], false)
        .unwrap();
    // While typing the raw text stands; blur commits the bounded value as input.
    assert_eq!(shown(&mut page), "[12.2]");
    assert!(page.blur_input(token, &id).unwrap());
    assert_eq!(shown(&mut page), "[10]");
}

#[test]
fn date_pickers_take_iso_values_within_bounds() {
    let (mut page, id, token) = page("type:'date',min:'2024-01-01',max:'2024-12-31'");
    let display = page.layout(300, 300, 1.0).unwrap();
    assert_eq!(
        display.nodes[1]
            .input
            .as_ref()
            .unwrap()
            .attributes
            .picker
            .as_deref(),
        Some("date")
    );
    assert!(
        page.edit_input(token, &id, &[replace("31/12/2024")], false)
            .unwrap_err()
            .contains("ISO")
    );
    page.edit_input(token, &id, &[replace("2025-02-01")], true)
        .unwrap();
    assert_eq!(shown(&mut page), "[2024-12-31]");
}

#[test]
fn attribute_wire_is_bounded_and_tag_specific() {
    use crate::protocol::{decode, field, tag};
    use serde_json::json;
    let one = |t: usize, id: usize, value: serde_json::Value| {
        decode(json!([t, -1, "", "", "", 1, id, value]).as_array().unwrap())
    };
    assert!(one(tag::INPUT, field::MAX_LENGTH, json!(0)).is_err());
    assert!(one(tag::INPUT, field::AUTO_COMPLETE, json!("password")).is_err());
    assert!(one(tag::TEXTAREA, field::KEYBOARD, json!("date")).is_err());
    assert!(one(tag::TEXTAREA, field::INPUT_MIN, json!(1)).is_err());
    assert!(one(tag::INPUT, field::INPUT_MIN, json!("2024-13-01")).is_err());
    assert!(one(tag::INPUT, field::INPUT_STEP, json!(0)).is_err());
    assert!(one(tag::SEARCH, field::KEYBOARD, json!("text")).is_err());
    assert!(one(tag::INPUT, field::SPELL_CHECK, json!(1)).is_ok());
}

#[test]
fn the_frame_carries_the_keyboard_configuration_core_decided() {
    let (mut page, _, _) = page("enterKeyHint:'go',autoComplete:'current-password'");
    let display = page.layout(300, 300, 1.0).unwrap();
    let json = serde_json::to_value(display.nodes[1].input.as_ref().unwrap()).unwrap();
    assert_eq!(json["ime"]["return_key"], "go");
    assert_eq!(json["ime"]["secure"], true);
    assert_eq!(json["ime"]["suggestions"], false);
    assert_eq!(json["ime"]["hide_on_submit"], true);
    assert_eq!(json["ime"]["auto_complete"], "current-password");
}
