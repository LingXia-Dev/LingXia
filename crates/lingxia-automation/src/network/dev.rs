//! Network scenarios and recordings driven by a dev session
//! (`lxdev scenario …`, `lxdev network record …`) while no test runs, and the recording and call log a
//! host automation run reads through its host object.
//!
//! A dev scenario is impossible to miss: installing it, and every request it
//! answers, logs a warning to the session log, and it stands aside while a
//! host automation run is active so it never steers a test.

use super::capture::{self, REPORT_CALLS, Recording};
use super::registry::{
    self, DEV_SESSION_OWNER, DevClearReason, InstalledScenario, Registry, RouteAction, RuleSlot,
    now_ms,
};
use super::scenario;
use lingxia_log::{LogBuilder, LogLevel, LogTag};
use serde_json::{Value, json};
use std::collections::VecDeque;

/// Calls `status` lists.
const STATUS_CALLS: usize = 20;

pub(crate) fn warn(appid: &str, message: String) {
    LogBuilder::new(LogTag::Native, message)
        .with_appid(appid.to_string())
        .with_level(LogLevel::Warn);
}

/// `'name:variant' (file)` for log lines.
pub(crate) fn label(dev: &InstalledScenario) -> String {
    let name = dev.label();
    match &dev.source {
        Some(source) if dev.name.is_some() => format!("'{name}' ({source})"),
        _ => format!("'{name}'"),
    }
}

pub(crate) fn describe(action: &RouteAction) -> String {
    match action {
        RouteAction::Fulfill(fulfill) => format!("fulfill {}", fulfill.status),
        RouteAction::Sse(_) => "an sse stream".into(),
        RouteAction::Abort(kind) => format!("abort ({})", kind.as_str()),
        RouteAction::Continue => "continue".into(),
        RouteAction::Patch(_) => "continue + patchJson".into(),
        RouteAction::Hang { .. } => "hang".into(),
    }
}

/// An installed scenario before the table assigns its id and owner.
pub(crate) fn installed(parsed: &scenario::Scenario, source: Option<&str>) -> InstalledScenario {
    InstalledScenario {
        id: 0,
        owner: String::new(),
        appid: String::new(),
        name: parsed.resolved.name.clone(),
        variant: parsed.resolved.variant.clone(),
        source: source.map(str::to_string),
        rules: parsed
            .resolved
            .rules
            .iter()
            .map(|rule| RuleSlot {
                index: rule.index,
                target: rule.target.label(),
                kind: rule.target.kind(),
                route_id: None,
                hits: 0,
            })
            .collect(),
        installed_ms: now_ms(),
        calls: VecDeque::new(),
        calls_total: 0,
        companion: false,
    }
}

