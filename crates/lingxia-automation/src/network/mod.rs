//! `NetworkDriver` — test-only interception of an lxapp's Logic `fetch` and
//! `Rong.SSE`.
//!
//! Routes live in a process table keyed by run and appid. Only a host
//! automation run can install them, and the run's finalization clears them,
//! so no route outlives the run that installed it; the one other owner is a
//! dev session's scenario (`dev`), which stands aside while a run is active.
//! Logic contexts get a thin `fetch` wrapper, and `Rong.SSE` a wrapper, whose
//! fast path is one atomic load while no route, run, or recording exists.
//! While a run is active the wrappers also log every call (`capture`) for
//! failure reports; a recording captures real responses as a scenario, and
//! a contract capture (`captureResponses()`) keeps what the app received
//! for `lxdev test --openapi`. All three share one observation per call and
//! one read of a response body.

mod capture;
pub(crate) mod dev;
mod registry;
mod scenario;

use crate::auto_err;
use crate::resolve::{json_to_js, upgrade_authorized};
use lxapp::LxApp;
use registry::{
    AbortKind, Fulfill, MAX_DELAY_MS, ResponseBody, RouteAction, RouteSpec, SentRequest, SseAnswer,
    SseStep, UrlMatcher,
};
use rong::{
    AnyJSTypedArray, Class, HostError, JSArrayBuffer, JSContext, JSFunc, JSObject, JSResult,
    JSValue, Source, function::Optional, js_class, js_method,
};
use serde_json::Value;
use std::rc::Rc;
use std::sync::{Arc, Weak};

/// Marks a host automation context with the run that owns its routes.
#[derive(Clone)]
pub(crate) struct NetworkRunScope {
    run_id: String,
    active: Arc<dyn Fn() -> bool + Send + Sync>,
}

/// Bind a host automation context to its run. Routes installed from this
/// context belong to `run_id` and are refused once `active` turns false.
pub(crate) fn attach_run_scope(
    ctx: &JSContext,
    run_id: String,
    active: impl Fn() -> bool + Send + Sync + 'static,
) {
    registry::with_registry(|routes| {
        routes.begin_run(&run_id);
        if let Some(dev) = &routes.dev {
            dev::warn(
                &dev.appid,
                format!(
                    "dev scenario {} stands aside while automation run {run_id} runs",
                    dev::label(dev)
                ),
            );
        }
    });
    ctx.set_state(NetworkRunScope {
        run_id,
        active: Arc::new(active),
    });
}

/// Remove every route and request record a run owns. Called on every
/// terminal transition of the run.
pub(crate) fn clear_run(run_id: &str) {
    registry::with_registry(|routes| routes.clear_run(run_id));
}

/// Give a host run's `__LINGXIA_AUTOMATION_HOST__` the network plumbing the
/// test framework uses: `networkLog(sinceMs, limit?)` (the Logic `fetch`
/// calls a failed spec reports) and `networkRecord('start' | 'stop', name?)`
/// (`lxdev test --record-network`). `secrets` are masked in both.
pub(crate) fn attach_host_functions(
    ctx: &JSContext,
    host: &JSObject,
    run_id: &str,
    secrets: Vec<String>,
) -> JSResult<()> {
    let log_secrets = secrets.clone();
    host.set(
        "networkLog",
        JSFunc::new(
            ctx,
            move |ctx: JSContext, since: f64, limit: Option<f64>| -> JSResult<JSValue> {
                let since = if since.is_finite() && since > 0.0 {
                    since as u64
                } else {
                    0
                };
                let limit = limit
                    .filter(|limit| limit.is_finite() && *limit >= 0.0)
                    .map(|limit| limit as usize);
                json_to_js(&ctx, &dev::run_calls(since, limit, &log_secrets))
            },
        )?,
    )?;
    let run_id = run_id.to_string();
    host.set(
        "networkRecord",
        JSFunc::new(
            ctx,
            move |ctx: JSContext, command: String, name: Option<String>| -> JSResult<JSValue> {
                let value = dev::run_record(&run_id, &command, name.as_deref(), &secrets)
                    .map_err(auto_err)?;
                json_to_js(&ctx, &value)
            },
        )?,
    )?;
    Ok(())
}

fn run_scope(ctx: &JSContext) -> JSResult<NetworkRunScope> {
    ctx.get_state::<NetworkRunScope>().cloned().ok_or_else(|| {
        auto_err("network routes are available only inside a host automation run (lxdev test)")
    })
}

fn fetch_failed(kind: AbortKind) -> rong::RongJSError {
    // Same shape as a transport failure from Rong's `fetch`.
    let detail = format!("aborted by test route: {}", kind.as_str());
    HostError::new(rong::error::E_IO, "fetch failed")
        .with_name("TypeError")
        .with_data(rong::err_data!({ detail: (detail) }))
        .into()
}

// ----------------------------- driver side -----------------------------

#[js_class(clone)]
pub(crate) struct JSNetworkDriver {
    lxapp: Weak<LxApp>,
}

impl JSNetworkDriver {
    /// Authorization is checked per call, so reading `.network` never throws.
    pub(crate) fn new(lxapp: Weak<LxApp>) -> Self {
        Self { lxapp }
    }
}

#[js_class(rename = "NetworkDriver")]
impl JSNetworkDriver {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(
            rong::error::E_ILLEGAL_CONSTRUCTOR,
            "Use lx.automation().lxapp().network",
        )
        .into())
    }

    /// Install a route. The newest matching route handles a request.
    #[js_method]
    async fn route(
        &self,
        ctx: JSContext,
        pattern: JSValue,
        handler: JSObject,
    ) -> JSResult<JSObject> {
        let app = upgrade_authorized(&ctx, &self.lxapp)?;
        let scope = run_scope(&ctx)?;
        let (matcher, method, times) = parse_pattern(pattern)?;
        let answers = parse_handler(&handler)?;
        let label = matcher.label();
        let spec = RouteSpec::new(matcher, method, times, answers);
        let appid = app.appid.clone();
        let id = registry::with_registry(|routes| {
            routes.install(&scope.run_id, &appid, spec, || (scope.active)())
        })
        .map_err(auto_err)?;
        Ok(
            Class::lookup::<JSNetworkRoute>(&ctx)?.instance(JSNetworkRoute {
                id,
                run_id: scope.run_id,
                appid,
                pattern: label,
            }),
        )
    }

    /// Install every route of a scenario at once. The first matching entry
    /// of the scenario answers; routes added later still take precedence.
    #[js_method]
    async fn scenario(&self, ctx: JSContext, scenario: JSObject) -> JSResult<JSObject> {
        let app = upgrade_authorized(&ctx, &self.lxapp)?;
        let scope = run_scope(&ctx)?;
        let json = scenario
            .to_json_string()
            .map_err(|err| auto_err(format!("scenario must be JSON-compatible: {err}")))?;
        let value: Value = serde_json::from_str(&json)
            .map_err(|err| auto_err(format!("scenario must be JSON-compatible: {err}")))?;
        let parsed =
            scenario::parse_scenario(&value).map_err(|err| auto_err(format!("scenario: {err}")))?;
        let appid = app.appid.clone();
        let routes = registry::with_registry(|routes| {
            routes.install_all(&scope.run_id, &appid, parsed.routes, || (scope.active)())
        })
        .map_err(auto_err)?;
        Ok(
            Class::lookup::<JSNetworkScenario>(&ctx)?.instance(JSNetworkScenario {
                name: parsed.name,
                run_id: scope.run_id,
                appid,
                routes,
            }),
        )
    }

    /// Remove every route this run installed for the app.
    #[js_method(rename = "unrouteAll")]
    async fn unroute_all(&self, ctx: JSContext) -> JSResult<u32> {
        let app = upgrade_authorized(&ctx, &self.lxapp)?;
        let scope = run_scope(&ctx)?;
        let removed =
            registry::with_registry(|routes| routes.remove_app(&scope.run_id, &app.appid));
        Ok(removed as u32)
    }

    /// Requests this run's routes matched for the app, oldest first. The log
    /// spans the whole run, across specs.
    #[js_method]
    async fn requests(&self, ctx: JSContext) -> JSResult<JSValue> {
        let app = upgrade_authorized(&ctx, &self.lxapp)?;
        let scope = run_scope(&ctx)?;
        requests_js(&ctx, &scope.run_id, &app.appid, None)
    }

    /// Record the app's Logic `fetch` responses (status, content type, JSON
    /// body) until the run ends. `{ maxBodyBytes }` bounds each body.
    #[js_method(rename = "captureResponses")]
    async fn capture_responses(&self, ctx: JSContext, options: Optional<JSValue>) -> JSResult<()> {
        let app = upgrade_authorized(&ctx, &self.lxapp)?;
        let scope = run_scope(&ctx)?;
        let limit = match options.0.and_then(JSValue::into_object) {
            None => capture::DEFAULT_CAPTURE_BODY_BYTES,
            Some(object) => match object.get_opt::<_, f64>("maxBodyBytes")? {
                None => capture::DEFAULT_CAPTURE_BODY_BYTES,
                Some(bytes) if bytes >= 1.0 && bytes.fract() == 0.0 => bytes as usize,
                Some(bytes) => {
                    return Err(auto_err(format!(
                        "captureResponses maxBodyBytes must be a positive integer, got {bytes}"
                    )));
                }
            },
        };
        registry::with_registry(|routes| {
            routes
                .captures
                .enable(&scope.run_id, &app.appid, limit, || (scope.active)())
        })
        .map_err(auto_err)
    }

    /// Captured responses of the app in this run, oldest first; `{ since }`
    /// skips entries up to that `seq`.
    #[js_method]
    async fn responses(&self, ctx: JSContext, options: Optional<JSValue>) -> JSResult<JSValue> {
        let app = upgrade_authorized(&ctx, &self.lxapp)?;
        let scope = run_scope(&ctx)?;
        let since = match options.0.and_then(JSValue::into_object) {
            None => 0,
            Some(object) => object
                .get_opt::<_, f64>("since")?
                .filter(|since| *since > 0.0)
                .map_or(0, |since| since as u64),
        };
        let entries = registry::with_registry(|routes| {
            routes.captures.responses(&scope.run_id, &app.appid, since)
        });
        let list: Vec<Value> = entries
            .into_iter()
            .map(|entry| {
                serde_json::json!({
                    "seq": entry.seq,
                    "method": entry.method,
                    "url": entry.url,
                    "source": entry.source.as_str(),
                    "pattern": entry.pattern,
                    "status": entry.status,
                    "contentType": entry.content_type,
                    "body": entry.body,
                    "bodyTruncated": entry.body_truncated,
                    "timestamp": entry.timestamp_ms,
                })
            })
            .collect();
        json_to_js(&ctx, &Value::Array(list))
    }
}

