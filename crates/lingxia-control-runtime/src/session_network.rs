//! `session.network.*`: dev-session network scenarios and recordings
//! (`lxdev network …`) over the automation runtime's route table.

use lingxia_automation::runtime::network;
use lingxia_control_protocol::methods::session::network as method;
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
            let appid = target_appid(explicit.clone())?;
            let mut status = network::use_scenario(&appid, scenario, text("source").as_deref())
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
        other => Err(format!("(usage): unknown network method {other}")),
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
