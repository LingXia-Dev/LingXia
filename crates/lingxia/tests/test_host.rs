//! `lingxia::test_host!` drives routes the way a page calls them.

use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

#[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq)]
struct Echo {
    value: String,
}

#[lingxia::native("t.echo")]
async fn echo(input: Echo) -> Echo {
    input
}

#[derive(serde::Deserialize)]
struct Add {
    a: u32,
    b: u32,
}

#[lingxia::native("t.add", sync)]
fn add(input: Add) -> u32 {
    input.a + input.b
}

#[derive(Default, lingxia::FromLxApp)]
struct Counter(AtomicU32);

#[lingxia::native("t.count")]
async fn count(counter: lingxia::LxAppState<Counter>) -> u32 {
    counter.0.fetch_add(1, Ordering::Relaxed) + 1
}

#[lingxia::native("t.progress", stream)]
async fn progress(progress: lingxia::Progress<u32>) -> lingxia::Result<u32> {
    for step in 1..=3 {
        progress.send(step)?;
    }
    Ok(6)
}

static CANCELED: AtomicBool = AtomicBool::new(false);

/// Cancel drops an async route at its next `.await`.
struct MarkCanceled;

impl Drop for MarkCanceled {
    fn drop(&mut self) {
        CANCELED.store(true, Ordering::SeqCst);
    }
}

#[lingxia::native("t.forever", stream)]
async fn forever(progress: lingxia::Progress<u32>) -> lingxia::Result<u32> {
    let _mark = MarkCanceled;
    progress.send(0)?;
    std::future::pending::<()>().await;
    Ok(0)
}

struct App {
    name: &'static str,
}

impl lingxia::HostAddon for App {}

// What `lingxia::host!(App)` declares; a route's `Host<A>` is checked against it.
type __LingxiaHostAddon = App;

#[lingxia::native("t.addon")]
async fn addon(host: lingxia::Host<App>) -> String {
    host.name.to_string()
}

#[lingxia::native("t.home", audience = "control-app-only")]
async fn home() -> &'static str {
    "home"
}

#[lingxia::native("t.fail")]
async fn fail() -> lingxia::Result<()> {
    Err(lingxia::Error::not_found("no such note"))
}

fn __lingxia_install_routes() {
    use lingxia::host::{RouteAudience::*, register_host_entry};
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        register_host_entry(__lingxia_route_echo("t", "echo", AppSessionOnly));
        register_host_entry(__lingxia_route_add("t", "add", AppSessionOnly));
        register_host_entry(__lingxia_route_count("t", "count", AppSessionOnly));
        register_host_entry(__lingxia_route_progress("t", "progress", AppSessionOnly));
        register_host_entry(__lingxia_route_forever("t", "forever", AppSessionOnly));
        register_host_entry(__lingxia_route_addon("t", "addon", AppSessionOnly));
        register_host_entry(__lingxia_route_home("t", "home", ControlAppOnly));
        register_host_entry(__lingxia_route_fail("t", "fail", AppSessionOnly));
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn calls_async_and_sync_routes_and_reports_errors() {
    let host = lingxia::test_host!();
    let echo: Echo = host.call("t.echo", json!({ "value": "hi" })).await.unwrap();
    assert_eq!(echo, Echo { value: "hi".into() });
    let sum: u32 = host.call("t.add", json!({ "a": 2, "b": 3 })).await.unwrap();
    assert_eq!(sum, 5);

    let err = host.call::<()>("t.fail", ()).await.unwrap_err();
    assert!(err.to_string().contains("no such note"), "{err}");
    let err = host.call::<()>("t.missing", ()).await.unwrap_err();
    assert!(err.to_string().contains("not registered"), "{err}");
}

#[tokio::test(flavor = "multi_thread")]
async fn lxapp_state_is_fresh_for_each_test_host() {
    let first = lingxia::test_host!();
    assert_eq!(first.call::<u32>("t.count", ()).await.unwrap(), 1);
    assert_eq!(first.call::<u32>("t.count", ()).await.unwrap(), 2);
    let second = lingxia::test_host!();
    assert_eq!(second.call::<u32>("t.count", ()).await.unwrap(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn streams_yield_events_then_the_result_and_dropping_cancels() {
    let host = lingxia::test_host!();
    let mut stream = host.stream::<u32, u32>("t.progress", ());
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event.unwrap());
    }
    assert_eq!(events, [1, 2, 3]);
    assert_eq!(stream.result().await.unwrap(), 6);

    let mut stream = host.stream::<u32, u32>("t.forever", ());
    assert_eq!(stream.next().await.unwrap().unwrap(), 0);
    drop(stream);
    for _ in 0..200 {
        if CANCELED.load(Ordering::SeqCst) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("dropping the stream did not cancel the route");
}

#[tokio::test(flavor = "multi_thread")]
async fn addons_and_session_classes_apply_per_test_host() {
    let host = lingxia::test_host!().with_addon(App { name: "notes" });
    assert_eq!(host.call::<String>("t.addon", ()).await.unwrap(), "notes");
    let err = lingxia::test_host!()
        .call::<String>("t.addon", ())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Host<"), "{err}");

    assert!(host.call::<String>("t.home", ()).await.is_err());
    let home = host.as_class(lingxia::AppSessionClass::ControlApp);
    assert_eq!(home.call::<String>("t.home", ()).await.unwrap(), "home");
    assert_eq!(home.call::<String>("t.addon", ()).await.unwrap(), "notes");
}
