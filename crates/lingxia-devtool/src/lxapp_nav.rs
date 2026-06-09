use crate::util::run_async;
use lingxia_devtool_protocol::handlers;
use serde::Deserialize;
use serde_json::{Value, json};

pub(crate) fn handle_lxapp_nav_command(
    handler: &str,
    args: Option<Value>,
) -> Option<Result<Option<Value>, String>> {
    if !handler.starts_with("lxapp.nav.") {
        return None;
    }

    Some(handle_lxapp_nav_command_impl(handler, args))
}

fn handle_lxapp_nav_command_impl(
    handler: &str,
    args: Option<Value>,
) -> Result<Option<Value>, String> {
    match handler {
        handlers::lxapp_nav::TO => {
            let args: PageNavArgs = parse_args(handler, args)?;
            let info = run_async(lingxia::dev::lxapp_dev_nav_to(
                args.appid.as_deref(),
                &args.page,
                args.query,
            ))?;
            serde_json::to_value(info)
                .map(Some)
                .map_err(|err| err.to_string())
        }
        handlers::lxapp_nav::REDIRECT => {
            let args: PageNavArgs = parse_args(handler, args)?;
            let info = run_async(lingxia::dev::lxapp_dev_nav_redirect(
                args.appid.as_deref(),
                &args.page,
                args.query,
            ))?;
            serde_json::to_value(info)
                .map(Some)
                .map_err(|err| err.to_string())
        }
        handlers::lxapp_nav::SWITCH_TAB => {
            let args: PageNavArgs = parse_args(handler, args)?;
            let info = run_async(lingxia::dev::lxapp_dev_nav_switch_tab(
                args.appid.as_deref(),
                &args.page,
                args.query,
            ))?;
            serde_json::to_value(info)
                .map(Some)
                .map_err(|err| err.to_string())
        }
        handlers::lxapp_nav::RELAUNCH => {
            let args: PageNavArgs = parse_args(handler, args)?;
            let info = run_async(lingxia::dev::lxapp_dev_nav_relaunch(
                args.appid.as_deref(),
                &args.page,
                args.query,
            ))?;
            serde_json::to_value(info)
                .map(Some)
                .map_err(|err| err.to_string())
        }
        handlers::lxapp_nav::BACK => {
            let args: BackArgs = parse_args(handler, args)?;
            let info = run_async(lingxia::dev::lxapp_dev_nav_back(
                args.appid.as_deref(),
                args.delta.unwrap_or(1),
            ))?;
            serde_json::to_value(info)
                .map(Some)
                .map_err(|err| err.to_string())
        }
        _ => Err(format!("unknown lxapp nav handler: {}", handler)),
    }
}

fn parse_args<T>(handler: &str, args: Option<Value>) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(args.unwrap_or_else(|| json!({})))
        .map_err(|err| format!("invalid args for {}: {}", handler, err))
}

#[derive(Deserialize)]
struct PageNavArgs {
    #[serde(default)]
    appid: Option<String>,
    page: String,
    #[serde(default)]
    query: Option<Value>,
}

#[derive(Deserialize)]
struct BackArgs {
    #[serde(default)]
    appid: Option<String>,
    #[serde(default)]
    delta: Option<u32>,
}
