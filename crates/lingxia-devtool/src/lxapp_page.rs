use crate::util::{png_response, run_async};
use lingxia_devtool_protocol::handlers;
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;

const DEFAULT_EVAL_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_QUERY_TEXT_LIMIT: usize = 4096;

pub(crate) fn handle_lxapp_page_command(
    handler: &str,
    args: Option<Value>,
) -> Option<Result<Option<Value>, String>> {
    if !handler.starts_with("lxapp.page.") {
        return None;
    }

    Some(handle_lxapp_page_command_impl(handler, args))
}

fn handle_lxapp_page_command_impl(
    handler: &str,
    args: Option<Value>,
) -> Result<Option<Value>, String> {
    match handler {
        handlers::lxapp_page::CURRENT => {
            let args: PageTargetArgs = parse_args(handler, args)?;
            let info = lingxia::dev::lxapp_dev_page_current(args.appid.as_deref())?;
            serde_json::to_value(info)
                .map(Some)
                .map_err(|err| err.to_string())
        }
        handlers::lxapp_page::LIST => {
            let args: AppArgs = parse_args(handler, args)?;
            let pages = lingxia::dev::lxapp_dev_page_list(args.appid.as_deref())?;
            Ok(Some(page_list_response(pages)))
        }
        handlers::lxapp_page::INFO => {
            let args: PageTargetArgs = parse_args(handler, args)?;
            let info =
                lingxia::dev::lxapp_dev_page_info(args.appid.as_deref(), args.page.as_deref())?;
            serde_json::to_value(info)
                .map(Some)
                .map_err(|err| err.to_string())
        }
        handlers::lxapp_page::WAIT => {
            let args: WaitArgs = parse_args(handler, args)?;
            let state = args.state.unwrap_or(if args.selector.is_some() {
                lingxia::dev::LxAppDevPageWaitState::Attached
            } else {
                lingxia::dev::LxAppDevPageWaitState::Ready
            });
            let timeout = Duration::from_millis(args.timeout_ms.unwrap_or(5000));
            let result = run_async(lingxia::dev::lxapp_dev_page_wait(
                args.appid.as_deref(),
                args.page.as_deref(),
                args.selector.as_deref(),
                args.index,
                state,
                timeout,
            ))?;
            serde_json::to_value(result)
                .map(Some)
                .map_err(|err| err.to_string())
        }
        handlers::lxapp_page::EVAL => {
            let args: EvalArgs = parse_args(handler, args)?;
            let timeout = Duration::from_millis(args.timeout_ms.unwrap_or_else(|| {
                u64::try_from(DEFAULT_EVAL_TIMEOUT.as_millis()).unwrap_or(5000)
            }));
            let value = run_async(async move {
                tokio::time::timeout(
                    timeout,
                    lingxia::dev::lxapp_dev_page_eval(
                        args.appid.as_deref(),
                        args.page.as_deref(),
                        &args.js,
                    ),
                )
                .await
                .map_err(|_| format!("lxapp page eval timed out after {}ms", timeout.as_millis()))?
            })?;
            Ok(Some(json!({ "value": value })))
        }
        handlers::lxapp_page::QUERY => {
            let args: QueryArgs = parse_args(handler, args)?;
            let max_text = args
                .max_text
                .or_else(|| (!args.full).then_some(DEFAULT_QUERY_TEXT_LIMIT));
            run_async(lingxia::dev::lxapp_dev_page_query(
                args.appid.as_deref(),
                args.page.as_deref(),
                &args.selector,
                args.index,
                args.all,
                max_text,
            ))
            .map(Some)
        }
        handlers::lxapp_page::CLICK => {
            let args: SelectorActionArgs = parse_args(handler, args)?;
            let info =
                lingxia::dev::lxapp_dev_page_info(args.appid.as_deref(), args.page.as_deref())?;
            run_async(lingxia::dev::lxapp_dev_page_click(
                Some(&info.appid),
                Some(&info.instance_id),
                &args.selector,
                args.index,
            ))?;
            Ok(Some(page_action_response("click", info)))
        }
        handlers::lxapp_page::TYPE => {
            let args: TextActionArgs = parse_args(handler, args)?;
            let info =
                lingxia::dev::lxapp_dev_page_info(args.appid.as_deref(), args.page.as_deref())?;
            run_async(lingxia::dev::lxapp_dev_page_type(
                Some(&info.appid),
                Some(&info.instance_id),
                &args.selector,
                args.index,
                &args.text,
            ))?;
            Ok(Some(page_action_response("type", info)))
        }
        handlers::lxapp_page::FILL => {
            let args: TextActionArgs = parse_args(handler, args)?;
            let info =
                lingxia::dev::lxapp_dev_page_info(args.appid.as_deref(), args.page.as_deref())?;
            run_async(lingxia::dev::lxapp_dev_page_fill(
                Some(&info.appid),
                Some(&info.instance_id),
                &args.selector,
                args.index,
                &args.text,
            ))?;
            Ok(Some(page_action_response("fill", info)))
        }
        handlers::lxapp_page::PRESS => {
            let args: PressArgs = parse_args(handler, args)?;
            let info =
                lingxia::dev::lxapp_dev_page_info(args.appid.as_deref(), args.page.as_deref())?;
            run_async(lingxia::dev::lxapp_dev_page_press(
                Some(&info.appid),
                Some(&info.instance_id),
                &args.key,
                args.selector.as_deref(),
                args.index,
            ))?;
            Ok(Some(page_action_response("press", info)))
        }
        handlers::lxapp_page::SCROLL => {
            let args: ScrollArgs = parse_args(handler, args)?;
            let info =
                lingxia::dev::lxapp_dev_page_info(args.appid.as_deref(), args.page.as_deref())?;
            run_async(lingxia::dev::lxapp_dev_page_scroll(
                Some(&info.appid),
                Some(&info.instance_id),
                args.dx,
                args.dy,
            ))?;
            Ok(Some(page_action_response("scroll", info)))
        }
        handlers::lxapp_page::SCROLL_TO => {
            let args: SelectorActionArgs = parse_args(handler, args)?;
            let info =
                lingxia::dev::lxapp_dev_page_info(args.appid.as_deref(), args.page.as_deref())?;
            run_async(lingxia::dev::lxapp_dev_page_scroll_to(
                Some(&info.appid),
                Some(&info.instance_id),
                &args.selector,
            ))?;
            Ok(Some(page_action_response("scroll_to", info)))
        }
        handlers::lxapp_page::BACK => {
            let args: BackArgs = parse_args(handler, args)?;
            let info = run_async(lingxia::dev::lxapp_dev_nav_back(
                args.appid.as_deref(),
                args.delta.unwrap_or(1),
            ))?;
            Ok(Some(json!({ "ok": true, "action": "back", "page": info })))
        }
        handlers::lxapp_page::SCREENSHOT => {
            let parsed: PageTargetArgs = parse_args(handler, args)?;
            let (info, bytes) = run_async(lingxia::dev::lxapp_dev_page_screenshot_with_info(
                parsed.appid.as_deref(),
                parsed.page.as_deref(),
            ))?;
            Ok(Some(png_response(
                "page",
                "css_pixels",
                &bytes,
                [
                    ("appid", json!(info.appid)),
                    ("page", json!(info.name)),
                    ("path", json!(info.path)),
                    ("instance_id", json!(info.instance_id)),
                ],
            )))
        }
        _ => Err(format!("unknown lxapp page handler: {}", handler)),
    }
}

