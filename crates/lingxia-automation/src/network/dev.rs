//! Network scenarios and recordings driven by a dev session
//! (`lxdev network …`) while no test runs, and the recording and call log a
//! host automation run reads through its host object.
//!
//! A dev scenario is impossible to miss: installing it, and every request it
//! answers, logs a warning to the session log, and it stands aside while a
//! host automation run is active so it never steers a test.

use super::capture::{self, REPORT_CALLS, Recording};
use super::registry::{self, DEV_SESSION_OWNER, DevScenario, RouteAction, now_ms};
use super::scenario;
use lingxia_log::{LogBuilder, LogLevel, LogTag};
use serde_json::{Value, json};

pub(crate) fn warn(appid: &str, message: String) {
    LogBuilder::new(LogTag::Native, message)
        .with_appid(appid.to_string())
        .with_level(LogLevel::Warn);
}

/// `'name' (file)` for log lines.
pub(crate) fn label(dev: &DevScenario) -> String {
    let name = dev.name.as_deref().unwrap_or("unnamed");
    match &dev.source {
        Some(source) => format!("'{name}' ({source})"),
        None => format!("'{name}'"),
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

/// Install `scenario` for `appid` until [`clear_scenario`] or the session
/// ends, replacing a scenario installed earlier. Returns [`status`].
pub fn use_scenario(appid: &str, scenario: &Value, source: Option<&str>) -> Result<Value, String> {
    let parsed = scenario::parse_scenario(scenario)?;
    let count = parsed.routes.len();
    let dev = registry::with_registry(|routes| {
        routes.clear_run(DEV_SESSION_OWNER);
        let installed = routes.install_all(DEV_SESSION_OWNER, appid, parsed.routes, || true)?;
        let dev = DevScenario {
            name: parsed.name,
            source: source.map(str::to_string),
            appid: appid.to_string(),
            route_ids: installed.iter().map(|(id, _)| *id).collect(),
            installed_ms: now_ms(),
        };
        routes.dev = Some(dev.clone());
        Ok::<_, String>(dev)
    })?;
    warn(
        appid,
        format!(
            "dev network scenario {} is ACTIVE: {count} route{} answer this app's Logic fetch \
             until `lxdev network scenario clear` or the dev session ends",
            label(&dev),
            if count == 1 { "" } else { "s" }
        ),
    );
    Ok(status())
}

/// Remove the dev scenario. Returns whether one was installed.
pub fn clear_scenario() -> bool {
    let cleared = registry::with_registry(|routes| {
        let dev = routes.dev.clone();
        routes.clear_run(DEV_SESSION_OWNER);
        dev
    });
    if let Some(dev) = &cleared {
        warn(
            &dev.appid,
            format!("dev network scenario {} cleared", label(dev)),
        );
    }
    cleared.is_some()
}

/// The active dev scenario and recording, for `lxdev network … status`.
pub fn status() -> Value {
    registry::with_registry(|routes| {
        let scenario = routes.dev.as_ref().map(|dev| {
            let entries: Vec<Value> = dev
                .route_ids
                .iter()
                .map(|id| {
                    let remaining = routes.remaining(DEV_SESSION_OWNER, *id);
                    json!({
                        "id": id,
                        "installed": remaining.is_some(),
                        "timesLeft": remaining.flatten(),
                        "hits": routes.hits(DEV_SESSION_OWNER, *id),
                    })
                })
                .collect();
            let patterns = routes.requests(DEV_SESSION_OWNER, &dev.appid);
            json!({
                "name": dev.name,
                "source": dev.source,
                "appid": dev.appid,
                "installedAt": scenario::iso_utc(dev.installed_ms as i64),
                "routes": entries,
                "requests": patterns.len(),
                "lastRequests": patterns
                    .iter()
                    .rev()
                    .take(10)
                    .rev()
                    .map(|entry| json!({
                        "method": entry.method,
                        "url": capture::redact_url(&entry.url, capture::REDACTED),
                        "pattern": entry.pattern,
                        "action": entry.action,
                        "status": entry.status,
                        "time": scenario::iso_utc(entry.timestamp_ms as i64),
                    }))
                    .collect::<Vec<_>>(),
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
            // Dev routes stand aside while a test run is active.
            "suspended": scenario.is_some() && routes.runs_active(),
            "scenario": scenario,
            "recording": recording,
        })
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
    let had_scenario = clear_scenario();
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
