//! `NetworkDriver` — test-only interception of an lxapp's Logic `fetch`.
//!
//! Routes live in a process table keyed by run and appid. Only a host
//! automation run can install them, and the run's finalization clears them,
//! so no route outlives the run that installed it. Logic contexts get a thin
//! `fetch` wrapper whose fast path is one atomic load while no route exists.

mod registry;

use crate::auto_err;
use crate::resolve::{json_to_js, upgrade_authorized};
use lxapp::LxApp;
use registry::{
    AbortKind, Fulfill, MAX_DELAY_MS, ResponseBody, RouteAction, RouteSpec, SentRequest, UrlMatcher,
};
use rong::{
    AnyJSTypedArray, Class, HostError, JSArrayBuffer, JSContext, JSFunc, JSObject, JSResult,
    JSValue, Source, js_class, js_method,
};
use serde_json::Value;
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
        let action = parse_handler(&handler)?;
        let label = matcher.label();
        let spec = RouteSpec {
            matcher,
            method,
            times,
            action,
        };
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

fn requests_js(
    ctx: &JSContext,
    run_id: &str,
    appid: &str,
    route: Option<u64>,
) -> JSResult<JSValue> {
    let entries = registry::with_registry(|routes| routes.requests(run_id, appid));
    let list: Vec<Value> = entries
        .into_iter()
        .filter(|entry| route.is_none_or(|id| entry.route_id == id))
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
    let method = object
        .get_opt::<_, String>("method")?
        .map(|method| method.trim().to_ascii_uppercase())
        .filter(|method| !method.is_empty() && method != "*");
    if let Some(method) = &method
        && !method.bytes().all(|b| b.is_ascii_alphabetic() || b == b'-')
    {
        return Err(auto_err(format!("invalid route method '{method}'")));
    }
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
/// merge-patching the real JSON response; `{ hang: true }` never answers.
/// The four are exclusive.
fn parse_handler(handler: &JSObject) -> JSResult<RouteAction> {
    let binary = binary_body(handler)?;
    let json = handler
        .to_json_string()
        .map_err(|err| auto_err(format!("route handler must be JSON-compatible: {err}")))?;
    let value: Value = serde_json::from_str(&json)
        .map_err(|err| auto_err(format!("route handler must be JSON-compatible: {err}")))?;
    parse_handler_value(&value, binary).map_err(auto_err)
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
            && !["abort", "continue", "hang", "patchJson"].contains(&key.as_str())
    }) {
        return Err(format!("unknown route handler option '{unknown}'"));
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
    let delay_ms = match fields.get("delay") {
        None | Some(Value::Null) => 0,
        Some(Value::Number(n)) => n
            .as_u64()
            .filter(|delay| *delay <= u64::from(MAX_DELAY_MS))
            .map(|delay| delay as u32)
            .ok_or_else(|| {
                format!("route delay must be an integer in 0..={MAX_DELAY_MS} ms, got {n}")
            })?,
        Some(other) => return Err(format!("route delay must be a number, got {other}")),
    };
    Ok(Fulfill {
        status,
        status_text,
        headers,
        body: body.filter(|body| !body.is_empty() || !matches!(status, 204 | 205 | 304)),
        delay_ms,
    })
}

// ------------------------------ Logic side ------------------------------

