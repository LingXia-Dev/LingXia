//! `view.environment`: a test run pins a page's theme and stills it, then restores it.
use super::*;

#[test]
fn a_test_override_pins_the_theme_stills_the_caret_and_clears() {
    let worker = ViewWorker::spawn("test-view").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render({env}){return h('text',{},`${env.theme} ${env.reducedMotion}`)}"),
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
        serde_json::json!({"theme": "dark", "still": true, "textScale": null, "displayLanguage": "en"})
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
