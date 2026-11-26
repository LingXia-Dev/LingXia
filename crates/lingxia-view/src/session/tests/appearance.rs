//! A theme change repaints a page in the new palette; its View runs again only if it depends on
//! the appearance.
use super::*;

/// A page whose `runs` text counts its renders, over a box in the `surface` role.
fn page_source(body: &str, classes: &str) -> String {
    source(&format!(
        "globalThis.runs=0;function render({{env}}){{globalThis.runs++;{body}}}"
    ))
    .replace(
        "__lingxiaPublish(render);",
        &format!("configureClasses({classes});__lingxiaPublish(render);"),
    )
}

const BOX: &str = "h('view',{style:{width:60,height:40,backgroundColor:'surface'}})";

fn runs(host: &ViewHost<Fake>) -> String {
    host.automation_request(&serde_json::json!({"op": "query", "css": "[data-testid=\"runs\"]"}))
        .unwrap()["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

fn colours(host: &ViewHost<Fake>) -> Vec<[u8; 4]> {
    let mut colours: Vec<[u8; 4]> = host
        .adapter()
        .log
        .lock()
        .unwrap()
        .rects
        .iter()
        .map(|(colour, _)| *colour)
        .collect();
    colours.sort();
    colours.dedup();
    colours
}

fn set_theme(platform: &Platform, host: &ViewHost<Fake>, theme: &str) {
    host.adapter()
        .environment
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .value.appearance = crate::protocol::env::Appearance::parse(theme).unwrap();
    host.environment_changed();
    platform.settle(&[host]);
    run_frames(platform, host, 4);
}

fn shown(platform: &Platform, worker: &Arc<ViewWorker>, module: &str) -> ViewHost<Fake> {
    let host = platform.host(worker, module, false);
    host.set_visible(true);
    platform.settle(&[&host]);
    run_frames(platform, &host, 4);
    host
}

#[test]
fn a_page_of_role_colours_repaints_a_theme_change_without_running_its_view() {
    let worker = ViewWorker::spawn("test-appearance-roles").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = shown(
        &platform,
        &worker,
        &page_source(
            &format!(
                "return h('page',{{}},{BOX},h('text',{{'data-testid':'runs'}},'runs '+globalThis.runs))"
            ),
            "[]",
        ),
    );
    let (before, light) = (runs(&host), colours(&host));
    let presented = host.adapter().log.lock().unwrap().presented.len();
    set_theme(&platform, &host, "dark");
    assert!(
        host.adapter().log.lock().unwrap().presented.len() > presented,
        "a frame presented"
    );
    assert_ne!(colours(&host), light, "in the dark palette");
    assert_eq!(runs(&host), before, "and the View did not run");
    set_theme(&platform, &host, "light");
    assert_eq!(colours(&host), light);
    assert_eq!(runs(&host), before);
}

#[test]
fn a_page_that_reads_the_theme_or_has_a_theme_class_still_renders_on_a_theme_change() {
    for (name, body, classes) in [
        (
            "env.appearance",
            format!(
                "return h('page',{{}},{BOX},h('text',{{'data-testid':'look'}},'look '+env.appearance),h('text',{{'data-testid':'runs'}},'runs '+globalThis.runs))"
            ),
            "[]",
        ),
        (
            "dark: class",
            format!(
                "return h('page',{{}},{BOX},h('view',{{className:'dark:mark',style:{{width:20,height:20}}}}),h('text',{{'data-testid':'runs'}},'runs '+globalThis.runs))"
            ),
            "[{name:'dark:mark',when:{appearance:'dark'},style:{backgroundColor:'#ff0000'}}]",
        ),
    ] {
        let worker = ViewWorker::spawn("test-appearance-reads").unwrap();
        let platform = Platform::new(&[&worker]);
        let host = shown(&platform, &worker, &page_source(&body, classes));
        let before = runs(&host);
        set_theme(&platform, &host, "dark");
        assert_ne!(runs(&host), before, "{name}: the View ran again");
        if name == "dark: class" {
            assert!(
                colours(&host).contains(&[255, 0, 0, 255]),
                "{name}: its dark style shows"
            );
        } else {
            let look = host
                .automation_request(
                    &serde_json::json!({"op": "query", "css": "[data-testid=\"look\"]"}),
                )
                .unwrap();
            assert_eq!(look["text"], "look dark", "{name}");
        }
    }
}

#[test]
fn a_page_that_starts_reading_the_theme_renders_on_the_next_change() {
    let worker = ViewWorker::spawn("test-appearance-later").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = shown(
        &platform,
        &worker,
        &page_source(
            &format!(
                "const on=useLocal('on',false);return h('page',{{}},{BOX},h('button',{{'data-testid':'on',onClick(){{on.set(true)}}}},'On'),h('text',{{'data-testid':'look'}},on.value?'look '+env.appearance:'look -'),h('text',{{'data-testid':'runs'}},'runs '+globalThis.runs))"
            ),
            "[]",
        ),
    );
    let before = runs(&host);
    set_theme(&platform, &host, "dark");
    assert_eq!(runs(&host), before, "nothing read it yet");
    host.automation_request(
        &serde_json::json!({"op":"act","css":"[data-testid=\"on\"]","action":{"kind":"click"}}),
    )
    .unwrap();
    platform.settle(&[&host]);
    run_frames(&platform, &host, 4);
    let look = |host: &ViewHost<Fake>| {
        host.automation_request(
            &serde_json::json!({"op": "query", "css": "[data-testid=\"look\"]"}),
        )
        .unwrap()["text"]
            .clone()
    };
    assert_eq!(
        look(&host),
        "look dark",
        "the render after the skipped change sees the new theme"
    );
    set_theme(&platform, &host, "light");
    assert_eq!(look(&host), "look light");
}

#[cfg(feature = "hot-swap")]
#[test]
fn a_hot_swap_changes_whether_a_theme_change_runs_the_view() {
    let worker = ViewWorker::spawn("test-appearance-swap").unwrap();
    let platform = Platform::new(&[&worker]);
    let dev = "\nconfigureSourceMap({sources:[null],mappings:''});\nenableHotSwap();\n";
    let plain = page_source(
        &format!(
            "return h('page',{{}},{BOX},h('text',{{'data-testid':'look'}},'look -'),h('text',{{'data-testid':'runs'}},'runs '+globalThis.runs))"
        ),
        "[]",
    ) + dev;
    let reading = page_source(
        &format!(
            "return h('page',{{}},{BOX},h('text',{{'data-testid':'look'}},'look '+env.appearance),h('text',{{'data-testid':'runs'}},'runs '+globalThis.runs))"
        ),
        "[]",
    ) + dev;
    let host = shown(&platform, &worker, &plain);
    let look = |host: &ViewHost<Fake>| {
        host.automation_request(
            &serde_json::json!({"op": "query", "css": "[data-testid=\"look\"]"}),
        )
        .unwrap()["text"]
            .clone()
    };
    let before = runs(&host);
    set_theme(&platform, &host, "dark");
    assert_eq!(runs(&host), before, "the first module reads no theme");
    host.swap_module(reading);
    platform.settle(&[&host]);
    run_frames(&platform, &host, 4);
    assert_eq!(look(&host), "look dark");
    set_theme(&platform, &host, "light");
    assert_eq!(look(&host), "look light", "the swapped-in module reads it");
    host.swap_module(plain);
    platform.settle(&[&host]);
    run_frames(&platform, &host, 4);
    let before = runs(&host);
    set_theme(&platform, &host, "dark");
    assert_eq!(runs(&host), before, "and swapped back, it does not");
}

#[test]
fn a_theme_change_racing_a_render_leaves_the_page_reading_the_new_theme() {
    for theme_first in [false, true] {
        let worker = ViewWorker::spawn("test-appearance-race").unwrap();
        let platform = Platform::new(&[&worker]);
        let host = shown(
            &platform,
            &worker,
            &page_source(
                &format!(
                    "const n=useLocal('n',0);return h('page',{{}},{BOX},h('button',{{'data-testid':'pick',onClick(){{n.set(n.value+1)}}}},'Pick'),h('text',{{'data-testid':'look'}},'look '+env.appearance+' '+n.value))"
                ),
                "[]",
            ),
        );
        let pick = || {
            host.automation_request(
                &serde_json::json!({"op":"act","css":"[data-testid=\"pick\"]","action":{"kind":"click"}}),
            )
            .unwrap();
        };
        let theme = |value: &str| {
            host.adapter()
                .environment
                .lock()
                .unwrap()
                .as_mut()
                .unwrap()
                .value.appearance = crate::protocol::env::Appearance::parse(value).unwrap();
            host.environment_changed();
        };
        for (step, value) in ["dark", "light", "dark"].into_iter().enumerate() {
            if theme_first {
                theme(value);
                pick();
            } else {
                pick();
                theme(value);
            }
            platform.settle(&[&host]);
            run_frames(&platform, &host, 6);
            let look = host
                .automation_request(
                    &serde_json::json!({"op": "query", "css": "[data-testid=\"look\"]"}),
                )
                .unwrap()["text"]
                .clone();
            assert_eq!(
                look,
                format!("look {value} {}", step + 1),
                "theme first: {theme_first}"
            );
        }
    }
}
