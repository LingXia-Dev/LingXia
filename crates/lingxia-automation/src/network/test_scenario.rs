//! `lx.automation().lxapp().scenario(file, variant?)`: a scenario file
//! installed by a host run. Its `http` rules become routes of the run; its
//! `function` rules go to the dev session's companion under the run's
//! owner (`test:<run>`), above any dev scenario there. A run holds one
//! scenario per app: installing another replaces it, and the run's end
//! removes it.

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

/// Install `definition` with `variant` for the app, replacing the scenario
/// this run installed for it before.
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

    // The previous scenario of this run and app stops answering first, so a
    // failure below leaves nothing of either.
    let previous =
        registry::with_registry(|routes| routes.remove_run_scenario(&scope.run_id, &appid));
    let functions = parsed.has_function_rules();
    if functions {
        use_functions(&owner, &parsed.resolved)
            .await
            .map_err(auto_err)?;
    } else if previous.as_ref().is_some_and(|previous| previous.companion) {
        clear_functions(&owner).await;
    }

    let mut slot = super::dev::installed(&parsed, None);
    slot.companion = functions;
    let installed = registry::with_registry(|routes| {
        routes.install_scenario(&scope.run_id, &appid, slot, parsed.http, || {
            (scope.active)()
        })
    });
    let installed = match installed {
        Ok(installed) => installed,
        Err(err) => {
            if functions {
                clear_functions(&owner).await;
            }
            return Err(auto_err(err));
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

async fn use_functions(owner: &str, resolved: &Resolved) -> Result<(), String> {
    if let Some(reason) = companion::unavailable() {
        return Err(format!(
            "scenario: {}",
            resolved.functions_unsupported(reason)
        ));
    }
    let params =
        serde_json::to_value(resolved.companion_use(owner, None)).map_err(|err| err.to_string())?;
    match companion::request(method::SCENARIO_USE, params).await {
        Ok(_) => Ok(()),
        Err(UpstreamError { code, message, .. })
            if code == method::UNSUPPORTED || code == "unknown_method" =>
        {
            Err(format!(
                "scenario: {}",
                resolved.functions_unsupported(&message)
            ))
        }
        Err(UpstreamError {
            code,
            message,
            data,
        }) => Err(format!(
            "scenario: {}",
            resolved.companion_error(&code, &message, data.as_ref())
        )),
    }
}

async fn clear_functions(owner: &str) {
    // The dev server also clears a run's owner when the run ends; a failure
    // here only delays that.
    let _ = companion::request(method::SCENARIO_CLEAR, json!({ "owner": owner })).await;
}

/// Handle returned by `scenario()`.
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
                    "time": call.time_ms,
                    "kind": "http",
                    "method": call.method,
                    "url": call.url,
                    "rule": call.rule,
                    "status": call.status,
                    "answeredBy": match call.rule {
                        Some(index) => format!("rule {index} ({})", self.label),
                        None => "real".to_string(),
                    },
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

    async fn function_calls(&self, since: u64) -> Result<Vec<Value>, String> {
        let params = json!({ "since": since, "owner": self.owner });
        let result = companion::request(method::SCENARIO_CALLS, params)
            .await
            .map_err(|err| format!("scenario calls: {}", err.message))?;
        let calls: protocol::CallsResult =
            serde_json::from_value(result).map_err(|err| format!("scenario calls: {err}"))?;
        Ok(calls
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
                    "time": call.time,
                    "kind": "function",
                    "function": call.function,
                    "rule": rule,
                    "outcome": call.outcome,
                    "answeredBy": match rule {
                        Some(index) => format!("rule {index} ({})", self.label),
                        None => "companion default".to_string(),
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
            .collect())
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
            "Use lx.automation().lxapp().scenario()",
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
                self.function_calls(current.installed_ms)
                    .await
                    .map_err(auto_err)?,
            );
            out.sort_by_key(|call| call["time"].as_u64().unwrap_or_default());
        }
        out.retain(|call| filter.keeps(call));
        json_to_js(&ctx, &Value::Array(out))
    }

    /// Remove the scenario; resolves how many of its rules were still
    /// installed.
    #[js_method]
    async fn unroute(&self, ctx: JSContext) -> JSResult<u32> {
        self.owned(&ctx)?;
        let removed = registry::with_registry(|routes| {
            let installed = routes
                .scenario(self.id)
                .map(|scenario| scenario.owner.clone() == self.run_id)
                .unwrap_or(false);
            if !installed {
                return None;
            }
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
        if scenario.companion {
            clear_functions(&self.owner).await;
        }
        Ok((still + functions) as u32)
    }
}
