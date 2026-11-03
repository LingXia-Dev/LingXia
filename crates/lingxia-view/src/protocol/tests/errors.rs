//! Native error codes a malformed host or wire input reaches, each by its code.
use serde_json::{Value, json};

#[track_caller]
fn code(result: Result<impl Sized, String>) -> String {
    let error = result.err().expect("expected an error");
    error.split(':').next().unwrap().to_owned()
}

fn wire(values: Value) -> Result<Vec<crate::protocol::Record>, String> {
    crate::protocol::decode(values.as_array().unwrap())
}

#[test]
fn malformed_wire_names_the_broken_part() {
    assert_eq!(code(wire(json!([0, -1, "", ""]))), "UI_BUFFER");
    assert_eq!(code(wire(json!([0, -1, "", "", "", 2, 0]))), "UI_BUFFER");
    assert_eq!(code(wire(json!([0, "x", "", "", "", 0]))), "UI_PARENT");
    assert_eq!(code(wire(json!([0, -1, "", 7, "", 0]))), "UI_TEST_ID");
    assert_eq!(code(wire(json!([0, -1, "", "", "", 100000]))), "UI_FIELDS");
}

#[test]
fn host_inputs_outside_their_contract_fail_by_code() {
    let mut page = crate::ViewSession::new(&crate::session::tests::source(
        "function render(){return h('text',{},'x')}",
    ))
    .unwrap();
    assert_eq!(code(page.set_environment(json!("dark"))), "UI_ENVIRONMENT");
    assert_eq!(
        code(page.set_environment(json!({"palette": {"revision": 1, "colors": ["red"]}}))),
        "UI_ENVIRONMENT"
    );
    assert_eq!(code(page.begin_bridge("")), "UI_BRIDGE");
    assert_eq!(code(page.receive_bridge(&json!({"v": 3}))), "UI_BRIDGE");
    assert_eq!(code(page.receive_state(&json!({"v": 1}))), "UI_STATE");
    assert_eq!(code(page.request_leave("sideways")), "UI_LEAVE");
    assert_eq!(
        code(crate::interaction::chrome::ChromeNode::parse("[")),
        "UI_CHROME"
    );
    assert_eq!(
        code(crate::scene::color_filter::Saturation::new(-1.0)),
        "UI_FILTER"
    );
}
