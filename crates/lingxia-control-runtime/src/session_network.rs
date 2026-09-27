//! `session.network.*`: dev-session mocks, network scenarios and
//! recordings (the HTTP half of `lxdev mock …`, and `lxdev network …`) over
//! the automation runtime's route table.

use lingxia_automation::runtime::network;
use lingxia_control_protocol::methods::session::network as method;
use lingxia_control_protocol::mock::{DEV_OWNER, MockMode};
use serde_json::{Value, json};

pub(crate) fn handle(handler: &str, args: Option<Value>) -> Result<Option<Value>, String> {
    let args = args.unwrap_or(Value::Null);
    let text = |key: &str| {
        args.get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };
    match handler {
        method::SCENARIO_USE => {
            let scenario = args
                .get("scenario")
                .ok_or_else(|| "(usage): scenario is required".to_string())?;
            let explicit = text("appid");
            let dry_run = args.get("dryRun").and_then(Value::as_bool) == Some(true);
            let appid = if dry_run {
                explicit.clone().unwrap_or_default()
            } else {
                target_appid(explicit.clone())?
            };
            let mut status = network::use_scenario(
                &appid,
                scenario,
                text("variant").as_deref(),
                text("source").as_deref(),
                dry_run,
            )
            .map_err(|err| format!("(usage): invalid scenario: {err}"))?;
            if let Some(warning) = explicit.and_then(|appid| unknown_app_warning(&appid)) {
                status["warning"] = json!(warning);
            }
            Ok(Some(status))
        }
        method::SCENARIO_CLEAR => Ok(Some(json!({ "cleared": network::clear_scenario() }))),
        method::STATUS => Ok(Some(network::status())),
        method::RECORD_START => {
            let appid = match text("appid") {
                Some(appid) => Some(appid),
                None => target_appid(None).ok(),
            };
            network::record_start(appid.as_deref(), text("match").as_deref())
                .map_err(|err| format!("(usage): {err}"))?;
            let mut status = network::status();
            if let Some(warning) = text("appid").and_then(|appid| unknown_app_warning(&appid)) {
                status["warning"] = json!(warning);
            }
            Ok(Some(status))
        }
        method::RECORD_STOP => network::record_stop(text("name").as_deref())
            .map(Some)
            .map_err(|err| format!("(usage): {err}")),
        method::MOCK_LOAD => {
            let appid = text("appid").ok_or("(usage): appid is required")?;
            let source = args
                .get("source")
                .and_then(Value::as_str)
                .ok_or("(usage): source is required")?;
            let keys: Vec<String> = serde_json::from_value(args["keys"].clone())
                .map_err(|_| "(usage): keys must be an array of handler keys".to_string())?;
            let baseline = mode_arg(&args, "baseline")?;
            let config = args.get("config").filter(|config| !config.is_null());
            network::mock_load(&appid, source, &keys, config, baseline)
                .map(Some)
                .map_err(|err| format!("(usage): {err}"))
        }
        method::MOCK_UNLOAD => {
            let appid = text("appid").ok_or("(usage): appid is required")?;
            Ok(Some(network::mock_unload(&appid)))
        }
        method::MOCK_SET => {
            if let Some(owner) = text("owner")
                && owner != DEV_OWNER
            {
                return Err(format!(
                    "(usage): a dev session sets the '{DEV_OWNER}' owner, not '{owner}'"
                ));
            }
            let mode =
                mode_arg(&args, "mode")?.ok_or("(usage): mode must be \"all\" or \"none\"")?;
            let targets: Vec<String> = match args.get("targets") {
                None | Some(Value::Null) => Vec::new(),
                Some(targets) => serde_json::from_value(targets.clone())
                    .map_err(|_| "(usage): targets must be an array of strings".to_string())?,
            };
            network::mock_set(mode, targets)
                .map(Some)
                .map_err(|err| format!("(usage): {err}"))
        }
        method::MOCK_RESET => Ok(Some(network::mock_reset(
            text("owner").as_deref() == Some(DEV_OWNER),
        ))),
        method::MOCK_STATUS => Ok(Some(network::mock_status())),
        other => Err(format!("(usage): unknown network method {other}")),
    }
}

