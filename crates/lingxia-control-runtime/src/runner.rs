//! `runner.*` methods: the simulated environment (device preset, orientation,
//! appearance, display language) owned by the runner host.

use lingxia_control_protocol::methods;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Mutex;

/// The session's display-language override, from the launch environment or
/// `runner.set`. Installing is a takeover, so only the latest owner matters.
static SESSION_LANGUAGE: Mutex<Option<lxapp::DisplayLanguageSessionOwner>> = Mutex::new(None);

/// Shadow the display language for this dev session.
pub(crate) fn set_session_language(preference: lxapp::DisplayLanguagePreference) {
    let owner = lxapp::install_display_language_session_override(preference);
    if let Ok(mut slot) = SESSION_LANGUAGE.lock() {
        *slot = Some(owner);
    }
}

/// Drop this dev session's display-language override, if any.
pub(crate) fn clear_session_language() {
    let owner = SESSION_LANGUAGE
        .lock()
        .ok()
        .and_then(|mut slot| slot.take());
    if let Some(owner) = owner {
        lxapp::clear_display_language_session_override(owner);
    }
}

pub(crate) fn handle_runner_command(
    handler: &str,
    args: Option<Value>,
) -> Option<Result<Option<Value>, String>> {
    if !handler.starts_with("runner.") {
        return None;
    }
    Some(handle_runner_command_impl(handler, args))
}

#[derive(Deserialize)]
struct RunnerSetArgs {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    landscape: Option<bool>,
    #[serde(default)]
    appearance: Option<String>,
    #[serde(default)]
    language: Option<String>,
}

/// The device state with the effective display language added.
fn with_language(state: Value) -> Value {
    let mut state = state;
    if let Value::Object(map) = &mut state {
        map.insert(
            "language".to_string(),
            Value::String(lxapp::display_language()),
        );
    }
    state
}

fn handle_runner_command_impl(handler: &str, args: Option<Value>) -> Result<Option<Value>, String> {
    match handler {
        methods::runner::PRESETS => serde_json::to_value(lingxia::dev::device_list()?)
            .map(Some)
            .map_err(|err| err.to_string()),
        methods::runner::GET => serde_json::to_value(lingxia::dev::device_get()?)
            .map(|state| Some(with_language(state)))
            .map_err(|err| err.to_string()),
        methods::runner::SET => {
            let parsed: RunnerSetArgs = match args {
                Some(value) => serde_json::from_value(value)
                    .map_err(|e| format!("invalid args for {}: {}", handler, e))?,
                None => RunnerSetArgs {
                    id: None,
                    landscape: None,
                    appearance: None,
                    language: None,
                },
            };
            if parsed.id.is_none()
                && parsed.landscape.is_none()
                && parsed.appearance.is_none()
                && parsed.language.is_none()
            {
                return Err(
                    "runner.set requires at least one of id, landscape, appearance, language"
                        .into(),
                );
            }
            let appearance = parsed
                .appearance
                .as_deref()
                .map(str::parse::<lingxia::dev::Appearance>)
                .transpose()?;
            let language = parsed
                .language
                .as_deref()
                .map(str::parse::<lxapp::DisplayLanguagePreference>)
                .transpose()?;
            let device_changes =
                parsed.id.is_some() || parsed.landscape.is_some() || appearance.is_some();
            let state = if device_changes {
                crate::util::run_async(lingxia::dev::device_set(
                    parsed.id.as_deref(),
                    parsed.landscape,
                    appearance,
                ))?
            } else {
                lingxia::dev::device_get()?
            };
            if let Some(language) = language {
                set_session_language(language);
            }
            serde_json::to_value(state)
                .map(|state| Some(with_language(state)))
                .map_err(|err| err.to_string())
        }
        other => Err(format!("unknown runner handler: {}", other)),
    }
}