/// Handle returned by `route()`.
#[js_class(clone)]
pub(crate) struct JSNetworkRoute {
    id: u64,
    run_id: String,
    appid: String,
    pattern: String,
}

impl JSNetworkRoute {
    fn owned_scope(&self, ctx: &JSContext) -> JSResult<()> {
        let scope = run_scope(ctx)?;
        if scope.run_id == self.run_id {
            Ok(())
        } else {
            Err(auto_err("this route belongs to another automation run"))
        }
    }
}

#[js_class(rename = "NetworkRoute")]
impl JSNetworkRoute {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(
            rong::error::E_ILLEGAL_CONSTRUCTOR,
            "Use lx.automation().lxapp().network.route()",
        )
        .into())
    }

    #[js_method(getter, enumerable)]
    fn id(&self) -> f64 {
        self.id as f64
    }

    #[js_method(getter, enumerable)]
    fn pattern(&self) -> String {
        self.pattern.clone()
    }

    /// Remove this route. Resolves `false` when it already expired
    /// (`times`) or was removed.
    #[js_method]
    async fn unroute(&self, ctx: JSContext) -> JSResult<bool> {
        self.owned_scope(&ctx)?;
        Ok(registry::with_registry(|routes| {
            routes.remove(&self.run_id, self.id)
        }))
    }

    /// Requests this route handled, oldest first.
    #[js_method]
    async fn requests(&self, ctx: JSContext) -> JSResult<JSValue> {
        self.owned_scope(&ctx)?;
        requests_js(&ctx, &self.run_id, &self.appid, Some(self.id))
    }
}

/// Handle returned by `scenario()`: its routes, removable together.
#[js_class(clone)]
pub(crate) struct JSNetworkScenario {
    name: Option<String>,
    run_id: String,
    appid: String,
    /// `(route id, pattern)` in file order.
    routes: Vec<(u64, String)>,
}

#[js_class(rename = "NetworkScenario")]
impl JSNetworkScenario {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(
            rong::error::E_ILLEGAL_CONSTRUCTOR,
            "Use lx.automation().lxapp().network.scenario()",
        )
        .into())
    }

    #[js_method(getter, enumerable)]
    fn name(&self) -> Option<String> {
        self.name.clone()
    }

    /// One route handle per scenario entry, in file order.
    #[js_method(getter, enumerable)]
    fn routes(&self, ctx: JSContext) -> JSResult<rong::JSArray> {
        let array = rong::JSArray::new(&ctx)?;
        for (id, pattern) in &self.routes {
            array.push(
                Class::lookup::<JSNetworkRoute>(&ctx)?.instance(JSNetworkRoute {
                    id: *id,
                    run_id: self.run_id.clone(),
                    appid: self.appid.clone(),
                    pattern: pattern.clone(),
                }),
            )?;
        }
        Ok(array)
    }

    /// Remove every route of the scenario; resolves how many were still
    /// installed.
    #[js_method]
    async fn unroute(&self, ctx: JSContext) -> JSResult<u32> {
        owned_run(&ctx, &self.run_id)?;
        Ok(registry::with_registry(|routes| {
            self.routes
                .iter()
                .filter(|(id, _)| routes.remove(&self.run_id, *id))
                .count() as u32
        }))
    }

    /// Requests any route of the scenario handled, oldest first.
    #[js_method]
    async fn requests(&self, ctx: JSContext) -> JSResult<JSValue> {
        owned_run(&ctx, &self.run_id)?;
        let ids: Vec<u64> = self.routes.iter().map(|(id, _)| *id).collect();
        requests_js_filtered(&ctx, &self.run_id, &self.appid, &|id| ids.contains(&id))
    }
}

fn owned_run(ctx: &JSContext, run_id: &str) -> JSResult<()> {
    let scope = run_scope(ctx)?;
    if scope.run_id == run_id {
        Ok(())
    } else {
        Err(auto_err("this route belongs to another automation run"))
    }
}

fn requests_js(
    ctx: &JSContext,
    run_id: &str,
    appid: &str,
    route: Option<u64>,
) -> JSResult<JSValue> {
    requests_js_filtered(ctx, run_id, appid, &|id| {
        route.is_none_or(|route| route == id)
    })
}

fn requests_js_filtered(
    ctx: &JSContext,
    run_id: &str,
    appid: &str,
    keep: &dyn Fn(u64) -> bool,
) -> JSResult<JSValue> {
    let entries = registry::with_registry(|routes| routes.requests(run_id, appid));
    let list: Vec<Value> = entries
        .into_iter()
        .filter(|entry| keep(entry.route_id))
        .map(|entry| {
            serde_json::json!({
                "routeId": entry.route_id,
                "pattern": entry.pattern,
                "method": entry.method,
                "url": entry.url,
                "action": entry.action,
                "status": entry.status,
                "headers": entry
                    .request
                    .headers
                    .into_iter()
                    .map(|(name, value)| (name, Value::String(value)))
                    .collect::<serde_json::Map<_, _>>(),
                "body": entry.request.body,
                "bodyTruncated": entry.request.body_truncated,
                "timestamp": entry.timestamp_ms,
            })
        })
        .collect();
    json_to_js(ctx, &Value::Array(list))
}

type ParsedPattern = (UrlMatcher, Option<String>, Option<u32>);

/// `pattern` is a URL glob string, a `RegExp`, or `{ url, method?, times? }`.
fn parse_pattern(pattern: JSValue) -> JSResult<ParsedPattern> {
    if pattern.is_string() {
        let glob: String = pattern.to_rust()?;
        return Ok((UrlMatcher::glob(&glob).map_err(auto_err)?, None, None));
    }
    let object = pattern.into_object().ok_or_else(|| {
        auto_err("route pattern must be a URL glob, a RegExp, or { url, method?, times? }")
    })?;
    if !object.has_property("url")? {
        return Ok((parse_url_matcher(object.into_js_value())?, None, None));
    }
    let matcher = parse_url_matcher(object.get::<_, JSValue>("url")?)?;
    let method = match object.get_opt::<_, String>("method")? {
        Some(method) => scenario::parse_method(&method).map_err(auto_err)?,
        None => None,
    };
    let times = match object.get_opt::<_, f64>("times")? {
        None => None,
        Some(times) if times >= 1.0 && times.fract() == 0.0 && times <= u32::MAX as f64 => {
            Some(times as u32)
        }
        Some(times) => {
            return Err(auto_err(format!(
                "route times must be a positive integer, got {times}"
            )));
        }
    };
    Ok((matcher, method, times))
}