/// `"all"` / `"none"` under `key`, `None` when absent.
fn mode_arg(args: &Value, key: &str) -> Result<Option<MockMode>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => MockMode::parse(text)
            .map(Some)
            .ok_or_else(|| format!("(usage): {key} must be \"all\" or \"none\", got \"{text}\"")),
        Some(other) => Err(format!(
            "(usage): {key} must be \"all\" or \"none\", got {other}"
        )),
    }
}

/// The app a dev scenario applies to: the one named, the home lxapp, or the
/// current one.
fn target_appid(explicit: Option<String>) -> Result<String, String> {
    if let Some(appid) = explicit {
        return Ok(appid);
    }
    if let Some(home) = lingxia_app_context::home_app_id() {
        return Ok(home.to_string());
    }
    let (current, _, _) = lxapp::get_current_lxapp();
    if current.is_empty() {
        Err("(unavailable): no lxapp is running; open one or pass --appid".to_string())
    } else {
        Ok(current)
    }
}

/// A warning when `appid` names no running, installed or bundled lxapp: a
/// typo there would otherwise leave the scenario silently unused.
fn unknown_app_warning(appid: &str) -> Option<String> {
    let known = lxapp::try_get(appid).is_some()
        || lxapp::installed_lxapp_path(appid, lxapp::default_channel()).is_some()
        || lxapp::bundled_lxapp_asset_available(appid);
    unknown_app_message(appid, known)
}

fn unknown_app_message(appid: &str, known: bool) -> Option<String> {
    if known {
        return None;
    }
    let message = format!(
        "lxapp '{appid}' is not running or installed in this host; \
         nothing is affected until an app with that id opens"
    );
    log::warn!("{message}");
    Some(message)
}

