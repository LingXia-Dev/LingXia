use super::*;
use serde_json::json;

fn handler(value: Value) -> Result<RouteAction, String> {
    parse_handler_value(&value)
}

#[test]
fn handler_json_body_sets_content_type() {
    let RouteAction::Fulfill(fulfill) = handler(json!({
        "status": 501,
        "json": { "error": "unsupported_by_firmware" }
    }))
    .unwrap() else {
        panic!("expected fulfill");
    };
    assert_eq!(fulfill.status, 501);
    assert_eq!(
        fulfill.body.as_deref(),
        Some(r#"{"error":"unsupported_by_firmware"}"#)
    );
    assert_eq!(
        fulfill.headers,
        vec![("content-type".to_string(), "application/json".to_string())]
    );

    // A non-string body is JSON shorthand; an explicit header wins.
    let RouteAction::Fulfill(fulfill) = handler(json!({
        "body": [1, 2],
        "headers": { "Content-Type": "application/problem+json" }
    }))
    .unwrap() else {
        panic!("expected fulfill");
    };
    assert_eq!(fulfill.status, 200);
    assert_eq!(fulfill.body.as_deref(), Some("[1,2]"));
    assert_eq!(fulfill.headers.len(), 1);
}

#[test]
fn handler_string_body_is_verbatim() {
    let RouteAction::Fulfill(fulfill) =
        handler(json!({ "status": 404, "body": "missing", "statusText": "Not Found" })).unwrap()
    else {
        panic!("expected fulfill");
    };
    assert_eq!(fulfill.body.as_deref(), Some("missing"));
    assert_eq!(fulfill.status_text.as_deref(), Some("Not Found"));
    assert!(fulfill.headers.is_empty());
}

#[test]
fn handler_abort_and_continue() {
    assert_eq!(
        handler(json!({ "abort": "failed" })).unwrap(),
        RouteAction::Abort("failed".into())
    );
    assert_eq!(
        handler(json!({ "abort": true })).unwrap(),
        RouteAction::Abort("failed".into())
    );
    assert_eq!(
        handler(json!({ "continue": true })).unwrap(),
        RouteAction::Continue
    );
}

#[test]
fn handler_rejects_ambiguous_or_invalid_options() {
    for bad in [
        json!({ "abort": "failed", "status": 500 }),
        json!({ "abort": "failed", "continue": true }),
        json!({ "continue": true, "body": "x" }),
        json!({ "status": 99 }),
        json!({ "status": 600 }),
        json!({ "status": "500" }),
        json!({ "status": 204, "body": "x" }),
        json!({ "body": "x", "json": {} }),
        json!({ "headers": { "bad header": "x" } }),
        json!({ "headers": { "x": 1 } }),
        json!({ "statusText": "a\nb" }),
        json!({ "abort": "" }),
        json!({ "fulfil": {} }),
        json!("nope"),
    ] {
        assert!(handler(bad.clone()).is_err(), "{bad} should be rejected");
    }
}

#[test]
fn empty_no_content_response_has_null_body() {
    let RouteAction::Fulfill(fulfill) = handler(json!({ "status": 204 })).unwrap() else {
        panic!("expected fulfill");
    };
    assert_eq!(fulfill.body, None);
}

/// Drive the real Logic `fetch` wrapper in a Rong context against the
/// process route table.
mod interceptor {
    use super::super::*;
    use rong::{Rong, RongJS};

    const APPID: &str = "network-interceptor-test";
    const RUN: &str = "network-interceptor-run";

    fn eval_with_interceptor(script: &'static str) -> String {
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
                    install_fetch_interceptor(&ctx, |_| {
                        Some(LogicTarget {
                            appid: APPID.to_string(),
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
            action: parse_handler_value(&action).unwrap(),
        };
        registry::with_registry(|routes| routes.install(RUN, APPID, spec, || true)).unwrap()
    }

    #[test]
    fn routes_fulfill_abort_and_expire_in_logic_fetch() {
        install(
            "https://api.test/v1/devices/*",
            Some("PATCH"),
            None,
            serde_json::json!({ "status": 501, "json": { "error": "unsupported_by_firmware" } }),
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
        assert_eq!(out["body"]["error"], "unsupported_by_firmware");
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
}