fn parse_url_matcher(value: JSValue) -> JSResult<UrlMatcher> {
    if value.is_string() {
        let glob: String = value.to_rust()?;
        return UrlMatcher::glob(&glob).map_err(auto_err);
    }
    let Some(object) = value.into_object() else {
        return Err(auto_err("route url must be a glob string or a RegExp"));
    };
    match (
        object.get_opt::<_, String>("source")?,
        object.get_opt::<_, String>("flags")?,
    ) {
        (Some(source), Some(flags)) => UrlMatcher::regex(&source, &flags).map_err(auto_err),
        _ => Err(auto_err("route url must be a glob string or a RegExp")),
    }
}

/// `{ status?, statusText?, headers?, body? | json?, contentType?, delay? }`
/// fulfills; `{ abort: 'failed' }` rejects like a network error;
/// `{ continue: true, patchJson? }` passes the request through, optionally
/// merge-patching the real JSON response; `{ hang: true }` never answers;
/// `{ sse: [...] }` streams events. The five are exclusive.
/// `{ sequence: [...] }` lists such answers, served in call order.
fn parse_handler(handler: &JSObject) -> JSResult<Vec<RouteAction>> {
    let binary = binary_body(handler)?;
    let json = handler
        .to_json_string()
        .map_err(|err| auto_err(format!("route handler must be JSON-compatible: {err}")))?;
    let value: Value = serde_json::from_str(&json)
        .map_err(|err| auto_err(format!("route handler must be JSON-compatible: {err}")))?;
    parse_answers(&value, binary).map_err(auto_err)
}

/// A single answer or a `sequence`. A binary top-level `body` belongs to a
/// single answer; sequence items take text and JSON bodies.
fn parse_answers(value: &Value, binary: Option<Vec<u8>>) -> Result<Vec<RouteAction>, String> {
    scenario::parse_answers_with(value, &|item| {
        let bytes = if std::ptr::eq(item, value) {
            binary.clone()
        } else {
            None
        };
        parse_handler_value(item, bytes)
    })
}

/// Bytes of an `ArrayBuffer` or typed-array `body`, which JSON would mangle.
fn binary_body(handler: &JSObject) -> JSResult<Option<Vec<u8>>> {
    if !handler.has_property("body")? {
        return Ok(None);
    }
    let body = handler.get::<_, JSValue>("body")?;
    if body.is_array_buffer() {
        return Ok(Some(body.to_rust::<JSArrayBuffer>()?.as_bytes().to_vec()));
    }
    if let Some(object) = body.into_object()
        && let Some(view) = AnyJSTypedArray::from_object(object)
    {
        return view
            .as_bytes()
            .map(|bytes| Some(bytes.to_vec()))
            .ok_or_else(|| auto_err("route body buffer is detached"));
    }
    Ok(None)
}

const FULFILL_KEYS: [&str; 7] = [
    "status",
    "statusText",
    "headers",
    "body",
    "json",
    "contentType",
    "delay",
];

fn parse_handler_value(value: &Value, binary: Option<Vec<u8>>) -> Result<RouteAction, String> {
    let Value::Object(fields) = value else {
        return Err("route handler must be an object".into());
    };
    if let Some(unknown) = fields.keys().find(|key| {
        !FULFILL_KEYS.contains(&key.as_str())
            && !["abort", "continue", "hang", "patchJson", "sse"].contains(&key.as_str())
    }) {
        return Err(format!("unknown route handler option '{unknown}'"));
    }
    if let Some(items) = fields.get("sse") {
        return parse_sse(fields, items).map(RouteAction::Sse);
    }
    let fulfill_key = FULFILL_KEYS.iter().find(|key| fields.contains_key(**key));
    let patch = fields.get("patchJson");
    match fields.get("hang") {
        None => {}
        Some(Value::Bool(true)) => {
            if let Some(other) = fulfill_key.copied().or_else(|| {
                ["abort", "continue", "patchJson"]
                    .into_iter()
                    .find(|key| fields.contains_key(*key))
            }) {
                return Err(format!(
                    "route handler cannot combine hang with '{other}'; \
                     choose one of fulfill, abort, continue, or hang"
                ));
            }
            return Ok(RouteAction::Hang { token: 0 });
        }
        Some(other) => return Err(format!("route hang must be true, got {other}")),
    }
    if patch.is_some() && !fields.contains_key("continue") {
        return Err(
            "route patchJson patches the real response; pass it with continue: true".into(),
        );
    }
    let abort = match fields.get("abort") {
        None => None,
        Some(Value::String(kind)) => Some(
            AbortKind::parse(kind)
                .ok_or_else(|| format!("route abort must be 'failed', got '{kind}'"))?,
        ),
        Some(other) => return Err(format!("route abort must be 'failed', got {other}")),
    };
    let pass = match fields.get("continue") {
        None => false,
        Some(Value::Bool(true)) => true,
        Some(other) => return Err(format!("route continue must be true, got {other}")),
    };
    let exclusive = |chosen: &str| match fulfill_key {
        Some(key) => Err(format!(
            "route handler cannot combine {chosen} with the fulfill option '{key}'; \
             choose one of fulfill, abort, continue, or hang"
        )),
        None => Ok(()),
    };
    match (abort, pass) {
        (Some(_), true) => {
            Err("route handler cannot combine abort with continue; choose one of fulfill, abort, continue, or hang".into())
        }
        (Some(kind), false) => exclusive("abort").map(|()| RouteAction::Abort(kind)),
        (None, true) => exclusive("continue").and_then(|()| match patch {
            None => Ok(RouteAction::Continue),
            Some(patch) if patch.to_string().len() > registry::MAX_BODY_BYTES => Err(format!(
                "route patchJson exceeds the {}-byte limit",
                registry::MAX_BODY_BYTES
            )),
            Some(patch) => Ok(RouteAction::Patch(patch.clone())),
        }),
        (None, false) => parse_fulfill(fields, binary).map(RouteAction::Fulfill),
    }
}

fn parse_fulfill(
    fields: &serde_json::Map<String, Value>,
    binary: Option<Vec<u8>>,
) -> Result<Fulfill, String> {
    let status = match fields.get("status") {
        None => 200,
        Some(Value::Number(n)) => n
            .as_u64()
            .filter(|s| (200..=599).contains(s))
            .map(|s| s as u16)
            .ok_or_else(|| format!("route status must be an integer in 200..=599, got {n}"))?,
        Some(other) => return Err(format!("route status must be a number, got {other}")),
    };
    let status_text = match fields.get("statusText") {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) if !text.contains(['\r', '\n']) => Some(text.clone()),
        Some(_) => return Err("route statusText must be a single-line string".into()),
    };
    let mut headers = parse_headers(fields)?;
    let (body, implied_type) = match (fields.get("json"), fields.get("body"), binary) {
        (Some(_), Some(_), _) => {
            return Err("route handler takes either body or json, not both".into());
        }
        (Some(json), None, _) => (
            Some(ResponseBody::Text(json.to_string())),
            Some("application/json"),
        ),
        (None, Some(_), Some(bytes)) => (Some(ResponseBody::Binary(bytes)), None),
        (None, None, _) | (None, Some(Value::Null), None) => (None, None),
        (None, Some(Value::String(text)), None) => (Some(ResponseBody::Text(text.clone())), None),
        (None, Some(_), None) => {
            return Err(
                "route body must be a string, ArrayBuffer, or Uint8Array; pass JSON values as json"
                    .into(),
            );
        }
    };
    let content_type = match fields.get("contentType") {
        None | Some(Value::Null) => implied_type.map(str::to_string),
        Some(Value::String(value)) => Some(value.clone()),
        Some(_) => return Err("route contentType must be a string".into()),
    };
    if let Some(content_type) = content_type
        && !headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("content-type"))
    {
        http::HeaderValue::try_from(content_type.as_str())
            .map_err(|_| "invalid route contentType".to_string())?;
        headers.push(("content-type".into(), content_type));
    }
    if let Some(body) = &body {
        if body.len() > registry::MAX_BODY_BYTES {
            return Err(format!(
                "route body exceeds the {}-byte limit",
                registry::MAX_BODY_BYTES
            ));
        }
        if !body.is_empty() && matches!(status, 204 | 205 | 304) {
            return Err(format!("a {status} response cannot have a body"));
        }
    }
    let delay_ms = parse_delay(fields.get("delay"), "route delay")?;
    Ok(Fulfill {
        status,
        status_text,
        headers,
        body: body.filter(|body| !body.is_empty() || !matches!(status, 204 | 205 | 304)),
        delay_ms,
    })
}

