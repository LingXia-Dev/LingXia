#[cfg(feature = "browser")]
use lingxia_devtool_protocol::handlers;
#[cfg(feature = "browser")]
use serde::Deserialize;
use serde_json::Value;
#[cfg(feature = "browser")]
use serde_json::json;
#[cfg(feature = "browser")]
use std::time::{Duration, Instant};

#[cfg(feature = "browser")]
const OPEN_WAIT_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(feature = "browser")]
const OPEN_WAIT_INTERVAL: Duration = Duration::from_millis(50);

pub(crate) fn handle_browser_command(
    handler: &str,
    args: Option<Value>,
) -> Option<Result<Option<Value>, String>> {
    if !is_browser_handler(handler) {
        return None;
    }

    Some(handle_browser_command_impl(handler, args))
}

fn is_browser_handler(handler: &str) -> bool {
    handler.starts_with("browser.")
}

#[cfg(feature = "browser")]
fn handle_browser_command_impl(
    handler: &str,
    args: Option<Value>,
) -> Result<Option<Value>, String> {
    match handler {
        handlers::browser::OPEN => {
            let args: OpenArgs = parse_args(handler, args)?;
            let tab_id = lingxia_browser::open(&args.url, args.tab_id.as_deref())
                .map_err(|err| err.to_string())?;
            present_browser_tab(&tab_id);
            wait_for_tab_navigation(&tab_id, &args.url)?;
            Ok(Some(json!({ "tab_id": tab_id })))
        }
        handlers::browser::TABS => {
            let _args: EmptyArgs = parse_args(handler, args)?;
            serde_json::to_value(lingxia_browser::tabs())
                .map(Some)
                .map_err(|err| err.to_string())
        }
        handlers::browser::CLOSE => {
            let args: TabArgs = parse_args(handler, args)?;
            lingxia_browser::close(&args.tab_id).map_err(|err| err.to_string())?;
            Ok(None)
        }
        handlers::browser::EVAL => {
            let args: EvalArgs = parse_args(handler, args)?;
            run_async(lingxia_browser::evaluate_javascript(&args.tab_id, &args.js)).map(Some)
        }
        handlers::browser::CLICK => {
            let args: SelectorArgs = parse_args(handler, args)?;
            run_async(lingxia_browser::click(&args.tab_id, &args.selector))?;
            Ok(None)
        }
        handlers::browser::TYPE => {
            let args: TypeArgs = parse_args(handler, args)?;
            run_async(lingxia_browser::type_text(
                &args.tab_id,
                &args.selector,
                &args.text,
            ))?;
            Ok(None)
        }
        handlers::browser::PRESS => {
            let args: PressArgs = parse_args(handler, args)?;
            run_async(lingxia_browser::press(&args.tab_id, &args.key))?;
            Ok(None)
        }
        handlers::browser::SCROLL => {
            let args: ScrollArgs = parse_args(handler, args)?;
            run_async(lingxia_browser::scroll(&args.tab_id, args.dx, args.dy))?;
            Ok(None)
        }
        handlers::browser::SCROLL_TO => {
            let args: SelectorArgs = parse_args(handler, args)?;
            run_async(lingxia_browser::scroll_to(&args.tab_id, &args.selector))?;
            Ok(None)
        }
        _ => Err(format!("unknown browser handler: {}", handler)),
    }
}

#[cfg(not(feature = "browser"))]
fn handle_browser_command_impl(
    handler: &str,
    _args: Option<Value>,
) -> Result<Option<Value>, String> {
    Err(format!(
        "{} is unavailable because lingxia-devtool was built without the browser feature",
        handler
    ))
}

#[cfg(feature = "browser")]
fn parse_args<T>(handler: &str, args: Option<Value>) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(args.unwrap_or_else(|| json!({})))
        .map_err(|err| format!("invalid args for {}: {}", handler, err))
}

#[cfg(feature = "browser")]
fn run_async<T, E>(future: impl std::future::Future<Output = Result<T, E>>) -> Result<T, String>
where
    E: std::fmt::Display,
{
    lingxia::tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| err.to_string())?
        .block_on(future)
        .map_err(|err| err.to_string())
}

#[cfg(all(feature = "browser", target_os = "macos"))]
fn present_browser_tab(tab_id: &str) {
    let _ = lingxia::apple::present_internal_browser_tab(tab_id);
}

#[cfg(all(feature = "browser", not(target_os = "macos")))]
fn present_browser_tab(_tab_id: &str) {}

#[cfg(feature = "browser")]
fn wait_for_tab_navigation(tab_id: &str, requested_url: &str) -> Result<(), String> {
    let expected_url = normalize_expected_url(requested_url);
    if expected_url.is_empty() {
        return Ok(());
    }

    let deadline = Instant::now() + OPEN_WAIT_TIMEOUT;
    loop {
        let current_url = lingxia_browser::tabs()
            .into_iter()
            .find(|tab| tab.tab_id == tab_id)
            .and_then(|tab| tab.current_url);

        if current_url.as_deref() == Some(expected_url.as_str()) {
            return Ok(());
        }

        if Instant::now() >= deadline {
            return Err(format!(
                "browser tab {} did not load {} within {}ms",
                tab_id,
                expected_url,
                OPEN_WAIT_TIMEOUT.as_millis()
            ));
        }

        std::thread::sleep(OPEN_WAIT_INTERVAL);
    }
}

#[cfg(feature = "browser")]
fn normalize_expected_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() >= "http://".len() && trimmed[..7].eq_ignore_ascii_case("http://") {
        format!("https://{}", &trimmed[7..])
    } else {
        trimmed.to_string()
    }
}

#[cfg(feature = "browser")]
#[derive(Deserialize)]
struct EmptyArgs {}

#[cfg(feature = "browser")]
#[derive(Deserialize)]
struct OpenArgs {
    url: String,
    #[serde(default)]
    tab_id: Option<String>,
}

#[cfg(feature = "browser")]
#[derive(Deserialize)]
struct TabArgs {
    tab_id: String,
}

#[cfg(feature = "browser")]
#[derive(Deserialize)]
struct EvalArgs {
    tab_id: String,
    js: String,
}

#[cfg(feature = "browser")]
#[derive(Deserialize)]
struct SelectorArgs {
    tab_id: String,
    selector: String,
}

#[cfg(feature = "browser")]
#[derive(Deserialize)]
struct TypeArgs {
    tab_id: String,
    selector: String,
    text: String,
}

#[cfg(feature = "browser")]
#[derive(Deserialize)]
struct PressArgs {
    tab_id: String,
    key: String,
}

#[cfg(feature = "browser")]
#[derive(Deserialize)]
struct ScrollArgs {
    tab_id: String,
    dx: f64,
    dy: f64,
}
