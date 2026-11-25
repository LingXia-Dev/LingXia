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
    assert_eq!(
        code(
            page.receive_logic(crate::session::logic::ToView::ChannelClose {
                id: "c".into(),
                code: None,
                reason: None,
            })
        ),
        "UI_BRIDGE"
    );
    assert_eq!(
        code(
            page.write_state(crate::session::state::StateWrite::Snapshot {
                rev: 1,
                state: json!([]),
            })
        ),
        "UI_STATE"
    );
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

#[test]
fn every_field_of_a_common_kind_decodes_without_an_arm_of_its_own() {
    use crate::protocol::generated::{FIELDS, field, tag};
    for (id, (name, kind)) in FIELDS.iter().enumerate() {
        let (good, bad) = match *kind {
            "flag" => (json!(1), json!("x")),
            "handle" => (json!(7), json!("x")),
            "element-id" => (json!("a"), json!("a b")),
            "element-ids" => (json!("a b"), json!("")),
            _ => continue,
        };
        let tag = if id == field::WEB_HANDLES_DRAGS {
            tag::WEBVIEW
        } else {
            tag::COLUMN
        };
        let check = |value: &Value| super::super::values::check(tag, id, value);
        assert!(check(&good).is_ok(), "{name}: {:?}", check(&good));
        assert!(check(&bad).is_err(), "{name}");
    }
}

// A free-text string field falls through to the colour grammar unless it has an arm of its own.
#[test]
fn a_page_title_is_text_not_a_colour() {
    use crate::protocol::generated::{field, tag};
    let check = |value: Value| super::super::values::check(tag::PAGE, field::PAGE_TITLE, &value);
    assert!(check(json!("Adaptive Layout")).is_ok());
    assert!(check(json!(1)).is_err());
}

fn step(name: &str, times: f64, plus: f64) -> Value {
    let slot = crate::protocol::SCALES
        .iter()
        .position(|(slot, _)| *slot == name)
        .unwrap();
    json!([slot, times, plus])
}

fn styled(id: usize, value: Value) -> Result<Vec<crate::protocol::Record>, String> {
    wire(json!([0, -1, "", "", "", 1, id, value]))
}

#[test]
fn a_scale_step_decodes_on_the_fields_its_family_lists() {
    use crate::protocol::field;
    for (id, value) in [
        (field::PADDING_TOP, step("space", 4.0, 0.0)),
        (field::FONT_SIZE, step("text.sm", 1.0, 0.0)),
        (field::LINE_HEIGHT, step("text.sm", 1.25, 0.0)),
        (field::LINE_HEIGHT, step("leading.sm", 1.0, 0.0)),
        (field::LINE_HEIGHT, step("space", 6.0, 0.0)),
        (field::LETTER_SPACING, step("text.sm", -0.025, 0.0)),
        (field::RADIUS, step("radius.lg", 1.0, 0.0)),
        (field::RADIUS, step("radius.lg", 1.0, -2.0)),
        (field::MARGIN_TOP, step("space", -2.0, 0.0)),
        (field::INSET_LEFT, step("space", -1.0, -1.0)),
        (field::WIDTH, step("em", 1.25, 0.0)),
        (field::MAX_HEIGHT, step("space", 64.0, 0.0)),
    ] {
        let records = styled(id, value.clone()).unwrap();
        let (slot, times, plus) = (
            value[0].as_u64().unwrap() as usize,
            value[1].as_f64().unwrap(),
            value[2].as_f64().unwrap(),
        );
        if crate::protocol::SCALES[slot].0 == "em" {
            assert_eq!(records[0].fields[0], (id, value), "em waits for layout");
            continue;
        }
        let (_, resolved) = &records[0].fields[0];
        assert!(resolved.is_number(), "{resolved}");
        assert_eq!(
            records[0].fields.steps(),
            [(
                id,
                crate::protocol::scales::Step::Number { slot, times, plus }
            )]
        );
    }
    let transition = |duration: Value, easing: Value| {
        json!({"properties": ["opacity"], "durationMs": duration, "delayMs": 0, "easing": easing})
            .to_string()
    };
    let ok = transition(
        step("duration.base", 1.0, 0.0),
        step("ease.enter", 1.0, 0.0),
    );
    styled(field::TRANSITION, json!(ok)).unwrap();
}