fn parse_headers(fields: &serde_json::Map<String, Value>) -> Result<Vec<(String, String)>, String> {
    let mut headers = Vec::new();
    match fields.get("headers") {
        None | Some(Value::Null) => {}
        Some(Value::Object(map)) => {
            for (name, value) in map {
                let Value::String(value) = value else {
                    return Err(format!("route header '{name}' must be a string"));
                };
                http::HeaderName::try_from(name.as_str())
                    .map_err(|_| format!("invalid route header name '{name}'"))?;
                http::HeaderValue::try_from(value.as_str())
                    .map_err(|_| format!("invalid value for route header '{name}'"))?;
                headers.push((name.clone(), value.clone()));
            }
        }
        Some(_) => return Err("route headers must be an object of strings".into()),
    }
    Ok(headers)
}

fn parse_delay(value: Option<&Value>, what: &str) -> Result<u32, String> {
    match value {
        None | Some(Value::Null) => Ok(0),
        Some(Value::Number(n)) => n
            .as_u64()
            .filter(|delay| *delay <= u64::from(MAX_DELAY_MS))
            .map(|delay| delay as u32)
            .ok_or_else(|| format!("{what} must be an integer in 0..={MAX_DELAY_MS} ms, got {n}")),
        Some(other) => Err(format!("{what} must be a number, got {other}")),
    }
}

/// Items one SSE answer may list.
const MAX_SSE_ITEMS: usize = 1_000;

/// `{ sse: [...], headers?, delay? }`: a `text/event-stream` answer.
fn parse_sse(fields: &serde_json::Map<String, Value>, items: &Value) -> Result<SseAnswer, String> {
    if let Some(other) = fields
        .keys()
        .find(|key| !["sse", "headers", "delay"].contains(&key.as_str()))
    {
        return Err(format!(
            "route handler cannot combine sse with '{other}'; an sse answer takes only headers and delay"
        ));
    }
    let Value::Array(items) = items else {
        return Err("route sse must be an array of events".into());
    };
    if items.len() > MAX_SSE_ITEMS {
        return Err(format!(
            "route sse may list at most {MAX_SSE_ITEMS} items, got {}",
            items.len()
        ));
    }
    let steps = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            parse_sse_item(item, index + 1 == items.len())
                .map_err(|err| format!("sse[{index}]: {err}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut headers = parse_headers(fields)?;
    if !headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("content-type"))
    {
        headers.push(("content-type".into(), "text/event-stream".into()));
    }
    let size: usize = steps
        .iter()
        .filter_map(SseStep::frame)
        .map(|frame| frame.len())
        .sum();
    if size > registry::MAX_BODY_BYTES {
        return Err(format!(
            "route sse exceeds the {}-byte limit",
            registry::MAX_BODY_BYTES
        ));
    }
    Ok(SseAnswer {
        headers,
        steps,
        delay_ms: parse_delay(fields.get("delay"), "route delay")?,
        hold: 0,
    })
}

fn parse_sse_item(value: &Value, last: bool) -> Result<SseStep, String> {
    let Value::Object(fields) = value else {
        return Err("an sse item must be an object".into());
    };
    let only = |key: &str| -> Result<(), String> {
        match fields.keys().find(|other| other.as_str() != key) {
            Some(other) => Err(format!("'{key}' cannot be combined with '{other}'")),
            None => Ok(()),
        }
    };
    if let Some(comment) = fields.get("comment") {
        only("comment")?;
        let Value::String(comment) = comment else {
            return Err("comment must be a string".into());
        };
        return Ok(SseStep::Comment(comment.clone()));
    }
    if let Some(delay) = fields.get("delayMs") {
        only("delayMs")?;
        return parse_delay(Some(delay), "delayMs").map(SseStep::Delay);
    }
    if let Some(drop) = fields.get("drop") {
        only("drop")?;
        if drop != &Value::Bool(true) {
            return Err(format!("drop must be true, got {drop}"));
        }
        if !last {
            return Err("drop closes the stream, so it must be the last item".into());
        }
        return Ok(SseStep::Drop);
    }
    if let Some(unknown) = fields
        .keys()
        .find(|key| !["event", "data", "id", "retry"].contains(&key.as_str()))
    {
        return Err(format!(
            "unknown sse item field '{unknown}' (an item is an event {{ event?, data, id?, retry? }}, {{ comment }}, {{ delayMs }}, or {{ drop: true }})"
        ));
    }
    let data = match fields.get("data") {
        Some(Value::String(text)) => text.clone(),
        Some(other) => other.to_string(),
        None => return Err("an sse event needs data".into()),
    };
    let line = |key: &str| -> Result<Option<String>, String> {
        match fields.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(text)) if !text.contains(['\r', '\n', '\0']) => {
                Ok(Some(text.clone()))
            }
            Some(_) => Err(format!("{key} must be a single-line string")),
        }
    };
    let retry = match fields.get("retry") {
        None | Some(Value::Null) => None,
        Some(Value::Number(n)) => Some(
            n.as_u64()
                .ok_or_else(|| format!("retry must be a non-negative integer, got {n}"))?,
        ),
        Some(other) => return Err(format!("retry must be a number, got {other}")),
    };
    Ok(SseStep::Event {
        event: line("event")?.filter(|event| !event.is_empty()),
        data,
        id: line("id")?,
        retry,
    })
}

// ------------------------------ Logic side ------------------------------

