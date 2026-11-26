//! `view.environment`: a test run pins a page's theme and stills it, then restores it.
use super::*;

#[test]
fn a_test_override_pins_the_theme_stills_the_caret_and_clears() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render({env}){return h('text',{},`${env.appearance} ${env.reducedMotion}`)}"),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    assert!(
        last(&host).contains(&"light false".to_owned()),
        "{:?}",
        last(&host)
    );
    let request = |value: serde_json::Value| {
        let mut value = value;
        value["op"] = "environment".into();
        host.automation_request(&value)
    };
    assert!(
        request(serde_json::json!({"theme": "sepia"}))
            .unwrap_err()
            .starts_with("E_INVALID_ARG")
    );
    assert!(
        request(serde_json::json!({"still": "yes"}))
            .unwrap_err()
            .starts_with("E_INVALID_ARG")
    );

    let pinned = request(serde_json::json!({"theme": "dark", "still": true})).unwrap();
    assert_eq!(
        (&pinned["theme"], &pinned["still"]),
        (&"dark".into(), &true.into())
    );
    platform.settle(&[&host]);
    // A read changes nothing and names what a baseline records but a test cannot set.
    let read = request(serde_json::json!({"read": true})).unwrap();
    assert_eq!(
        read,
        serde_json::json!({"theme": "dark", "still": true, "now": null, "contrast": null, "textScale": null, "language": "en"})
    );
    assert!(
        last(&host).contains(&"dark true".to_owned()),
        "{:?}",
        last(&host)
    );

    // A still page's caret stays shown and its blink timer stops.
    let state = CaretState {
        text: String::new(),
        anchor: 0,
        focus: 0,
        composition: (-1, -1),
    };
    host.caret(true, Some(state));
    assert!(platform.advance(Duration::from_millis(500)) <= 1);
    assert_eq!(platform.advance(Duration::from_secs(3)), 0);
    assert!(host.caret_visible());

    let cleared = request(serde_json::json!({"clear": true})).unwrap();
    assert_eq!(
        (&cleared["theme"], &cleared["still"]),
        (&serde_json::Value::Null, &false.into())
    );
    platform.settle(&[&host]);
    assert!(
        last(&host).contains(&"light false".to_owned()),
        "{:?}",
        last(&host)
    );
    // Released, the caret blinks again.
    assert_eq!(platform.advance(Duration::from_millis(500)), 1);
    assert!(!host.caret_visible());
}

#[test]
fn a_test_override_pins_the_text_scale_and_the_contrast() {
    let worker = ViewWorker::spawn("test-view-scale").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render({env}){return h('view',{style:{alignItems:'flex-start'}},h('text',{'data-testid':'t',style:{fontSize:20}},`contrast ${env.highContrast}`))}"),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let request = |value: serde_json::Value| {
        let mut value = value;
        value["op"] = "environment".into();
        host.automation_request(&value)
    };
    let height = || {
        host.automation_request(&serde_json::json!({"op": "query", "css": "[data-testid=\"t\"]"}))
            .unwrap()["rect"]["height"]
            .as_f64()
            .unwrap()
    };
    let plain = height();
    assert!(last(&host).contains(&"contrast false".to_owned()));
    for bad in [
        serde_json::json!({"textScale": 9}),
        serde_json::json!({"textScale": "large"}),
        serde_json::json!({"contrast": "more"}),
    ] {
        assert!(request(bad).unwrap_err().starts_with("E_INVALID_ARG"));
    }

    // Above the page's maxTextScale (2 by default) a pin fails rather than clamping unseen.
    let above = request(serde_json::json!({"textScale": 2.5})).unwrap_err();
    assert!(
        above.starts_with("E_INVALID_ARG") && above.contains("maxTextScale 2"),
        "{above}"
    );
    host.update_environment(r#"{"kind":"textScaleLimit","max":3}"#)
        .unwrap();
    request(serde_json::json!({"textScale": 2.5})).unwrap();

    let pinned = request(serde_json::json!({"textScale": 2, "contrast": "high"})).unwrap();
    assert_eq!(pinned["contrast"], "high");
    platform.settle(&[&host]);
    assert!(last(&host).contains(&"contrast true".to_owned()));
    let doubled = height();
    assert!(
        (doubled / plain - 2.0).abs() < 0.1,
        "text at twice its size: {plain} -> {doubled}"
    );
    let read = request(serde_json::json!({"read": true})).unwrap();
    assert_eq!(
        read["textScale"],
        serde_json::json!([[0.0, 0.0], [16384.0, 32768.0]])
    );

    request(serde_json::json!({"clear": true})).unwrap();
    platform.settle(&[&host]);
    assert_eq!(height(), plain);
    assert!(last(&host).contains(&"contrast false".to_owned()));
}