#[test]
fn a_scale_step_outside_its_family_fails_with_the_slot_name() {
    use crate::protocol::field;
    let error = |id, value| styled(id, value).unwrap_err();
    assert_eq!(
        error(field::PADDING_TOP, step("radius.lg", 1.0, 0.0)),
        "UI_SCALE: paddingTop does not take radius.lg"
    );
    assert_eq!(
        error(field::RADIUS, step("em", 1.0, 0.0)),
        "UI_SCALE: radius does not take em"
    );
    // A field no family lists reads an array by its own grammar.
    assert_eq!(
        error(field::COLOR, step("space", 1.0, 0.0)),
        "UI_STYLE: expected color"
    );
    let slots = crate::protocol::SCALES.len();
    for bad in [
        json!([1.5, 1, 0]),
        json!([slots, 1, 0]),
        json!([-1, 1, 0]),
        json!([0, 1]),
        json!([0, 1, 0, 0]),
        json!([0, "1", 0]),
    ] {
        assert_eq!(code(styled(field::PADDING_TOP, bad)), "UI_SCALE");
    }
    assert_eq!(
        error(field::PADDING_TOP, step("space", -1.0, 0.0)),
        "UI_STYLE: paddingTop resolves below 0"
    );
    assert_eq!(
        code(styled(field::FONT_SIZE, step("text.sm", -1.0, -1.0))),
        "UI_STYLE"
    );
    let transition = |duration: Value, easing: Value| {
        json!(json!({"properties": ["opacity"], "durationMs": duration, "delayMs": 0, "easing": easing})
            .to_string())
    };
    for (duration, easing) in [
        (step("duration.base", 2.0, 0.0), json!("linear")),
        (step("duration.base", 1.0, 10.0), json!("linear")),
        (step("ease.enter", 1.0, 0.0), json!("linear")),
        (json!(120), step("duration.fast", 1.0, 0.0)),
        (json!(120), step("ease.exit", 1.0, 1.0)),
    ] {
        assert_eq!(
            code(styled(field::TRANSITION, transition(duration, easing))),
            "UI_TRANSITION"
        );
    }
}

#[test]
fn a_state_style_holds_paint_only_so_no_scale_step() {
    use crate::protocol::field;
    let pairs = json!([field::OPACITY, step("space", 1.0, 0.0)]).to_string();
    assert_eq!(
        code(wire(json!([
            3,
            -1,
            "",
            "",
            "",
            1,
            field::PRESSED_STYLE,
            pairs
        ]))),
        "UI_STYLE"
    );
}

#[test]
fn an_array_on_a_field_no_scale_lists_reaches_the_session_unchanged() {
    use crate::protocol::{SCALE_STEP_FIELDS, field, tag};
    assert!(!SCALE_STEP_FIELDS.contains(&field::SELECTION_REQUEST));
    let request = json!([1, 0, 5]);
    let records = wire(json!([
        tag::INPUT,
        -1,
        "",
        "",
        "",
        1,
        field::SELECTION_REQUEST,
        request.clone()
    ]))
    .unwrap();
    assert_eq!(
        records[0].fields.as_slice(),
        &[(field::SELECTION_REQUEST, request)]
    );
    assert!(records[0].fields.steps().is_empty());
}

#[test]
fn size_keywords_decimal_keyboards_and_the_scale_flag_decode() {
    use crate::protocol::field;
    for id in [
        field::MIN_WIDTH_KEYWORD,
        field::MIN_HEIGHT_KEYWORD,
        field::MAX_WIDTH_KEYWORD,
        field::MAX_HEIGHT_KEYWORD,
    ] {
        styled(id, json!("min-content")).unwrap();
        assert_eq!(code(styled(id, json!("auto"))), "UI_STYLE");
    }
    wire(json!([8, -1, "", "", "", 1, field::KEYBOARD, "decimal"])).unwrap();
    wire(json!([20, -1, "", "", "", 1, field::SCALE_REFS, 1])).unwrap();
}

#[test]
fn motion_steps_in_a_transition_resolve_and_follow_new_scales() {
    use crate::protocol::{field, scales};
    let encoded = json!({"properties": ["opacity"], "durationMs": step("duration.base", 1.0, 0.0), "delayMs": 0, "easing": step("ease.exit", 1.0, 0.0)}).to_string();
    let records = styled(field::TRANSITION, json!(encoded)).unwrap();
    let resolved: Value = serde_json::from_str(records[0].fields[0].1.as_str().unwrap()).unwrap();
    assert_eq!(resolved["durationMs"], json!(200.0));
    assert_eq!(resolved["easing"], json!([0.4, 0.0, 1.0, 1.0]));

    let mut pushed = scales::Scales::builtin().push();
    let base = crate::protocol::SCALES
        .iter()
        .position(|(slot, _)| *slot == "duration.base")
        .unwrap();
    pushed["values"][base] = json!(400);
    let rescaled = scales::rescale(&records, &scales::Scales::decode(&pushed).unwrap())
        .unwrap()
        .unwrap();
    let resolved: Value = serde_json::from_str(rescaled[0].fields[0].1.as_str().unwrap()).unwrap();
    assert_eq!(resolved["durationMs"], json!(400.0));
    // A plain transition is not a step and is kept as written.
    let plain =
        json!({"properties": ["opacity"], "durationMs": 120, "delayMs": 0, "easing": "linear"})
            .to_string();
    let records = styled(field::TRANSITION, json!(plain)).unwrap();
    assert_eq!(records[0].fields[0].1, json!(plain));
    assert!(records[0].fields.steps().is_empty());
}