/// Wraps the global `fetch` of a context. `host` carries the Rust side:
///
/// - `active()` is the fast path: false while no route, run, or recording
///   watches Logic `fetch`.
/// - `observe(kind, method, url)` opens a call-log entry: `null`, or
///   `[id, record, contract]`: `record` asks for the real response body
///   whatever its type (a recording), `contract` for the status and content
///   type of what the app receives and its body when JSON (a contract
///   capture). Either way the body is read once, here, and the app gets an
///   equivalent buffered `Response`.
/// - `decide(method, url, headersJson, body, bodyOverflow, id)` returns
///   `undefined` to pass through, a fulfillment record, `{ sse }` (a
///   `text/event-stream` answer, held open while `holds(hold)`), `{ patch }`
///   (pass through, then `patchBody(patch, text)` rewrites the JSON body
///   host-side), `{ hang }` (held while `holds(hang)`), or throws the abort
///   error.
/// - `settle(id, status, error, contentType, body, note)` closes the entry;
///   a second settle of the same id is ignored.
///
/// Request bodies are read only when available synchronously (string,
/// `URLSearchParams`, `ArrayBuffer`, typed array); streams, `Blob`,
/// `FormData`, and a `Request` object's own body are recorded as `null`.
const FETCH_INTERCEPTOR: &str = r#"(function (originalFetch, host) {
  'use strict';
  // One wrapper per context: installing twice would observe every call twice.
  const WRAPPED = Symbol.for('lingxia.automation.network.wrapped');
  if (typeof originalFetch !== 'function' || originalFetch[WRAPPED] === true) return;
  const active = host.active;
  const decide = host.decide;
  const observe = host.observe;
  const settle = host.settle;
  const holds = host.holds;
  const bodyLimit = host.bodyLimit;
  const binaryLimit = host.binaryLimit;
  const ResponseCtor = globalThis.Response;
  const RequestCtor = globalThis.Request;
  const HeadersCtor = globalThis.Headers;
  const StreamCtor = globalThis.ReadableStream;
  const Params = globalThis.URLSearchParams;
  const Decoder = globalThis.TextDecoder;
  const Encoder = globalThis.TextEncoder;
  const setTimer = globalThis.setTimeout;
  const clearTimer = globalThis.clearTimeout;
  const sentHeaders = function (request, init) {
    const source = init && init.headers != null ? init.headers : request ? request.headers : null;
    const pairs = [];
    if (source == null || typeof HeadersCtor !== 'function') return pairs;
    try {
      new HeadersCtor(source).forEach(function (value, name) { pairs.push([name, value]); });
    } catch (_) {}
    return pairs;
  };
  const cutText = function (text) {
    if (text.length <= bodyLimit) return [text, false];
    let end = bodyLimit;
    const last = text.charCodeAt(end - 1);
    if (last >= 0xd800 && last <= 0xdbff) end -= 1;
    return [text.slice(0, end), true];
  };
  const sentBody = function (init) {
    try {
      const body = init ? init.body : null;
      if (body == null) return [null, false];
      if (typeof body === 'string') return cutText(body);
      if (typeof Params === 'function' && body instanceof Params) return cutText(String(body));
      let bytes = null;
      if (body instanceof ArrayBuffer) bytes = new Uint8Array(body);
      else if (ArrayBuffer.isView(body)) bytes = new Uint8Array(body.buffer, body.byteOffset, body.byteLength);
      if (bytes === null || typeof Decoder !== 'function') return [null, false];
      const overflow = bytes.byteLength > bodyLimit;
      return [new Decoder().decode(overflow ? bytes.subarray(0, bodyLimit) : bytes), overflow];
    } catch (_) {
      return [null, false];
    }
  };
  const errorText = function (error) {
    try {
      if (error && typeof error === 'object' && 'message' in error) {
        return String(error.name || 'Error') + ': ' + String(error.message);
      }
      return String(error);
    } catch (_) {
      return 'Error';
    }
  };
  const withUrl = function (response, url) {
    try { Object.defineProperty(response, 'url', { value: url, enumerable: true }); } catch (_) {}
    return response;
  };
  const textual = function (type) {
    return type.indexOf('text/') === 0 || /json|xml|javascript|x-www-form-urlencoded|graphql/.test(type);
  };
  // Must agree with `capture::is_json_type`: streaming JSON types are not
  // buffered for a contract capture, or an app reading them incrementally
  // would stall.
  const JSON_TYPE = /^\s*(application|text)\/([^;\s]*\+)?json\s*(;|$)/i;
  const typeOf = function (response) {
    try { return String(response.headers.get('content-type') || '').toLowerCase(); } catch (_) { return ''; }
  };
  // The one place a response body is read for the Rust side: read it once,
  // settle the call with it, and hand the app an equivalent Response. A
  // recording takes any body (event streams and large binary bodies are only
  // noted); a contract capture only a JSON one. Anything else is settled
  // without reading, and the app gets its response untouched.
  const consume = function (id, response, url, watch) {
    const status = response.status;
    const type = typeOf(response);
    let isText = true;
    let note = null;
    if (watch.record) {
      isText = textual(type);
      const declared = Number(response.headers.get('content-length'));
      if (type.indexOf('text/event-stream') === 0) note = 'event stream body not recorded; write an sse answer';
      else if (!isText && declared > binaryLimit) note = 'binary body of ' + declared + ' bytes not recorded';
    }
    const read = watch.record ? note === null : watch.contract && JSON_TYPE.test(type);
    if (!read) {
      try { settle(id, status, null, type, null, note); } catch (_) {}
      return response;
    }
    return (isText ? response.text() : response.arrayBuffer()).then(function (body) {
      try { settle(id, status, null, type, body, null); } catch (_) {}
      const headers = new HeadersCtor(response.headers);
      headers.delete('content-length');
      headers.delete('content-encoding');
      const empty = isText ? body.length === 0 : body.byteLength === 0;
      return withUrl(new ResponseCtor(empty ? null : body, {
        status: status,
        statusText: response.statusText,
        headers: headers,
      }), response.url || url);
    });
  };
  const NO_BODY = { record: false, contract: false };
  const track = function (promise, id, watch, url) {
    if (!id) return promise;
    return promise.then(function (response) {
      if (watch.record || watch.contract) return consume(id, response, url, watch);
      try { settle(id, response.status, null, typeOf(response), null, null); } catch (_) {}
      return response;
    }, function (error) {
      try { settle(id, 0, errorText(error), null, null, null); } catch (_) {}
      throw error;
    });
  };
  const HANG_POLL_MS = 200;
  // A text/event-stream body that plays the route's items, honouring
  // delays; `drop` closes it, otherwise it stays open while the route holds.
  const sseBody = function (hit, signal) {
    const steps = hit.sse;
    return new StreamCtor({
      start: function (controller) {
        const encoder = new Encoder();
        const listens = signal && typeof signal.addEventListener === 'function';
        let index = 0;
        let timer = null;
        let done = false;
        const finish = function (error) {
          if (done) return;
          done = true;
          if (timer !== null && typeof clearTimer === 'function') clearTimer(timer);
          timer = null;
          if (listens) signal.removeEventListener('abort', onAbort);
          try {
            if (error !== undefined) controller.error(error);
            else controller.close();
          } catch (_) {}
        };
        const onAbort = function () { finish(signal.reason); };
        if (listens) signal.addEventListener('abort', onAbort);
        const pump = function () {
          timer = null;
          while (!done && index < steps.length) {
            const step = steps[index++];
            if (step.drop) return finish();
            if (step.delay > 0) { timer = setTimer(pump, step.delay); return; }
            if (typeof step.text === 'string') {
              try { controller.enqueue(encoder.encode(step.text)); } catch (_) { return finish(); }
            }
          }
          if (done) return;
          if (!(hit.hold > 0)) return finish();
          const check = function () {
            timer = null;
            if (done) return;
            if (holds(hit.hold)) { timer = setTimer(check, HANG_POLL_MS); return; }
            finish();
          };
          timer = setTimer(check, HANG_POLL_MS);
        };
        pump();
      },
    });
  };
  const answer = function (self, args, hit, url, init, id, contract) {
    const signal = init && init.signal;
    if (typeof hit.patch === 'string') {
      // The patch reads the real body anyway: settle the call with the
      // patched text the app receives.
      return originalFetch.apply(self, args).then(function (response) {
        return response.text().then(function (text) {
          const headers = new HeadersCtor(response.headers);
          // The body is re-encoded text of a new length.
          headers.delete('content-length');
          headers.delete('content-encoding');
          const body = text === '' ? text : host.patchBody(hit.patch, text, response.url || url);
          if (id) {
            try { settle(id, response.status, null, typeOf(response), contract ? body : null, null); } catch (_) {}
          }
          const patched = new ResponseCtor(body, {
            status: response.status,
            statusText: response.statusText,
            headers: headers,
          });
          return withUrl(patched, response.url || url);
        });
      });
    }
    if (signal && signal.aborted) return Promise.reject(signal.reason);
    if (hit.hang > 0) {
      if (typeof setTimer !== 'function') {
        return Promise.reject(new Error('route hang needs setTimeout in this context'));
      }
      return new Promise(function (_resolve, reject) {
        let timer = null;
        const listens = signal && typeof signal.addEventListener === 'function';
        const onAbort = function () {
          if (timer !== null && typeof clearTimer === 'function') clearTimer(timer);
          reject(signal.reason);
        };
        if (listens) signal.addEventListener('abort', onAbort);
        const check = function () {
          if (holds(hit.hang)) { timer = setTimer(check, HANG_POLL_MS); return; }
          if (listens) signal.removeEventListener('abort', onAbort);
          try { host.released(); } catch (error) { reject(error); }
        };
        timer = setTimer(check, HANG_POLL_MS);
      });
    }
    const streams = Array.isArray(hit.sse);
    if (streams && (typeof setTimer !== 'function' || typeof StreamCtor !== 'function' || typeof Encoder !== 'function')) {
      return Promise.reject(new Error('route sse needs setTimeout, ReadableStream and TextEncoder in this context'));
    }
    const respond = function () {
      const response = new ResponseCtor(streams ? sseBody(hit, signal) : hit.body, {
        status: hit.status,
        statusText: hit.statusText,
        headers: hit.headers,
      });
      return withUrl(response, url);
    };
    if (!(hit.delay > 0)) {
      try { return Promise.resolve(respond()); } catch (error) { return Promise.reject(error); }
    }
    if (typeof setTimer !== 'function') {
      return Promise.reject(new Error('route delay needs setTimeout in this context'));
    }
    return new Promise(function (resolve, reject) {
      let timer = null;
      const listens = signal && typeof signal.addEventListener === 'function';
      const onAbort = function () {
        if (timer !== null && typeof clearTimer === 'function') clearTimer(timer);
        reject(signal.reason);
      };
      if (listens) signal.addEventListener('abort', onAbort);
      timer = setTimer(function () {
        if (listens) signal.removeEventListener('abort', onAbort);
        try { resolve(respond()); } catch (error) { reject(error); }
      }, hit.delay);
    });
  };
  const fetch = function fetch(input, init) {
    if (!active()) return originalFetch.apply(this, arguments);
    const self = this;
    const args = arguments;
    let method, url, request;
    try {
      request = typeof RequestCtor === 'function' && input instanceof RequestCtor ? input : null;
      url = request ? request.url
        : (input !== null && typeof input === 'object' && typeof input.href === 'string') ? input.href
        : String(input);
      const raw = init && init.method != null ? init.method : request ? request.method : 'GET';
      method = String(raw).toUpperCase();
    } catch (_) {
      return originalFetch.apply(this, arguments);
    }
    let seen = null;
    try { seen = observe('fetch', method, url); } catch (_) {}
    const id = seen ? seen[0] : 0;
    const watch = seen ? { record: Boolean(seen[1]), contract: Boolean(seen[2]) } : NO_BODY;
    let hit;
    try {
      const body = sentBody(init);
      hit = decide(method, url, JSON.stringify(sentHeaders(request, init)), body[0], body[1], id);
    } catch (error) {
      if (id) { try { settle(id, 0, errorText(error), null, null, null); } catch (_) {} }
      return Promise.reject(error);
    }
    if (hit === undefined || hit === null) {
      return track(originalFetch.apply(this, arguments), id, watch, url);
    }
    // A route's answer is never recorded, and a fulfilled one is captured
    // when it is decided; a patch settles with its own body.
    return track(answer(self, args, hit, url, init, id, watch.contract), id, NO_BODY, url);
  };
  Object.defineProperty(fetch, WRAPPED, { value: true });
  globalThis.fetch = fetch;
})"#;

