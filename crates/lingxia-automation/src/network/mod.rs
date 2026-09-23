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
use registry::{Fulfill, RouteAction, RouteSpec, UrlMatcher};
use rong::{
    Class, HostError, JSContext, JSFunc, JSObject, JSResult, JSValue, Source, js_class, js_method,
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

fn fetch_failed(reason: &str) -> rong::RongJSError {
    // Same shape as a transport failure from Rong's `fetch`.
    HostError::new(rong::error::E_IO, "fetch failed")
        .with_name("TypeError")
        .with_data(rong::err_data!({ detail: (format!("aborted by test route: {reason}")) }))
        .into()
}

// ----------------------------- driver side -----------------------------

#[js_class(clone)]
pub(crate) struct JSNetworkDriver {
    lxapp: Weak<LxApp>,
}

impl JSNetworkDriver {
    pub(crate) fn new(lxapp: &Arc<LxApp>) -> Self {
        Self {
            lxapp: Arc::downgrade(lxapp),
        }
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

    /// Requests this run's routes matched for the app, oldest first.
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

/// `{ status?, statusText?, headers?, body?, json?, contentType? }` fulfills;
/// `{ abort: 'failed' }` rejects like a network error; `{ continue: true }`
/// passes the request through.
fn parse_handler(handler: &JSObject) -> JSResult<RouteAction> {
    let json = handler
        .to_json_string()
        .map_err(|err| auto_err(format!("route handler must be JSON-compatible: {err}")))?;
    let value: Value = serde_json::from_str(&json)
        .map_err(|err| auto_err(format!("route handler must be JSON-compatible: {err}")))?;
    parse_handler_value(&value).map_err(auto_err)
}

fn parse_handler_value(value: &Value) -> Result<RouteAction, String> {
    let Value::Object(fields) = value else {
        return Err("route handler must be an object".into());
    };
    const KNOWN: [&str; 8] = [
        "status",
        "statusText",
        "headers",
        "body",
        "json",
        "contentType",
        "abort",
        "continue",
    ];
    if let Some(unknown) = fields.keys().find(|key| !KNOWN.contains(&key.as_str())) {
        return Err(format!("unknown route handler option '{unknown}'"));
    }
    let fulfill_keys = KNOWN[..6].iter().any(|key| fields.contains_key(*key));
    let abort = match fields.get("abort") {
        None | Some(Value::Null) | Some(Value::Bool(false)) => None,
        Some(Value::Bool(true)) => Some("failed".to_string()),
        Some(Value::String(reason)) if !reason.is_empty() => Some(reason.clone()),
        Some(_) => return Err("route abort must be a non-empty string such as 'failed'".into()),
    };
    let pass = matches!(fields.get("continue"), Some(Value::Bool(true)));
    match (abort, pass, fulfill_keys) {
        (Some(_), true, _) | (Some(_), _, true) | (_, true, true) => {
            Err("route handler must choose one of fulfill, abort, or continue".into())
        }
        (Some(reason), false, false) => Ok(RouteAction::Abort(reason)),
        (None, true, false) => Ok(RouteAction::Continue),
        (None, false, _) => parse_fulfill(fields).map(RouteAction::Fulfill),
    }
}

fn parse_fulfill(fields: &serde_json::Map<String, Value>) -> Result<Fulfill, String> {
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
    let (body, implied_type) = match (fields.get("json"), fields.get("body")) {
        (Some(_), Some(_)) => {
            return Err("route handler takes either body or json, not both".into());
        }
        (Some(json), None) => (Some(json.to_string()), Some("application/json")),
        (None, None) | (None, Some(Value::Null)) => (None, None),
        (None, Some(Value::String(text))) => (Some(text.clone()), None),
        // A non-string body is shorthand for `json`.
        (None, Some(other)) => (Some(other.to_string()), Some("application/json")),
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
    Ok(Fulfill {
        status,
        status_text,
        headers,
        body: body.filter(|body| !body.is_empty() || !matches!(status, 204 | 205 | 304)),
    })
}

// ------------------------------ Logic side ------------------------------

/// Wraps the global `fetch` of a context. `active()` is the no-route fast
/// path; `decide(method, url)` returns `undefined` to pass through, a
/// fulfillment record, or throws the abort error.
const FETCH_INTERCEPTOR: &str = r#"(function (originalFetch, active, decide) {
  'use strict';
  const ResponseCtor = globalThis.Response;
  const RequestCtor = globalThis.Request;
  const fetch = function fetch(input, init) {
    if (!active()) return originalFetch.apply(this, arguments);
    let method, url;
    try {
      const request = typeof RequestCtor === 'function' && input instanceof RequestCtor ? input : null;
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
      hit = decide(method, url);
    } catch (error) {
      return Promise.reject(error);
    }
    if (hit === undefined || hit === null) return originalFetch.apply(this, arguments);
    const signal = init && init.signal;
    if (signal && signal.aborted) return Promise.reject(signal.reason);
    try {
      const response = new ResponseCtor(hit.body, {
        status: hit.status,
        statusText: hit.statusText,
        headers: hit.headers,
      });
      try { Object.defineProperty(response, 'url', { value: url, enumerable: true }); } catch (_) {}
      return Promise.resolve(response);
    } catch (error) {
      return Promise.reject(error);
    }
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
        move |ctx: JSContext, method: String, url: String| -> JSResult<JSValue> {
            let Some(target) = resolve(&ctx) else {
                return Ok(JSValue::undefined(&ctx));
            };
            let host = url
                .parse::<http::Uri>()
                .ok()
                .and_then(|uri| uri.host().map(str::to_string));
            let allowed = || host.as_deref().is_some_and(|host| (target.allowed)(host));
            let action = registry::with_registry(|routes| {
                routes.decide(&target.appid, &method, &url, allowed)
            });
            match action {
                None | Some(RouteAction::Continue) => Ok(JSValue::undefined(&ctx)),
                Some(RouteAction::Abort(reason)) => Err(fetch_failed(&reason)),
                Some(RouteAction::Fulfill(fulfill)) => fulfillment_js(&ctx, fulfill),
            }
        },
    )?;
    let installer = ctx.eval::<JSFunc>(Source::from_bytes(FETCH_INTERCEPTOR))?;
    installer.call::<_, ()>(None, (original, active, decide))
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
        Some(body) => object.set("body", body)?,
        None => object.set("body", JSValue::null(ctx))?,
    };
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
