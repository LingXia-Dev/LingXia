//! `lx.automation().lxapp().mock`: what a host run controls of the app's
//! mocks.
//!
//! - `use(file, variant?)` installs a scenario file. Its `http` rules
//!   become routes of the run; its `function` rules go to the dev session's
//!   companion under the run's owner (`test:<run>`), above any dev scenario
//!   there. A run holds one product scenario: installing another replaces
//!   both halves, and the run's end removes it.
//! - `reset()` starts the app's mock handler state over (each spec starts
//!   fresh), and asks a companion that switches mocks to do the same for
//!   the run's owner.

use super::companion::{self, UpstreamError};
use super::registry::{self, InstalledScenario, ScenarioCall};
use super::{auto_err, run_scope, scenario};
use crate::resolve::{json_to_js, upgrade_authorized};
use lingxia_control_protocol::methods::session::companion as method;
use lingxia_control_protocol::scenario::{self as format, Resolved, Target, companion as protocol};
use lxapp::LxApp;
use rong::{Class, HostError, JSContext, JSObject, JSResult, JSValue, function::Optional};
use rong::{js_class, js_method};
use serde_json::{Value, json};
use std::sync::Weak;

/// `lx.automation().lxapp().mock`.
#[js_class(clone)]
pub(crate) struct JSMockDriver {
    lxapp: Weak<LxApp>,
}

impl JSMockDriver {
    /// Authorization is checked per call, so reading `.mock` never throws.
    pub(crate) fn new(lxapp: Weak<LxApp>) -> Self {
        Self { lxapp }
    }
}

#[js_class(rename = "MockDriver")]
impl JSMockDriver {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(
            rong::error::E_ILLEGAL_CONSTRUCTOR,
            "Use lx.automation().lxapp().mock",
        )
        .into())
    }

    /// Install a scenario file (a `variant` of it) for this lxapp in the
    /// host run: its `http` rules answer Logic `fetch` before the mock
    /// selection, its `function` rules go to the dev session's companion.
    /// Replaces the scenario the run installed before; the
    /// run's end removes it.
    #[js_method(rename = "use")]
    async fn use_scenario(
        &self,
        ctx: JSContext,
        definition: JSObject,
        variant: Optional<JSValue>,
    ) -> JSResult<JSObject> {
        // `undefined` and `null` mean no variant, not the text "undefined".
        let variant = match variant.0 {
            Some(value) if value.is_string() => Some(value.to_rust::<String>()?),
            Some(value) if !value.is_undefined() && !value.is_null() => {
                return Err(auto_err("scenario variant must be a string"));
            }
            _ => None,
        };
        install(ctx, &self.lxapp, definition, variant).await
    }

    /// Start the app's mock handler state over: the next intercepted call
    /// evaluates `mocks/index.ts` again. A companion that switches mocks is
    /// asked to do the same for the run's owner. Resolves
    /// `{ generation, function: { reset, reason? } | null }`.
    #[js_method]
    async fn reset(&self, ctx: JSContext) -> JSResult<JSValue> {
        let app = upgrade_authorized(&ctx, &self.lxapp)?;
        let scope = run_scope(&ctx)?;
        let appid = app.appid.clone();
        let generation = registry::with_registry(|routes| {
            routes.release_mock_holds(Some(&appid));
            routes
                .mocks
                .reset(Some(&appid), super::mocks::Fresh::Spec)
                .first()
                .map(|(_, generation)| *generation)
        });
        let function = if companion::upstream().is_some() {
            let owner = format::test_owner(&scope.run_id);
            match companion::request(method::MOCK_RESET, json!({ "owner": owner })).await {
                Ok(result) => Some(result),
                // No companion, or one that does not switch mocks.
                Err(UpstreamError { code, .. }) if code == method::UNSUPPORTED => None,
                Err(UpstreamError { message, .. }) => {
                    Some(json!({ "reset": false, "reason": message }))
                }
            }
        } else {
            None
        };
        json_to_js(
            &ctx,
            &json!({ "generation": generation, "function": function }),
        )
    }
}