/// Wraps `Rong.SSE`, the native SSE client of Logic, and returns the
/// wrapper, which lxapp installs before it freezes `Rong`. With nothing watching
/// the network it constructs the native client unchanged. Otherwise every
/// connection attempt goes through `host.decide` like a `fetch` (`GET`,
/// `accept: text/event-stream`, `last-event-id` on reconnects), and an
/// answering route is played by a client that follows the native one:
/// events as `{ type, data, id, origin }`, `retry` and `id` fields, and
/// reconnects with backoff when the stream ends, unless `reconnect.enabled`
/// is false or `maxRetries` is spent. A pass-through (no route, `continue`)
/// hands the connection to the native client, carrying `Last-Event-ID`.
const SSE_INTERCEPTOR: &str = r#"(function (OriginalSSE, host) {
  'use strict';
  const WRAPPED = Symbol.for('lingxia.automation.network.wrapped');
  if (typeof OriginalSSE !== 'function' || OriginalSSE[WRAPPED] === true) return OriginalSSE;
  const active = host.active;
  const decide = host.decide;
  const observe = host.observe;
  const settle = host.settle;
  const holds = host.holds;
  const URLCtor = globalThis.URL;
  const Decoder = globalThis.TextDecoder;
  const setTimer = globalThis.setTimeout;
  const clearTimer = globalThis.clearTimeout;
  const HOLD_POLL_MS = 200;
  const failure = function (message) {
    const error = new Error(message);
    error.code = 'E_IO';
    return error;
  };
  const errorText = function (error) {
    try { return error && typeof error === 'object' && 'message' in error ? String(error.message) : String(error); }
    catch (_) { return 'error'; }
  };
  const count = function (value, fallback, min) {
    return typeof value === 'number' && isFinite(value) && value >= min ? Math.floor(value) : fallback;
  };
  // A pass-through connection belongs to the native client; its call-log
  // entry settles when the first read opens the stream (200) or fails.
  const settleOnOpen = function (sse, call) {
    if (!call) return sse;
    const settleOnce = function (status, error) {
      try { settle(call, status, error, null, null, null); } catch (_) {}
    };
    const original = sse.next;
    if (typeof original !== 'function') { settleOnce(200, null); return sse; }
    let pending = true;
    try {
      Object.defineProperty(sse, 'next', {
        configurable: true,
        writable: true,
        value: function () {
          const result = original.apply(sse, arguments);
          if (pending) {
            pending = false;
            Promise.resolve(result).then(
              function () { settleOnce(200, null); },
              function (error) { settleOnce(0, errorText(error)); },
            );
          }
          return result;
        },
      });
    } catch (_) {
      settleOnce(200, null);
    }
    return sse;
  };
  const headerPairs = function (headers) {
    const pairs = [];
    if (headers && typeof headers === 'object') {
      Object.keys(headers).forEach(function (name) { pairs.push([String(name).toLowerCase(), String(headers[name])]); });
    }
    return pairs;
  };
  // A fulfilled text/event-stream body, as the native parser reads it.
  const parseBody = function (text) {
    const steps = [];
    let event = null, data = [], id = null, retry = null;
    const flush = function () {
      if (data.length > 0 || id !== null || retry !== null) {
        steps.push({ event: event, data: data.length > 0 ? data.join('\n') : null, id: id, retry: retry });
      }
      event = null; data = []; id = null; retry = null;
    };
    String(text).split(/\r\n|\r|\n/).forEach(function (line) {
      if (line === '') return flush();
      if (line.charAt(0) === ':') return;
      const colon = line.indexOf(':');
      const field = colon < 0 ? line : line.slice(0, colon);
      let value = colon < 0 ? '' : line.slice(colon + 1);
      if (value.charAt(0) === ' ') value = value.slice(1);
      if (field === 'event') event = value;
      else if (field === 'data') data.push(value);
      else if (field === 'id' && value.indexOf('\0') < 0) id = value;
      else if (field === 'retry' && /^[0-9]+$/.test(value)) retry = Number(value);
    });
    flush();
    // A fulfilled body ends: the server closed the stream.
    steps.push({ drop: true });
    return steps;
  };
  const attempt = function (href, pairs, lastId) {
    const headers = pairs.slice();
    headers.push(['accept', 'text/event-stream']);
    if (lastId !== null) headers.push(['last-event-id', lastId]);
    let seen = null;
    try { seen = observe('sse', 'GET', href); } catch (_) {}
    const call = seen ? seen[0] : 0;
    let hit;
    try {
      hit = decide('GET', href, JSON.stringify(headers), null, false, call);
    } catch (error) {
      return { kind: 'transport', message: 'sse request failed: ' + errorText(error), call: call, delay: 0 };
    }
    if (hit === undefined || hit === null || typeof hit.patch === 'string') return { kind: 'real', call: call };
    if (hit.hang > 0) return { kind: 'hang', token: hit.hang, call: call, delay: 0 };
    if (Array.isArray(hit.sse)) {
      return { kind: 'stream', steps: hit.sse, hold: hit.hold || 0, delay: hit.delay || 0, call: call };
    }
    let type = '';
    (hit.headers || []).forEach(function (pair) {
      if (String(pair[0]).toLowerCase() === 'content-type') type = String(pair[1]).toLowerCase();
    });
    if (hit.status !== 200) {
      return { kind: 'fail', status: hit.status, message: 'sse server returned status ' + hit.status, call: call, delay: hit.delay || 0 };
    }
    if (type.indexOf('text/event-stream') !== 0) {
      return { kind: 'fail', status: hit.status, message: 'invalid sse content-type: ' + (type || '<empty>'), call: call, delay: hit.delay || 0 };
    }
    let text = '';
    if (typeof hit.body === 'string') text = hit.body;
    else if (hit.body != null && typeof Decoder === 'function') text = new Decoder().decode(new Uint8Array(hit.body));
    return { kind: 'stream', steps: parseBody(text), hold: 0, delay: hit.delay || 0, call: call };
  };
  const fake = function (url, href, origin, opts, pairs, policy, first) {
    const self = Object.create(OriginalSSE.prototype);
    const signal = opts.signal;
    const wakers = [];
    let current = first;
    let steps = [];
    let index = 0;
    let hold = 0;
    let lastId = null;
    let retries = 0;
    let delay = policy.base;
    let closed = false;
    let delegate = null;
    let chain = Promise.resolve();
    const close = function () {
      if (closed) return;
      closed = true;
      if (delegate) { try { delegate.close(); } catch (_) {} }
      while (wakers.length > 0) wakers.shift()();
    };
    if (signal && typeof signal.addEventListener === 'function') {
      if (signal.aborted) close();
      else signal.addEventListener('abort', close);
    }
    const sleep = function (ms) {
      return new Promise(function (resolve) {
        if (closed) return resolve();
        let timer = null;
        const wake = function () {
          if (timer !== null) clearTimer(timer);
          timer = null;
          resolve();
        };
        wakers.push(wake);
        timer = setTimer(function () {
          timer = null;
          const at = wakers.indexOf(wake);
          if (at >= 0) wakers.splice(at, 1);
          resolve();
        }, ms);
      });
    };
    const held = function (token) {
      if (closed || !holds(token)) return Promise.resolve();
      return sleep(HOLD_POLL_MS).then(function () { return held(token); });
    };
    const done = function () { return { done: true, value: undefined }; };
    const backoff = function () {
      const wait = Math.min(delay, policy.max);
      delay = Math.min(wait * 2, policy.max);
      return wait;
    };
    const mayRetry = function () {
      return policy.enabled && (policy.maxRetries === null || retries < policy.maxRetries);
    };
    const note = function (call, status, error) {
      if (call) { try { settle(call, status, error, null, null, null); } catch (_) {} }
    };
    const advance = async function () {
      for (;;) {
        if (closed) return done();
        if (delegate) return delegate.next();
        if (current) {
          const opening = current;
          current = null;
          if (opening.kind === 'real') {
            const headers = {};
            pairs.forEach(function (pair) { headers[pair[0]] = pair[1]; });
            if (lastId !== null) headers['last-event-id'] = lastId;
            delegate = settleOnOpen(new OriginalSSE(url, Object.assign({}, opts, { headers: headers })), opening.call);
            continue;
          }
          if (opening.delay > 0) {
            await sleep(opening.delay);
            if (closed) return done();
          }
          let message = opening.message;
          if (opening.kind === 'hang') {
            await held(opening.token);
            if (closed) return done();
            message = 'sse request failed: released by test route: hang ended';
          }
          if (opening.kind === 'transport' || opening.kind === 'hang') {
            note(opening.call, 0, message);
            if (!mayRetry()) { close(); throw failure(message); }
            retries += 1;
            await sleep(backoff());
            if (closed) return done();
            current = attempt(href, pairs, lastId);
            continue;
          }
          if (opening.kind === 'fail') {
            note(opening.call, opening.status, message);
            close();
            throw failure(message);
          }
          note(opening.call, 200, null);
          steps = opening.steps;
          index = 0;
          hold = opening.hold;
          retries = 0;
          delay = policy.base;
        }
        while (index < steps.length) {
          const step = steps[index++];
          if (step.delay > 0) {
            await sleep(step.delay);
            if (closed) return done();
            continue;
          }
          if (step.drop) { index = steps.length; hold = 0; break; }
          if (step.comment !== undefined) continue;
          if (typeof step.retry === 'number') delay = Math.min(Math.max(1, step.retry), policy.max);
          if (typeof step.id === 'string') lastId = step.id;
          if (step.data === null || step.data === undefined) continue;
          return {
            done: false,
            value: { type: step.event || 'message', data: String(step.data), id: lastId === null ? '' : lastId, origin: origin },
          };
        }
        if (hold > 0) {
          const token = hold;
          hold = 0;
          await held(token);
          if (closed) return done();
        }
        // The stream ended: reconnect as the native client does.
        if (!mayRetry()) { close(); return done(); }
        retries += 1;
        await sleep(backoff());
        if (closed) return done();
        current = attempt(href, pairs, lastId);
      }
    };
    const next = function () {
      const run = chain.then(advance);
      chain = run.then(function () {}, function () {});
      return run;
    };
    Object.defineProperty(self, 'next', { value: next });
    Object.defineProperty(self, 'close', { value: close });
    Object.defineProperty(self, 'return', { value: function () { close(); return Promise.resolve(done()); } });
    Object.defineProperty(self, 'url', { get: function () { return url; } });
    Object.defineProperty(self, Symbol.asyncIterator, { value: function () { return this; } });
    return self;
  };
  const SSE = function SSE(url, options) {
    if (new.target === undefined) throw new TypeError("Class constructor SSE cannot be invoked without 'new'");
    if (!active() || typeof setTimer !== 'function' || typeof URLCtor !== 'function') {
      return new OriginalSSE(url, options);
    }
    const href = String(url);
    let parsed;
    try { parsed = new URLCtor(href); } catch (_) { return new OriginalSSE(url, options); }
    if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') return new OriginalSSE(url, options);
    const opts = options !== null && typeof options === 'object' ? options : {};
    const reconnect = opts.reconnect !== null && typeof opts.reconnect === 'object' ? opts.reconnect : {};
    const base = Math.max(1, count(reconnect.baseDelayMs, 1000, 0));
    const policy = {
      enabled: typeof reconnect.enabled === 'boolean' ? reconnect.enabled : true,
      base: base,
      max: Math.max(base, count(reconnect.maxDelayMs, 30000, 1)),
      maxRetries: count(reconnect.maxRetries, null, 0),
    };
    const pairs = headerPairs(opts.headers);
    const origin = parsed.protocol.slice(0, -1) + '://' + parsed.host;
    const first = attempt(href, pairs, null);
    if (first.kind === 'real') return settleOnOpen(new OriginalSSE(url, options), first.call);
    return fake(url, href, origin, opts, pairs, policy, first);
  };
  SSE.prototype = OriginalSSE.prototype;
  Object.defineProperty(SSE, WRAPPED, { value: true });
  return SSE;
})"#;

