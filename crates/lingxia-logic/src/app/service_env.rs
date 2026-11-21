use crate::authorization::{self, LogicRoute};
use crate::i18n::{js_internal_error, js_invalid_parameter_error, js_service_unavailable_error};
use lingxia_app_context::{AppEnv, ServiceEnvSnapshot, ServiceEnvSwitch};
use lingxia_platform::traits::app_runtime::AppRuntime;
use rong::{IntoJSObject, JSContext, JSResult};

#[derive(Debug, Clone, IntoJSObject)]
struct HostServiceEnvState {
    /// The environment the app was built for.
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
    /// Environments it may switch to.
    #[ts_type = "readonly HostAppEnv[]"]
    available: Vec<String>,
    /// Switching is allowed in this build.
    #[js_name = "canSwitch"]
    can_switch: bool,
}

#[derive(Debug, Clone, IntoJSObject)]
struct HostServiceEnvSwitchResult {
    /// The environments after the switch.
    state: HostServiceEnvState,
    /// The app asked the OS to close it so the next launch uses the new environment;
    /// `false` with no `exitError` when `env` was already running.
    #[js_name = "exitRequested"]
    exit_requested: bool,
    /// If present, the environment was saved; close and reopen the app manually.
    #[js_name = "exitError"]
    exit_error: Option<String>,
}

/// The running and next-launch service environments. Control app only.
fn service_env(ctx: JSContext) -> JSResult<HostServiceEnvState> {
    authorization::require(&ctx, LogicRoute::AppServiceEnv)?;
    Ok(state_from_snapshot(
        lingxia_app_context::service_env_snapshot(),
    ))
}

/// Save `env` for next launch and request exit; nothing to do when it is
/// already running. A dev build cannot switch. Control app only. Save failures
/// throw before exit. If `exitRequested` is false and `exitError` is set, ask
/// the user to close and reopen the app.
fn set_service_env(ctx: JSContext, env: String) -> JSResult<HostServiceEnvSwitchResult> {
    let invocation = authorization::require(&ctx, LogicRoute::AppSetServiceEnv)?;
    let target = AppEnv::parse(&env).ok_or_else(|| {
        js_invalid_parameter_error(format!("service env must be 'dev' or 'prod', got '{env}'"))
    })?;
    let snapshot = match lingxia_app_context::prepare_switch(target).map_err(switch_error)? {
        ServiceEnvSwitch::Unchanged(snapshot) => {
            return Ok(HostServiceEnvSwitchResult {
                state: state_from_snapshot(snapshot),
                exit_requested: false,
                exit_error: None,
            });
        }
        ServiceEnvSwitch::RestartRequired(snapshot) => snapshot,
    };
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
        can_switch: snapshot.can_toggle(),
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
        fn serviceEnv = service_env;
        fn setServiceEnv(
            ts_params = "env: HostAppEnv",
            ts_return = "HostServiceEnvSwitchResult"
        ) = set_service_env;
    }
}
