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
                            let spec = RouteSpec::new(
                                UrlMatcher::glob(&glob).map_err(auto_err)?,
                                None,
                                None,
                                parse_handler(&handler)?,
                            );
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
                    install_sse_interceptor(&ctx, move |_| {
                        Some(LogicTarget {
                            appid: appid.to_string(),
                            allowed: Box::new(|host| host != "blocked.test"),
                        })
                    })?;
                    let begin = JSFunc::new(&ctx, move || {
                        registry::with_registry(|routes| routes.begin_run(run));
                    })?;
                    ctx.global().set("__beginRun", begin)?;
                    let calls = JSFunc::new(&ctx, move |ctx: JSContext| -> JSResult<JSValue> {
                        crate::resolve::json_to_js(&ctx, &dev::run_calls(0, Some(200), &[]))
                    })?;
                    ctx.global().set("__calls", calls)?;
                    let record = JSFunc::new(
                        &ctx,
                        move |ctx: JSContext, command: String| -> JSResult<JSValue> {
                            let value = dev::run_record(run, &command, Some("rec"), &[])
                                .map_err(auto_err)?;
                            crate::resolve::json_to_js(&ctx, &value)
                        },
                    )?;
                    ctx.global().set("__record", record)?;
                    ctx.eval_async::<String>(Source::from_bytes(script)).await
                })
                .await
                .unwrap();
            handle.join().await.unwrap()
        })
    }

    fn install(pattern: &str, method: Option<&str>, times: Option<u32>, action: Value) -> u64 {
        let spec = RouteSpec::new(
            UrlMatcher::glob(pattern).unwrap(),
            method.map(str::to_string),
            times,
            parse_answers(&action, None).unwrap(),
        );
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

    /// Serve an event stream on a loopback port, once.
    fn sse_server(body: &'static str) -> String {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Some(Ok(mut stream)) = listener.incoming().next() {
                let mut request = [0u8; 4096];
                let _ = stream.read(&mut request);
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\
                     content-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        format!("http://{addr}")
    }

    #[test]
    fn sse_routes_stream_to_fetch_and_hold_open_without_drop() {
        const APP: &str = "network-interceptor-sse-fetch";
        const OWNER: &str = "network-interceptor-sse-fetch-run";
        let out = eval_with_interceptor(
            APP,
            OWNER,
            r#"(async () => { try {
              const out = {};
              __route('https://api.test/events', { sse: [
                { event: 'ready', data: { n: 1 }, id: '1' },
                { comment: 'keepalive' },
                { delayMs: 80 },
                { data: 'line 1\nline 2', retry: 50 },
                { drop: true },
              ] });
              const started = Date.now();
              const response = await fetch('https://api.test/events');
              out.status = response.status;
              out.type = response.headers.get('content-type');
              out.text = await response.text();
              out.elapsed = Date.now() - started;

              const openId = __route('https://api.test/live', { sse: [{ data: 'hello' }] });
              let settled = 'pending';
              const live = await fetch('https://api.test/live');
              const reading = live.text().then((text) => { settled = text; });
              await new Promise((resolve) => setTimeout(resolve, 400));
              out.whileOpen = settled;
              __unroute(openId);
              await reading;
              out.afterUnroute = settled;
              return JSON.stringify(out);
            } catch (error) {
              return JSON.stringify({ fatal: `${error}\n${error && error.stack}` });
            } })()"#,
        );
        let out: Value = serde_json::from_str(&out).unwrap();
        assert!(out.get("fatal").is_none(), "{out}");
        assert_eq!(out["status"], 200);
        assert_eq!(out["type"], "text/event-stream");
        assert_eq!(
            out["text"],
            "event: ready\nid: 1\ndata: {\"n\":1}\n\n: keepalive\nretry: 50\ndata: line 1\ndata: line 2\n\n"
        );
        assert!(out["elapsed"].as_u64().unwrap() >= 70, "{out}");
        assert_eq!(out["whileOpen"], "pending");
        assert_eq!(out["afterUnroute"], "data: hello\n\n");
        clear_run(OWNER);
    }

    #[test]
    fn rong_sse_is_routed_and_reconnects_with_last_event_id() {
        const APP: &str = "network-interceptor-rong-sse";
        const OWNER: &str = "network-interceptor-rong-sse-run";
        let real = sse_server("id: r1\ndata: from the server\n\n");
        let script: &'static str = Box::leak(
            format!(
                r#"(async () => {{ try {{
              const out = {{}};
              out.native = typeof Rong.SSE;
              __route('https://api.test/stream', {{ sequence: [
                {{ sse: [
                  {{ event: 'ready', data: 'one', id: 'e1' }},
                  {{ data: {{ two: 2 }}, id: 'e2', retry: 20 }},
                  {{ drop: true }},
                ] }},
                {{ status: 503 }},
              ] }});
              const sse = new Rong.SSE('https://api.test/stream', {{
                headers: {{ Authorization: 'Bearer x' }},
                reconnect: {{ baseDelayMs: 10, maxDelayMs: 50 }},
              }});
              out.instance = sse instanceof Rong.SSE;
              out.url = sse.url;
              const events = [];
              try {{
                for await (const event of sse) events.push(event);
                out.ended = 'done';
              }} catch (error) {{
                out.ended = error.message;
              }}
              out.events = events;

              // An unrouted stream is the native client, unchanged; in a
              // run its call-log entry settles when the stream opens.
              __beginRun();
              const native = new Rong.SSE('{real}/events', {{ reconnect: {{ enabled: false }} }});
              const first = await native.next();
              out.nativeEvent = first.value && first.value.data;
              native.close();
              out.nativeCall = __calls()
                .filter((call) => call.kind === 'sse' && call.url.indexOf('/events') >= 0)
                .map((call) => [call.status, typeof call.durationMs]);

              // A held stream ends when its route goes; no reconnect here.
              const holdId = __route('https://api.test/held', {{ sse: [{{ data: 'hi' }}] }});
              const held = new Rong.SSE('https://api.test/held', {{ reconnect: {{ enabled: false }} }});
              out.heldFirst = (await held.next()).value.data;
              const pending = held.next();
              await new Promise((resolve) => setTimeout(resolve, 300));
              __unroute(holdId);
              out.heldEnd = (await pending).done;
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
        assert_eq!(out["native"], "function");
        assert_eq!(out["instance"], true);
        assert_eq!(out["url"], "https://api.test/stream");
        assert_eq!(
            out["events"],
            serde_json::json!([
                { "type": "ready", "data": "one", "id": "e1", "origin": "https://api.test" },
                { "type": "message", "data": "{\"two\":2}", "id": "e2", "origin": "https://api.test" },
            ])
        );
        assert_eq!(out["ended"], "sse server returned status 503");
        assert_eq!(out["nativeEvent"], "from the server");
        assert_eq!(out["nativeCall"], serde_json::json!([[200, "number"]]));
        assert_eq!(out["heldFirst"], "hi");
        assert_eq!(out["heldEnd"], true);

        let log = registry::with_registry(|routes| routes.requests(OWNER, APP));
        let stream: Vec<_> = log
            .iter()
            .filter(|entry| entry.url == "https://api.test/stream")
            .collect();
        assert_eq!(stream.len(), 2, "{log:?}");
        let header = |entry: &registry::RequestEntry, name: &str| {
            entry
                .request
                .headers
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        };
        assert_eq!(
            header(stream[0], "accept").as_deref(),
            Some("text/event-stream")
        );
        assert_eq!(
            header(stream[0], "authorization").as_deref(),
            Some("Bearer x")
        );
        assert_eq!(header(stream[0], "last-event-id"), None);
        assert_eq!(header(stream[1], "last-event-id").as_deref(), Some("e2"));
        assert_eq!(stream[1].status, Some(503));
        clear_run(OWNER);
    }

    #[test]
    fn records_real_traffic_and_logs_every_call_of_a_run() {
        const APP: &str = "network-interceptor-capture";
        const OWNER: &str = "network-interceptor-capture-run";
        let base = json_server(r#"{"user":"ada","access_token":"secret-token"}"#, 1);
        // A contract capture shares the recording's single read of the body.
        registry::with_registry(|routes| routes.captures.enable(OWNER, APP, 1024, || true))
            .unwrap();
        let script: &'static str = Box::leak(
            format!(
                r#"(async () => {{ try {{
              const out = {{}};
              __beginRun();
              __record('start');
              __route('https://api.test/fake', {{ status: 418, json: {{ teapot: true }} }});
              const real = await fetch('{base}/me?token=abc');
              out.real = await real.json();
              out.realStatus = real.status;
              out.fake = (await fetch('https://api.test/fake')).status;
              try {{ await fetch('http://127.0.0.1:1/closed'); }} catch (error) {{ out.closed = error.name; }}
              out.scenario = __record('stop');
              out.calls = __calls().filter((call) => call.url.indexOf('/closed') >= 0 || call.url.indexOf('/me') >= 0 || call.url.indexOf('/fake') >= 0);
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
        // The app still reads the real body after it was recorded.
        assert_eq!(out["real"]["user"], "ada");
        assert_eq!(out["realStatus"], 200);
        assert_eq!(out["fake"], 418);
        // Tests share the process route table: pick this test's routes.
        let routes: Vec<&Value> = out["scenario"]["rules"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|route| {
                let url = route["http"].as_str().unwrap_or_default();
                url.contains(&base) || url.ends_with("/closed") || url.contains("api.test/fake")
            })
            .collect();
        assert_eq!(routes.len(), 2, "{out}");
        assert_eq!(routes[0]["http"], format!("GET {base}/me?token=*"));
        assert_eq!(routes[0]["json"]["user"], "ada");
        assert_eq!(routes[0]["json"]["access_token"], "***");
        assert_eq!(routes[1]["abort"], "failed");

        let calls = out["calls"].as_array().unwrap();
        assert_eq!(calls.len(), 3, "{out}");
        assert_eq!(calls[0]["url"], format!("{base}/me?token=***"));
        assert_eq!(calls[0]["source"], "network");
        assert_eq!(calls[0]["status"], 200);
        assert!(calls[0]["durationMs"].is_u64());
        assert_eq!(calls[1]["source"], "route");
        assert_eq!(calls[1]["status"], 418);
        assert!(
            calls[2]["error"].as_str().unwrap().contains("TypeError"),
            "{out}"
        );
        // The recording redacts secret fields; the contract capture keeps the
        // body a schema is checked against, and never the query.
        let captured = registry::with_registry(|routes| routes.captures.responses(OWNER, APP, 0));
        let captured: Vec<_> = captured
            .iter()
            .map(|entry| (entry.source.as_str(), entry.url.clone(), entry.status))
            .collect();
        assert_eq!(
            captured,
            vec![
                ("network", format!("{base}/me"), 200),
                ("route", "https://api.test/fake".to_string(), 418),
            ]
        );
        let body = registry::with_registry(|routes| routes.captures.responses(OWNER, APP, 0))[0]
            .body
            .clone()
            .unwrap();
        assert!(body.contains("secret-token"), "{body}");
        clear_run(OWNER);
    }

    #[test]
    fn captures_routed_patched_and_real_responses_without_taking_the_body() {
        const APP: &str = "network-interceptor-contract";
        const OWNER: &str = "network-interceptor-contract-run";
        let base = json_server(r#"{"items":[{"id":"d1"}],"total":1}"#, 3);
        registry::with_registry(|routes| {
            routes.begin_run(OWNER);
            routes.captures.enable(OWNER, APP, 1024, || true)
        })
        .unwrap();
        let script: &'static str = Box::leak(
            format!(
                r#"(async () => {{ try {{
              const out = {{}};
              const real = await fetch('{base}/items?token=secret');
              out.real = await real.json();
              out.realUrl = real.url;
              __route('{base}/patched', {{ continue: true, patchJson: {{ total: 0 }} }});
              out.patched = await (await fetch('{base}/patched')).json();
              __route('https://api.test/v1/devices/*', {{ status: 201, json: {{ id: 7 }} }});
              out.routed = await (await fetch('https://api.test/v1/devices/d1', {{ method: 'post' }})).json();
              __route('https://api.test/page', {{ body: '<p>hi</p>', contentType: 'text/html' }});
              out.page = await (await fetch('https://api.test/page')).text();
              __route('https://api.test/down', {{ abort: 'failed' }});
              try {{ await fetch('https://api.test/down'); }} catch (error) {{ out.down = error.name; }}
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
        // The app still reads every body in full.
        assert_eq!(out["real"]["total"], 1);
        assert!(
            out["realUrl"]
                .as_str()
                .unwrap()
                .ends_with("/items?token=secret"),
            "{out}"
        );
        assert_eq!(
            out["patched"],
            serde_json::json!({ "items": [{ "id": "d1" }], "total": 0 })
        );
        assert_eq!(out["routed"]["id"], 7);
        assert_eq!(out["page"], "<p>hi</p>");
        assert_eq!(out["down"], "TypeError");

        let log = registry::with_registry(|routes| routes.captures.responses(OWNER, APP, 0));
        let summary: Vec<_> = log
            .iter()
            .map(|entry| {
                (
                    entry.method.as_str(),
                    entry.source.as_str(),
                    entry.status,
                    entry.body.is_some(),
                )
            })
            .collect();
        // One entry per response the app received; an abort received none.
        assert_eq!(
            summary,
            vec![
                ("GET", "network", 200, true),
                ("GET", "patch", 200, true),
                ("POST", "route", 201, true),
                ("GET", "route", 200, false),
            ]
        );
        // Never the query: it can carry a credential.
        assert_eq!(log[0].url, format!("{base}/items"));
        assert_eq!(
            log[0].body.as_deref(),
            Some(r#"{"items":[{"id":"d1"}],"total":1}"#)
        );
        assert_eq!(
            log[1].pattern.as_deref(),
            Some(format!("{base}/patched").as_str())
        );
        assert_eq!(
            serde_json::from_str::<Value>(log[1].body.as_deref().unwrap()).unwrap()["total"],
            0
        );
        assert_eq!(log[2].body.as_deref(), Some(r#"{"id":7}"#));
        assert_eq!(
            log[2].pattern.as_deref(),
            Some("https://api.test/v1/devices/*")
        );
        clear_run(OWNER);
        registry::with_registry(|routes| {
            assert!(routes.captures.responses(OWNER, APP, 0).is_empty());
            assert!(!routes.captures.capturing(APP));
        });
    }

    #[test]
    fn unwatched_calls_reach_the_natives_and_wrapping_is_idempotent() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let out = rt.block_on(async {
            let pool = Rong::<RongJS>::builder()
                .shared()
                .workers(1)
                .build()
                .unwrap();
            let worker = pool.worker(0).unwrap();
            let handle = worker
                .spawn(async move |js_runtime, _receiver| -> JSResult<String> {
                    let ctx = js_runtime.context();
                    rong_modules::init(&ctx, ["timer", "url"])?;
                    ctx.eval::<()>(Source::from_bytes(
                        r#"globalThis.__sent = Promise.resolve('native');
                        globalThis.fetch = function () { return globalThis.__sent; };
                        function NativeSSE(url) { this.url = url; this.native = true; }
                        globalThis.Rong = { SSE: NativeSSE };"#,
                    ))?;
                    // No app owns this context: nothing is ever watched.
                    for _ in 0..2 {
                        install_fetch_interceptor(&ctx, |_| None)?;
                        install_sse_interceptor(&ctx, |_| None)?;
                    }
                    ctx.eval::<String>(Source::from_bytes(
                        r#"(function () {
                          const wrapped = Symbol.for('lingxia.automation.network.wrapped');
                          const sse = new Rong.SSE('https://a.test/stream');
                          return JSON.stringify({
                            samePromise: fetch('https://a.test/x') === __sent,
                            fetchWrapped: fetch[wrapped] === true,
                            sseNative: sse.native === true && sse instanceof Rong.SSE,
                            sseWrapped: Rong.SSE[wrapped] === true,
                          });
                        })()"#,
                    ))
                })
                .await
                .unwrap();
            handle.join().await.unwrap()
        });
        let out: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(
            out,
            serde_json::json!({
                "samePromise": true,
                "fetchWrapped": true,
                "sseNative": true,
                "sseWrapped": true,
            })
        );
    }
}

mod scenarios {
    use super::super::capture::{
        self, Call, CallLog, RecordedBody, Recording, Settled, Watch, redact_json, redact_url,
        url_pattern,
    };
    use super::super::registry::{DEV_SESSION_OWNER, Registry, SseStep};
    use super::super::scenario::{iso_utc, parse_scenario, render_action, render_templates};
    use super::super::*;
    use serde_json::Value;
    use serde_json::json;

    fn fulfill_status(action: &RouteAction) -> u16 {
        match action {
            RouteAction::Fulfill(fulfill) => fulfill.status,
            other => panic!("expected fulfill, got {other:?}"),
        }
    }

    fn decide(registry: &mut Registry, method: &str, url: &str) -> Option<RouteAction> {
        registry.decide("app", method, url, SentRequest::default, || true)
    }

    #[test]
    fn a_sequence_answers_in_call_order_and_repeats_its_last_answer() {
        let answers = parse_answers(
            &json!({ "sequence": [{ "status": 500 }, { "status": 502 }, { "json": { "ok": true } }] }),
            None,
        )
        .unwrap();
        let mut registry = Registry::default();
        let spec = RouteSpec::new(UrlMatcher::glob("**/status").unwrap(), None, None, answers);
        registry.install("run", "app", spec, || true).unwrap();
        let statuses: Vec<u16> = (0..5)
            .map(|_| fulfill_status(&decide(&mut registry, "GET", "https://h/status").unwrap()))
            .collect();
        assert_eq!(statuses, vec![500, 502, 200, 200, 200]);

        for bad in [
            json!({ "sequence": [] }),
            json!({ "sequence": { "status": 200 } }),
            json!({ "sequence": [{ "status": 200 }], "status": 200 }),
            json!({ "sequence": [{ "sequence": [{ "status": 200 }] }] }),
            json!({ "sequence": [{ "status": 200 }, { "abort": "nope" }] }),
        ] {
            assert!(
                parse_answers(&bad, None).is_err(),
                "{bad} should be rejected"
            );
        }
        let err = parse_answers(&json!({ "sequence": [{}, { "bogus": 1 }] }), None).unwrap_err();
        assert!(err.starts_with("sequence[1]:"), "{err}");
    }

    fn routes(scenario: &super::super::scenario::Scenario) -> Vec<&RouteSpec> {
        scenario.http.iter().map(|(_, spec)| spec).collect()
    }

    #[test]
    fn a_scenario_file_parses_every_answer_shape() {
        let scenario = parse_scenario(
            &json!({
                "$schema": "./scenario.schema.json",
                "name": "outage",
                "description": "the status API fails, then recovers",
                "rules": [
                    { "http": "get **/v1/status", "times": 3,
                      "sequence": [{ "status": 503 }, { "json": { "up": true } }] },
                    { "http": "* /devices\\/\\w+$/i", "status": 404, "note": "gone" },
                    { "http": "GET **/icon", "bodyBase64": "iVBORw==", "contentType": "image/png" },
                    { "http": "GET **/events", "sse": [
                        { "event": "ready", "data": { "n": 1 }, "id": "1" },
                        { "comment": "keepalive" },
                        { "delayMs": 50 },
                        { "data": "bye" },
                        { "drop": true }
                    ] },
                    { "http": "GET **/slow", "hang": true },
                    { "http": "GET **/real", "continue": true, "patchJson": { "x": null } },
                    { "http": "POST **/down", "abort": "failed" },
                    { "function": "orders.submit", "fault": "unknown" }
                ]
            }),
            None,
        )
        .unwrap();
        assert_eq!(scenario.resolved.name.as_deref(), Some("outage"));
        assert!(scenario.has_function_rules());
        let routes = routes(&scenario);
        assert_eq!(routes.len(), 7);
        assert_eq!(
            scenario
                .http
                .iter()
                .map(|(index, _)| *index)
                .collect::<Vec<_>>(),
            [1, 2, 3, 4, 5, 6, 7]
        );
        let status = routes[0];
        assert_eq!(status.method.as_deref(), Some("GET"));
        assert_eq!(status.times, Some(3));
        assert_eq!(status.answers.len(), 2);
        assert_eq!(routes[1].method, None);
        assert!(routes[1].matcher.is_match("https://h/DEVICES/abc"));
        assert!(!routes[1].matcher.is_match("https://h/devices/abc/x"));
        match &routes[2].answers[0] {
            RouteAction::Fulfill(fulfill) => {
                assert_eq!(
                    fulfill.body,
                    Some(ResponseBody::Binary(vec![0x89, b'P', b'N', b'G']))
                );
            }
            other => panic!("expected a binary fulfillment, got {other:?}"),
        }
        match &routes[3].answers[0] {
            RouteAction::Sse(sse) => {
                assert!(!sse.stays_open());
                assert_eq!(sse.steps.len(), 5);
                assert_eq!(
                    sse.steps[0],
                    SseStep::Event {
                        event: Some("ready".into()),
                        data: r#"{"n":1}"#.into(),
                        id: Some("1".into()),
                        retry: None,
                    }
                );
                assert_eq!(
                    sse.headers,
                    vec![("content-type".to_string(), "text/event-stream".to_string())]
                );
            }
            other => panic!("expected sse, got {other:?}"),
        }
    }

    #[test]
    fn scenario_errors_name_the_rule_and_the_problem() {
        let rules = |rules: Value| json!({ "rules": rules });
        let cases = [
            (json!([]), "JSON object"),
            (json!({ "rules": [] }), "non-empty rules array"),
            (
                json!({ "routes": [] }),
                "'routes' is the old scenario format",
            ),
            (
                rules(json!([{ "status": 200 }])),
                "rules[0]: a rule needs a target",
            ),
            (
                rules(json!([{ "http": "GET **/a" }])),
                "rules[0]: an http rule needs an answer",
            ),
            (
                rules(
                    json!([{ "http": "GET **/a", "status": 200 }, { "http": "GET **/b", "stauts": 200 }]),
                ),
                "rules[1]: unknown route handler option 'stauts'",
            ),
            (
                rules(json!([{ "http": "GET **/{a,b", "status": 200 }])),
                "unclosed",
            ),
            (
                rules(json!([{ "http": "GET /(?=x)/", "status": 200 }])),
                "not supported",
            ),
            (
                rules(json!([{ "http": "GET **", "times": 0, "status": 200 }])),
                "times",
            ),
            (
                rules(json!([{ "http": "GE7 **", "status": 200 }])),
                "method",
            ),
            (
                rules(json!([{ "http": "GET **", "bodyBase64": "***", "status": 200 }])),
                "base64",
            ),
            (
                rules(json!([{ "http": "GET **", "bodyBase64": "AA==", "body": "x" }])),
                "one of body, json, or bodyBase64",
            ),
            (
                rules(
                    json!([{ "http": "POST **", "match": { "json": { "a": "/(/" } }, "status": 200 }]),
                ),
                "rules[0]: match.json.a: /(/ is not a supported regex",
            ),
            (
                json!({ "variants": { "b": { "rules": [{ "http": "GET **", "stauts": 1 }] } } }),
                "variants.b.rules[0]: unknown route handler option 'stauts'",
            ),
        ];
        for (file, expected) in cases {
            let variant = file.get("variants").map(|_| "b");
            let err = parse_scenario(&file, variant).unwrap_err();
            assert!(err.contains(expected), "{file}: {err}");
        }
    }

    #[test]
    fn a_variant_puts_its_rules_before_the_shared_ones() {
        let file = json!({
            "name": "Wi-Fi",
            "rules": [
                { "http": "GET **/wifi/main", "json": { "ssid": "shared" } },
                { "http": "GET **/wifi/clients", "json": [] }
            ],
            "variants": {
                "a": { "rules": [{ "http": "GET **/wifi/main", "json": { "ssid": "A" } }] },
                "b": { "rules": [{ "http": "GET **/wifi/main", "json": { "ssid": "B" } }] }
            }
        });
        let ssid = |variant: Option<&str>| {
            let scenario = parse_scenario(&file, variant).unwrap();
            let mut registry = Registry::default();
            let slot = super::super::dev::installed(&scenario, None);
            registry
                .install_scenario("run", "app", slot, scenario.http, || true)
                .unwrap();
            match decide(&mut registry, "GET", "https://h/wifi/main").unwrap() {
                RouteAction::Fulfill(Fulfill {
                    body: Some(ResponseBody::Text(text)),
                    ..
                }) => serde_json::from_str::<Value>(&text).unwrap()["ssid"].clone(),
                other => panic!("expected json, got {other:?}"),
            }
        };
        assert_eq!(ssid(Some("a")), "A");
        assert_eq!(ssid(Some("b")), "B");
        assert_eq!(ssid(None), "shared");
        let err = parse_scenario(&file, Some("c")).unwrap_err();
        assert!(err.contains("no variant 'c' (variants: a, b)"), "{err}");
    }

    #[test]
    fn the_first_scenario_rule_answers_before_later_ones() {
        let scenario = parse_scenario(
            &json!({ "rules": [
                { "http": "GET **/v1/devices/special", "status": 418 },
                { "http": "GET **/v1/devices/*", "status": 200 }
            ] }),
            None,
        )
        .unwrap();
        let mut registry = Registry::default();
        let slot = super::super::dev::installed(&scenario, None);
        let installed = registry
            .install_scenario("run", "app", slot, scenario.http, || true)
            .unwrap();
        assert_eq!(installed.rules.len(), 2);
        assert!(installed.rules.iter().all(|rule| rule.route_id.is_some()));
        let special = decide(&mut registry, "GET", "https://h/v1/devices/special").unwrap();
        assert_eq!(fulfill_status(&special), 418);
        let other = decide(&mut registry, "GET", "https://h/v1/devices/d1").unwrap();
        assert_eq!(fulfill_status(&other), 200);
        // A route added later still takes precedence over the scenario.
        let spec = RouteSpec::new(
            UrlMatcher::glob("**/v1/devices/special").unwrap(),
            None,
            None,
            vec![parse_handler_value(&json!({ "status": 204 }), None).unwrap()],
        );
        registry.install("run", "app", spec, || true).unwrap();
        let special = decide(&mut registry, "GET", "https://h/v1/devices/special").unwrap();
        assert_eq!(fulfill_status(&special), 204);
        let hits: Vec<u64> = registry
            .scenario(installed.id)
            .unwrap()
            .rules
            .iter()
            .map(|rule| rule.hits)
            .collect();
        assert_eq!(hits, [1, 1]);
        let slot = super::super::dev::installed(
            &parse_scenario(
                &json!({ "rules": [{ "http": "GET x", "status": 200 }] }),
                None,
            )
            .unwrap(),
            None,
        );
        assert!(
            registry
                .install_scenario("run", "app", slot, vec![], || false)
                .is_err()
        );
    }

    fn body_request(body: &str) -> impl FnOnce() -> SentRequest + '_ {
        move || SentRequest::new(vec![], Some(body.to_string()), false)
    }

    #[test]
    fn match_json_picks_the_rule_and_explains_a_miss() {
        let scenario = parse_scenario(
            &json!({
                "name": "Devices",
                "rules": [
                    { "http": "PATCH **/devices/*", "match": { "json": { "name": "Office" } }, "status": 409 },
                    { "http": "PATCH **/devices/*", "match": { "json": { "tags": ["a", "/^b/"] } }, "status": 202 },
                    { "http": "GET **/devices/*", "json": {} }
                ]
            }),
            None,
        )
        .unwrap();
        let mut registry = Registry::default();
        let slot = super::super::dev::installed(&scenario, Some("devices"));
        let installed = registry
            .install_scenario("run", "app", slot, scenario.http, || true)
            .unwrap();
        let mut patch = |body: &str| {
            registry.decide_route(
                "app",
                "PATCH",
                "https://h/devices/1",
                body_request(body),
                || true,
                0,
            )
        };
        let (office, _) = patch(r#"{"name":"Office","extra":1}"#);
        let office = office.unwrap();
        assert_eq!(fulfill_status(&office.action), 409);
        assert_eq!(office.rule, Some((1, "Devices".to_string())));
        let (tagged, _) = patch(r#"{"name":"Den","tags":["a","bee"]}"#);
        assert_eq!(fulfill_status(&tagged.unwrap().action), 202);
        let (missed, no_match) = patch(r#"{"name":"Den","tags":["a"]}"#);
        assert!(missed.is_none());
        let no_match = no_match.unwrap();
        assert_eq!(no_match.owners, ["run"]);
        assert_eq!(
            no_match.message,
            "no rule matched PATCH https://h/devices/1 (2 rules for this target: \
             rule 1 match.json.name: expected \"Office\", got \"Den\"; \
             rule 2 match.json.tags: expected an array of 2, got 1)"
        );
        let (not_json, no_match) = patch("name=Office");
        assert!(not_json.is_none());
        assert!(
            no_match
                .unwrap()
                .message
                .contains("rule 1 match.json: the request body is not JSON")
        );
        // A method no rule targets reaches nobody's scenario.
        let (other, no_match) = registry.decide_route(
            "app",
            "DELETE",
            "https://h/devices/1",
            body_request("{}"),
            || true,
            0,
        );
        assert!(other.is_none() && no_match.is_none());

        let scenario = registry.scenario(installed.id).unwrap();
        assert_eq!(scenario.calls_total, 4);
        let rules: Vec<Option<usize>> = scenario.calls.iter().map(|call| call.rule).collect();
        assert_eq!(rules, [Some(1), Some(2), None, None]);
        assert_eq!(
            scenario.calls[0].body,
            Some(json!({ "name": "Office", "extra": 1 }))
        );
        assert_eq!(scenario.calls[3].body, Some(json!("name=Office")));
        assert!(
            scenario.calls[2]
                .no_match
                .as_deref()
                .unwrap()
                .starts_with("no rule matched")
        );
        let hits: Vec<u64> = scenario.rules.iter().map(|rule| rule.hits).collect();
        assert_eq!(hits, [1, 1, 0]);
    }

    #[test]
    fn a_sequence_and_times_hold_per_rule() {
        let scenario = parse_scenario(
            &json!({ "rules": [
                { "http": "GET **/status", "times": 2, "sequence": [{ "status": 503 }, { "status": 200 }] },
                { "http": "GET **/status", "status": 204 },
                { "http": "GET **/other", "sequence": [{ "status": 500 }, { "status": 201 }] }
            ] }),
            None,
        )
        .unwrap();
        let mut registry = Registry::default();
        let slot = super::super::dev::installed(&scenario, None);
        let installed = registry
            .install_scenario("run", "app", slot, scenario.http, || true)
            .unwrap();
        let statuses: Vec<u16> = (0..4)
            .map(|_| fulfill_status(&decide(&mut registry, "GET", "https://h/status").unwrap()))
            .collect();
        // Rule 1 answers twice, then rule 2 takes over.
        assert_eq!(statuses, [503, 200, 204, 204]);
        let other: Vec<u16> = (0..3)
            .map(|_| fulfill_status(&decide(&mut registry, "GET", "https://h/other").unwrap()))
            .collect();
        assert_eq!(other, [500, 201, 201]);
        let scenario = registry.scenario(installed.id).unwrap();
        let hits: Vec<u64> = scenario.rules.iter().map(|rule| rule.hits).collect();
        assert_eq!(hits, [2, 2, 3]);
        let status = super::super::dev::rules_json(scenario, &registry);
        assert_eq!(status[0]["installed"], false);
        assert_eq!(status[1]["timesLeft"], Value::Null);
    }

    #[test]
    fn a_run_scenario_replaces_the_runs_previous_one() {
        let first = parse_scenario(
            &json!({ "rules": [{ "http": "GET **/a", "status": 201 }] }),
            None,
        )
        .unwrap();
        let second = parse_scenario(
            &json!({ "rules": [{ "http": "GET **/b", "status": 202 }] }),
            None,
        )
        .unwrap();
        let mut registry = Registry::default();
        let slot = super::super::dev::installed(&first, None);
        registry
            .install_scenario("run", "app", slot, first.http, || true)
            .unwrap();
        assert!(decide(&mut registry, "GET", "https://h/a").is_some());
        let slot = super::super::dev::installed(&second, None);
        registry
            .install_scenario("run", "app", slot, second.http, || true)
            .unwrap();
        assert_eq!(decide(&mut registry, "GET", "https://h/a"), None);
        assert!(decide(&mut registry, "GET", "https://h/b").is_some());
        assert_eq!(registry.run_scenarios.len(), 1);
        registry.clear_run("run");
        assert!(registry.run_scenarios.is_empty());
        assert_eq!(decide(&mut registry, "GET", "https://h/b"), None);
    }

    #[test]
    fn templates_render_relative_times_when_served() {
        // 2023-11-14T22:13:20.000Z
        let now = 1_700_000_000_000;
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso_utc(now as i64), "2023-11-14T22:13:20.000Z");
        assert_eq!(iso_utc(951_782_400_123), "2000-02-29T00:00:00.123Z");
        assert_eq!(iso_utc(-1), "1969-12-31T23:59:59.999Z");
        assert_eq!(
            render_templates("{{now}}", now).as_deref(),
            Some("2023-11-14T22:13:20.000Z")
        );
        assert_eq!(
            render_templates("at {{now-2h}} until {{ now+30m }}", now).as_deref(),
            Some("at 2023-11-14T20:13:20.000Z until 2023-11-14T22:43:20.000Z")
        );
        assert_eq!(
            render_templates("{{nowMs}}/{{nowMs+1s}}/{{now-1d}}", now).as_deref(),
            Some("1700000000000/1700000001000/2023-11-13T22:13:20.000Z")
        );
        assert_eq!(
            render_templates("{{now+500ms}}", now).as_deref(),
            Some("2023-11-14T22:13:20.500Z")
        );
        assert_eq!(render_templates("plain", now), None);
        assert_eq!(render_templates("{{name}} {{now+2y}} {{now", now), None);

        let answers = parse_answers(
            &json!({
                "json": { "seen": "{{now-5m}}", "n": 1 },
                "headers": { "x-at": "{{nowMs}}" }
            }),
            None,
        )
        .unwrap();
        let RouteAction::Fulfill(fulfill) = render_action(&answers[0], now) else {
            panic!("expected fulfill");
        };
        // Compared as JSON: key order depends on serde_json's
        // `preserve_order`, which a workspace build may turn on.
        let Some(ResponseBody::Text(body)) = &fulfill.body else {
            panic!("expected a text body, got {:?}", fulfill.body);
        };
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(body).unwrap(),
            json!({ "n": 1, "seen": "2023-11-14T22:08:20.000Z" })
        );
        assert!(
            fulfill
                .headers
                .contains(&("x-at".into(), "1700000000000".into()))
        );

        let sse = parse_answers(
            &json!({ "sse": [{ "data": { "at": "{{now}}" }, "id": "{{nowMs}}" }] }),
            None,
        )
        .unwrap();
        let RouteAction::Sse(sse) = render_action(&sse[0], now) else {
            panic!("expected sse");
        };
        assert_eq!(
            sse.steps[0],
            SseStep::Event {
                event: None,
                data: r#"{"at":"2023-11-14T22:13:20.000Z"}"#.into(),
                id: Some("1700000000000".into()),
                retry: None,
            }
        );
    }

    #[test]
    fn sse_items_frame_as_an_event_stream() {
        let event = SseStep::Event {
            event: Some("update".into()),
            data: "line 1\nline 2".into(),
            id: Some("7".into()),
            retry: Some(250),
        };
        assert_eq!(
            event.frame().as_deref(),
            Some("event: update\nid: 7\nretry: 250\ndata: line 1\ndata: line 2\n\n")
        );
        assert_eq!(
            SseStep::Comment("ping".into()).frame().as_deref(),
            Some(": ping\n")
        );
        assert_eq!(SseStep::Delay(5).frame(), None);
        let open = parse_answers(&json!({ "sse": [{ "data": "x" }] }), None).unwrap();
        let RouteAction::Sse(open) = &open[0] else {
            panic!("expected sse");
        };
        assert!(open.stays_open());

        for bad in [
            json!({ "sse": [{ "drop": true }, { "data": "late" }] }),
            json!({ "sse": [{ "drop": false }] }),
            json!({ "sse": [{ "event": "x" }] }),
            json!({ "sse": [{ "data": "x", "name": "y" }] }),
            json!({ "sse": [{ "event": "a\nb", "data": "x" }] }),
            json!({ "sse": [{ "id": "a\u{0}b", "data": "x" }] }),
            json!({ "sse": [{ "delayMs": MAX_DELAY_MS + 1 }] }),
            json!({ "sse": [{ "comment": "x", "data": "y" }] }),
            json!({ "sse": [{ "data": "x", "retry": -1 }] }),
            json!({ "sse": {} }),
            json!({ "sse": [], "status": 200 }),
            json!({ "sse": [], "abort": "failed" }),
        ] {
            assert!(
                parse_answers(&bad, None).is_err(),
                "{bad} should be rejected"
            );
        }
    }

    #[test]
    fn credentials_never_reach_a_log_or_a_recording() {
        assert_eq!(
            redact_url(
                "https://user:pw@api.test/v1?access_token=abc&page=2&API_KEY=k#frag",
                "***"
            ),
            "https://api.test/v1?access_token=***&page=2&API_KEY=***#frag"
        );
        assert_eq!(
            redact_url("https://api.test/a?x=1", "***"),
            "https://api.test/a?x=1"
        );
        let mut body = json!({
            "user": { "name": "Ada", "password": "hunter2", "idToken": "t" },
            "sessions": [{ "access_token": "a", "refresh-token": "r", "expires": 3600 }],
            "password": null
        });
        redact_json(&mut body);
        assert_eq!(
            body,
            json!({
                "user": { "name": "Ada", "password": "***", "idToken": "***" },
                "sessions": [{ "access_token": "***", "refresh-token": "***", "expires": 3600 }],
                "password": null
            })
        );
        assert_eq!(
            url_pattern("https://api.test/v1/me?token=abc&x=1"),
            "https://api.test/v1/me?token=*&x=1"
        );
        let regex = url_pattern("https://api.test/v1/{id}?token=abc");
        assert!(regex.starts_with("/^") && regex.ends_with("$/"), "{regex}");
        let matcher = super::super::scenario::parse_url_string(&regex).unwrap();
        assert!(matcher.is_match("https://api.test/v1/{id}?token=other"));
        assert!(!matcher.is_match("https://api.test/v1/x?token=other"));

        let mut calls = CallLog::new();
        let id = calls.begin(
            "app",
            "fetch",
            "get",
            "https://api.test/me?token=abc&q=secret-9",
            Watch::default(),
        );
        calls.settle(
            id,
            &Settled {
                status: Some(200),
                ..Settled::default()
            },
        );
        let recent = calls.recent(0, 20, &["secret-9".to_string()]);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0]["url"], "https://api.test/me?token=***&q=***");
        assert_eq!(recent[0]["method"], "GET");
        assert_eq!(recent[0]["source"], "network");
        assert_eq!(recent[0]["status"], 200);
    }

    fn exchange(recording: &mut Recording, method: &str, url: &str, settled: Settled) {
        let mut log = CallLog::new();
        let id = log.begin(
            "app",
            "fetch",
            method,
            url,
            Watch {
                record: true,
                contract: false,
            },
        );
        let call: Call = log.settle(id, &settled).expect("a recorded call");
        recording.push(&call, settled);
    }

    #[test]
    fn secret_fields_match_across_case_and_separators() {
        for name in [
            "token",
            "access_token",
            "accessToken",
            "Refresh-Token",
            "id_token",
            "api_key",
            "apiKey",
            "X-Api-Key",
            "secret",
            "client_secret",
            "password",
            "Authorization",
            "session",
            "session_id",
            "sessionId",
            "cookie",
            "Set-Cookie",
        ] {
            assert!(capture::is_secret_field(name), "{name}");
        }
        for name in ["tokens_left", "user", "sessions", "id", "signature"] {
            assert!(!capture::is_secret_field(name), "{name}");
        }
    }

    #[test]
    fn recorded_bodies_lose_secret_fields_tokens_and_bearers() {
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.c2lnbmF0dXJl";
        let mut recording = Recording::new("run", None, None);
        let json_body = format!(
            r#"{{"token":"t1","apiKey":"k","session_id":"s","nested":[{{"client_secret":"c"}}],"note":"Bearer abc.def","jwt":"{jwt}","user":"ann"}}"#
        );
        exchange(
            &mut recording,
            "GET",
            "https://a.test/json",
            Settled {
                status: Some(200),
                content_type: Some("application/json".into()),
                body: Some(RecordedBody::Text(json_body)),
                ..Settled::default()
            },
        );
        exchange(
            &mut recording,
            "GET",
            "https://a.test/text",
            Settled {
                status: Some(200),
                content_type: Some("text/plain".into()),
                body: Some(RecordedBody::Text(format!(
                    "auth: Bearer sk-live_123 and id {jwt} end"
                ))),
                ..Settled::default()
            },
        );
        let scenario = recording.to_scenario("rec");
        let text = scenario.to_string();
        for leaked in [
            "t1",
            "\"k\"",
            "\"s\"",
            "\"c\"",
            "abc.def",
            "sk-live_123",
            jwt,
        ] {
            assert!(!text.contains(leaked), "{leaked} leaked: {text}");
        }
        let routes = scenario["rules"].as_array().unwrap();
        assert_eq!(routes[0]["json"]["user"], "ann");
        assert_eq!(routes[0]["json"]["note"], "Bearer ***");
        assert_eq!(routes[1]["body"], "auth: Bearer *** and id *** end");
    }

    #[test]
    fn a_recording_becomes_a_scenario_file() {
        let mut recording = Recording::new("run", None, None);
        let json_answer = |status: u16, body: &str| Settled {
            status: Some(status),
            content_type: Some("application/json; charset=utf-8".into()),
            body: Some(RecordedBody::Text(body.into())),
            ..Settled::default()
        };
        exchange(
            &mut recording,
            "POST",
            "https://api.test/auth/token",
            json_answer(
                200,
                r#"{"access_token":"a","refresh_token":"r","expires_in":60}"#,
            ),
        );
        exchange(
            &mut recording,
            "GET",
            "https://api.test/items",
            json_answer(200, r#"{"items":[]}"#),
        );
        exchange(
            &mut recording,
            "GET",
            "https://api.test/items",
            json_answer(200, r#"{"items":[]}"#),
        );
        exchange(
            &mut recording,
            "GET",
            "https://api.test/status",
            json_answer(503, r#"{"up":false}"#),
        );
        exchange(
            &mut recording,
            "GET",
            "https://api.test/status",
            json_answer(200, r#"{"up":true}"#),
        );
        exchange(
            &mut recording,
            "GET",
            "https://api.test/icon.png",
            Settled {
                status: Some(200),
                content_type: Some("image/png".into()),
                body: Some(RecordedBody::Binary(vec![1, 2, 3])),
                ..Settled::default()
            },
        );
        exchange(
            &mut recording,
            "GET",
            "https://api.test/video",
            Settled {
                status: Some(200),
                content_type: Some("video/mp4".into()),
                body: Some(RecordedBody::Binary(vec![
                    0;
                    capture::MAX_BINARY_BODY_BYTES + 1
                ])),
                ..Settled::default()
            },
        );
        exchange(
            &mut recording,
            "GET",
            "https://api.test/offline",
            Settled {
                error: Some("TypeError: fetch failed".into()),
                ..Settled::default()
            },
        );
        let scenario = recording.to_scenario("smoke");
        let routes = scenario["rules"].as_array().unwrap();
        assert_eq!(scenario["name"], "smoke");
        assert_eq!(routes.len(), 6);
        assert_eq!(
            routes[0],
            json!({
                "http": "POST https://api.test/auth/token", "status": 200,
                "contentType": "application/json; charset=utf-8",
                "json": { "access_token": "***", "refresh_token": "***", "expires_in": 60 }
            })
        );
        // Identical answers collapse; differing ones become a sequence.
        assert_eq!(routes[1]["json"], json!({ "items": [] }));
        assert_eq!(routes[2]["sequence"][0]["status"], 503);
        assert_eq!(routes[2]["sequence"][1]["json"], json!({ "up": true }));
        assert_eq!(routes[3]["bodyBase64"], "AQID");
        assert!(routes[4].get("bodyBase64").is_none());
        assert!(
            routes[4]["note"].as_str().unwrap().contains("not recorded"),
            "{}",
            routes[4]
        );
        assert_eq!(routes[5]["abort"], "failed");
        // What it wrote parses back as a scenario.
        parse_scenario(&scenario, None).unwrap();
    }

    fn install_dev(registry: &mut Registry, file: Value, variant: Option<&str>) -> u64 {
        let scenario = parse_scenario(&file, variant).unwrap();
        let slot = super::super::dev::installed(&scenario, Some("wifi"));
        let dev = registry
            .install_scenario(DEV_SESSION_OWNER, "app", slot, scenario.http, || true)
            .unwrap();
        let id = dev.id;
        registry.dev = Some(dev);
        id
    }

    #[test]
    fn dev_scenario_routes_stand_aside_while_a_run_is_active_and_return_after() {
        let mut registry = Registry::default();
        install_dev(
            &mut registry,
            json!({ "rules": [{ "http": "* **", "status": 299 }] }),
            None,
        );
        assert_eq!(
            fulfill_status(&decide(&mut registry, "GET", "https://h/x").unwrap()),
            299
        );
        registry.begin_run("run-1");
        let status = super::super::dev::status_of(&registry);
        assert_eq!(status["suspended"], true);
        assert_eq!(decide(&mut registry, "GET", "https://h/x"), None);
        // The run's own scenario answers meanwhile.
        let run = parse_scenario(
            &json!({ "rules": [{ "http": "GET **", "status": 201 }] }),
            None,
        )
        .unwrap();
        let slot = super::super::dev::installed(&run, None);
        registry
            .install_scenario("run-1", "app", slot, run.http, || true)
            .unwrap();
        assert_eq!(
            fulfill_status(&decide(&mut registry, "GET", "https://h/x").unwrap()),
            201
        );
        registry.clear_run("run-1");
        // The dev scenario is back, untouched.
        let status = super::super::dev::status_of(&registry);
        assert_eq!(status["suspended"], false);
        assert_eq!(status["active"], true);
        assert_eq!(
            fulfill_status(&decide(&mut registry, "GET", "https://h/x").unwrap()),
            299
        );
        registry.clear_run(DEV_SESSION_OWNER);
        assert_eq!(decide(&mut registry, "GET", "https://h/x"), None);
    }

    #[test]
    fn dev_scenario_status_counts_hits_per_rule_and_says_why_it_was_cleared() {
        use super::super::registry::DevClearReason;
        let mut registry = Registry::default();
        let status = super::super::dev::status_of(&registry);
        assert_eq!(status["active"], false);
        assert!(status["lastCleared"].is_null());
        // Nothing installed: nothing to remember.
        assert!(
            registry
                .end_dev_scenario(DevClearReason::Cleared, 0)
                .is_none()
        );
        assert!(registry.dev_cleared.is_none());

        install_dev(
            &mut registry,
            json!({
                "name": "Wi-Fi",
                "rules": [{ "http": "GET **/wifi/main", "status": 299 }, { "function": "orders.submit", "fault": "unknown" }],
                "variants": { "b": { "rules": [{ "http": "POST **/wifi/main", "match": { "json": { "ssid": "B" } }, "status": 201 }] } }
            }),
            Some("b"),
        );
        decide(&mut registry, "GET", "https://h/wifi/main").unwrap();
        decide(&mut registry, "GET", "https://h/wifi/main").unwrap();
        let (none, _) = registry.decide_route(
            "app",
            "POST",
            "https://h/wifi/main",
            body_request(r#"{"ssid":"A"}"#),
            || true,
            0,
        );
        assert!(none.is_none());
        let status = super::super::dev::status_of(&registry);
        assert_eq!(status["active"], true);
        assert_eq!(status["scenario"]["source"], "wifi");
        assert_eq!(status["scenario"]["label"], "Wi-Fi:b");
        assert_eq!(status["scenario"]["variant"], "b");
        let rules = status["scenario"]["rules"].as_array().unwrap();
        assert_eq!(
            rules
                .iter()
                .map(|rule| rule["target"].as_str().unwrap())
                .collect::<Vec<_>>(),
            [
                "POST **/wifi/main",
                "GET **/wifi/main",
                "function orders.submit"
            ]
        );
        assert_eq!(rules[0]["hits"], 0);
        assert_eq!(rules[1]["hits"], 2);
        assert!(rules[2].get("hits").is_none());
        assert_eq!(status["scenario"]["requests"], 3);
        let last = status["scenario"]["lastRequests"].as_array().unwrap();
        assert_eq!(last[2]["rule"], Value::Null);
        assert!(
            last[2]["noMatch"]
                .as_str()
                .unwrap()
                .contains("rule 1 match.json.ssid")
        );

        let ended = registry.end_dev_scenario(DevClearReason::SessionEnded, 86_400_000);
        assert_eq!(ended.unwrap().name.as_deref(), Some("Wi-Fi"));
        assert_eq!(decide(&mut registry, "GET", "https://h/wifi/main"), None);
        let status = super::super::dev::status_of(&registry);
        assert_eq!(status["active"], false);
        assert_eq!(status["lastCleared"]["reason"], "session_ended");
        assert_eq!(status["lastCleared"]["label"], "Wi-Fi:b");
        assert_eq!(
            status["lastCleared"]["clearedAt"],
            "1970-01-02T00:00:00.000Z"
        );
    }

    #[test]
    fn calls_say_which_rule_answered_while_a_dev_scenario_is_active() {
        let mut registry = Registry::default();
        assert_eq!(registry.observe("app", "fetch", "GET", "https://h/a"), None);
        install_dev(
            &mut registry,
            json!({ "name": "Wi-Fi", "rules": [
                { "http": "GET **/a", "status": 201 },
                { "http": "POST **/a", "match": { "json": { "x": 1 } }, "status": 202 }
            ] }),
            None,
        );
        let mut call = |method: &str, body: &str| {
            let (id, _) = registry
                .observe("app", "fetch", method, "https://h/a")
                .unwrap();
            registry.decide_route(
                "app",
                method,
                "https://h/a",
                body_request(body),
                || true,
                id,
            );
        };
        call("GET", "");
        call("POST", r#"{"x":2}"#);
        let calls = registry.calls.recent(0, 20, &[]);
        assert_eq!(calls[0]["answeredBy"], "rule 1 (Wi-Fi)");
        assert_eq!(calls[0]["route"]["rule"], 1);
        assert_eq!(calls[1]["answeredBy"], "real");
        assert!(
            calls[1]["noMatch"]
                .as_str()
                .unwrap()
                .contains("rule 2 match.json.x: expected 1, got 2")
        );
    }

    #[test]
    fn calls_are_observed_only_while_a_run_or_recording_watches() {
        let mut registry = Registry::default();
        assert_eq!(registry.observe("app", "fetch", "GET", "https://h/a"), None);
        registry.begin_run("run");
        let (id, watch) = registry
            .observe("app", "fetch", "GET", "https://h/a")
            .unwrap();
        assert_eq!(watch, Watch::default());
        let spec = RouteSpec::new(
            UrlMatcher::glob("**/a").unwrap(),
            None,
            None,
            vec![parse_handler_value(&json!({ "status": 201 }), None).unwrap()],
        );
        registry.install("run", "app", spec, || true).unwrap();
        registry.decide_route(
            "app",
            "GET",
            "https://h/a",
            SentRequest::default,
            || true,
            id,
        );
        registry.settle(
            id,
            Settled {
                status: Some(201),
                ..Settled::default()
            },
        );
        let calls = registry.calls.recent(0, 20, &[]);
        assert_eq!(calls[0]["source"], "route");
        assert_eq!(calls[0]["route"]["pattern"], "**/a");
        assert_eq!(calls[0]["answeredBy"], "route **/a");
        assert_eq!(calls[0]["status"], 201);
        registry.clear_run("run");
        assert_eq!(registry.observe("app", "fetch", "GET", "https://h/a"), None);

        registry.dev_recording = Some(Recording::new(
            DEV_SESSION_OWNER,
            Some("app".into()),
            Some(UrlMatcher::glob("**/api/**").unwrap()),
        ));
        assert_eq!(
            registry.observe("app", "fetch", "GET", "https://h/other"),
            None
        );
        assert_eq!(
            registry.observe("other", "fetch", "GET", "https://h/api/x"),
            None
        );
        let (id, watch) = registry
            .observe("app", "fetch", "GET", "https://h/api/x")
            .unwrap();
        assert!(watch.record && !watch.contract);
        registry.settle(
            id,
            Settled {
                status: Some(200),
                content_type: Some("text/plain".into()),
                body: Some(RecordedBody::Text("hi".into())),
                ..Settled::default()
            },
        );
        assert_eq!(registry.dev_recording.as_ref().unwrap().exchanges.len(), 1);
        // A request the app cancelled is not an answer.
        let (id, _) = registry
            .observe("app", "fetch", "GET", "https://h/api/y")
            .unwrap();
        registry.settle(
            id,
            Settled {
                error: Some("AbortError: aborted".into()),
                ..Settled::default()
            },
        );
        assert_eq!(registry.dev_recording.as_ref().unwrap().exchanges.len(), 1);
    }
    fn settle_text(registry: &mut Registry, id: u64, body: &str) {
        registry.settle(
            id,
            Settled {
                status: Some(200),
                content_type: Some("text/plain".into()),
                body: Some(RecordedBody::Text(body.into())),
                ..Settled::default()
            },
        );
    }

    #[test]
    fn a_dev_recording_pauses_while_a_run_records_its_own() {
        let mut registry = Registry::default();
        registry.dev_recording = Some(Recording::new(DEV_SESSION_OWNER, None, None));
        let (id, watch) = registry
            .observe("app", "fetch", "GET", "https://h/before")
            .unwrap();
        assert!(watch.record);
        settle_text(&mut registry, id, "dev");

        // A run starts: the dev recording captures nothing of it, with or
        // without a run recording of its own.
        registry.begin_run("run");
        let (id, watch) = registry
            .observe("app", "fetch", "GET", "https://h/unrecorded")
            .unwrap();
        assert!(!watch.record);
        settle_text(&mut registry, id, "test traffic");
        registry.run_recording = Some(Recording::new("run", None, None));
        let (id, watch) = registry
            .observe("app", "fetch", "GET", "https://h/during")
            .unwrap();
        assert!(watch.record);
        settle_text(&mut registry, id, "run");
        let dev = registry.dev_recording.as_ref().unwrap();
        assert_eq!(dev.exchanges.len(), 1);
        assert_eq!(dev.exchanges[0].url, "https://h/before");
        let run = registry.run_recording.as_ref().unwrap();
        assert_eq!(run.exchanges.len(), 1);
        assert_eq!(run.exchanges[0].url, "https://h/during");

        // A call the dev recording saw start still lands there when it
        // settles during a run, never in the run's recording.
        registry.clear_run("run");
        assert!(registry.run_recording.is_none());
        let (late, _) = registry
            .observe("app", "fetch", "GET", "https://h/late")
            .unwrap();
        registry.begin_run("run-2");
        registry.run_recording = Some(Recording::new("run-2", None, None));
        settle_text(&mut registry, late, "late");
        assert_eq!(registry.dev_recording.as_ref().unwrap().exchanges.len(), 2);
        assert!(
            registry
                .run_recording
                .as_ref()
                .unwrap()
                .exchanges
                .is_empty()
        );

        // The run ends: the dev recording resumes.
        registry.clear_run("run-2");
        let (id, watch) = registry
            .observe("app", "fetch", "GET", "https://h/after")
            .unwrap();
        assert!(watch.record);
        settle_text(&mut registry, id, "dev again");
        assert_eq!(registry.dev_recording.as_ref().unwrap().exchanges.len(), 3);
    }

    #[test]
    fn a_run_recording_masks_the_runs_secret_args() {
        let mut recording = Recording::new("run", None, None);
        exchange(
            &mut recording,
            "GET",
            "https://api.test/v1/tenant-s3cr3t/items",
            Settled {
                status: Some(200),
                content_type: Some("text/plain".into()),
                body: Some(RecordedBody::Text("key=s3cr3t-value".into())),
                ..Settled::default()
            },
        );
        let scenario = super::super::dev::run_scenario(
            &recording,
            "rec",
            &["s3cr3t-value".to_string(), "s3cr3t".to_string()],
        );
        let text = scenario.to_string();
        assert!(!text.contains("s3cr3t"), "{text}");
        assert_eq!(scenario["rules"][0]["body"], "key=***");
    }
}