/// Which app a Logic context belongs to and whether its network policy
/// admits a host.
pub(crate) struct LogicTarget {
    pub appid: String,
    pub allowed: Box<dyn Fn(&str) -> bool>,
}

type Resolve = Rc<dyn Fn(&JSContext) -> Option<LogicTarget>>;

/// The Rust half of the Logic interceptors, as one JS object.
fn interceptor_host(ctx: &JSContext, resolve: Resolve) -> JSResult<JSObject> {
    let host = JSObject::new(ctx);
    host.set("active", JSFunc::new(ctx, registry::any_active)?)?;
    host.set("bodyLimit", registry::MAX_REQUEST_BODY_BYTES as f64)?;
    host.set("binaryLimit", capture::MAX_BINARY_BODY_BYTES as f64)?;
    let decide_target = resolve.clone();
    host.set(
        "decide",
        JSFunc::new(
            ctx,
            move |ctx: JSContext,
                  method: String,
                  url: String,
                  headers: String,
                  body: Option<String>,
                  overflow: bool,
                  call: f64|
                  -> JSResult<JSValue> {
                let Some(target) = decide_target(&ctx) else {
                    return Ok(JSValue::undefined(&ctx));
                };
                let host = url
                    .parse::<http::Uri>()
                    .ok()
                    .and_then(|uri| uri.host().map(str::to_string));
                let allowed = || host.as_deref().is_some_and(|host| (target.allowed)(host));
                let sent = || {
                    let headers = serde_json::from_str(&headers).unwrap_or_default();
                    SentRequest::new(headers, body, overflow)
                };
                let (decision, dev_label) = registry::with_registry(|routes| {
                    let decision = routes.decide_route(
                        &target.appid,
                        &method,
                        &url,
                        sent,
                        allowed,
                        call as u64,
                    );
                    let dev_label = decision
                        .as_ref()
                        .filter(|decision| decision.owner == registry::DEV_SESSION_OWNER)
                        .and_then(|_| routes.dev.as_ref().map(dev::label));
                    (decision, dev_label)
                });
                let action = decision.map(|decision| decision.action);
                if let (Some(label), Some(action)) = (dev_label, &action) {
                    dev::warn(
                        &target.appid,
                        format!(
                            "dev scenario {label} answered {method} {} with {}",
                            capture::redact_url(&url, capture::REDACTED),
                            dev::describe(action)
                        ),
                    );
                }
                match action {
                    None | Some(RouteAction::Continue) => Ok(JSValue::undefined(&ctx)),
                    Some(RouteAction::Abort(kind)) => Err(fetch_failed(kind)),
                    Some(RouteAction::Fulfill(fulfill)) => fulfillment_js(&ctx, fulfill),
                    Some(RouteAction::Sse(sse)) => sse_js(&ctx, sse),
                    Some(RouteAction::Patch(patch)) => {
                        let object = JSObject::new(&ctx);
                        object.set("patch", patch.to_string())?;
                        Ok(object.into_js_value())
                    }
                    Some(RouteAction::Hang { token }) => {
                        let object = JSObject::new(&ctx);
                        object.set("hang", token as f64)?;
                        Ok(object.into_js_value())
                    }
                }
            },
        )?,
    )?;
    host.set(
        "observe",
        JSFunc::new(
            ctx,
            move |ctx: JSContext, kind: String, method: String, url: String| -> JSResult<JSValue> {
                let Some(target) = resolve(&ctx) else {
                    return Ok(JSValue::null(&ctx));
                };
                let kind = if kind == "sse" { "sse" } else { "fetch" };
                match registry::with_registry(|routes| {
                    routes.observe(&target.appid, kind, &method, &url)
                }) {
                    Some((id, watch)) => {
                        json_to_js(&ctx, &serde_json::json!([id, watch.record, watch.contract]))
                    }
                    None => Ok(JSValue::null(&ctx)),
                }
            },
        )?,
    )?;
    host.set(
        "settle",
        JSFunc::new(
            ctx,
            |id: f64,
             status: f64,
             error: Option<String>,
             content_type: Option<String>,
             body: JSValue,
             note: Option<String>|
             -> JSResult<()> {
                let body = if let Some(note) = note {
                    Some(capture::RecordedBody::Omitted(note))
                } else if body.is_string() {
                    Some(capture::RecordedBody::Text(body.to_rust::<String>()?))
                } else if body.is_array_buffer() {
                    Some(capture::RecordedBody::Binary(
                        body.to_rust::<JSArrayBuffer>()?.as_bytes().to_vec(),
                    ))
                } else {
                    None
                };
                let status = (100.0..=999.0).contains(&status).then_some(status as u16);
                registry::with_registry(|routes| {
                    routes.settle(
                        id as u64,
                        capture::Settled {
                            status,
                            error,
                            content_type: content_type.filter(|kind| !kind.is_empty()),
                            body,
                        },
                    )
                });
                Ok(())
            },
        )?,
    )?;
    host.set(
        "patchBody",
        JSFunc::new(
            ctx,
            |patch: String, text: String, url: String| -> JSResult<String> {
                apply_patch_json(&patch, &text).map_err(|message| {
                    HostError::new(
                        rong::error::E_IO,
                        format!("test route patchJson: {message} ({url})"),
                    )
                    .with_name("TypeError")
                    .into()
                })
            },
        )?,
    )?;
    host.set(
        "holds",
        JSFunc::new(ctx, |token: f64| -> bool {
            registry::with_registry(|routes| routes.holds(token as u64))
        })?,
    )?;
    host.set(
        "released",
        JSFunc::new(ctx, || -> JSResult<()> {
            Err(HostError::new(rong::error::E_IO, "fetch failed")
                .with_name("TypeError")
                .with_data(rong::err_data!({ detail: ("released by test route: hang ended") }))
                .into())
        })?,
    )?;
    Ok(host)
}

