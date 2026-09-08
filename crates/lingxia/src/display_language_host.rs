use crate::host::{HostResult, StreamContext};
use lxapp::DisplayLanguagePreference;
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SetPreferenceInput {
    preference: DisplayLanguagePreference,
}

/// The language the product is set to. Documents render in the *effective*
/// language, which the bridge hands them; only the surface that edits the
/// setting needs the preference behind it.
#[lingxia::framework_native("app.getDisplayLanguagePreference", audience = "control-only")]
fn get_display_language_preference() -> HostResult<DisplayLanguagePreference> {
    Ok(lxapp::display_language_state().preference)
}

#[lingxia::framework_native("app.setDisplayLanguagePreference", audience = "control-only")]
fn set_display_language_preference(
    input: SetPreferenceInput,
) -> HostResult<DisplayLanguagePreference> {
    lxapp::set_display_language_preference(input.preference)?;
    Ok(lxapp::display_language_state().preference)
}

#[lingxia::framework_native(
    "app.watchDisplayLanguagePreference",
    stream,
    audience = "control-only"
)]
async fn watch_display_language_preference(
    mut stream: StreamContext<DisplayLanguagePreference>,
) -> HostResult<()> {
    let (initial, mut receiver) = lxapp::subscribe_display_language_state();
    let mut revision = initial.revision;
    // The state moves whenever the effective language does; the preference does
    // not. A system flip under `auto` must not wake this stream.
    let mut preference = initial.state.preference;
    stream.send(preference.clone())?;
    loop {
        tokio::select! {
            _ = stream.canceled() => return Ok(()),
            received = receiver.recv() => match received {
                Some(update) if update.revision > revision => {
                    revision = update.revision;
                    if update.state.preference != preference {
                        preference = update.state.preference;
                        stream.send(preference.clone())?;
                    }
                }
                Some(_) => {}
                None => return stream.end(()),
            }
        }
    }
}

pub(crate) fn register() {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        crate::host::register_host_entry(get_display_language_preference_host());
        crate::host::register_host_entry(set_display_language_preference_host());
        crate::host::register_host_entry(watch_display_language_preference_host());
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_product_language_preference_is_control_only() {
        for route in [
            get_display_language_preference_host(),
            set_display_language_preference_host(),
            watch_display_language_preference_host(),
        ] {
            assert_eq!(route.audience(), crate::host::RouteAudience::ControlOnly);
        }
    }

    /// Reading the effective language is not a route at all: every document is
    /// handed it at injection and pushed every change. Nothing here is the way
    /// an ordinary lxapp or a control surface follows the language.
    #[test]
    fn only_a_control_caller_reaches_the_preference() {
        use crate::host::{AuthenticatedCaller, authorize, host_route_schema};
        use lxapp::AppSessionClass;

        unsafe extern "Rust" {
            #[link_name = "lingxia_lxapp_test_authenticated_caller_v1"]
            fn test_authenticated_caller(
                app_id: &str,
                session_id: u64,
                class: AppSessionClass,
            ) -> AuthenticatedCaller;
            #[link_name = "lingxia_lxapp_test_browser_caller_v1"]
            fn test_browser_caller() -> AuthenticatedCaller;
        }

        register();
        // SAFETY: these symbols are private workspace test harnesses and are
        // not reachable through lxapp's safe downstream API.
        let standard =
            unsafe { test_authenticated_caller("test.standard", 1, AppSessionClass::StandardApp) };
        let surface =
            unsafe { test_authenticated_caller("test.surface", 3, AppSessionClass::ControlSurface) };
        let control =
            unsafe { test_authenticated_caller("test.control", 2, AppSessionClass::ControlApp) };
        let browser = unsafe { test_browser_caller() };
        let audience = get_display_language_preference_host().audience();

        for caller in [&standard, &surface] {
            assert!(!authorize(caller, audience));
            assert!(
                !host_route_schema(caller)
                    .methods
                    .contains_key("app.getDisplayLanguagePreference")
            );
        }
        for caller in [&control, &browser] {
            assert!(authorize(caller, audience));
            assert!(
                host_route_schema(caller)
                    .methods
                    .contains_key("app.getDisplayLanguagePreference")
            );
        }
    }
}
