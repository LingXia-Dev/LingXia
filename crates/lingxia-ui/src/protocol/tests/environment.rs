//! The schema's environment section against what the builder and the session produce.
use crate::platform::DeviceFacts;
use crate::protocol::{ENVIRONMENT_KEYS, check_environment};

fn facts() -> DeviceFacts {
    DeviceFacts {
        width: 828,
        height: 1792,
        scale: 2.0,
        os: "android".into(),
        smallest_width: 414.0,
        locale: "en".into(),
        animation_scale: 1.0,
        text_scale: Some(vec![[0.0, 0.0], [16384.0, 16384.0]]),
        ..Default::default()
    }
}

#[test]
fn the_builder_emits_exactly_the_device_keys_the_schema_names() {
    let env = facts().environment().unwrap().value;
    let origin = |key: &str| {
        ENVIRONMENT_KEYS
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, o)| *o)
    };
    for key in env.as_object().unwrap().keys() {
        assert_eq!(
            origin(key),
            Some("device"),
            "{key} is not a device key in the schema"
        );
    }
    for (key, origin) in ENVIRONMENT_KEYS {
        if *origin != "device" {
            assert!(
                env.get(*key).is_none(),
                "{key} comes from the {origin}, not the device"
            );
        }
    }
    // With the host's pushes the snapshot passes the generated check; without them it does not.
    assert!(!check_environment(&env));
    let mut pushed = env.clone();
    pushed["canDock"] = false.into();
    pushed["sizeClass"] = "compact".into();
    assert!(check_environment(&pushed));
    pushed["theme"] = "sepia".into();
    assert!(!check_environment(&pushed));
}

#[test]
fn the_session_fills_the_session_keys() {
    let source = crate::session::tests::source(
        "function render({env}){return h('text',{},Object.keys(env).sort().join(','))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let mut env = facts().environment().unwrap().value;
    env["canDock"] = false.into();
    page.set_environment(env).unwrap();
    let shown = page.layout(400, 400, 1.0).unwrap().nodes[0].text.clone();
    let keys: Vec<&str> = shown.split(',').collect();
    for (key, _) in ENVIRONMENT_KEYS
        .iter()
        .filter(|(key, _)| *key != "textScale")
    {
        assert!(keys.contains(key), "{key} missing from env: {shown}");
    }
}
