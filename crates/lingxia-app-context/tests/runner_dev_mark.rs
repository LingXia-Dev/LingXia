use lingxia_app_context::{AppConfig, app_config, dev_mark, register_runner_host, set_app_config};

// This integration test owns a process so registration cannot affect other tests.
#[test]
fn registering_a_runner_hides_the_mark_without_removing_the_dev_connection() {
    let config = AppConfig::parse_and_validate(
        r#"{
            "productName": "Dev host",
            "productVersion": "1.0.0",
            "env": "dev",
            "devWsUrl": "ws://127.0.0.1:39337"
        }"#,
    )
    .unwrap();
    set_app_config(config).unwrap();
    assert_eq!(dev_mark(false).as_deref(), Some("39337"));
    assert_eq!(dev_mark(true).as_deref(), Some("39337"));

    register_runner_host();
    assert_eq!(dev_mark(false), None);
    assert_eq!(dev_mark(true), None);
    assert_eq!(
        app_config().unwrap().dev_ws_url.as_deref(),
        Some("ws://127.0.0.1:39337")
    );
}