/// Install the route-aware `fetch` wrapper in a context whose `fetch` is
/// already registered. A context without `fetch` is left untouched.
pub(crate) fn install_fetch_interceptor(
    ctx: &JSContext,
    resolve: impl Fn(&JSContext) -> Option<LogicTarget> + 'static,
) -> JSResult<()> {
    let original = ctx.global().get::<_, JSValue>("fetch")?;
    if original.is_undefined() {
        return Ok(());
    }
    let host = interceptor_host(ctx, Rc::new(resolve))?;
    let installer = ctx.eval::<JSFunc>(Source::from_bytes(FETCH_INTERCEPTOR))?;
    installer.call::<_, ()>(None, (original, host))
}

/// The route-aware wrapper of a `Rong.SSE` class.
pub(crate) fn wrap_sse(
    ctx: &JSContext,
    original: JSValue,
    resolve: impl Fn(&JSContext) -> Option<LogicTarget> + 'static,
) -> JSResult<JSValue> {
    let host = interceptor_host(ctx, Rc::new(resolve))?;
    let installer = ctx.eval::<JSFunc>(Source::from_bytes(SSE_INTERCEPTOR))?;
    installer.call::<_, JSValue>(None, (original, host))
}

/// Replace `Rong.SSE` in a context whose `Rong` is not frozen (tests).
#[cfg(test)]
pub(crate) fn install_sse_interceptor(
    ctx: &JSContext,
    resolve: impl Fn(&JSContext) -> Option<LogicTarget> + 'static,
) -> JSResult<()> {
    let Some(rong) = ctx.global().get::<_, JSValue>("Rong")?.into_object() else {
        return Ok(());
    };
    let original = rong.get::<_, JSValue>("SSE")?;
    if original.is_undefined() {
        return Ok(());
    }
    rong.set("SSE", wrap_sse(ctx, original, resolve)?)
        .map(|_| ())
}

/// Apply a route's merge patch to a real response body.
fn apply_patch_json(patch: &str, body: &str) -> Result<String, String> {
    let patch: Value =
        serde_json::from_str(patch).map_err(|err| format!("invalid patch: {err}"))?;
    let mut target: Value = serde_json::from_str(body)
        .map_err(|_| "the response body is not JSON, so it cannot be patched".to_string())?;
    registry::merge_patch(&mut target, &patch);
    Ok(target.to_string())
}

fn fulfillment_js(ctx: &JSContext, fulfill: Fulfill) -> JSResult<JSValue> {
    let object = JSObject::new(ctx);
    object.set("status", fulfill.status as f64)?;
    if let Some(text) = fulfill.status_text {
        object.set("statusText", text)?;
    }
    object.set("headers", headers_js(ctx, fulfill.headers)?)?;
    match fulfill.body {
        Some(ResponseBody::Text(text)) => object.set("body", text)?,
        Some(ResponseBody::Binary(bytes)) => {
            object.set("body", JSArrayBuffer::from_bytes_owned(ctx, bytes)?)?
        }
        None => object.set("body", JSValue::null(ctx))?,
    };
    object.set("delay", f64::from(fulfill.delay_ms))?;
    Ok(object.into_js_value())
}

fn headers_js(ctx: &JSContext, headers: Vec<(String, String)>) -> JSResult<JSValue> {
    let headers = headers
        .into_iter()
        .map(|(name, value)| Value::Array(vec![Value::String(name), Value::String(value)]))
        .collect();
    json_to_js(ctx, &Value::Array(headers))
}

/// `{ status: 200, headers, delay, hold, sse: [step] }`. A step carries its
/// wire `text` for `fetch` and its fields for `Rong.SSE`.
fn sse_js(ctx: &JSContext, sse: SseAnswer) -> JSResult<JSValue> {
    let steps: Vec<Value> = sse
        .steps
        .iter()
        .map(|step| match step {
            SseStep::Event {
                event,
                data,
                id,
                retry,
            } => serde_json::json!({
                "text": step.frame(),
                "event": event,
                "data": data,
                "id": id,
                "retry": retry,
            }),
            SseStep::Comment(comment) => {
                serde_json::json!({ "text": step.frame(), "comment": comment })
            }
            SseStep::Delay(ms) => serde_json::json!({ "delay": ms }),
            SseStep::Drop => serde_json::json!({ "drop": true }),
        })
        .collect();
    let object = JSObject::new(ctx);
    object.set("status", 200.0)?;
    object.set("headers", headers_js(ctx, sse.headers)?)?;
    object.set("delay", f64::from(sse.delay_ms))?;
    object.set("hold", sse.hold as f64)?;
    object.set("sse", json_to_js(ctx, &Value::Array(steps))?)?;
    Ok(object.into_js_value())
}

fn logic_target(ctx: &JSContext) -> Option<LogicTarget> {
    let app = LxApp::from_ctx(ctx).ok()?;
    let appid = app.appid.clone();
    let weak = Arc::downgrade(&app);
    Some(LogicTarget {
        appid,
        allowed: Box::new(move |host| {
            weak.upgrade()
                .is_some_and(|app| app.is_domain_allowed(host))
        }),
    })
}

/// Logic-context hook: route an lxapp's `fetch` by its appid and policy.
pub(crate) fn install_logic_fetch_interceptor(ctx: &JSContext) -> JSResult<()> {
    install_fetch_interceptor(ctx, logic_target)
}

/// `Rong.SSE` wrapper registered with lxapp, applied to every Logic context
/// before `Rong` is frozen: route an lxapp's SSE connections like its
/// `fetch`.
pub(crate) fn wrap_logic_sse(ctx: &JSContext, original: JSValue) -> JSResult<JSValue> {
    wrap_sse(ctx, original, logic_target)
}

#[cfg(test)]
mod tests;
