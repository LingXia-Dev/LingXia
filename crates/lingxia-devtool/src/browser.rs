#[cfg(feature = "browser")]
use crate::util::{png_response, run_async};
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
#[cfg(feature = "browser")]
const DEFAULT_BROWSER_WAIT_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(feature = "browser")]
const DEFAULT_QUERY_TEXT_LIMIT: usize = 4096;

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
        handlers::browser::CURRENT => {
            let _args: EmptyArgs = parse_args(handler, args)?;
            serde_json::to_value(lingxia_browser::current_tab())
                .map(Some)
                .map_err(|err| err.to_string())
        }
        handlers::browser::ACTIVATE => {
            let args: TabArgs = parse_args(handler, args)?;
            let tab_id = resolve_tab_id(&args.tab_id)?;
            let info = lingxia_browser::activate(&tab_id).map_err(|err| err.to_string())?;
            present_browser_tab(&tab_id);
            serde_json::to_value(info)
                .map(Some)
                .map_err(|err| err.to_string())
        }
        handlers::browser::CLOSE => {
            let args: TabArgs = parse_args(handler, args)?;
            let tab_id = resolve_tab_id(&args.tab_id)?;
            lingxia_browser::close(&tab_id).map_err(|err| err.to_string())?;
            Ok(None)
        }
        handlers::browser::RELOAD => {
            let args: TabArgs = parse_args(handler, args)?;
            let tab_id = resolve_tab_id(&args.tab_id)?;
            lingxia_browser::reload(&tab_id).map_err(|err| err.to_string())?;
            Ok(None)
        }
        handlers::browser::BACK => {
            let args: TabArgs = parse_args(handler, args)?;
            let tab_id = resolve_tab_id(&args.tab_id)?;
            lingxia_browser::go_back(&tab_id).map_err(|err| err.to_string())?;
            Ok(None)
        }
        handlers::browser::FORWARD => {
            let args: TabArgs = parse_args(handler, args)?;
            let tab_id = resolve_tab_id(&args.tab_id)?;
            lingxia_browser::go_forward(&tab_id).map_err(|err| err.to_string())?;
            Ok(None)
        }
        handlers::browser::EVAL => {
            let args: EvalArgs = parse_args(handler, args)?;
            let tab_id = resolve_tab_id(&args.tab_id)?;
            let timeout = wait_timeout(args.timeout_ms);
            run_async(async move {
                let initial_url = if args.wait_navigation {
                    Some(
                        lingxia_browser::current_url(&tab_id)
                            .await
                            .map_err(|err| err.to_string())?,
                    )
                } else {
                    None
                };
                let value = lingxia_browser::evaluate_javascript(&tab_id, &args.js)
                    .await
                    .map_err(|err| err.to_string())?;
                if let Some(initial_url) = initial_url {
                    let navigation = lingxia_browser::wait(
                        &tab_id,
                        lingxia_browser::BrowserWaitCondition::Navigation {
                            initial_url,
                            wait_until_complete: args.complete,
                        },
                        timeout,
                    )
                    .await
                    .map_err(|err| err.to_string())?;
                    Ok::<Option<Value>, String>(Some(json!({
                        "value": value,
                        "navigation": navigation,
                    })))
                } else {
                    Ok::<Option<Value>, String>(Some(value))
                }
            })
        }
        handlers::browser::QUERY => {
            let args: SelectorArgs = parse_args(handler, args)?;
            let tab_id = resolve_tab_id(&args.tab_id)?;
            let max_text = if args.full {
                None
            } else {
                Some(args.max_text.unwrap_or(DEFAULT_QUERY_TEXT_LIMIT))
            };
            run_async(lingxia_browser::query_with_max_text(
                &tab_id,
                &args.selector,
                max_text,
            ))
            .and_then(|result| serde_json::to_value(result).map_err(|err| err.to_string()))
            .map(Some)
        }
        handlers::browser::WAIT => {
            let args: WaitArgs = parse_args(handler, args)?;
            let tab_id = resolve_tab_id(&args.tab_id)?;
            let timeout = wait_timeout(args.timeout_ms);
            run_async(lingxia_browser::wait(&tab_id, args.condition, timeout))
                .and_then(|result| serde_json::to_value(result).map_err(|err| err.to_string()))
                .map(Some)
        }
        handlers::browser::WAIT_URL => {
            let args: WaitUrlArgs = parse_args(handler, args)?;
            let tab_id = resolve_tab_id(&args.tab_id)?;
            let timeout = wait_timeout(args.timeout_ms);
            if args.url.is_some() == args.contains.is_some() {
                return Err("pass exactly one of url or contains".to_string());
            }
            let result = if let Some(text) = args.contains {
                run_async(lingxia_browser::wait_for_url_contains(
                    &tab_id, &text, timeout,
                ))
            } else {
                run_async(lingxia_browser::wait_for_url(
                    &tab_id,
                    &args.url.ok_or_else(|| "url is required".to_string())?,
                    timeout,
                ))
            }?;
            serde_json::to_value(result)
                .map(Some)
                .map_err(|err| err.to_string())
        }
        handlers::browser::WAIT_NAVIGATION => {
            let args: WaitNavigationArgs = parse_args(handler, args)?;
            let tab_id = resolve_tab_id(&args.tab_id)?;
            let timeout = wait_timeout(args.timeout_ms);
            let result = if let Some(from_url) = args.from_url {
                run_async(lingxia_browser::wait(
                    &tab_id,
                    lingxia_browser::BrowserWaitCondition::Navigation {
                        initial_url: Some(from_url),
                        wait_until_complete: args.complete,
                    },
                    timeout,
                ))
            } else {
                run_async(lingxia_browser::wait_for_navigation(
                    &tab_id,
                    timeout,
                    args.complete,
                ))
            };
            result
                .and_then(|result| serde_json::to_value(result).map_err(|err| err.to_string()))
                .map(Some)
        }
        handlers::browser::CLICK => {
            let args: SelectorArgs = parse_args(handler, args)?;
            let tab_id = resolve_tab_id(&args.tab_id)?;
            let timeout = wait_timeout(args.timeout_ms);
            run_async(async move {
                let initial_url = if args.wait_navigation {
                    Some(
                        lingxia_browser::current_url(&tab_id)
                            .await
                            .map_err(|err| err.to_string())?,
                    )
                } else {
                    None
                };
                lingxia_browser::click(&tab_id, &args.selector)
                    .await
                    .map_err(|err| err.to_string())?;
                wait_after_action(&tab_id, initial_url, args.complete, timeout).await
            })
        }
        handlers::browser::TYPE => {
            let args: TypeArgs = parse_args(handler, args)?;
            let tab_id = resolve_tab_id(&args.tab_id)?;
            run_async(lingxia_browser::type_text(
                &tab_id,
                &args.selector,
                &args.text,
            ))?;
            Ok(None)
        }
        handlers::browser::FILL => {
            let args: TypeArgs = parse_args(handler, args)?;
            let tab_id = resolve_tab_id(&args.tab_id)?;
            run_async(lingxia_browser::fill(&tab_id, &args.selector, &args.text))?;
            Ok(None)
        }
        handlers::browser::PRESS => {
            let args: PressArgs = parse_args(handler, args)?;
            let tab_id = resolve_tab_id(&args.tab_id)?;
            let timeout = wait_timeout(args.timeout_ms);
            run_async(async move {
                let initial_url = if args.wait_navigation {
                    Some(
                        lingxia_browser::current_url(&tab_id)
                            .await
                            .map_err(|err| err.to_string())?,
                    )
                } else {
                    None
                };
                lingxia_browser::press(&tab_id, &args.key)
                    .await
                    .map_err(|err| err.to_string())?;
                wait_after_action(&tab_id, initial_url, args.complete, timeout).await
            })
        }
        handlers::browser::SCROLL => {
            let args: ScrollArgs = parse_args(handler, args)?;
            let tab_id = resolve_tab_id(&args.tab_id)?;
            run_async(lingxia_browser::scroll(&tab_id, args.dx, args.dy))?;
            Ok(None)
        }
        handlers::browser::SCROLL_TO => {
            let args: SelectorArgs = parse_args(handler, args)?;
            let tab_id = resolve_tab_id(&args.tab_id)?;
            run_async(lingxia_browser::scroll_to(&tab_id, &args.selector))?;
            Ok(None)
        }
        handlers::browser::COOKIES_LIST => {
            let args: CookieListArgs = parse_args(handler, args)?;
            let tab_id = resolve_tab_id(&args.tab_id)?;
            let cookies = if args.visible {
                run_async(lingxia_browser::list_cookies(&tab_id))
            } else {
                run_async(lingxia_browser::list_all_cookies(&tab_id))
            };
            cookies
                .and_then(|result| serde_json::to_value(result).map_err(|err| err.to_string()))
                .map(Some)
        }
        handlers::browser::COOKIES_SET => {
            let args: CookieSetArgs = parse_args(handler, args)?;
            let tab_id = resolve_tab_id(&args.tab_id)?;
            run_async(lingxia_browser::set_cookie(&tab_id, args.cookie))?;
            Ok(None)
        }
        handlers::browser::COOKIES_DELETE => {
            let args: CookieDeleteArgs = parse_args(handler, args)?;
            let tab_id = resolve_tab_id(&args.tab_id)?;
            run_async(lingxia_browser::delete_cookie(
                &tab_id,
                &args.name,
                &args.domain,
                &args.path,
            ))?;
            Ok(None)
        }
        handlers::browser::COOKIES_CLEAR => {
            let args: TabArgs = parse_args(handler, args)?;
            let tab_id = resolve_tab_id(&args.tab_id)?;
            run_async(lingxia_browser::clear_cookies(&tab_id))?;
            Ok(None)
        }
        handlers::browser::SCREENSHOT => {
            let args: TabArgs = parse_args(handler, args)?;
            let tab_id = resolve_tab_id(&args.tab_id)?;
            let bytes = run_async(lingxia_browser::take_screenshot(&tab_id))?;
            Ok(Some(png_response(&bytes, [("tab_id", json!(tab_id))])))
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
fn wait_timeout(timeout_ms: Option<u64>) -> Duration {
    timeout_ms
        .map(Duration::from_millis)
        .unwrap_or(DEFAULT_BROWSER_WAIT_TIMEOUT)
}

#[cfg(feature = "browser")]
fn resolve_tab_id(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.eq_ignore_ascii_case("current") {
        return lingxia_browser::current_tab()
            .map(|tab| tab.tab_id)
            .ok_or_else(|| "no current browser tab".to_string());
    }
    Ok(trimmed.to_string())
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
async fn wait_after_action(
    tab_id: &str,
    initial_url: Option<Option<String>>,
    complete: bool,
    timeout: Duration,
) -> Result<Option<Value>, String> {
    let Some(initial_url) = initial_url else {
        return Ok(None);
    };
    let result = lingxia_browser::wait(
        tab_id,
        lingxia_browser::BrowserWaitCondition::Navigation {
            initial_url,
            wait_until_complete: complete,
        },
        timeout,
    )
    .await
    .map_err(|err| err.to_string())?;
    serde_json::to_value(result)
        .map(Some)
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
struct CookieListArgs {
    tab_id: String,
    #[serde(default)]
    visible: bool,
}

#[cfg(feature = "browser")]
#[derive(Deserialize)]
struct EvalArgs {
    tab_id: String,
    js: String,
    #[serde(default)]
    wait_navigation: bool,
    #[serde(default)]
    complete: bool,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[cfg(feature = "browser")]
#[derive(Deserialize)]
struct WaitArgs {
    tab_id: String,
    condition: lingxia_browser::BrowserWaitCondition,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[cfg(feature = "browser")]
#[derive(Deserialize)]
struct WaitUrlArgs {
    tab_id: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    contains: Option<String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[cfg(feature = "browser")]
#[derive(Deserialize)]
struct WaitNavigationArgs {
    tab_id: String,
    #[serde(default)]
    from_url: Option<String>,
    #[serde(default)]
    complete: bool,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[cfg(feature = "browser")]
#[derive(Deserialize)]
struct SelectorArgs {
    tab_id: String,
    selector: String,
    #[serde(default)]
    max_text: Option<usize>,
    #[serde(default)]
    full: bool,
    #[serde(default)]
    wait_navigation: bool,
    #[serde(default)]
    complete: bool,
    #[serde(default)]
    timeout_ms: Option<u64>,
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
    #[serde(default)]
    wait_navigation: bool,
    #[serde(default)]
    complete: bool,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[cfg(feature = "browser")]
#[derive(Deserialize)]
struct ScrollArgs {
    tab_id: String,
    dx: f64,
    dy: f64,
}

#[cfg(feature = "browser")]
#[derive(Deserialize)]
struct CookieSetArgs {
    tab_id: String,
    cookie: lingxia_browser::WebViewCookieSetRequest,
}

#[cfg(feature = "browser")]
#[derive(Deserialize)]
struct CookieDeleteArgs {
    tab_id: String,
    name: String,
    domain: String,
    #[serde(default = "default_cookie_path")]
    path: String,
}

#[cfg(feature = "browser")]
fn default_cookie_path() -> String {
    "/".to_string()
}