/// Wraps the global `fetch` of a context. `active()` is the no-route fast
/// path; `decide(method, url, headersJson, body, bodyOverflow)` returns
/// `undefined` to pass through, a fulfillment record, `{ patch }` (pass
/// through, then `patchBody(patch, text)` rewrites the JSON body host-side),
/// `{ hang }` (held while `holds(hang)`), or throws the abort error. Request bodies are read only when available synchronously (string,
/// `URLSearchParams`, `ArrayBuffer`, typed array); streams, `Blob`,
/// `FormData`, and a `Request` object's own body are recorded as `null`.
const FETCH_INTERCEPTOR: &str = r#"(function (originalFetch, active, decide, bodyLimit, patchBody, holds, released) {
  'use strict';
  const ResponseCtor = globalThis.Response;
  const RequestCtor = globalThis.Request;
  const HeadersCtor = globalThis.Headers;
  const Params = globalThis.URLSearchParams;
  const Decoder = globalThis.TextDecoder;
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
  const HANG_POLL_MS = 200;
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
    let hit;
    try {
      const body = sentBody(init);
      hit = decide(method, url, JSON.stringify(sentHeaders(request, init)), body[0], body[1]);
    } catch (error) {
      return Promise.reject(error);
    }
    if (hit === undefined || hit === null) return originalFetch.apply(this, arguments);
    const signal = init && init.signal;
    if (typeof hit.patch === 'string') {
      return originalFetch.apply(self, args).then(function (response) {
        return response.text().then(function (text) {
          const headers = new HeadersCtor(response.headers);
          // The body is re-encoded text of a new length.
          headers.delete('content-length');
          headers.delete('content-encoding');
          const patched = new ResponseCtor(text === '' ? text : patchBody(hit.patch, text, response.url || url), {
            status: response.status,
            statusText: response.statusText,
            headers: headers,
          });
          try { Object.defineProperty(patched, 'url', { value: response.url || url, enumerable: true }); } catch (_) {}
          return patched;
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
          try { released(); } catch (error) { reject(error); }
        };
        timer = setTimer(check, HANG_POLL_MS);
      });
    }
    const respond = function () {
      const response = new ResponseCtor(hit.body, {
        status: hit.status,
        statusText: hit.statusText,
        headers: hit.headers,
      });
      try { Object.defineProperty(response, 'url', { value: url, enumerable: true }); } catch (_) {}
      return response;
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
  globalThis.fetch = fetch;
})"#;

/// Which app a Logic context belongs to and whether its network policy
/// admits a host.
pub(crate) struct LogicTarget {
    pub appid: String,
    pub allowed: Box<dyn Fn(&str) -> bool>,
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
    let active = JSFunc::new(ctx, registry::any_active)?;
    let decide = JSFunc::new(
        ctx,
        move |ctx: JSContext,
              method: String,
              url: String,
              headers: String,
              body: Option<String>,
              overflow: bool|
              -> JSResult<JSValue> {
            let Some(target) = resolve(&ctx) else {
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
            let action = registry::with_registry(|routes| {
                routes.decide(&target.appid, &method, &url, sent, allowed)
            });
            match action {
                None | Some(RouteAction::Continue) => Ok(JSValue::undefined(&ctx)),
                Some(RouteAction::Abort(kind)) => Err(fetch_failed(kind)),
                Some(RouteAction::Fulfill(fulfill)) => fulfillment_js(&ctx, fulfill),
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
    )?;
    let patch_body = JSFunc::new(
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
    )?;
    let holds = JSFunc::new(ctx, |token: f64| -> bool {
        registry::with_registry(|routes| routes.holds(token as u64))
    })?;
    let released = JSFunc::new(ctx, || -> JSResult<()> {
        Err(HostError::new(rong::error::E_IO, "fetch failed")
            .with_name("TypeError")
            .with_data(rong::err_data!({ detail: ("released by test route: hang ended") }))
            .into())
    })?;
    let installer = ctx.eval::<JSFunc>(Source::from_bytes(FETCH_INTERCEPTOR))?;
    let limit = registry::MAX_REQUEST_BODY_BYTES as f64;
    installer.call::<_, ()>(
        None,
        (original, active, decide, limit, patch_body, holds, released),
    )
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
    let headers = fulfill
        .headers
        .into_iter()
        .map(|(name, value)| Value::Array(vec![Value::String(name), Value::String(value)]))
        .collect();
    object.set("headers", json_to_js(ctx, &Value::Array(headers))?)?;
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

/// Logic-context hook: route an lxapp's `fetch` by its appid and policy.
pub(crate) fn install_logic_fetch_interceptor(ctx: &JSContext) -> JSResult<()> {
    install_fetch_interceptor(ctx, |ctx| {
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
    })
}

#[cfg(test)]
mod tests;
