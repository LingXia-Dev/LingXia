use super::*;
use serde_json::json;

fn handler(value: Value) -> Result<RouteAction, String> {
    parse_handler_value(&value, None)
}

fn fulfilled(value: Value) -> Fulfill {
    match handler(value).unwrap() {
        RouteAction::Fulfill(fulfill) => fulfill,
        other => panic!("expected fulfill, got {other:?}"),
    }
}

fn text(body: &str) -> Option<ResponseBody> {
    Some(ResponseBody::Text(body.into()))
}

#[test]
fn handler_json_body_sets_content_type() {
    let fulfill = fulfilled(json!({
        "status": 501,
        "json": { "error": "not_implemented" }
    }));
    assert_eq!(fulfill.status, 501);
    assert_eq!(fulfill.body, text(r#"{"error":"not_implemented"}"#));
    assert_eq!(
        fulfill.headers,
        vec![("content-type".to_string(), "application/json".to_string())]
    );

    // An explicit header wins over the implied JSON type.
    let fulfill = fulfilled(json!({
        "json": [1, 2],
        "headers": { "Content-Type": "application/problem+json" }
    }));
    assert_eq!(fulfill.status, 200);
    assert_eq!(fulfill.body, text("[1,2]"));
    assert_eq!(fulfill.headers.len(), 1);
}

#[test]
fn handler_string_body_is_verbatim() {
    let fulfill = fulfilled(json!({ "status": 404, "body": "missing", "statusText": "Not Found" }));
    assert_eq!(fulfill.body, text("missing"));
    assert_eq!(fulfill.status_text.as_deref(), Some("Not Found"));
    assert!(fulfill.headers.is_empty());
    assert_eq!(fulfill.delay_ms, 0);
}

#[test]
fn handler_binary_body_replaces_the_json_view() {
    // `binary_body` found an ArrayBuffer/typed array; JSON saw an object.
    let action = parse_handler_value(
        &json!({ "body": { "0": 1, "1": 2 }, "contentType": "application/octet-stream" }),
        Some(vec![1, 2]),
    )
    .unwrap();
    let RouteAction::Fulfill(fulfill) = action else {
        panic!("expected fulfill");
    };
    assert_eq!(fulfill.body, Some(ResponseBody::Binary(vec![1, 2])));
    assert!(parse_handler_value(&json!({ "body": {}, "json": 1 }), Some(vec![1])).is_err());
}

#[test]
fn handler_delay_is_bounded() {
    assert_eq!(fulfilled(json!({ "delay": 250 })).delay_ms, 250);
    assert_eq!(
        fulfilled(json!({ "delay": MAX_DELAY_MS })).delay_ms,
        MAX_DELAY_MS
    );
    for bad in [
        json!({ "delay": MAX_DELAY_MS + 1 }),
        json!({ "delay": -1 }),
        json!({ "delay": 1.5 }),
        json!({ "delay": "10" }),
    ] {
        assert!(handler(bad.clone()).is_err(), "{bad} should be rejected");
    }
}

#[test]
fn handler_abort_and_continue() {
    assert_eq!(
        handler(json!({ "abort": "failed" })).unwrap(),
        RouteAction::Abort(AbortKind::Failed)
    );
    assert_eq!(
        handler(json!({ "continue": true })).unwrap(),
        RouteAction::Continue
    );
}

#[test]
fn handler_patch_and_hang() {
    assert_eq!(
        handler(json!({ "continue": true, "patchJson": { "a": null, "b": [1] } })).unwrap(),
        RouteAction::Patch(json!({ "a": null, "b": [1] }))
    );
    assert_eq!(
        handler(json!({ "hang": true })).unwrap(),
        RouteAction::Hang { token: 0 }
    );
    for bad in [
        json!({ "patchJson": {} }),
        json!({ "continue": true, "patchJson": {}, "status": 200 }),
        json!({ "hang": true, "continue": true }),
        json!({ "hang": true, "delay": 10 }),
        json!({ "hang": true, "abort": "failed" }),
        json!({ "hang": false }),
        json!({ "hang": 1 }),
    ] {
        assert!(handler(bad.clone()).is_err(), "{bad} should be rejected");
    }
    let err = handler(json!({ "patchJson": { "a": 1 } })).unwrap_err();
    assert!(err.contains("continue: true"), "{err}");
}

#[test]
fn patch_applies_to_json_bodies_only() {
    assert_eq!(
        apply_patch_json(
            r#"{"items":[],"total":null}"#,
            r#"{"items":[1,2],"total":2,"page":1}"#
        )
        .unwrap(),
        r#"{"items":[],"page":1}"#
    );
    let err = apply_patch_json("{}", "<html>").unwrap_err();
    assert!(err.contains("not JSON"), "{err}");
}

#[test]
fn handler_kinds_are_exclusive() {
    let err = handler(json!({ "status": 500, "abort": "failed" })).unwrap_err();
    assert!(err.contains("abort") && err.contains("'status'"), "{err}");
    let err = handler(json!({ "continue": true, "delay": 5 })).unwrap_err();
    assert!(err.contains("continue") && err.contains("'delay'"), "{err}");
    let err = handler(json!({ "abort": "failed", "continue": true })).unwrap_err();
    assert!(err.contains("choose one"), "{err}");
}

#[test]
fn handler_rejects_ambiguous_or_invalid_options() {
    for bad in [
        json!({ "abort": true }),
        json!({ "abort": "timedout" }),
        json!({ "abort": "" }),
        json!({ "abort": null }),
        json!({ "continue": false }),
        json!({ "continue": true, "body": "x" }),
        json!({ "status": 99 }),
        json!({ "status": 600 }),
        json!({ "status": "500" }),
        json!({ "status": 204, "body": "x" }),
        json!({ "body": "x", "json": {} }),
        json!({ "body": [1, 2] }),
        json!({ "body": { "a": 1 } }),
        json!({ "body": 5 }),
        json!({ "headers": { "bad header": "x" } }),
        json!({ "headers": { "x": 1 } }),
        json!({ "statusText": "a\nb" }),
        json!({ "fulfil": {} }),
        json!("nope"),
    ] {
        assert!(handler(bad.clone()).is_err(), "{bad} should be rejected");
    }
}

#[test]
fn empty_no_content_response_has_null_body() {
    assert_eq!(fulfilled(json!({ "status": 204 })).body, None);
}

/// Drive the real Logic `fetch` wrapper in a Rong context against the
/// process route table.
mod interceptor {
    use super::super::*;
    use rong::{Rong, RongJS};

    const APPID: &str = "network-interceptor-test";
    const RUN: &str = "network-interceptor-run";

    /// Evaluate `script` in a context whose `fetch` routes as `appid` in
    /// `run`. `__route(glob, handler)` installs a route through the same
    /// handler parsing as `NetworkDriver.route`.
    fn eval_with_interceptor(
        appid: &'static str,
        run: &'static str,
        script: &'static str,
    ) -> String {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let pool = Rong::<RongJS>::builder()
                .shared()
                .workers(1)
                .build()
                .unwrap();
            let worker = pool.worker(0).unwrap();
            let handle = worker
                .spawn(async move |js_runtime, _receiver| -> JSResult<String> {
                    let ctx = js_runtime.context();
                    rong_modules::init(
                        &ctx,
                        [
                            "timer",
                            "event",
                            "exception",
                            "abort",
                            "encoding",
                            "url",
                            "buffer",
                            "stream",
                            "http",
                        ],
                    )?;
                    let route = JSFunc::new(
                        &ctx,
                        move |glob: String, handler: JSObject| -> JSResult<f64> {
                            let spec = RouteSpec {
                                matcher: UrlMatcher::glob(&glob).map_err(auto_err)?,
                                method: None,
                                times: None,
                                action: parse_handler(&handler)?,
                            };
                            let id = registry::with_registry(|routes| {
                                routes.install(run, appid, spec, || true)
                            })
                            .map_err(auto_err)?;
                            Ok(id as f64)
                        },
                    )?;
                    ctx.global().set("__route", route)?;
                    let unroute = JSFunc::new(&ctx, move |id: f64| -> bool {
                        registry::with_registry(|routes| routes.remove(run, id as u64))
                    })?;
                    ctx.global().set("__unroute", unroute)?;
                    install_fetch_interceptor(&ctx, move |_| {
                        Some(LogicTarget {
                            appid: appid.to_string(),
                            allowed: Box::new(|host| host != "blocked.test"),
                        })
                    })?;
                    ctx.eval_async::<String>(Source::from_bytes(script)).await
                })
                .await
                .unwrap();
            handle.join().await.unwrap()
        })
    }

    fn install(pattern: &str, method: Option<&str>, times: Option<u32>, action: Value) -> u64 {
        let spec = RouteSpec {
            matcher: UrlMatcher::glob(pattern).unwrap(),
            method: method.map(str::to_string),
            times,
            action: parse_handler_value(&action, None).unwrap(),
        };
        registry::with_registry(|routes| routes.install(RUN, APPID, spec, || true)).unwrap()
    }

    #[test]
    fn routes_fulfill_abort_and_expire_in_logic_fetch() {
        install(
            "https://api.test/v1/devices/*",
            Some("PATCH"),
            None,
            serde_json::json!({ "status": 501, "json": { "error": "not_implemented" } }),
        );
        install(
            "https://api.test/v1/devices/*",
            Some("GET"),
            Some(1),
            serde_json::json!({ "abort": "failed" }),
        );
        install(
            "https://blocked.test/**",
            None,
            None,
            serde_json::json!({ "status": 200, "body": "should not be served" }),
        );

        let out = eval_with_interceptor(
            APPID,
            RUN,
            r#"(async () => {
              const out = {};
              out.name = fetch.name;
              const patched = await fetch('https://api.test/v1/devices/d1', {
                method: 'patch', body: '{}',
              });
              out.status = patched.status;
              out.ok = patched.ok;
              out.type = patched.headers.get('content-type');
              out.url = patched.url;
              out.body = await patched.json();
              try {
                await fetch(new Request('https://api.test/v1/devices/d1'));
                out.abort = 'resolved';
              } catch (error) {
                out.abort = `${error.name}: ${error.message}`;
              }
              // The one-shot abort expired: the GET now reaches the real
              // transport, which refuses the unroutable test host.
              try {
                await fetch('https://api.test/v1/devices/d1');
                out.expired = 'resolved';
              } catch (error) {
                out.expired = error.message;
              }
              // A fulfillment never answers a host the app policy refuses.
              try {
                const blocked = await fetch('https://blocked.test/x');
                out.blocked = await blocked.text();
              } catch (error) {
                out.blocked = 'passed-through';
              }
              return JSON.stringify(out);
            })()"#,
        );
        let out: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(out["name"], "fetch");
        assert_eq!(out["status"], 501);
        assert_eq!(out["ok"], false);
        assert_eq!(out["type"], "application/json");
        assert_eq!(out["url"], "https://api.test/v1/devices/d1");
        assert_eq!(out["body"]["error"], "not_implemented");
        assert_eq!(out["abort"], "TypeError: fetch failed");
        assert_ne!(out["expired"], "resolved");
        assert_eq!(out["blocked"], "passed-through");

        let log = registry::with_registry(|routes| routes.requests(RUN, APPID));
        let actions: Vec<_> = log
            .iter()
            .map(|entry| (entry.method.as_str(), entry.action, entry.status))
            .collect();
        assert_eq!(
            actions,
            vec![
                ("PATCH", "fulfill", Some(501)),
                ("GET", "abort", None),
                ("GET", "continue", None),
            ]
        );

        clear_run(RUN);
        assert!(registry::with_registry(|routes| routes.requests(RUN, APPID)).is_empty());
    }

    /// Serve `body` as JSON on a loopback port for `connections` requests.
    fn json_server(body: &'static str, connections: usize) -> String {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming().take(connections) {
                let Ok(mut stream) = stream else { continue };
                let mut request = [0u8; 4096];
                let _ = stream.read(&mut request);
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\nx-origin: real\r\n\
                     content-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        format!("http://{addr}")
    }

    #[test]
    fn patches_the_real_response_and_holds_a_hang_until_released() {
        const APP: &str = "network-interceptor-patch";
        const OWNER: &str = "network-interceptor-patch-run";
        let base = json_server(r#"{"devices":[{"id":"d1"}],"total":1,"cursor":"x"}"#, 2);
        let script: &'static str = Box::leak(
            format!(
                r#"(async () => {{ try {{
              const out = {{}};
              const patchId = __route('{base}/devices', {{ continue: true, patchJson: {{ total: 0, devices: [], cursor: null }} }});
              const patched = await fetch('{base}/devices');
              out.status = patched.status;
              out.origin = patched.headers.get('x-origin');
              out.body = await patched.json();
              __unroute(patchId);
              const real = await fetch('{base}/devices');
              out.real = await real.json();

              __route('https://api.test/stall', {{ hang: true }});
              const controller = new AbortController();
              setTimeout(() => controller.abort(), 50);
              try {{
                await fetch('https://api.test/stall', {{ signal: controller.signal }});
                out.aborted = 'resolved';
              }} catch (error) {{
                out.aborted = error.name;
              }}
              const hangId = __route('https://api.test/held', {{ hang: true }});
              let settled = 'pending';
              const held = fetch('https://api.test/held').then(
                () => {{ settled = 'resolved'; }},
                (error) => {{ settled = `${{error.name}}: ${{error.message}}`; }},
              );
              await new Promise((resolve) => setTimeout(resolve, 450));
              out.whileHeld = settled;
              __unroute(hangId);
              await held;
              out.released = settled;
              return JSON.stringify(out);
            }} catch (error) {{
              return JSON.stringify({{ fatal: `${{error}}
${{error && error.stack}}` }});
            }} }})()"#
            )
            .into_boxed_str(),
        );
        let out = eval_with_interceptor(APP, OWNER, script);
        let out: Value = serde_json::from_str(&out).unwrap();
        assert!(out.get("fatal").is_none(), "{out}");
        assert_eq!(out["status"], 200);
        assert_eq!(out["origin"], "real");
        assert_eq!(
            out["body"],
            serde_json::json!({ "devices": [], "total": 0 })
        );
        assert_eq!(out["real"]["total"], 1);
        assert_eq!(out["aborted"], "AbortError");
        assert_eq!(out["whileHeld"], "pending");
        assert_eq!(out["released"], "TypeError: fetch failed");
        let actions: Vec<_> = registry::with_registry(|routes| routes.requests(OWNER, APP))
            .into_iter()
            .map(|entry| entry.action)
            .collect();
        assert_eq!(actions, vec!["continue", "hang", "hang"]);
        clear_run(OWNER);
    }

    #[test]
    fn records_request_and_serves_binary_and_delayed_responses() {
        const APP: &str = "network-interceptor-record";
        const OWNER: &str = "network-interceptor-record-run";
        let out = eval_with_interceptor(
            APP,
            OWNER,
            r#"(async () => { try {
              const out = {};
              __route('https://api.test/echo', { status: 201, delay: 60 });
              __route('https://api.test/bytes', { body: new Uint8Array([0, 1, 255]) });
              __route('https://api.test/buffer', { body: new Uint8Array([7, 8]).buffer });
              __route('https://api.test/slow', { delay: 5000 });
              try {
                __route('https://api.test/x', { status: 500, abort: 'failed' });
                out.mixed = 'accepted';
              } catch (error) {
                out.mixed = error.message;
              }
              try {
                __route('https://api.test/x', { body: { a: 1 } });
                out.objectBody = 'accepted';
              } catch (error) {
                out.objectBody = error.message;
              }

              const started = Date.now();
              const echoed = await fetch('https://api.test/echo', {
                method: 'POST',
                headers: { 'X-Trace': 'abc', 'Content-Type': 'application/json' },
                body: JSON.stringify({ name: 'Office' }),
              });
              out.elapsed = Date.now() - started;
              out.status = echoed.status;
              await fetch('https://api.test/echo', {
                method: 'PUT',
                body: new TextEncoder().encode('x'.repeat(70000)),
              });

              const bytes = await fetch('https://api.test/bytes');
              out.bytes = Array.from(new Uint8Array(await bytes.arrayBuffer()));
              const buffer = await fetch('https://api.test/buffer');
              out.buffer = Array.from(new Uint8Array(await buffer.arrayBuffer()));

              const controller = new AbortController();
              setTimeout(() => controller.abort(), 20);
              const slowStarted = Date.now();
              try {
                await fetch('https://api.test/slow', { signal: controller.signal });
                out.slow = 'resolved';
              } catch (error) {
                out.slow = error.name;
              }
              out.slowElapsed = Date.now() - slowStarted;
              return JSON.stringify(out);
            } catch (error) {
              return JSON.stringify({ fatal: `${error}\n${error && error.stack}` });
            } })()"#,
        );
        let out: Value = serde_json::from_str(&out).unwrap();
        assert!(out.get("fatal").is_none(), "{out}");
        assert!(
            out["mixed"].as_str().unwrap().contains("choose one"),
            "{out}"
        );
        assert!(
            out["objectBody"].as_str().unwrap().contains("json"),
            "{out}"
        );
        assert_eq!(out["status"], 201);
        assert!(out["elapsed"].as_u64().unwrap() >= 50, "{out}");
        assert_eq!(out["bytes"], serde_json::json!([0, 1, 255]));
        assert_eq!(out["buffer"], serde_json::json!([7, 8]));
        assert_eq!(out["slow"], "AbortError");
        assert!(out["slowElapsed"].as_u64().unwrap() < 4000, "{out}");

        let log = registry::with_registry(|routes| routes.requests(OWNER, APP));
        assert_eq!(log.len(), 5);
        let post = &log[0];
        assert_eq!(post.method, "POST");
        assert_eq!(post.request.body.as_deref(), Some(r#"{"name":"Office"}"#));
        assert!(!post.request.body_truncated);
        assert!(
            post.request
                .headers
                .contains(&("x-trace".to_string(), "abc".to_string())),
            "{:?}",
            post.request.headers
        );
        let put = &log[1];
        assert!(put.request.body_truncated);
        assert_eq!(
            put.request.body.as_ref().map(String::len),
            Some(registry::MAX_REQUEST_BODY_BYTES)
        );
        assert_eq!(log[2].request.body, None);
        clear_run(OWNER);
    }
}