/// The dev session that installed a scenario disconnected.
pub(crate) fn session_ended() {
    network::session_ended();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The route table is process-wide; these tests take turns.
    fn serial() -> std::sync::MutexGuard<'static, ()> {
        static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
        SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn a_dev_scenario_reports_why_it_stopped_answering() {
        let _serial = serial();
        let appid = "control.runtime.scenario.test";
        let scenario = |rules: Value| {
            Some(json!({
                "scenario": {
                    "name": "Wi-Fi",
                    "rules": rules,
                    "variants": { "offline": { "rules": [{ "http": "GET https://h.invalid/wifi", "status": 503 }] } }
                },
                "variant": "offline",
                "source": "wifi",
                "appid": appid,
            }))
        };
        let status = handle(
            method::SCENARIO_USE,
            scenario(json!([
                { "http": "* https://h.invalid/**", "status": 200 },
                { "function": "orders.submit", "fault": "unknown" }
            ])),
        )
        .unwrap()
        .unwrap();
        assert_eq!(status["active"], true);
        assert_eq!(status["scenario"]["source"], "wifi");
        assert_eq!(status["scenario"]["label"], "Wi-Fi:offline");
        let rules = status["scenario"]["rules"].as_array().unwrap();
        assert_eq!(rules.len(), 3);
        assert_eq!(rules[0]["target"], "GET https://h.invalid/wifi");
        assert_eq!(rules[0]["hits"], 0);
        assert_eq!(rules[2]["kind"], "function");
        assert!(rules[2].get("hits").is_none());

        // An invalid file leaves the active scenario answering, and a dry
        // run installs nothing.
        let err = handle(
            method::SCENARIO_USE,
            scenario(json!([{ "http": "GET x", "stauts": 1 }])),
        )
        .unwrap_err();
        assert!(
            err.contains("rules[0]: unknown route handler option 'stauts'"),
            "{err}"
        );
        let mut dry = scenario(json!([])).unwrap();
        dry["dryRun"] = json!(true);
        dry["scenario"]["name"] = json!("other");
        let checked = handle(method::SCENARIO_USE, Some(dry)).unwrap().unwrap();
        assert_eq!(checked["valid"], true);
        let status = handle(method::STATUS, None).unwrap().unwrap();
        assert_eq!(status["active"], true);
        assert_eq!(status["scenario"]["label"], "Wi-Fi:offline");

        session_ended();
        let status = handle(method::STATUS, None).unwrap().unwrap();
        assert_eq!(status["active"], false);
        assert_eq!(status["lastCleared"]["reason"], "session_ended");
        assert_eq!(status["lastCleared"]["source"], "wifi");
        assert_eq!(status["lastCleared"]["label"], "Wi-Fi:offline");
        assert!(status["lastCleared"]["clearedAt"].is_string());
    }

    #[test]
    fn mocks_load_select_reset_and_report() {
        let _serial = serial();
        let appid = "control.runtime.mock.test";
        let load = |keys: Value, config: Value| {
            handle(
                method::MOCK_LOAD,
                Some(json!({
                    "appid": appid,
                    "source": "({ 'GET **/devices': { json: [] } })",
                    "keys": keys,
                    "config": config,
                })),
            )
        };
        let loaded = load(json!(["GET **/devices"]), json!({ "mock": "all" }))
            .unwrap()
            .unwrap();
        assert_eq!(loaded["handlers"], 1);
        let first = loaded["generation"].as_u64().unwrap();
        // An invalid load keeps the previous one.
        let err = load(json!(["GET **/devices"]), json!({ "targets": [] })).unwrap_err();
        assert_eq!(
            err,
            "(usage): mocks/config.json: unknown key \"targets\"; the keys are mock and overrides"
        );
        let err = load(json!(["**/devices"]), Value::Null).unwrap_err();
        assert!(err.contains("is not a target"), "{err}");

        let set = handle(
            method::MOCK_SET,
            Some(json!({ "mode": "none", "targets": ["GET **/devices"] })),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            set["entries"],
            json!([{ "mode": "none", "targets": ["GET **/devices"] }])
        );
        let err = handle(
            method::MOCK_SET,
            Some(json!({ "owner": "test:x", "mode": "all" })),
        )
        .unwrap_err();
        assert!(err.contains("sets the 'dev' owner"), "{err}");
        let err = handle(method::MOCK_SET, Some(json!({ "mode": "on" }))).unwrap_err();
        assert!(err.contains("mode must be"), "{err}");

        let status = handle(method::MOCK_STATUS, None).unwrap().unwrap();
        let app = status["apps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|app| app["appid"] == appid)
            .unwrap()
            .clone();
        assert_eq!(app["handlers"], 1);
        assert_eq!(app["generation"].as_u64(), Some(first));
        assert_eq!(
            app["selection"],
            "all — from mocks/config.json · live: none 'GET **/devices'; lxdev mock reset to return"
        );
        assert_eq!(
            status["live"],
            json!([{ "mode": "none", "targets": ["GET **/devices"] }])
        );

        let reset = handle(method::MOCK_RESET, Some(json!({ "owner": "dev" })))
            .unwrap()
            .unwrap();
        assert!(reset["generation"].as_u64().unwrap() > first);
        let status = handle(method::MOCK_STATUS, None).unwrap().unwrap();
        assert_eq!(status["live"], json!([]));
    }

    #[test]
    fn an_unknown_appid_is_warned_about() {
        assert_eq!(unknown_app_message("com.example.app", true), None);
        let warning = unknown_app_message("com.exmaple.app", false).unwrap();
        assert!(warning.contains("'com.exmaple.app'"), "{warning}");
        assert_eq!(
            unknown_app_warning("no.such.app.for.this.test"),
            Some(
                "lxapp 'no.such.app.for.this.test' is not running or installed in this host; \
             nothing is affected until an app with that id opens"
                    .to_string()
            )
        );
    }
}