/// Install `definition` with `variant` for the app, replacing the scenario
/// this run installed before.
pub(crate) async fn install(
    ctx: JSContext,
    lxapp: &Weak<LxApp>,
    definition: JSObject,
    variant: Option<String>,
) -> JSResult<JSObject> {
    let app = upgrade_authorized(&ctx, lxapp)?;
    let scope = run_scope(&ctx)?;
    let json = definition
        .to_json_string()
        .map_err(|err| auto_err(format!("scenario must be JSON-compatible: {err}")))?;
    let value: Value = serde_json::from_str(&json)
        .map_err(|err| auto_err(format!("scenario must be JSON-compatible: {err}")))?;
    let variant = variant.filter(|variant| !variant.is_empty());
    let parsed = scenario::parse_scenario(&value, variant.as_deref())
        .map_err(|err| auto_err(format!("scenario: {err}")))?;
    let owner = format::test_owner(&scope.run_id);
    let appid = app.appid.clone();
    let label = parsed.resolved.label("scenario");

    // One product scenario per run, including its shared companion half.
    let _guard = scope.scenario_lock.lock().await;
    let attempt = (scope.admit)().map_err(auto_err)?;
    let previous = registry::with_registry(|routes| {
        routes
            .run_scenarios
            .iter()
            .find(|scenario| scenario.owner == scope.run_id)
            .cloned()
    });
    let functions = parsed.has_function_rules();
    registry::with_registry(|routes| {
        routes.scenario_pending.insert(scope.run_id.clone());
    });
    let update = if functions {
        use_functions(&owner, &parsed.resolved).await
    } else if previous.as_ref().is_some_and(|previous| previous.companion) {
        clear_functions_checked(&owner)
            .await
            .map_err(FunctionChangeError::Unknown)
    } else {
        Ok(())
    };
    if let Err(error) = update {
        let error = match error {
            FunctionChangeError::Rejected(message) => {
                registry::with_registry(|routes| {
                    routes.scenario_pending.remove(&scope.run_id);
                });
                return Err(auto_err(message));
            }
            FunctionChangeError::Unknown(message) => message,
        };
        // A transport timeout may mean the companion applied the change. Keep
        // the old HTTP rules, but never let this context continue on that guess.
        (scope.revoke)(format!("scenario replacement did not complete: {error}"));
        return Err(HostError::new("E_SCENARIO_STATE_UNKNOWN", error).into());
    }

    let mut slot = super::dev::installed(&parsed, None);
    slot.companion = functions;
    let installed = registry::with_registry(|routes| {
        routes.install_scenario_admitted(&scope.run_id, &appid, slot, parsed.http, || {
            let current = (scope.admit)()?;
            if current != attempt {
                return Err("scenario attempt ended during installation".into());
            }
            Ok(current)
        })
    });
    let installed = match installed {
        Ok(installed) => installed,
        Err(err) => {
            (scope.revoke)(format!("scenario installation did not complete: {err}"));
            if functions {
                clear_functions(&owner).await;
            }
            return Err(HostError::new("E_SCENARIO_STATE_UNKNOWN", err).into());
        }
    };
    Ok(Class::lookup::<JSScenario>(&ctx)?.instance(JSScenario {
        id: installed.id,
        run_id: scope.run_id,
        appid,
        owner,
        label,
        resolved: parsed.resolved,
        snapshot: installed,
    }))
}

#[derive(Debug)]
enum FunctionChangeError {
    Rejected(String),
    Unknown(String),
}

impl FunctionChangeError {
    fn from_upstream(resolved: &Resolved, error: UpstreamError) -> Self {
        let message = if error.code == method::UNSUPPORTED {
            resolved.functions_unsupported(&error.message)
        } else {
            resolved.companion_error(&error.code, &error.message, error.data.as_ref())
        };
        let message = format!("scenario: {message}");
        // These protocol rejections guarantee the previous rules still answer.
        // A disconnect, including `unavailable`, may follow an applied request.
        if error.code == method::UNSUPPORTED || error.code == "invalid_rules" {
            Self::Rejected(message)
        } else {
            Self::Unknown(message)
        }
    }
}

