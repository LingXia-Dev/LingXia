use crate::authorization::{self, LogicRoute};
use crate::i18n::{js_internal_error, js_invalid_parameter_error, js_service_unavailable_error};
use lingxia_app_context::ServiceEnvSnapshot;
use lingxia_platform::traits::app_runtime::AppRuntime;
use rong::{IntoJSObject, JSContext, JSResult};

#[derive(Debug, Clone, IntoJSObject)]
struct HostServiceEnvState {
    #[js_name = "buildEnv"]
    #[ts_type = "HostAppEnv"]
    build_env: String,
    /// Environment this process is running; unchanged until the app restarts.
    #[js_name = "serviceEnv"]
    #[ts_type = "HostAppEnv"]
    service_env: String,
    /// Environment that will be used after closing and reopening the app.
    #[js_name = "nextLaunchEnv"]
    #[ts_type = "HostAppEnv"]
    next_launch_env: String,
    #[ts_type = "HostAppEnv[]"]
    available: Vec<String>,
    #[js_name = "canToggle"]
    can_toggle: bool,
}

#[derive(Debug, Clone, IntoJSObject)]
struct HostServiceEnvSwitchResult {
    state: HostServiceEnvState,
    #[js_name = "exitRequested"]
    exit_requested: bool,
    /// If present, the environment was saved; close and reopen the app manually.
    #[js_name = "exitError"]
    exit_error: Option<String>,
}

/// Query the running and next-launch service environments. Control app only.
fn get_service_env(ctx: JSContext) -> JSResult<HostServiceEnvState> {
    authorization::require(&ctx, LogicRoute::AppGetServiceEnv)?;
    Ok(state_from_snapshot(
        lingxia_app_context::service_env_snapshot(),
    ))
}

/// Save the other service environment for next launch and request exit.
/// A dev build cannot switch. Control app only. Save failures throw before exit.
/// If exitRequested is false, ask the user to close and reopen the app manually.
/// Retrying before restart keeps the same target environment.
fn toggle_service_env(ctx: JSContext) -> JSResult<HostServiceEnvSwitchResult> {
    let invocation = authorization::require(&ctx, LogicRoute::AppToggleServiceEnv)?;
    let snapshot = lingxia_app_context::prepare_toggle().map_err(switch_error)?;
    let exit_error = invocation
        .lxapp()
        .runtime
        .exit()
        .err()
        .map(|err| err.to_string());
    Ok(HostServiceEnvSwitchResult {
        state: state_from_snapshot(snapshot),
        exit_requested: exit_error.is_none(),
        exit_error,
    })
}

fn switch_error(error: lingxia_app_context::ServiceEnvError) -> rong::RongJSError {
    match error {
        lingxia_app_context::ServiceEnvError::NotInitialized => {
            js_service_unavailable_error(error.to_string())
        }
        lingxia_app_context::ServiceEnvError::Invalid(detail) => js_invalid_parameter_error(detail),
        lingxia_app_context::ServiceEnvError::Persist(detail) => js_internal_error(detail),
    }
}

fn state_from_snapshot(snapshot: ServiceEnvSnapshot) -> HostServiceEnvState {
    HostServiceEnvState {
        can_toggle: snapshot.can_toggle(),
        build_env: snapshot.build_env.as_str().to_string(),
        service_env: snapshot.service_env.as_str().to_string(),
        next_launch_env: snapshot.next_launch_env.as_str().to_string(),
        available: snapshot
            .available
            .iter()
            .map(|env| env.as_str().to_string())
            .collect(),
    }
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace HostAppApi = super::app_namespace(ctx)?;
        fn getServiceEnv = get_service_env;
        fn toggleServiceEnv(
            ts_return = "HostServiceEnvSwitchResult"
        ) = toggle_service_env;
    }
}
