use lingxia_app_context::{AppConfig, set_app_config};
use lingxia_service::applink::{self, AppLinkTarget};
use lingxia_update::Channel;
use std::sync::Mutex;

static DELIVERED: Mutex<Vec<AppLinkTarget>> = Mutex::new(Vec::new());

fn receive(target: AppLinkTarget) -> i32 {
    DELIVERED.lock().unwrap().push(target);
    1
}

#[test]
fn configured_server_is_accepted_only_by_the_scan_entrypoint() {
    // Config and handler are process-wide OnceLocks; this test owns its process.
    let config: AppConfig = serde_json::from_value(serde_json::json!({
        "productName": "Test", "productVersion": "1.0.0", "env": "prod",
        "lingxiaServer": "https://api.example.com",
        "appLinks": {"hosts": ["app.example.com"]}
    }))
    .unwrap();
    set_app_config(config).unwrap();
    applink::register_handler(receive);
    let link = "https://api.example.com/lxapp/open?appId=shop&channel=draft";
    assert_eq!(applink::deliver(link), 0);
    assert_eq!(applink::deliver_lxapp_only(link), 1);
    let delivered = DELIVERED.lock().unwrap().pop().unwrap();
    assert_eq!(delivered.appid, "shop");
    assert_eq!(delivered.release_type, Channel::Draft);
    for url in [
        "https://api.example.com/app/auth",
        "https://api.example.com:bogus/lxapp/open?appId=shop",
        "https://api.example.com:99999/lxapp/open?appId=shop",
        "https://other.example.com/lxapp/open?appId=shop",
    ] {
        assert_eq!(applink::deliver_lxapp_only(url), 0, "{url}");
    }
    assert_eq!(applink::deliver("https://app.example.com/app/auth"), 1);
}