async fn use_functions(owner: &str, resolved: &Resolved) -> Result<(), FunctionChangeError> {
    if let Some(reason) = companion::unavailable() {
        return Err(FunctionChangeError::Rejected(format!(
            "scenario: {}",
            resolved.functions_unsupported(reason)
        )));
    }
    let params = serde_json::to_value(resolved.companion_use(owner, None))
        .map_err(|err| FunctionChangeError::Rejected(err.to_string()))?;
    companion::request(method::SCENARIO_USE, params)
        .await
        .map(|_| ())
        .map_err(|error| FunctionChangeError::from_upstream(resolved, error))
}

async fn clear_functions(owner: &str) {
    // A failure here only leaves the rules until the run ends, when the dev
    // server clears its owner.
    let _ = clear_functions_checked(owner).await;
}

/// Empty the owner's `function` rules in the companion. Emptied, not
/// cleared: the run's owner keeps a dev scenario's function rules aside
/// until the run ends, when the dev server clears it. A host without a dev
/// session, or a companion without scenarios, holds none.
pub(crate) async fn clear_functions_checked(owner: &str) -> Result<(), String> {
    let params = json!({ "owner": owner, "scenario": {}, "rules": [] });
    match companion::request(method::SCENARIO_USE, params).await {
        Ok(_) => Ok(()),
        Err(UpstreamError { code, .. }) if code == method::UNSUPPORTED => Ok(()),
        Err(UpstreamError { code, message, .. }) => Err(format!("{code}: {message}")),
    }
}

/// Handle returned by `mock.use()`.
#[js_class(clone)]
pub(crate) struct JSScenario {
    id: u64,
    run_id: String,
    appid: String,
    owner: String,
    label: String,
    resolved: Resolved,
    /// As installed; hit counts are read live while it is installed.
    snapshot: InstalledScenario,
}

impl JSScenario {
    fn owned(&self, ctx: &JSContext) -> JSResult<()> {
        if run_scope(ctx)?.run_id == self.run_id {
            Ok(())
        } else {
            Err(auto_err("this scenario belongs to another automation run"))
        }
    }

    fn current(&self) -> InstalledScenario {
        registry::with_registry(|routes| routes.scenario(self.id).cloned())
            .unwrap_or_else(|| self.snapshot.clone())
    }

    /// HTTP calls that reached the scenario, as the spec sees them.
    fn http_calls(&self, calls: &[ScenarioCall]) -> Vec<Value> {
        calls
            .iter()
            .map(|call| {
                let mut entry = json!({
                    "seq": call.seq,
                    "time": call.time_ms,
                    "kind": "http",
                    "method": call.method,
                    "url": call.url,
                    "rule": call.rule,
                    "status": call.status,
                    "answeredBy": call.answered_by.clone().unwrap_or_else(|| "real".to_string()),
                });
                if let Some(body) = &call.body {
                    entry["body"] = body.clone();
                }
                if let Some(no_match) = &call.no_match {
                    entry["noMatch"] = json!(no_match);
                }
                entry
            })
            .collect()
    }

    /// Function calls the companion saw since `since` with a `seq` above
    /// `after`, and how far its log's drops reach.
    async fn function_calls(&self, since: u64, after: u64) -> Result<(Vec<Value>, u64), String> {
        let params = json!({ "since": since, "owner": self.owner, "after": after });
        let result = companion::request(method::SCENARIO_CALLS, params)
            .await
            .map_err(|err| format!("scenario calls: {}", err.message))?;
        let calls: protocol::CallsResult =
            serde_json::from_value(result).map_err(|err| format!("scenario calls: {err}"))?;
        let dropped_through = calls.dropped_through;
        let list = calls
            .calls
            .into_iter()
            .filter(|call| {
                call.owner
                    .as_deref()
                    .is_none_or(|owner| owner == self.owner)
            })
            .map(|call| {
                let rule = call
                    .rule
                    .filter(|_| call.owner.is_some())
                    .and_then(|position| self.resolved.function_rule(position))
                    .map(|rule| rule.index);
                let mut entry = json!({
                    "seq": call.seq,
                    "time": call.time,
                    "kind": "function",
                    "function": call.function,
                    "rule": rule,
                    "outcome": call.outcome,
                    "answeredBy": match (rule, call.handler.as_deref()) {
                        (Some(index), _) => format!("rule {index} ({})", self.label),
                        // The companion's mock selection chose the handler.
                        (None, Some(handler)) => format!("function {handler}"),
                        (None, None) => "companion default".to_string(),
                    },
                });
                if let Some(args) = call.args {
                    entry["args"] = args;
                }
                if let Some(no_match) = call.no_match {
                    entry["noMatch"] = json!(no_match);
                }
                entry
            })
            .collect();
        Ok((list, dropped_through))
    }
}

