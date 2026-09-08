use crate::host::{HostResult, StreamContext};
use lxapp::page_chrome::AppearancePreference;
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SetPreferenceInput {
    preference: AppearancePreference,
}

/// The product's light/dark setting. A document renders in the scheme it is
/// given; only the surface that edits the setting needs the preference.
#[lingxia::framework_native("app.getAppearancePreference", audience = "control-only")]
fn get_appearance_preference() -> HostResult<AppearancePreference> {
    Ok(lxapp::host_appearance_state().preference)
}

/// Product-wide light/dark. An lxapp that pinned its own scheme in its manifest
/// keeps it; the rest, and the host's own chrome, follow this.
#[lingxia::framework_native("app.setAppearancePreference", audience = "control-only")]
fn set_appearance_preference(input: SetPreferenceInput) -> HostResult<AppearancePreference> {
    lxapp::set_host_appearance_preference(input.preference).map(|state| state.preference)
}

#[lingxia::framework_native("app.watchAppearancePreference", stream, audience = "control-only")]
async fn watch_appearance_preference(
    mut stream: StreamContext<AppearancePreference>,
) -> HostResult<()> {
    let (initial, mut receiver) = lxapp::subscribe_host_appearance();
    let mut revision = initial.revision;
    // The state also moves when the system flips under `auto`; the preference
    // does not, and this stream is about the choice.
    let mut preference = initial.state.preference;
    stream.send(preference)?;
    loop {
        tokio::select! {
            _ = stream.canceled() => return Ok(()),
            received = receiver.recv() => match received {
                Some(update) if update.revision > revision => {
                    revision = update.revision;
                    if update.state.preference != preference {
                        preference = update.state.preference;
                        stream.send(preference)?;
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
        crate::host::register_host_entry(get_appearance_preference_host());
        crate::host::register_host_entry(set_appearance_preference_host());
        crate::host::register_host_entry(watch_appearance_preference_host());
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_product_wide_scheme_is_control_only() {
        for route in [
            get_appearance_preference_host(),
            set_appearance_preference_host(),
            watch_appearance_preference_host(),
        ] {
            assert_eq!(route.audience(), crate::host::RouteAudience::ControlOnly);
        }
    }

    #[test]
    fn an_ordinary_lxapp_session_reaches_none_of_them() {
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
        // SAFETY: private workspace test harnesses, absent from the safe API.
        let standard =
            unsafe { test_authenticated_caller("test.standard", 1, AppSessionClass::StandardApp) };
        let control =
            unsafe { test_authenticated_caller("test.control", 2, AppSessionClass::ControlApp) };
        let surface = unsafe {
            test_authenticated_caller("test.surface", 3, AppSessionClass::ControlSurface)
        };
        let browser = unsafe { test_browser_caller() };
        let audience = set_appearance_preference_host().audience();

        assert!(!authorize(&standard, audience));
        assert!(!authorize(&surface, audience));
        assert!(authorize(&control, audience));
        assert!(authorize(&browser, audience));
        assert!(
            !host_route_schema(&standard)
                .methods
                .contains_key("app.setAppearancePreference")
        );
        for caller in [&control, &browser] {
            assert!(
                host_route_schema(caller)
                    .methods
                    .contains_key("app.setAppearancePreference")
            );
        }
    }
}
