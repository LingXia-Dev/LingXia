use crate::host::{HostResult, StreamContext};
use lxapp::HostAppearanceState;
use lxapp::page_chrome::AppearancePreference;
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SetPreferenceInput {
    preference: AppearancePreference,
}

#[lingxia::framework_native("app.getAppearanceState", audience = "control-only")]
fn get_appearance_state() -> HostResult<HostAppearanceState> {
    Ok(lxapp::host_appearance_state())
}

/// Product-wide light/dark. An lxapp that pinned its own scheme keeps it; the
/// rest, and the host's own chrome, follow this.
#[lingxia::framework_native("app.setAppearancePreference", audience = "control-only")]
fn set_appearance_preference(input: SetPreferenceInput) -> HostResult<HostAppearanceState> {
    lxapp::set_host_appearance_preference(input.preference)
}

#[lingxia::framework_native("app.watchAppearanceState", stream, audience = "control-only")]
async fn watch_appearance_state(mut stream: StreamContext<HostAppearanceState>) -> HostResult<()> {
    let (initial, mut receiver) = lxapp::subscribe_host_appearance();
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
        crate::host::register_host_entry(get_appearance_state_host());
        crate::host::register_host_entry(set_appearance_preference_host());
        crate::host::register_host_entry(watch_appearance_state_host());
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_product_wide_scheme_is_control_only() {
        for route in [
            get_appearance_state_host(),
            set_appearance_preference_host(),
            watch_appearance_state_host(),
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