/// `{ http: 'PATCH **/devices/*' }`, `{ function: 'orders.submit' }`, or
/// `{ rule: 2 }`.
enum CallFilter {
    Any,
    Http(Option<String>, super::registry::UrlMatcher),
    Function(String),
    Rule(usize),
}

impl CallFilter {
    fn parse(value: Option<JSValue>) -> JSResult<Self> {
        let Some(object) = value.and_then(JSValue::into_object) else {
            return Ok(Self::Any);
        };
        if let Some(target) = object.get_opt::<_, String>("http")? {
            let Target::Http { method, url } =
                format::parse_http_target(&target).map_err(auto_err)?
            else {
                unreachable!("an http target");
            };
            let matcher = scenario::parse_url_string(&url).map_err(auto_err)?;
            return Ok(Self::Http(method, matcher));
        }
        if let Some(name) = object.get_opt::<_, String>("function")? {
            return Ok(Self::Function(name));
        }
        if let Some(rule) = object.get_opt::<_, f64>("rule")? {
            return Ok(Self::Rule(rule as usize));
        }
        Ok(Self::Any)
    }

    fn keeps(&self, call: &Value) -> bool {
        match self {
            Self::Any => true,
            Self::Http(method, matcher) => {
                call["kind"] == "http"
                    && method
                        .as_deref()
                        .is_none_or(|method| call["method"].as_str() == Some(method))
                    && call["url"]
                        .as_str()
                        .is_some_and(|url| matcher.is_match(url))
            }
            Self::Function(name) => call["kind"] == "function" && call["function"] == *name,
            Self::Rule(rule) => call["rule"].as_u64() == Some(*rule as u64),
        }
    }

    fn wants_functions(&self) -> bool {
        !matches!(self, Self::Http(..))
    }
}