fn page_list_response(pages: Vec<lingxia::dev::LxAppDevPageInfo>) -> Value {
    let appid = pages
        .first()
        .map(|page| page.appid.clone())
        .unwrap_or_default();
    json!({
        "appid": appid,
        "pages_count": pages.len(),
        "opened_pages_count": pages.iter().filter(|page| page.opened).count(),
        "pages": pages,
    })
}

fn page_action_response(action: &'static str, page: lingxia::dev::LxAppDevPageInfo) -> Value {
    json!({ "ok": true, "action": action, "page": page })
}

fn parse_args<T>(handler: &str, args: Option<Value>) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(args.unwrap_or_else(|| json!({})))
        .map_err(|err| format!("invalid args for {}: {}", handler, err))
}

#[derive(Deserialize, Default)]
struct AppArgs {
    #[serde(default)]
    appid: Option<String>,
}

#[derive(Deserialize, Default)]
struct PageTargetArgs {
    #[serde(default)]
    appid: Option<String>,
    #[serde(default)]
    page: Option<String>,
}

#[derive(Deserialize)]
struct WaitArgs {
    #[serde(default)]
    appid: Option<String>,
    #[serde(default)]
    page: Option<String>,
    #[serde(default, rename = "selector")]
    selector: Option<String>,
    #[serde(default)]
    index: Option<usize>,
    #[serde(default)]
    state: Option<lingxia::dev::LxAppDevPageWaitState>,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
struct EvalArgs {
    #[serde(default)]
    appid: Option<String>,
    #[serde(default)]
    page: Option<String>,
    js: String,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
struct QueryArgs {
    #[serde(default)]
    appid: Option<String>,
    #[serde(default)]
    page: Option<String>,
    selector: String,
    #[serde(default)]
    index: Option<usize>,
    #[serde(default)]
    all: bool,
    #[serde(default)]
    full: bool,
    #[serde(default)]
    max_text: Option<usize>,
}

#[derive(Deserialize)]
struct SelectorActionArgs {
    #[serde(default)]
    appid: Option<String>,
    #[serde(default)]
    page: Option<String>,
    selector: String,
    #[serde(default)]
    index: Option<usize>,
}

#[derive(Deserialize)]
struct TextActionArgs {
    #[serde(default)]
    appid: Option<String>,
    #[serde(default)]
    page: Option<String>,
    selector: String,
    text: String,
    #[serde(default)]
    index: Option<usize>,
}

#[derive(Deserialize)]
struct ScrollArgs {
    #[serde(default)]
    appid: Option<String>,
    #[serde(default)]
    page: Option<String>,
    #[serde(default)]
    dx: f64,
    #[serde(default)]
    dy: f64,
}

#[derive(Deserialize)]
struct PressArgs {
    #[serde(default)]
    appid: Option<String>,
    #[serde(default)]
    page: Option<String>,
    key: String,
    #[serde(default)]
    selector: Option<String>,
    #[serde(default)]
    index: Option<usize>,
}

#[derive(Deserialize)]
struct BackArgs {
    #[serde(default)]
    appid: Option<String>,
    #[serde(default)]
    delta: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::page_list_response;

    #[test]
    fn page_list_contract_contains_only_lxapp_pages() {
        let response = page_list_response(Vec::new());

        assert!(response.get("pages").is_some());
        assert!(response.get("surfaces").is_none());
    }
}
