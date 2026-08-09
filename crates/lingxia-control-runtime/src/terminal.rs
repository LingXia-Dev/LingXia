use serde_json::Value;

pub(crate) fn handle_terminal_command(
    method: &str,
    params: Option<Value>,
) -> Option<Result<Option<Value>, String>> {
    if !method.starts_with("terminal.") {
        return None;
    }
    Some(handle_terminal_command_impl(method, params))
}

#[cfg(feature = "terminal")]
fn handle_terminal_command_impl(
    method: &str,
    params: Option<Value>,
) -> Result<Option<Value>, String> {
    use lingxia_control_protocol::methods;
    use serde::Deserialize;

    #[derive(Default, Deserialize)]
    struct ResetArgs {
        scope: Option<String>,
    }

    #[derive(Deserialize)]
    struct ImportArgs {
        text: String,
        name: Option<String>,
    }

    #[derive(Deserialize)]
    struct PreviewArgs {
        scheme: Option<lingxia::terminal::TerminalTheme>,
        name: Option<String>,
    }

    let value = match method {
        methods::terminal::CONFIG_GET => to_value(lingxia::terminal::config_get()?)?,
        methods::terminal::CONFIG_APPLY => {
            let overlay = params.ok_or_else(|| format!("missing args for {method}"))?;
            to_value(lingxia::terminal::config_apply(overlay)?)?
        }
        methods::terminal::CONFIG_RESET => {
            let args: ResetArgs = parse_optional(method, params)?;
            to_value(lingxia::terminal::config_reset(args.scope.as_deref())?)?
        }
        methods::terminal::THEMES_LIST => to_value(lingxia::terminal::themes_list()?)?,
        methods::terminal::THEMES_IMPORT => {
            let args: ImportArgs = parse_required(method, params)?;
            to_value(lingxia::terminal::theme_import(
                &args.text,
                args.name.as_deref(),
            )?)?
        }
        methods::terminal::THEMES_PREVIEW => {
            let args: PreviewArgs = parse_required(method, params)?;
            lingxia::terminal::theme_preview(args.scheme, args.name.as_deref())?;
            serde_json::Value::Null
        }
        methods::terminal::THEMES_PREVIEW_END => {
            lingxia::terminal::theme_preview_end()?;
            serde_json::Value::Null
        }
        methods::terminal::FONTS_LIST => to_value(lingxia::terminal::fonts_list())?,
        other => return Err(format!("unknown terminal handler: {other}")),
    };
    Ok(Some(value))
}

#[cfg(feature = "terminal")]
fn to_value(value: impl serde::Serialize) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|error| error.to_string())
}

#[cfg(feature = "terminal")]
fn parse_required<T: serde::de::DeserializeOwned>(
    method: &str,
    params: Option<Value>,
) -> Result<T, String> {
    let params = params.ok_or_else(|| format!("missing args for {method}"))?;
    serde_json::from_value(params).map_err(|error| format!("invalid args for {method}: {error}"))
}

#[cfg(feature = "terminal")]
fn parse_optional<T: serde::de::DeserializeOwned + Default>(
    method: &str,
    params: Option<Value>,
) -> Result<T, String> {
    match params {
        Some(params) => serde_json::from_value(params)
            .map_err(|error| format!("invalid args for {method}: {error}")),
        None => Ok(T::default()),
    }
}

#[cfg(not(feature = "terminal"))]
fn handle_terminal_command_impl(
    method: &str,
    _params: Option<Value>,
) -> Result<Option<Value>, String> {
    Err(format!(
        "{method} is unavailable because lingxia-control-runtime was built without the terminal feature"
    ))
}