#[js_class(rename = "Scenario")]
impl JSScenario {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(
            rong::error::E_ILLEGAL_CONSTRUCTOR,
            "Use lx.automation().lxapp().mock.use()",
        )
        .into())
    }

    #[js_method(getter, enumerable)]
    fn name(&self) -> Option<String> {
        self.resolved.name.clone()
    }

    #[js_method(getter, enumerable)]
    fn variant(&self) -> Option<String> {
        self.resolved.variant.clone()
    }

    /// `{ index, target, kind, hits }` per rule, in precedence order. `hits`
    /// counts `http` rules; a `function` rule's is `null` (see `calls()`).
    #[js_method(getter, enumerable)]
    fn rules(&self, ctx: JSContext) -> JSResult<JSValue> {
        let current = self.current();
        let rules: Vec<Value> = current
            .rules
            .iter()
            .map(|rule| {
                json!({
                    "index": rule.index,
                    "target": rule.target,
                    "kind": rule.kind,
                    "hits": rule.route_id.map(|_| rule.hits),
                })
            })
            .collect();
        json_to_js(&ctx, &Value::Array(rules))
    }

    /// Calls that reached the scenario since it was installed, oldest
    /// first: HTTP requests its rules' targets matched (answered by a rule,
    /// or passed on when no `match` held), and Function calls the companion
    /// saw.
    #[js_method]
    async fn calls(&self, ctx: JSContext, filter: Optional<JSValue>) -> JSResult<JSValue> {
        self.owned(&ctx)?;
        let filter = CallFilter::parse(filter.0)?;
        let current = self.current();
        let calls: Vec<ScenarioCall> = current.calls.iter().cloned().collect();
        let mut out = self.http_calls(&calls);
        if current.companion && filter.wants_functions() {
            out.extend(
                self.function_calls(current.installed_ms, 0)
                    .await
                    .map_err(auto_err)?
                    .0,
            );
            out.sort_by_key(|call| call["time"].as_u64().unwrap_or_default());
        }
        out.retain(|call| filter.keeps(call));
        json_to_js(&ctx, &Value::Array(out))
    }

    /// `{ calls, droppedThrough }` for one target (`{ http }`,
    /// `{ function }` or `{ rule }`): its calls with a `seq` above `after`,
    /// oldest first, and the highest `seq` the log they come from dropped
    /// (0 when none). HTTP calls and Function calls count `seq` in separate
    /// logs (the scenario's, the companion's), and a target reads one.
    #[js_method(rename = "callsAfter")]
    async fn calls_after(&self, ctx: JSContext, target: JSValue, after: f64) -> JSResult<JSValue> {
        self.owned(&ctx)?;
        let filter = CallFilter::parse(Some(target))?;
        let after = if after.is_finite() && after > 0.0 {
            after as u64
        } else {
            0
        };
        let current = self.current();
        let functions = match &filter {
            CallFilter::Any => {
                return Err(auto_err(
                    "callsAfter needs { http }, { function } or { rule } as its target",
                ));
            }
            CallFilter::Http(..) => false,
            CallFilter::Function(_) => true,
            CallFilter::Rule(index) => current
                .rules
                .iter()
                .any(|rule| rule.index == *index && rule.kind == "function"),
        };
        let (mut calls, dropped_through) = if !functions {
            let fresh: Vec<ScenarioCall> = current
                .calls
                .iter()
                .filter(|call| call.seq > after)
                .cloned()
                .collect();
            (self.http_calls(&fresh), current.dropped_through())
        } else if current.companion {
            self.function_calls(current.installed_ms, after)
                .await
                .map_err(auto_err)?
        } else {
            (Vec::new(), 0)
        };
        calls.retain(|call| filter.keeps(call));
        json_to_js(
            &ctx,
            &json!({ "calls": calls, "droppedThrough": dropped_through }),
        )
    }

    /// Remove the scenario; resolves how many of its rules were still
    /// installed.
    #[js_method]
    async fn unroute(&self, ctx: JSContext) -> JSResult<u32> {
        self.owned(&ctx)?;
        let scope = run_scope(&ctx)?;
        let _guard = scope.scenario_lock.lock().await;
        let removed = registry::with_registry(|routes| {
            let installed = routes
                .scenario(self.id)
                .map(|scenario| scenario.owner.clone() == self.run_id)
                .unwrap_or(false);
            if !installed {
                return None;
            }
            routes.scenario_pending.insert(self.run_id.clone());
            let still = routes
                .scenario(self.id)
                .map(|scenario| {
                    scenario
                        .route_ids()
                        .filter(|id| routes.remaining(&self.run_id, *id).is_some())
                        .count()
                })
                .unwrap_or(0);
            routes
                .remove_run_scenario(&self.run_id, &self.appid)
                .map(|scenario| (scenario, still))
        });
        let Some((scenario, still)) = removed else {
            return Ok(0);
        };
        let functions = scenario
            .rules
            .iter()
            .filter(|rule| rule.kind == "function")
            .count();
        if scenario.companion
            && let Err(error) = clear_functions_checked(&self.owner).await
        {
            (scope.revoke)(format!("scenario removal did not complete: {error}"));
            return Err(HostError::new("E_SCENARIO_STATE_UNKNOWN", error).into());
        }
        registry::with_registry(|routes| {
            routes.scenario_pending.remove(&self.run_id);
        });
        Ok((still + functions) as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_atomic_protocol_rejections_preserve_the_previous_scenario() {
        let parsed = scenario::parse_scenario(
            &json!({ "rules": [{ "function": "orders.submit", "result": {} }] }),
            None,
        )
        .unwrap();
        for code in [
            method::UNSUPPORTED,
            "invalid_rules",
            "unavailable",
            "timeout",
            "internal_error",
        ] {
            let error = FunctionChangeError::from_upstream(
                &parsed.resolved,
                UpstreamError {
                    code: code.into(),
                    message: "refused".into(),
                    data: None,
                },
            );
            assert_eq!(
                matches!(error, FunctionChangeError::Rejected(_)),
                matches!(code, method::UNSUPPORTED | "invalid_rules"),
                "{code}: {error:?}"
            );
        }
    }
}
