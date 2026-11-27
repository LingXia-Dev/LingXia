//! Every published `lx` member exists at run time on this platform: one the host lacks is a stub
//! that rejects `E_NOT_SUPPORTED`, never a missing key.
//!
//! Booting the whole surface links the Apple host's Swift bridge, so on macOS this target is empty
//! unless built against stubs for those symbols. Collect the undefined `__swift_bridge__$…` names
//! from a failed link, write `stub.c` with `long <name>(void) { return 0; }` for each (but
//! `return 1;` for `__swift_bridge__$apply_appearance`, the one called at boot), `cc -c` it, then
//! `cargo rustc -p lingxia-logic --test lx_surface --profile test -- -C link-arg=<stub.o> --cfg lx_surface_stubs`
//! and run the binary. CI runs it on Linux (`control-security-gates.sh portable-rust`).
#![cfg(any(not(target_os = "macos"), lx_surface_stubs))]

use lxapp::lx::LxLogicExtension;
use rong::{JSEngine, Source};

#[tokio::test(flavor = "current_thread")]
async fn every_published_member_exists_at_run_time() {
    tokio::task::LocalSet::new()
        .run_until(check_surface())
        .await;
}

async fn check_surface() {
    let api: serde_json::Value =
        serde_json::from_str(include_str!("../../../packages/lingxia-types/api.json")).unwrap();
    let paths: Vec<&str> = api["members"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|member| {
            matches!(
                member["kind"].as_str(),
                Some("function" | "event" | "watcher" | "namespace")
            )
        })
        .map(|member| member["path"].as_str().unwrap())
        .filter(|path| path.starts_with("lx."))
        .collect();
    lingxia_host_config::set_app_config(
        lingxia_host_config::AppConfig::parse_and_validate(
            r#"{"productName":"Surface","productVersion":"1.0.0"}"#,
        )
        .unwrap(),
    )
    .unwrap();
    let runtime = <rong::RongJS as JSEngine>::runtime();
    let ctx = runtime.context();
    let _app = lxapp::test_context::bind_test_lxapp(&ctx, "lx-surface-test");
    lingxia_logic::LxLogicRuntime.init(&ctx).unwrap();
    ctx.global()
        .set("paths", serde_json::to_string(&paths).unwrap())
        .unwrap();
    let missing: String = ctx
        .eval(Source::from_bytes(
            r#"JSON.parse(paths).filter((path) => {
              let value = globalThis;
              for (const key of path.split('.')) {
                if (value == null || !(key in Object(value))) return true;
                value = value[key];
              }
              return value == null;
            }).join('\n')"#,
        ))
        .unwrap();
    assert!(missing.is_empty(), "missing at run time:\n{missing}");
    // A build without automation keeps the member, and it says so synchronously.
    let automation: String = ctx
        .eval(Source::from_bytes(
            r#"(() => { try { lx.automation(); return 'no throw'; } catch (e) { return `${e.code} ${lx.supports('automation')}`; } })()"#,
        ))
        .unwrap();
    assert_eq!(automation, "E_NOT_SUPPORTED false");
}