/// Install the `http` rules of `scenario` (a whole scenario file, `variant`
/// applied) for `appid` until [`clear_scenario`] or the session ends,
/// replacing a scenario installed earlier. `function` rules are listed in
/// the status but served by the dev session's companion. With `dry_run`
/// the file is only validated. Returns [`status`].
pub fn use_scenario(
    appid: &str,
    scenario: &Value,
    variant: Option<&str>,
    source: Option<&str>,
    dry_run: bool,
) -> Result<Value, String> {
    let parsed = scenario::parse_scenario(scenario, variant)?;
    if dry_run {
        return Ok(json!({
            "valid": true,
            "rules": parsed.resolved.rules.len(),
            "http": parsed.http.len(),
        }));
    }
    let slot = installed(&parsed, source);
    let http = parsed.http.len();
    let functions = parsed.resolved.rules.len() - http;
    let (dev, replaced) = registry::with_registry(|routes| {
        let replaced = routes.end_dev_scenario(DevClearReason::Replaced, now_ms());
        let dev = routes.install_scenario(DEV_SESSION_OWNER, appid, slot, parsed.http, || true)?;
        routes.dev = Some(dev.clone());
        Ok::<_, String>((dev, replaced))
    })?;
    if let Some(previous) = &replaced {
        warn(
            &previous.appid,
            format!(
                "dev scenario {} replaced by {}",
                label(previous),
                label(&dev)
            ),
        );
    }
    let companion = if functions > 0 {
        format!(
            " ({functions} function rule{} answered by the companion)",
            plural(functions)
        )
    } else {
        String::new()
    };
    warn(
        appid,
        format!(
            "dev scenario {} is ACTIVE: {http} http rule{} answer this app's Logic fetch{companion} \
             until `lxdev scenario clear` or the dev session ends",
            label(&dev),
            plural(http)
        ),
    );
    Ok(status())
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// Remove the dev scenario (`lxdev scenario clear`). Returns whether one was
/// installed.
pub fn clear_scenario() -> bool {
    end_scenario(DevClearReason::Cleared)
}

fn end_scenario(reason: DevClearReason) -> bool {
    let cleared = registry::with_registry(|routes| routes.end_dev_scenario(reason, now_ms()));
    if let Some(dev) = &cleared {
        let why = match reason {
            DevClearReason::Cleared => "cleared",
            DevClearReason::Replaced => "replaced",
            DevClearReason::SessionEnded => "cleared: the dev session disconnected",
        };
        warn(&dev.appid, format!("dev scenario {} {why}", label(dev)));
    }
    cleared.is_some()
}

/// The active dev scenario and recording, for `lxdev scenario status` and
/// `lxdev network status`.
pub fn status() -> Value {
    registry::with_registry(|routes| status_of(routes))
}

/// Per-rule status of an installed scenario.
pub(crate) fn rules_json(scenario: &InstalledScenario, routes: &Registry) -> Vec<Value> {
    scenario
        .rules
        .iter()
        .map(|rule| {
            let mut entry = json!({
                "index": rule.index,
                "target": rule.target,
                "kind": rule.kind,
            });
            if let Some(id) = rule.route_id {
                let remaining = routes.remaining(&scenario.owner, id);
                entry["hits"] = json!(rule.hits);
                entry["installed"] = json!(remaining.is_some());
                entry["timesLeft"] = json!(remaining.flatten());
            }
            entry
        })
        .collect()
}

/// A scenario's calls for a report: URL credentials redacted, no bodies.
pub(crate) fn scenario_calls_json(scenario: &InstalledScenario, limit: usize) -> Vec<Value> {
    let skip = scenario.calls.len().saturating_sub(limit);
    scenario
        .calls
        .iter()
        .skip(skip)
        .map(|call| {
            let mut entry = json!({
                "time": scenario::iso_utc(call.time_ms as i64),
                "method": call.method,
                "url": capture::redact_url(&call.url, capture::REDACTED),
                "rule": call.rule,
                "action": call.action,
                "status": call.status,
            });
            if let Some(no_match) = &call.no_match {
                entry["noMatch"] = json!(no_match);
            }
            entry
        })
        .collect()
}

pub(crate) fn status_of(routes: &Registry) -> Value {
    let scenario = routes.dev.as_ref().map(|dev| {
        json!({
            "name": dev.name,
            "variant": dev.variant,
            "label": dev.label(),
            "source": dev.source,
            "appid": dev.appid,
            "installedAt": scenario::iso_utc(dev.installed_ms as i64),
            "installedMs": dev.installed_ms,
            "rules": rules_json(dev, routes),
            "requests": dev.calls_total,
            "lastRequests": scenario_calls_json(dev, 10),
        })
    });
    let last_cleared = routes.dev_cleared.as_ref().map(|cleared| {
        json!({
            "name": cleared.scenario.name,
            "variant": cleared.scenario.variant,
            "label": cleared.scenario.label(),
            "source": cleared.scenario.source,
            "appid": cleared.scenario.appid,
            "reason": cleared.reason.as_str(),
            "clearedAt": scenario::iso_utc(cleared.cleared_ms as i64),
        })
    });
    let recording = routes
        .dev_recording
        .as_ref()
        .or(routes.run_recording.as_ref())
        .map(|recording| {
            let dev = recording.owner == DEV_SESSION_OWNER;
            json!({
                "owner": if dev { "dev-session" } else { "test-run" },
                "appid": recording.appid,
                "match": recording.matcher.as_ref().map(|m| m.label()),
                "startedAt": scenario::iso_utc(recording.started_ms as i64),
                "exchanges": recording.exchanges.len(),
                // A dev recording pauses while a test run is active.
                "paused": dev && routes.runs_active(),
            })
        });
    json!({
        "active": scenario.is_some(),
        // Dev routes stand aside while a test run is active, and answer
        // again when it ends.
        "suspended": scenario.is_some() && routes.runs_active(),
        "scenario": scenario,
        "lastCleared": last_cleared,
        "recording": recording,
        "calls": routes.calls.recent(0, STATUS_CALLS, &[]),
    })
}

fn start_recording(owner: &str, appid: Option<&str>, matcher: Option<&str>) -> Result<(), String> {
    let matcher = matcher
        .filter(|glob| !glob.trim().is_empty())
        .map(scenario::parse_url_string)
        .transpose()?;
    registry::with_registry(|routes| {
        let slot = if owner == DEV_SESSION_OWNER {
            &mut routes.dev_recording
        } else {
            &mut routes.run_recording
        };
        if let Some(active) = slot {
            return Err(if active.owner == DEV_SESSION_OWNER {
                "a dev-session network recording is already running; stop it first \
                 (`lxdev network record stop`)"
                    .to_string()
            } else {
                "another test run is recording network traffic".to_string()
            });
        }
        *slot = Some(Recording::new(owner, appid.map(str::to_string), matcher));
        Ok(())
    })
}

fn stop_recording(owner: &str) -> Option<Recording> {
    registry::with_registry(|routes| {
        let slot = if owner == DEV_SESSION_OWNER {
            &mut routes.dev_recording
        } else {
            &mut routes.run_recording
        };
        if slot
            .as_ref()
            .is_some_and(|recording| recording.owner == owner)
        {
            slot.take()
        } else {
            None
        }
    })
}

/// Start capturing real Logic `fetch` traffic of `appid` (every app when
/// `None`), optionally only URLs matching a glob or `/regex/`.
pub fn record_start(appid: Option<&str>, matcher: Option<&str>) -> Result<(), String> {
    start_recording(DEV_SESSION_OWNER, appid, matcher)?;
    warn(
        appid.unwrap_or("*"),
        "dev network recording started: real Logic fetch responses are being captured".into(),
    );
    Ok(())
}

/// Stop the dev recording: `{ scenario, exchanges, dropped }`.
pub fn record_stop(name: Option<&str>) -> Result<Value, String> {
    let recording = stop_recording(DEV_SESSION_OWNER)
        .ok_or_else(|| "no dev-session network recording is running".to_string())?;
    let name = name.map_or_else(
        || {
            format!(
                "recorded {}",
                scenario::iso_utc(recording.started_ms as i64)
            )
        },
        str::to_string,
    );
    Ok(json!({
        "scenario": recording.to_scenario(&name),
        "exchanges": recording.exchanges.len(),
        "dropped": recording.dropped,
    }))
}

/// The dev session that owned the scenario went away.
pub fn session_ended() {
    let had_scenario = end_scenario(DevClearReason::SessionEnded);
    let had_recording = stop_recording(DEV_SESSION_OWNER).is_some();
    if had_scenario || had_recording {
        log::warn!("dev session ended: its network scenario and recording were removed");
    }
}

// ------------------------------ host runs ------------------------------

/// `networkRecord('start' | 'stop', name?)` for the test framework's
/// `--record-network`: the scenario on stop, with every `secrets` value
/// (the run's `--secret-arg` values) masked; `null` when nothing recorded.
/// A dev-session recording does not block it: that one pauses during runs.
pub(crate) fn run_record(
    run_id: &str,
    command: &str,
    name: Option<&str>,
    secrets: &[String],
) -> Result<Value, String> {
    match command {
        "start" => start_recording(run_id, None, None).map(|()| Value::Null),
        "stop" => Ok(stop_recording(run_id)
            .map(|recording| run_scenario(&recording, name.unwrap_or("recorded"), secrets))
            .unwrap_or(Value::Null)),
        other => Err(format!("unknown networkRecord command '{other}'")),
    }
}

/// The scenario a run recording amounts to, `secrets` masked everywhere.
pub(crate) fn run_scenario(recording: &Recording, name: &str, secrets: &[String]) -> Value {
    let mut scenario = recording.to_scenario(name);
    capture::mask_strings(&mut scenario, secrets);
    scenario
}

/// `networkLog(sinceMs, limit?)`: Logic `fetch` calls since a spec started,
/// newest `limit` (default 20), with `secrets` masked.
pub(crate) fn run_calls(since_ms: u64, limit: Option<usize>, secrets: &[String]) -> Value {
    let limit = limit.unwrap_or(REPORT_CALLS).min(capture::MAX_CALLS);
    Value::Array(registry::with_registry(|routes| {
        routes.calls.recent(since_ms, limit, secrets)
    }))
}
