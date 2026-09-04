use crate::host::{HostResult, StreamContext};
use lxapp::{DisplayLanguagePreference, DisplayLanguageState, LanguageTag};
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SetPreferenceInput {
    preference: DisplayLanguagePreference,
}

#[lingxia::framework_native("app.getDisplayLanguage", audience = "authenticated-read-only")]
fn get_display_language() -> HostResult<LanguageTag> {
    Ok(lxapp::display_language_state().effective)
}

#[lingxia::framework_native("app.watchDisplayLanguage", stream, audience = "control-only")]
async fn watch_display_language(mut stream: StreamContext<LanguageTag>) -> HostResult<()> {
    let (initial, mut receiver) = lxapp::subscribe_display_language_effective();
    let mut revision = initial.revision;
    stream.send(initial.effective)?;
    loop {
        tokio::select! {
            _ = stream.canceled() => return Ok(()),
            received = receiver.recv() => match received {
                Some(update) if update.revision > revision => {
                    revision = update.revision;
                    stream.send(update.effective)?;
                }
                Some(_) => {}
                None => return stream.end(()),
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
    let (initial, mut receiver) = lxapp::subscribe_display_language_state();
    let mut revision = initial.revision;
    stream.send(initial.state)?;
    loop {
        tokio::select! {
            _ = stream.canceled() => return Ok(()),
            received = receiver.recv() => match received {
                Some(update) if update.revision > revision => {
                    revision = update.revision;
                    stream.send(update.state)?;
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
        crate::host::register_host_entry(get_display_language_host());
        crate::host::register_host_entry(watch_display_language_host());
        crate::host::register_host_entry(get_display_language_state_host());
        crate::host::register_host_entry(set_display_language_preference_host());
        crate::host::register_host_entry(watch_display_language_state_host());
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_get_is_read_only_but_persistent_streams_are_control_only() {
        assert_eq!(
            get_display_language_host().audience(),
            crate::host::RouteAudience::AuthenticatedReadOnly
        );
        for route in [
            watch_display_language_host(),
            get_display_language_state_host(),
            set_display_language_preference_host(),
            watch_display_language_state_host(),
        ] {
            assert_eq!(route.audience(), crate::host::RouteAudience::ControlOnly);
        }
    }

    #[test]
    fn effective_watch_schema_and_dispatch_require_a_control_caller() {
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
        let control =
            unsafe { test_authenticated_caller("test.control", 2, AppSessionClass::ControlApp) };
        let browser = unsafe { test_browser_caller() };
        let watch_audience = watch_display_language_host().audience();

        assert!(
            host_route_schema(&standard)
                .methods
                .contains_key("app.getDisplayLanguage")
        );
        assert!(
            !host_route_schema(&standard)
                .methods
                .contains_key("app.watchDisplayLanguage")
        );
        for caller in [&control, &browser] {
            assert!(
                host_route_schema(caller)
                    .methods
                    .contains_key("app.watchDisplayLanguage")
            );
        }

        assert!(!authorize(&standard, watch_audience));
        assert!(authorize(&control, watch_audience));
        assert!(authorize(&browser, watch_audience));
    }
}
