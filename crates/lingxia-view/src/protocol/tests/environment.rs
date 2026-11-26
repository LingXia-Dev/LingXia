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
fn the_builder_emits_the_device_keys_and_no_session_key() {
    let env = facts().environment().unwrap().value.to_json();
    let origin = |key: &str| {
        ENVIRONMENT_KEYS
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, o)| *o)
    };
    for key in env.as_object().unwrap().keys() {
        assert!(
            matches!(origin(key), Some("device" | "push")),
            "{key} is not a native key in the schema"
        );
    }
    for (key, origin) in ENVIRONMENT_KEYS {
        if *origin == "device" {
            assert!(env.get(*key).is_some(), "the device reports {key}");
        }
    }
    // Pushes the host has not made yet stay at their unpushed values.
    assert_eq!((env["canDock"].as_bool(), env.get("sizeClass")), (Some(false), None));
    assert!(check_environment(&env));
    let mut wrong = env.clone();
    wrong["appearance"] = "sepia".into();
    assert!(!check_environment(&wrong));
}

#[test]
fn the_session_fills_the_session_keys() {
    let source = crate::session::tests::source(
        "function render({env}){return h('text',{},Object.keys(env).sort().join(','))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let mut env = facts().environment().unwrap().value.to_json();
    env["palette"] =
        serde_json::json!({"revision": 1, "colors": vec!["#000000"; lingxia_theme::ROLE_COUNT]});
    page.set_environment(env).unwrap();
    let shown = page.layout(400, 400, 1.0).unwrap().nodes[0].text.clone();
    let keys: Vec<&str> = shown.split(',').collect();
    for (key, _) in ENVIRONMENT_KEYS
        .iter()
        .filter(|(key, kind)| *key != "textScale" && *kind != "push")
    {
        assert!(keys.contains(key), "{key} missing from env: {shown}");
    }
    assert!(
        !keys.contains(&"palette"),
        "pages never see colours: {shown}"
    );
}

#[test]
fn a_pushed_palette_repaints_roles_and_editors_default_to_line_strong() {
    let source = crate::session::tests::source(
        "function render(){return h('view',{},h('input',{style:{borderWidth:1,width:100,height:30}}))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let rects = |page: &mut crate::ViewSession| {
        let display = page.layout(200, 200, 1.0).unwrap();
        let commands = &display.retained.commands;
        let canvas = commands.iter().find_map(|paint| match paint {
            crate::scene::paint::Paint::Rect { color, .. } => Some(*color),
            _ => None,
        });
        let border = commands.iter().find_map(|paint| match paint {
            crate::scene::paint::Paint::Border { color, .. } => Some(*color),
            _ => None,
        });
        (canvas.unwrap(), border.unwrap())
    };
    let mut env = facts().environment().unwrap().value.to_json();
    page.set_environment(env.clone()).unwrap();
    let builtin = crate::theme::Palette::builtin(false);
    assert_eq!(
        rects(&mut page),
        (
            builtin.role(lingxia_theme::Role::Canvas),
            builtin.role(lingxia_theme::Role::LineStrong)
        )
    );
    let mut colors = vec!["#000000"; lingxia_theme::ROLE_COUNT];
    colors[lingxia_theme::Role::LineStrong as usize] = "#ff0000";
    env["palette"] = serde_json::json!({"revision": 2, "colors": colors});
    page.set_environment(env).unwrap();
    assert_eq!(rects(&mut page), ([0, 0, 0, 255], [255, 0, 0, 255]));
}
