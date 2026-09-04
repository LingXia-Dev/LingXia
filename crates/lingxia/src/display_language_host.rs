use crate::host::{HostResult, StreamContext};
use lxapp::{DisplayLanguagePreference, DisplayLanguageState, LanguageTag};
use serde::Deserialize;
use std::sync::OnceLock;
use tokio::sync::broadcast;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SetPreferenceInput {
    preference: DisplayLanguagePreference,
}

fn state_channel() -> &'static broadcast::Sender<DisplayLanguageState> {
    static CHANNEL: OnceLock<broadcast::Sender<DisplayLanguageState>> = OnceLock::new();
    CHANNEL.get_or_init(|| broadcast::channel(16).0)
}

fn effective_channel() -> &'static broadcast::Sender<LanguageTag> {
    static CHANNEL: OnceLock<broadcast::Sender<LanguageTag>> = OnceLock::new();
    CHANNEL.get_or_init(|| broadcast::channel(16).0)
}

#[lingxia::framework_native("app.getDisplayLanguage", audience = "authenticated-read-only")]
fn get_display_language() -> HostResult<LanguageTag> {
    Ok(lxapp::display_language_state().effective)
}

#[lingxia::framework_native(
    "app.watchDisplayLanguage",
    stream,
    audience = "authenticated-read-only"
)]
async fn watch_display_language(mut stream: StreamContext<LanguageTag>) -> HostResult<()> {
    let mut receiver = effective_channel().subscribe();
    stream.send(lxapp::display_language_state().effective)?;
    loop {
        tokio::select! {
            _ = stream.canceled() => return Ok(()),
            received = receiver.recv() => match received {
                Ok(language) => stream.send(language)?,
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    stream.send(lxapp::display_language_state().effective)?;
                }
                Err(broadcast::error::RecvError::Closed) => return stream.end(()),
            }
        }
    }
}

#[lingxia::framework_native("app.getDisplayLanguageState", audience = "control-only")]
fn get_display_language_state() -> HostResult<DisplayLanguageState> {
    Ok(lxapp::display_language_state())
}

#[lingxia::framework_native("app.setDisplayLanguagePreference", audience = "control-only")]
fn set_display_language_preference(input: SetPreferenceInput) -> HostResult<DisplayLanguageState> {
    lxapp::set_display_language_preference(input.preference)?;
    Ok(lxapp::display_language_state())
}

#[lingxia::framework_native("app.watchDisplayLanguageState", stream, audience = "control-only")]
async fn watch_display_language_state(
    mut stream: StreamContext<DisplayLanguageState>,
) -> HostResult<()> {
    let mut receiver = state_channel().subscribe();
    stream.send(lxapp::display_language_state())?;
    loop {
        tokio::select! {
            _ = stream.canceled() => return Ok(()),
            received = receiver.recv() => match received {
                Ok(state) => stream.send(state)?,
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    stream.send(lxapp::display_language_state())?;
                }
                Err(broadcast::error::RecvError::Closed) => return stream.end(()),
            }
        }
    }
}

pub(crate) fn register() {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        crate::host::register_host_entry(get_display_language_host());
        crate::host::register_host_entry(watch_display_language_host());
        crate::host::register_host_entry(get_display_language_state_host());
        crate::host::register_host_entry(set_display_language_preference_host());
        crate::host::register_host_entry(watch_display_language_state_host());
        lxapp::add_display_language_effective_listener(Box::new(|language| {
            let _ = effective_channel().send(language);
        }));
        lxapp::add_display_language_state_listener(Box::new(|state| {
            let _ = state_channel().send(state);
        }));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_routes_are_read_only_and_state_routes_are_control_only() {
        for route in [get_display_language_host(), watch_display_language_host()] {
            assert_eq!(
                route.audience(),
                crate::host::RouteAudience::AuthenticatedReadOnly
            );
        }
        for route in [
            get_display_language_state_host(),
            set_display_language_preference_host(),
            watch_display_language_state_host(),
        ] {
            assert_eq!(route.audience(), crate::host::RouteAudience::ControlOnly);
        }
    }
}
